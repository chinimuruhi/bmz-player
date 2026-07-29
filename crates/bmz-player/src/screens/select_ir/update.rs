use super::*;

impl SelectIrRanking {
    /// 毎フレーム呼ぶ。取得完了の取り込みと、カーソル譜面の取得予約を行う。
    /// `context` は rule mode / 解決済み LN policy / DOUBLE など
    /// ランキング条件を表す文字列。変わったらキャッシュを破棄する。
    pub fn update(
        &mut self,
        ir_config: &IrConfig,
        profile_root: &Path,
        context: &str,
        ln_policy: LnScorePolicy,
        double_option: DoubleOptionScoreBucket,
        rule_mode: RuleMode,
        selected: Option<[u8; 32]>,
    ) {
        if self.context != context {
            self.context = context.to_string();
            self.clear();
        }
        while let Ok((result_context, sha256, requested_at, result)) = self.receiver.try_recv() {
            if self.in_flight.as_ref().is_some_and(|(context, in_flight_sha, _)| {
                context == &result_context && *in_flight_sha == sha256
            }) {
                self.in_flight = None;
            }
            if result_context != self.context {
                tracing::debug!(
                    old_context = %result_context,
                    current_context = %self.context,
                    "discarding stale select IR ranking fetch"
                );
                continue;
            }
            if self.cache.get(&sha256).is_some_and(|entry| entry.completed_at >= requested_at) {
                tracing::debug!("discarding stale select IR ranking fetch");
                continue;
            }
            let completed_at = Instant::now();
            let entry = match result {
                Ok((global, self_and_rivals, rivals)) => CachedChartIr {
                    global: ranking_to_ir_snapshot(&global),
                    self_and_rivals: self_and_rivals.as_ref().map(ranking_to_ir_snapshot),
                    rival: rivals.as_ref().and_then(top_rival_snapshot),
                    global_ex_scores: ranking_ex_scores(&global),
                    rival_ex_scores: rivals.as_ref().map(ranking_ex_scores).unwrap_or_default(),
                    completed_at,
                },
                Err(error) => {
                    tracing::debug!(%error, "select IR ranking fetch failed");
                    CachedChartIr {
                        global: ResultIrSnapshot {
                            state: SkinIrState::Failed,
                            ..Default::default()
                        },
                        self_and_rivals: None,
                        rival: None,
                        global_ex_scores: Vec::new(),
                        rival_ex_scores: Vec::new(),
                        completed_at,
                    }
                }
            };
            self.insert_entry(sha256, entry);
        }

        let Some(provider) = enabled_provider(ir_config) else {
            return;
        };
        let Some(sha256) = selected else {
            self.pending = None;
            return;
        };
        if self.cache.contains_key(&sha256)
            || self.in_flight.as_ref().is_some_and(|(_, in_flight_sha, _)| *in_flight_sha == sha256)
        {
            self.pending = None;
            return;
        }
        match self.pending {
            Some((pending_sha, since)) if pending_sha == sha256 => {
                if since.elapsed() >= FETCH_DEBOUNCE && self.in_flight.is_none() {
                    self.pending = None;
                    let requested_at = Instant::now();
                    self.in_flight = Some((self.context.clone(), sha256, requested_at));
                    spawn_fetch(
                        ResultIrQuery {
                            profile_root: profile_root.to_path_buf(),
                            provider: provider.0,
                            base_url: provider.1,
                            chart_sha256_hex: hash_to_hex(&sha256),
                            ln_policy,
                            double_option,
                            rule_mode,
                        },
                        self.context.clone(),
                        sha256,
                        requested_at,
                        self.sender.clone(),
                    );
                }
            }
            _ => self.pending = Some((sha256, Instant::now())),
        }
    }

    /// コース行用のグローバルIRランキングをデバウンス付きで取得する。
    pub fn update_course(
        &mut self,
        ir_config: &IrConfig,
        context: &str,
        selected: Option<SelectCourseIrTarget>,
    ) {
        if self.context != context {
            self.context = context.to_string();
            self.clear();
        }
        while let Ok((result_context, target, requested_at, result)) =
            self.course_receiver.try_recv()
        {
            if self.course_in_flight.as_ref().is_some_and(|(context, current, _)| {
                context == &result_context && current == &target
            }) {
                self.course_in_flight = None;
            }
            if result_context != self.context {
                continue;
            }
            if self
                .course_cache
                .get(&target)
                .is_some_and(|entry| entry.completed_at >= requested_at)
            {
                continue;
            }
            let completed_at = Instant::now();
            let ir = match result {
                Ok(ranking) => result_ir_ranking_to_skin_snapshot(
                    &course_ranking_to_result_ir_ranking(&ranking),
                ),
                Err(error) => {
                    tracing::debug!(%error, "select course IR ranking fetch failed");
                    ResultIrSnapshot { state: SkinIrState::Failed, ..Default::default() }
                }
            };
            if self.course_cache.len() >= CACHE_CAPACITY && !self.course_cache.contains_key(&target)
            {
                self.course_cache.clear();
            }
            self.course_cache.insert(target, CachedCourseIr { ir, completed_at });
        }

        let Some(provider) = enabled_provider(ir_config) else {
            return;
        };
        let Some(target) = selected else {
            self.course_pending = None;
            return;
        };
        if self.course_cache.contains_key(&target)
            || self.course_in_flight.as_ref().is_some_and(|(_, current, _)| current == &target)
        {
            self.course_pending = None;
            return;
        }
        match &self.course_pending {
            Some((pending, since)) if pending == &target => {
                if since.elapsed() >= FETCH_DEBOUNCE && self.course_in_flight.is_none() {
                    self.course_pending = None;
                    let requested_at = Instant::now();
                    self.course_in_flight =
                        Some((self.context.clone(), target.clone(), requested_at));
                    spawn_course_fetch(
                        provider.0,
                        provider.1,
                        self.context.clone(),
                        target,
                        requested_at,
                        self.course_sender.clone(),
                    );
                }
            }
            _ => self.course_pending = Some((target, Instant::now())),
        }
    }

    /// Result 画面でスコア送信と同時に取得した Global ranking を選曲キャッシュへ反映する。
    pub fn cache_result_global_ranking(
        &mut self,
        chart_sha256_hex: &str,
        ranking: &ResultIrRanking,
    ) {
        if ranking.scope != IrRankingScope::Global {
            return;
        }
        let Ok(sha256) = hex_to_hash::<32>(chart_sha256_hex) else {
            tracing::warn!(
                chart = chart_sha256_hex,
                "discarding IR ranking for invalid chart hash"
            );
            return;
        };
        let (rival, rival_ex_scores) = self
            .cache
            .get(&sha256)
            .map(|entry| (entry.rival.clone(), entry.rival_ex_scores.clone()))
            .unwrap_or_default();
        self.insert_entry(
            sha256,
            CachedChartIr {
                global: result_ir_ranking_to_skin_snapshot(ranking),
                self_and_rivals: self
                    .cache
                    .get(&sha256)
                    .and_then(|entry| entry.self_and_rivals.clone()),
                rival,
                global_ex_scores: result_ranking_ex_scores(ranking),
                rival_ex_scores,
                completed_at: Instant::now(),
            },
        );
    }
}
