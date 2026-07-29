use super::*;

impl SelectIrRanking {
    /// 選択中譜面のスキン用 snapshot。IR 未設定なら Offline。
    pub fn snapshot_for(
        &self,
        ir_config: &IrConfig,
        selected: Option<[u8; 32]>,
    ) -> ResultIrSnapshot {
        self.snapshot_for_scope(ir_config, selected, SelectIrRankingScope::Global)
    }

    /// 対応 Select スキン用の、現在選択されている scope の snapshot。
    pub fn active_snapshot_for(
        &self,
        ir_config: &IrConfig,
        selected: Option<[u8; 32]>,
    ) -> ResultIrSnapshot {
        self.snapshot_for_scope(ir_config, selected, self.active_scope)
    }

    /// Select スキンの scope binding に従う snapshot を返す。
    ///
    /// `Global` は既存スキンとの互換性を保ち、常に全体ランキングを返す。
    pub fn snapshot_for_binding(
        &self,
        ir_config: &IrConfig,
        selected: Option<[u8; 32]>,
        binding: bmz_render::skin::IrScopeBinding,
    ) -> ResultIrSnapshot {
        match binding {
            bmz_render::skin::IrScopeBinding::Global => self.snapshot_for(ir_config, selected),
            bmz_render::skin::IrScopeBinding::Active => {
                self.active_snapshot_for(ir_config, selected)
            }
        }
    }

    /// 指定 scope の snapshot。Rival scope が未取得・非対応なら Global へ戻す。
    pub fn snapshot_for_scope(
        &self,
        ir_config: &IrConfig,
        selected: Option<[u8; 32]>,
        requested_scope: SelectIrRankingScope,
    ) -> ResultIrSnapshot {
        if enabled_provider(ir_config).is_none() {
            return ResultIrSnapshot::default();
        }
        let Some(sha256) = selected else {
            return ResultIrSnapshot::default();
        };
        let scope = if requested_scope == SelectIrRankingScope::SelfAndRivals
            && self.supports_scope(ir_config, Some(sha256), requested_scope)
        {
            requested_scope
        } else {
            SelectIrRankingScope::Global
        };
        let mut snapshot = self
            .cache
            .get(&sha256)
            .and_then(|entry| match scope {
                SelectIrRankingScope::Global => Some(entry.global.clone()),
                SelectIrRankingScope::SelfAndRivals => entry.self_and_rivals.clone(),
            })
            .map(|mut snapshot| {
                let entry = &self.cache[&sha256];
                match snapshot.state {
                    SkinIrState::Loaded => {
                        snapshot.connect_success_ms = Some(elapsed_since_ms(entry.completed_at));
                    }
                    SkinIrState::Failed => {
                        snapshot.connect_fail_ms = Some(elapsed_since_ms(entry.completed_at));
                    }
                    _ => {}
                }
                snapshot
            })
            .unwrap_or_else(|| {
                let begin_ms = self
                    .in_flight
                    .as_ref()
                    .filter(|(_, in_flight_sha, _)| *in_flight_sha == sha256)
                    .map(|(_, _, started_at)| elapsed_since_ms(*started_at));
                ResultIrSnapshot {
                    state: if begin_ms.is_some() {
                        SkinIrState::Loading
                    } else {
                        SkinIrState::Waiting
                    },
                    connect_begin_ms: begin_ms,
                    ..Default::default()
                }
            });
        snapshot.scope = match scope {
            SelectIrRankingScope::Global => SkinIrScope::Global,
            SelectIrRankingScope::SelfAndRivals => SkinIrScope::Rival,
        };
        snapshot.global_scope_supported = true;
        snapshot.rival_scope_supported =
            self.supports_scope(ir_config, Some(sha256), SelectIrRankingScope::SelfAndRivals);
        snapshot
    }

    pub fn active_scope(&self) -> SelectIrRankingScope {
        self.active_scope
    }

    /// scope を選ぶ。Self-and-Rivals は取得済みの場合だけ選択できる。
    pub fn select_scope(
        &mut self,
        ir_config: &IrConfig,
        selected: Option<[u8; 32]>,
        scope: SelectIrRankingScope,
    ) -> bool {
        if self.active_scope == scope || !self.supports_scope(ir_config, selected, scope) {
            return false;
        }
        self.active_scope = scope;
        true
    }

    pub fn toggle_scope(&mut self, ir_config: &IrConfig, selected: Option<[u8; 32]>) -> bool {
        let next = match self.active_scope {
            SelectIrRankingScope::Global => SelectIrRankingScope::SelfAndRivals,
            SelectIrRankingScope::SelfAndRivals => SelectIrRankingScope::Global,
        };
        self.select_scope(ir_config, selected, next)
    }

    /// コース行は global のみ。単曲の Self-and-Rivals は取得済み時だけ有効にする。
    pub fn supports_scope(
        &self,
        ir_config: &IrConfig,
        selected: Option<[u8; 32]>,
        scope: SelectIrRankingScope,
    ) -> bool {
        enabled_provider(ir_config).is_some()
            && match scope {
                SelectIrRankingScope::Global => selected.is_some(),
                SelectIrRankingScope::SelfAndRivals => selected
                    .and_then(|sha256| self.cache.get(&sha256))
                    .and_then(|entry| entry.self_and_rivals.as_ref())
                    .is_some(),
            }
    }

    pub fn course_snapshot_for(
        &self,
        ir_config: &IrConfig,
        selected: Option<&SelectCourseIrTarget>,
    ) -> ResultIrSnapshot {
        if enabled_provider(ir_config).is_none() {
            return ResultIrSnapshot::default();
        }
        let Some(target) = selected else {
            return ResultIrSnapshot::default();
        };
        let mut snapshot = self
            .course_cache
            .get(target)
            .map(|entry| {
                let mut snapshot = entry.ir.clone();
                match snapshot.state {
                    SkinIrState::Loaded => {
                        snapshot.connect_success_ms = Some(elapsed_since_ms(entry.completed_at));
                    }
                    SkinIrState::Failed => {
                        snapshot.connect_fail_ms = Some(elapsed_since_ms(entry.completed_at));
                    }
                    _ => {}
                }
                snapshot
            })
            .unwrap_or_else(|| {
                let begin_ms = self
                    .course_in_flight
                    .as_ref()
                    .filter(|(_, current, _)| current == target)
                    .map(|(_, _, started_at)| elapsed_since_ms(*started_at));
                ResultIrSnapshot {
                    state: if begin_ms.is_some() {
                        SkinIrState::Loading
                    } else {
                        SkinIrState::Waiting
                    },
                    connect_begin_ms: begin_ms,
                    ..Default::default()
                }
            });
        snapshot.scope = SkinIrScope::Global;
        snapshot.global_scope_supported = true;
        snapshot.rival_scope_supported = false;
        snapshot
    }

    /// 選択中譜面のライバルベスト (最上位 1 名)。未取得 / IR 未設定なら None。
    pub fn rival_for(
        &self,
        ir_config: &IrConfig,
        selected: Option<[u8; 32]>,
    ) -> Option<SelectRivalSnapshot> {
        enabled_provider(ir_config)?;
        self.cache.get(&selected?).and_then(|entry| entry.rival.clone())
    }

    pub fn target_ex_score_for(
        &self,
        ir_config: &IrConfig,
        selected: Option<[u8; 32]>,
        target: TargetOption,
        local_best_ex_score: Option<u32>,
    ) -> Option<u32> {
        enabled_provider(ir_config)?;
        let entry = self.cache.get(&selected?)?;
        match target {
            TargetOption::IrTop => entry.global_ex_scores.first().copied(),
            TargetOption::IrNext => {
                next_ex_score_above(&entry.global_ex_scores, local_best_ex_score.unwrap_or(0))
            }
            TargetOption::RivalTop => entry.rival_ex_scores.first().copied(),
            TargetOption::RivalNext => {
                next_ex_score_above(&entry.rival_ex_scores, local_best_ex_score.unwrap_or(0))
            }
            TargetOption::RivalIndex(index) => {
                entry.rival_ex_scores.get(index.saturating_sub(1) as usize).copied()
            }
            _ => None,
        }
    }

    /// ログイン状態が変わったとき等にキャッシュを破棄する。
    pub fn clear(&mut self) {
        self.cache.clear();
        self.in_flight = None;
        self.pending = None;
        self.course_cache.clear();
        self.course_in_flight = None;
        self.course_pending = None;
        self.active_scope = SelectIrRankingScope::Global;
    }

    pub(super) fn insert_entry(&mut self, sha256: [u8; 32], entry: CachedChartIr) {
        if self.cache.len() >= CACHE_CAPACITY && !self.cache.contains_key(&sha256) {
            self.cache.clear();
        }
        self.cache.insert(sha256, entry);
        if self.in_flight.as_ref().is_some_and(|(_, in_flight_sha, _)| *in_flight_sha == sha256) {
            self.in_flight = None;
        }
        if self.pending.as_ref().is_some_and(|(pending_sha, _)| *pending_sha == sha256) {
            self.pending = None;
        }
    }
}
