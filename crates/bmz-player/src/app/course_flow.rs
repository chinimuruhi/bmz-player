use super::*;

impl WinitApp {
    /// Returns the beatoraja-compatible course stage marker for the currently
    /// playing chart in the active course (1, 2, 3, 4 or Final).  None when no
    /// course is active.
    ///
    /// The final entry always maps to `Final` (OPTION_COURSE_STAGE_FINAL=289);
    /// earlier entries map to Stage1..4 by their 1-based index, clamped to
    /// Stage4 for courses longer than 4 + final entry.
    pub(super) fn current_course_stage_marker(&self) -> Option<CourseStageMarker> {
        let course = self.active_course.as_ref()?;
        let total = course.definition.entries.len();
        if total == 0 {
            return None;
        }
        let index = course.current_index.min(total - 1);
        let is_final = index + 1 == total;
        if is_final {
            return Some(CourseStageMarker::Final);
        }
        Some(match index {
            0 => CourseStageMarker::Stage1,
            1 => CourseStageMarker::Stage2,
            2 => CourseStageMarker::Stage3,
            _ => CourseStageMarker::Stage4,
        })
    }

    pub(super) fn current_course_titles(&self) -> [String; 10] {
        let Some(course) = self.active_course.as_ref() else {
            return Default::default();
        };
        course_titles_from_entries(
            course
                .definition
                .entries
                .iter()
                .map(|entry| (entry.title_hint.as_str(), entry.chart_id.is_some())),
        )
    }

    pub(super) fn apply_course_skin_context(&self, snapshot: &mut RenderSnapshot) {
        snapshot.course_stage = self.current_course_stage_marker();
        snapshot.course_titles = self.current_course_titles();
    }

    pub(super) fn start_course(&mut self, course_id: i64) {
        self.autoplay_folder = None;
        self.start_course_with_arrange(course_id, Vec::new(), false);
    }

    /// Start a course in PLAY mode.  When `arrange_overrides` is non-empty, the
    /// recorded per-entry arrange (seed/pattern) is reapplied so the whole
    /// course replays with the same arrangement; entries without an override at
    /// their index get a fresh arrange.  A fresh course start passes an empty
    /// vec.
    pub(super) fn start_course_with_arrange(
        &mut self,
        course_id: i64,
        arrange_overrides: Vec<AppliedArrange>,
        auto_advance_intermediate_results: bool,
    ) {
        let stored = match self.boot.library_db.list_courses() {
            Ok(courses) => courses.into_iter().find(|c| c.id == course_id),
            Err(error) => {
                tracing::error!(%error, course_id, "failed to load courses for start_course");
                return;
            }
        };
        let Some(stored) = stored else {
            tracing::warn!(course_id, "course not found");
            return;
        };
        let mut definition = stored.definition;
        if let Err(error) = hydrate_course_entry_title_hints(&self.boot.library_db, &mut definition)
        {
            tracing::warn!(%error, course_id, "failed to hydrate course entry titles");
        }
        if definition.entries.is_empty()
            || definition.entries.iter().any(|entry| entry.chart_id.is_none())
        {
            let resolved =
                definition.entries.iter().filter(|entry| entry.chart_id.is_some()).count();
            tracing::warn!(
                course_id,
                resolved,
                total = definition.entries.len(),
                "course is missing entries"
            );
            return;
        }
        let first_chart_id = definition.entries.first().and_then(|e| e.chart_id);
        let Some(first_chart_id) = first_chart_id else {
            tracing::warn!(course_id, "no resolved chart in course");
            return;
        };
        tracing::info!(
            course_id,
            title = %definition.title,
            same_arrange = !arrange_overrides.is_empty(),
            "starting course"
        );
        let mut entry_start_options = Vec::with_capacity(definition.entries.len());
        for index in 0..definition.entries.len() {
            let mut options = self.play_start_options();
            normalize_session_mode_for_course(&mut options);
            apply_course_constraints(&mut options, &definition.constraints);
            // Reapply each chart's recorded arrange after constraints so the
            // constraint clamp doesn't overwrite it (same ordering as replay).
            if let Some(arrange) = arrange_overrides.get(index) {
                apply_arrange_override(&mut options, arrange);
            }
            entry_start_options.push(options);
        }
        let options = entry_start_options[0].clone();
        let course_title = definition.title.clone();
        let app_config = self.play_session_app_config();
        let course_total_notes = match course_total_notes_for_definition(
            &self.boot.library_db,
            &definition,
            &app_config,
            self.boot.profile_config.play.ln_mode_policy,
            self.boot.profile_config.play.rule_mode,
            &entry_start_options,
        ) {
            Ok(total_notes) => total_notes,
            Err(error) => {
                tracing::error!(%error, course_id, "failed to count course notes from source");
                return;
            }
        };
        self.active_course = Some(ActiveCourseSession {
            course_id,
            definition,
            course_total_notes,
            current_index: 0,
            entry_results: Vec::new(),
            entry_start_options,
            auto_advance_intermediate_results,
        });
        self.begin_course_decide_for_chart(first_chart_id, options, &course_title);
    }

    /// Start a course in replay mode, replaying the saved per-chart inputs of
    /// the given `course_score_id`.  Each chart of the course is launched in
    /// sequence with its saved ReplayPlayer attached, so the user can watch
    /// the entire course attempt back to back.
    ///
    /// If `course_score_id` refers to a partial course attempt (e.g. failed
    /// at chart 2 of 4), only the played charts replay; the queue ends there
    /// and the course session naturally finishes the same way the original
    /// attempt did.
    ///
    /// Errors during replay load (missing file, chart re-imported with
    /// different bytes) abort with a logged warning rather than crashing.
    pub fn start_course_replay(&mut self, course_id: i64, course_score_id: i64) {
        self.start_course_replay_with_auto_advance(course_id, course_score_id, false);
    }

    pub(super) fn start_course_replay_with_auto_advance(
        &mut self,
        course_id: i64,
        course_score_id: i64,
        auto_advance_intermediate_results: bool,
    ) {
        let stored = match self.boot.library_db.list_courses() {
            Ok(courses) => courses.into_iter().find(|c| c.id == course_id),
            Err(error) => {
                tracing::error!(
                    %error,
                    course_id,
                    "failed to load courses for start_course_replay"
                );
                return;
            }
        };
        let Some(stored) = stored else {
            tracing::warn!(course_id, "course not found");
            return;
        };

        let entries = match self.boot.score_db.list_course_replays(course_score_id) {
            Ok(rows) => rows,
            Err(error) => {
                tracing::error!(
                    %error,
                    course_id,
                    course_score_id,
                    "failed to list course_replays rows"
                );
                return;
            }
        };
        if entries.is_empty() {
            tracing::warn!(course_id, course_score_id, "no replays saved for this attempt");
            return;
        }

        let entry_tuples: Vec<(i64, [u8; 32], String)> =
            entries.iter().map(|r| (r.position, r.chart_sha256, r.replay_path.clone())).collect();
        let replay_root = self.boot.profile_paths.root_dir.clone();
        let lookup = |chart_sha256: [u8; 32]| -> anyhow::Result<Option<i64>> {
            self.boot.library_db.chart_id_by_sha256(chart_sha256)
        };
        let queued = match crate::storage::replay::load_course_replays(
            &entry_tuples,
            &replay_root,
            lookup,
        ) {
            Ok(q) => q,
            Err(error) => {
                tracing::warn!(
                    %error,
                    course_id,
                    course_score_id,
                    "failed to load queued course replays"
                );
                return;
            }
        };

        let mut definition = stored.definition;
        if let Err(error) = hydrate_course_entry_title_hints(&self.boot.library_db, &mut definition)
        {
            tracing::warn!(%error, course_id, "failed to hydrate replay course entry titles");
        }
        let first_chart_id = definition.entries.iter().find_map(|e| e.chart_id);
        let Some(first_chart_id) = first_chart_id else {
            tracing::warn!(course_id, "no resolved chart in course");
            return;
        };
        tracing::info!(
            course_id,
            course_score_id,
            title = %definition.title,
            replays = queued.len(),
            "starting course replay"
        );
        let mut entry_start_options = Vec::with_capacity(definition.entries.len());
        for (index, entry) in definition.entries.iter().enumerate() {
            let mut options = self.play_start_options();
            options.session_mode = SessionMode::Normal;
            options.autoplay = false;
            apply_course_constraints(&mut options, &definition.constraints);
            if let Some(replay) = queued.get(index)
                && entry.chart_id == Some(replay.chart_id)
            {
                apply_queued_replay(&mut options, replay);
            }
            entry_start_options.push(options);
        }
        let options = entry_start_options[0].clone();
        let course_title = definition.title.clone();
        let app_config = self.play_session_app_config();
        let course_total_notes = match course_total_notes_for_definition(
            &self.boot.library_db,
            &definition,
            &app_config,
            self.boot.profile_config.play.ln_mode_policy,
            self.boot.profile_config.play.rule_mode,
            &entry_start_options,
        ) {
            Ok(total_notes) => total_notes,
            Err(error) => {
                tracing::error!(%error, course_id, "failed to count replay course notes from source");
                return;
            }
        };
        self.active_course = Some(ActiveCourseSession {
            course_id,
            definition,
            course_total_notes,
            current_index: 0,
            entry_results: Vec::new(),
            entry_start_options,
            auto_advance_intermediate_results,
        });
        self.begin_course_decide_for_chart(first_chart_id, options, &course_title);
    }

    /// コース曲間の中間リザルト状態かどうか。active_course を保持したまま
    /// finished_play だけが立ち、finished_course はまだ無い状態を指す。
    pub(super) fn is_course_intermediate_result(&self) -> bool {
        is_course_intermediate_result(
            self.active_course.is_some(),
            self.finished_course.is_some(),
            self.finished_play.is_some(),
        )
    }

    pub(super) fn course_intermediate_auto_advance_enabled(&self) -> bool {
        self.active_course.as_ref().is_some_and(|course| course.auto_advance_intermediate_results)
    }

    /// コース曲間の中間リザルト画面を表示する。直前に終わった曲の結果を
    /// finished_play に入れて Result スキンを出すが、active_course は保持し
    /// finished_course は立てないので「中間リザルト」状態になる。
    pub(super) fn show_course_intermediate_result(&mut self) {
        let last = self
            .active_course
            .as_ref()
            .and_then(|course| course.entry_results.last())
            .map(|entry| entry.finished.clone());
        let Some(last) = last else {
            // 直前結果が無い異常系では中間リザルトを出さず、次の曲へ進む。
            self.start_next_course_chart();
            return;
        };
        self.result_gauge_graph_type = last.summary.gauge_type as i32;
        self.finished_play = Some(last);
        self.result_exit = None;
        self.result_key5_held = false;
        self.result_key7_held = false;
        self.result_scene_started_at = Instant::now();
        self.ensure_result_skin_ready(ResultSkinSlot::Normal);
    }

    /// 中間リザルトを閉じて次の曲へ進む。finished_play をクリアして中間リザルト
    /// 状態を抜け、active_course はそのまま次の曲を開始する。
    pub(super) fn advance_to_next_course_chart(&mut self) {
        self.finished_play = None;
        self.result_exit = None;
        self.result_key5_held = false;
        self.result_key7_held = false;
        self.start_next_course_chart();
    }

    pub(super) fn autoplay_folder_has_next(&self) -> bool {
        self.autoplay_folder
            .as_ref()
            .is_some_and(|session| session.next_index < session.chart_ids.len())
    }

    pub(super) fn advance_autoplay_folder(&mut self) {
        let Some(session) = self.autoplay_folder.as_mut() else {
            self.leave_result();
            return;
        };
        let Some(&chart_id) = session.chart_ids.get(session.next_index) else {
            self.autoplay_folder = None;
            self.leave_result();
            return;
        };
        session.next_index += 1;
        self.finished_play = None;
        self.result_exit = None;
        self.result_key5_held = false;
        self.result_key7_held = false;
        let mut options = self.play_start_options();
        options.session_mode = SessionMode::Autoplay;
        options.autoplay = true;
        self.start_chart_with_options(chart_id, options);
        tracing::info!(chart_id, "advanced folder autoplay");
    }

    pub(super) fn finish_course_after_intermediate_result(&mut self) {
        self.finished_play = None;
        self.result_exit = None;
        self.result_key5_held = false;
        self.result_key7_held = false;
        self.finish_active_course();
    }

    /// コースの (current_index が指す) 次の曲を開始する。ゲージ持ち越しや
    /// replay / 同配置 arrange の適用は元の advance_course_after_finish と同じ。
    pub(super) fn start_next_course_chart(&mut self) {
        let Some(course) = &self.active_course else {
            return;
        };
        let next_index = course.current_index;
        let Some(next_chart_id) =
            course.definition.entries.get(next_index).and_then(|e| e.chart_id)
        else {
            return;
        };
        // Carry each gauge independently so auto-shift gauges that already
        // reached zero do not recover on the next chart.
        let carried_gauges = course.entry_results.last().map(|r| r.finished.gauge_carry.clone());
        let carried_combo = course.entry_results.last().map(|r| r.finished.course_combo);
        let Some(mut options) = course.entry_start_options.get(next_index).cloned() else {
            tracing::error!(next_index, "course entry start options are missing");
            return;
        };
        options.initial_gauge_values = carried_gauges;
        options.initial_course_combo = carried_combo;
        self.start_chart_with_options(next_chart_id, options);
    }

    pub(super) fn course_intermediate_exit_action(&self) -> ResultExitAction {
        let Some(course) = self.active_course.as_ref() else {
            return ResultExitAction::FinishCourse;
        };
        let failed = course.entry_results.last().is_some_and(|entry| {
            entry.finished.result.clear_type == bmz_core::clear::ClearType::Failed
        });
        let has_next_chart = course
            .definition
            .entries
            .get(course.current_index)
            .and_then(|entry| entry.chart_id)
            .is_some();
        course_intermediate_exit_action_for_state(failed, has_next_chart)
    }

    /// コース中間リザルトのコントロール処理。Key6 はゲージグラフ切替のみ許可し、
    /// それ以外の終了レーン (Key1-4/Key5/Key7) は retry せず次の曲へ進む。
    pub(super) fn handle_course_intermediate_control(
        &mut self,
        control: &PhysicalControl,
        pressed: bool,
        repeat: bool,
    ) -> bool {
        if pressed
            && !repeat
            && self.result_input_ready()
            && self.result_panel == 1
            && self.result_ir_scope_toggle_is_e1()
            && self.is_result_ir_scope_toggle_control(control)
            && self.toggle_result_ir_scope()
        {
            return true;
        }
        if pressed
            && !repeat
            && self.result_input_ready()
            && self.select_result_panel_for_control(control)
        {
            return true;
        }
        if pressed
            && !repeat
            && self.result_input_ready()
            && self.is_result_panel_toggle_control(control)
            && self.toggle_result_panel()
        {
            return true;
        }
        let Some(lane) = self.result_lane_for_control(control) else {
            return false;
        };
        match lane {
            Lane::Key6 => {
                if pressed && !repeat && self.result_input_ready() {
                    self.cycle_result_gauge_graph_type();
                }
                true
            }
            lane if lane_starts_result_exit(lane) => {
                if pressed && self.result_input_ready() {
                    self.begin_result_exit(self.course_intermediate_exit_action());
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn advance_course_after_finish(&mut self, finished: FinishedPlaySession) {
        let Some(course) = &mut self.active_course else {
            return;
        };
        let chart_id = self.last_started_chart_id.unwrap_or(0);
        // Beatoraja behavior: if any chart in the course is Failed, the course
        // ends immediately and remaining charts are skipped.
        let failed = finished.result.clear_type == bmz_core::clear::ClearType::Failed;
        course.entry_results.push(CourseEntryResult { chart_id, finished });
        course.current_index += 1;

        let next_chart_id =
            course.definition.entries.get(course.current_index).and_then(|e| e.chart_id);
        let has_next_entry = course.definition.entries.get(course.current_index).is_some();

        if should_show_course_stage_result(failed, has_next_entry, next_chart_id.is_some()) {
            // 次の曲をすぐ始めず、まず直前の曲の単曲リザルト (中間リザルト) を出す。
            // active_course を保持したまま finished_play に直前結果を入れることで、
            // view_state は Result を返し、入力は中間リザルト分岐へ入る。実際の次曲
            // 開始 (ゲージ持ち越し / replay / 同配置 arrange の適用を含む) は、結果画面
            // を閉じたとき advance_to_next_course_chart まで遅延する。
            self.show_course_intermediate_result();
            return;
        }

        self.finish_active_course();
    }

    pub(super) fn finish_active_course(&mut self) {
        let Some(course) = self.active_course.take() else {
            return;
        };
        let course_id = course.course_id;
        let course_identity = self.course_identity_with_stored(course_id);

        // Extract data needed to persist the course score before `into_result`
        // consumes `entry_results`.
        let chart_records: Vec<crate::storage::score_db::CourseScoreChartRecord> = course
            .entry_results
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let chart_sha256 = course_identity.as_ref()?.1.chart_sha256s.get(i).copied()?;
                Some(crate::storage::score_db::CourseScoreChartRecord {
                    position: i as i64,
                    chart_sha256,
                    ex_score: r.finished.result.score.ex_score(),
                    max_combo: r.finished.result.score.max_combo,
                    clear_type: r.finished.result.clear_type.as_str().to_string(),
                    gauge_value: r.finished.result.gauge_value,
                })
            })
            .collect();
        let replay_records: Vec<crate::storage::score_db::CourseReplayRecord> = course
            .entry_results
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let chart_sha256 = course_identity.as_ref()?.1.chart_sha256s.get(i).copied()?;
                Some(crate::storage::score_db::CourseReplayRecord {
                    position: i as i64,
                    chart_sha256,
                    replay_path: r.finished.stored.replay_path.clone(),
                })
            })
            .collect();
        let any_autoplay = course.entry_results.iter().any(|r| r.finished.result.autoplay);
        let any_replay_playback = course.entry_results.iter().any(|r| r.finished.replay_playback);
        // Collect score_history row ids written by per-chart store_play_result
        // so they can be tagged with the new course_score_id after insert.
        // Autoplay charts have score_history_id == 0 and are filtered out.
        let history_ids: Vec<i64> = course
            .entry_results
            .iter()
            .map(|r| r.finished.stored.score_history_id)
            .filter(|id| *id > 0)
            .collect();
        let last_finished = course.entry_results.last().map(|r| r.finished.clone());
        let max_combo: u32 =
            course.entry_results.iter().map(|r| r.finished.course_max_combo).max().unwrap_or(0);
        let course_arrange = course
            .entry_results
            .first()
            .map(|entry| entry.finished.arrange.to_persistent_str().to_string())
            .unwrap_or_else(|| "Normal".to_string());

        let mut course_result = course.into_result();
        tracing::info!(
            title = %course_result.title,
            total_ex_score = course_result.total_ex_score,
            course_clear = course_result.course_clear,
            course_failed = course_result.course_failed,
            played = course_result.played_entries,
            total = course_result.total_entries,
            trophies = ?course_result
                .trophy_results
                .iter()
                .filter(|t| t.achieved)
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>(),
            "course finished"
        );
        // Persist course score + per-chart replay paths.
        //
        // - Autoplay / replay playback courses are not saved, matching the
        //   per-chart policy in `finish_session_result`.
        // - The course clear type is taken from the last played chart's
        //   gauge survival result; a Failed at any point forces Failed.
        // - The per-chart replay files have already been written by
        //   `store_play_result` for each chart in the course; we only record
        //   the relative paths here so the course can be replayed back to back
        //   in a future iteration.
        // - TODO(course-replay-reload): launching a course via a "replay slot"
        //   from the select screen is out of scope for this change; only the
        //   save path is wired up.
        if !any_autoplay && !any_replay_playback {
            let Some((stored_course, identity)) = &course_identity else {
                tracing::warn!(
                    course_id,
                    "course identity unavailable; skipping course score save"
                );
                self.install_finished_course(course_result, None, None);
                if let Some(last) = last_finished {
                    self.result_gauge_graph_type = last.summary.gauge_type as i32;
                    self.finished_play = Some(last);
                    self.result_key5_held = false;
                    self.result_key7_held = false;
                    self.result_scene_started_at = Instant::now();
                    self.ensure_result_skin_ready(ResultSkinSlot::Course);
                }
                let clear_type = self
                    .finished_course
                    .as_ref()
                    .map(|course| course.final_clear_type)
                    .unwrap_or(bmz_core::clear::ClearType::Failed);
                self.play_course_result_entry_sound(clear_type);
                return;
            };
            let course_rule_mode = course_result.rule_mode;
            course_result.previous_best_score = self
                .boot
                .score_db
                .best_course_score(&identity.course_hash, course_rule_mode)
                .unwrap_or_else(|error| {
                    tracing::warn!(
                        %error,
                        course_id,
                        course_hash = %identity.course_hash,
                        rule_mode = course_rule_mode.as_str(),
                        "failed to read previous best course score"
                    );
                    None
                });
            let final_clear_type = course_result.final_clear_type;
            let played_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            // Store the names of trophies that were achieved on this attempt
            // as a JSON array of strings (for round-trip / audit) and
            // separately as structured rows in course_trophy_achievements via
            // CourseScoreInsert.achieved_trophies, which is what powers
            // per-trophy best queries.
            let achieved_trophies: Vec<String> = course_result
                .trophy_results
                .iter()
                .filter(|t| t.achieved)
                .map(|t| t.name.clone())
                .collect();
            let trophies_json =
                serde_json::to_string(&achieved_trophies).unwrap_or_else(|_| "[]".to_string());
            let insert = crate::storage::score_db::CourseScoreInsert {
                course_hash: identity.course_hash.clone(),
                rule_mode: course_rule_mode,
                source: stored_course.source.clone(),
                course_key: stored_course.definition.key.clone(),
                title: stored_course.definition.title.clone(),
                kind: identity.definition.kind.clone(),
                constraints_json: identity.constraints_json.clone(),
                chart_sha256s_json: identity.chart_sha256s_json.clone(),
                ex_score: course_result.total_ex_score,
                max_ex_score: course_result.max_ex_score,
                clear_type: final_clear_type.as_str().to_string(),
                gauge_type: course_result.final_gauge_type.as_str().to_string(),
                gauge_value: course_result.final_gauge_value,
                max_combo,
                bp: course_result.bp,
                course_failed: course_result.course_failed,
                course_clear: course_result.course_clear,
                arrange: course_arrange,
                trophies_json,
                played_at,
                charts: chart_records,
                replays: replay_records,
                achieved_trophies,
            };
            match self.boot.score_db.insert_course_score(&insert) {
                Ok(course_score_id) => {
                    // Backfill the per-chart `score_history` rows with the
                    // course attempt id so they can be filtered as part of
                    // this course play later.
                    if let Err(error) = self
                        .boot
                        .score_db
                        .tag_score_history_with_course(&history_ids, course_score_id)
                    {
                        tracing::warn!(
                            %error,
                            course_id,
                            course_score_id,
                            "failed to tag score_history rows with course_score_id"
                        );
                    }

                    // IR コーススコア送信ジョブを enqueue する (IR 未設定なら no-op)。
                    self.enqueue_ir_course_job(
                        course_id,
                        course_score_id,
                        &course_result,
                        course_rule_mode,
                        last_finished.as_ref().map(|f| f.stored.device_type),
                        &insert.gauge_type,
                        played_at,
                        &insert.arrange,
                        course_result
                            .entry_arranges
                            .first()
                            .and_then(|arrange| arrange.packed_beatoraja_seed_from_sides()),
                    );

                    course_result.course_score_id = Some(course_score_id);
                    course_result.course_played_at = Some(played_at);
                    // Update the four course replay slots that pass their
                    // configured rule.  Reuses the per-chart slot_rule_passes
                    // helper for identical semantics (Always overwrites
                    // unconditionally; Score / Bp / MaxCombo / Clear
                    // require strict improvement; empty slot always wins).
                    course_result.saved_replay_slots = self.update_course_replay_slots(
                        &identity.course_hash,
                        course_rule_mode,
                        course_score_id,
                        played_at,
                        course_result.total_ex_score,
                        course_result.bp,
                        max_combo,
                        final_clear_type as u8,
                    );
                    course_result.replay_slots = self
                        .boot
                        .score_db
                        .course_replay_slot_presence(&identity.course_hash, course_rule_mode)
                        .unwrap_or_else(|error| {
                            tracing::warn!(
                                %error,
                                course_id,
                                course_hash = %identity.course_hash,
                                rule_mode = course_rule_mode.as_str(),
                                "failed to read course replay slot presence"
                            );
                            [false; 4]
                        });
                    for (index, saved) in course_result.saved_replay_slots.iter().enumerate() {
                        if *saved {
                            course_result.replay_slots[index] = true;
                        }
                    }
                }
                Err(error) => {
                    tracing::error!(%error, course_id, "failed to persist course score");
                }
            }

            // Look up the best score *after* the insert above so the just-
            // saved attempt is reflected when it improved the record.  The
            // result overlay reads this to show a "BEST" section.
            course_result.best_score = self
                .boot
                .score_db
                .best_course_score(&identity.course_hash, course_rule_mode)
                .unwrap_or_else(|error| {
                    tracing::warn!(
                        %error,
                        course_id,
                        course_hash = %identity.course_hash,
                        rule_mode = course_rule_mode.as_str(),
                        "failed to read best course score"
                    );
                    None
                });
        }

        if course_result.saved_replay_slots.iter().any(|saved| *saved) {
            self.notify_obs_save_recording(crate::obs::ObsRecordingSaveReason::OnReplay);
        }
        let course_hash =
            course_identity.as_ref().map(|(_, identity)| identity.course_hash.clone());
        let rian_course_hash_v1 =
            course_identity.as_ref().map(|(_, identity)| identity.rian_course_hash_v1.clone());
        self.install_finished_course(course_result, course_hash, rian_course_hash_v1);
        // Use the last chart's result for the standard result skin display.
        if let Some(last) = last_finished {
            self.result_gauge_graph_type = last.summary.gauge_type as i32;
            self.finished_play = Some(last);
            self.result_key5_held = false;
            self.result_key7_held = false;
            self.result_scene_started_at = Instant::now();
            self.ensure_result_skin_ready(ResultSkinSlot::Course);
        }
        let clear_type = self
            .finished_course
            .as_ref()
            .map(|course| course.final_clear_type)
            .unwrap_or(bmz_core::clear::ClearType::Failed);
        self.play_course_result_entry_sound(clear_type);
    }

    /// コース定義から IR / score.db 用の identity (course_hash + charts sha256 +
    /// canonical constraints) を解決する。未解決の譜面 (sha256 不明) がある
    /// コースは score 保存 / IR 送信対象外。
    pub(super) fn course_identity_with_stored(
        &self,
        course_id: i64,
    ) -> Option<(
        crate::storage::library_db::StoredCourse,
        crate::ir::course_payload::IrCourseIdentity,
    )> {
        let stored = self
            .boot
            .library_db
            .list_courses()
            .ok()?
            .into_iter()
            .find(|course| course.id == course_id)?;
        let identity =
            crate::ir::course_payload::course_identity_from_stored(&self.boot.library_db, &stored)?;
        Some((stored, identity))
    }

    pub(super) fn ir_course_identity(
        &self,
        course_id: i64,
    ) -> Option<crate::ir::course_payload::IrCourseIdentity> {
        self.course_identity_with_stored(course_id).map(|(_, identity)| identity)
    }

    pub(super) fn course_result_ir_target(
        &self,
    ) -> Option<(String, String, String, String, bmz_gameplay::rule::RuleMode)> {
        let course = self.finished_course.as_ref()?;
        let course_hash = self.finished_course_hash.clone()?;
        let rian_course_hash_v1 = self.finished_course_rian_hash_v1.clone()?;
        let gauge = course.final_gauge_type.as_str().to_string();
        let ln_policy = self.boot.profile_config.play.ln_mode_policy.as_ir_str().to_string();
        Some((
            course_hash,
            rian_course_hash_v1,
            gauge,
            ln_policy,
            self.boot.profile_config.play.rule_mode,
        ))
    }

    pub(super) fn start_result_ir_for_finished_play(&mut self, finished: &FinishedPlaySession) {
        if finished.stored.score_history_id <= 0 {
            return;
        }
        let chart_sha256_hex = crate::storage::common::hash_to_hex(&finished.result.chart_sha256);
        if self.result_ir.as_ref().is_some_and(|state| {
            state.matches_chart_result(
                finished.stored.score_history_id,
                &chart_sha256_hex,
                finished.ln_policy,
                finished.double_option,
                finished.rule_mode,
            )
        }) {
            return;
        }
        self.result_ir = crate::screens::result_ir::spawn_result_ir_task(
            self.boot.profile_paths.root_dir.clone(),
            self.boot.profile_paths.score_db.clone(),
            self.boot.profile_paths.network_db.clone(),
            self.boot.app_paths.logs_dir.clone(),
            &self.boot.profile_config.ir,
            finished.stored.score_history_id,
            chart_sha256_hex,
            finished.ln_policy,
            finished.double_option,
            finished.rule_mode,
        );
    }

    /// コーススコアの IR 送信ジョブを enqueue する。IR 未設定 / 定義未解決なら no-op。
    #[allow(clippy::too_many_arguments)]
    pub(super) fn enqueue_ir_course_job(
        &mut self,
        course_id: i64,
        course_score_id: i64,
        course_result: &crate::screens::course_session::CourseResultSummary,
        rule_mode: bmz_gameplay::rule::RuleMode,
        device_type: Option<bmz_core::input::InputDeviceKind>,
        gauge: &str,
        played_at: i64,
        arrange: &str,
        random_seed: Option<i64>,
    ) {
        let enabled: Vec<_> = self
            .boot
            .profile_config
            .ir
            .providers
            .iter()
            .filter(|provider| {
                provider.enabled
                    && !provider.base_url.is_empty()
                    && (!crate::ir::rian_ir::is_rian_ir_config(provider)
                        || crate::ir::rian_ir::course_submission_supported(
                            self.boot.profile_config.play.ln_mode_policy,
                            self.double_option,
                        ))
            })
            .cloned()
            .collect();
        if enabled.is_empty() {
            return;
        }
        let Some(identity) = self.ir_course_identity(course_id) else {
            tracing::info!(course_id, "course has unresolved charts; skipping IR submission");
            return;
        };
        let definition = &identity.definition;
        let ln_setting = self.boot.profile_config.play.ln_mode_policy.as_ir_str().to_string();
        let payload = crate::ir::course_payload::build_course_submission(
            definition,
            course_result,
            &crate::ir::course_payload::IrCourseSubmissionContext {
                played_at,
                ln_policy_setting: ln_setting.clone(),
                rule_mode: rule_mode.as_str().to_string(),
                gauge: gauge.to_string(),
                device_type: device_type.unwrap_or(bmz_core::input::InputDeviceKind::Keyboard),
                arrange: arrange.to_string(),
                random_seed,
                idempotency_key: format!("bmz-course-{}-{course_score_id}", identity.course_hash),
            },
        );
        let Ok(payload_json) = serde_json::to_string(&payload) else {
            return;
        };
        let first_chart = definition
            .charts
            .first()
            .and_then(|sha| crate::storage::common::hex_to_hash::<32>(sha).ok())
            .unwrap_or([0; 32]);
        let ln_policy = crate::ln_policy::score_ln_policy(
            self.boot.profile_config.play.ln_mode_policy,
            crate::ln_policy::ChartLnProfile::default(),
        );
        for provider in enabled {
            let Some(provider_key) = crate::ir::provider_key::configured_provider_key(&provider)
            else {
                tracing::warn!(
                    provider = provider.provider,
                    "skipping IR course job because provider_key is missing; log in again"
                );
                continue;
            };
            if let Err(error) = self.boot.network_db.enqueue_ir_score_job(
                &crate::storage::network_db::NewIrScoreJob {
                    provider: provider_key.to_string(),
                    account_id: provider.account_id.clone(),
                    kind: crate::storage::network_db::IrJobKind::Course,
                    local_score_id: course_score_id,
                    chart_sha256: first_chart,
                    ln_policy,
                    payload_json: payload_json.clone(),
                    now: played_at,
                },
            ) {
                tracing::warn!(provider = provider.provider, provider_key, %error, "failed to enqueue IR course job");
            }
        }
    }

    pub(super) fn update_course_replay_slots(
        &mut self,
        course_hash: &str,
        rule_mode: bmz_gameplay::rule::RuleMode,
        course_score_id: i64,
        played_at: i64,
        ex_score: u32,
        bp: u32,
        max_combo: u32,
        clear_rank: u8,
    ) -> [bool; 4] {
        let slot_rules = self.boot.profile_config.replay.slot_rules;
        let candidate = crate::storage::play_result::CandidateMetrics {
            ex_score,
            bp,
            cb: bp,
            max_combo,
            clear_rank,
        };
        let mut saved_slots = [false; 4];
        for (slot_index, &rule) in slot_rules.iter().enumerate() {
            let slot = slot_index as u8;
            let prev = match self.boot.score_db.course_replay_slot(course_hash, rule_mode, slot) {
                Ok(record) => record,
                Err(error) => {
                    tracing::warn!(
                        %error,
                        course_hash,
                        rule_mode = rule_mode.as_str(),
                        slot,
                        "failed to read course_replay_slot; skipping rule eval"
                    );
                    continue;
                }
            };
            let prev_metrics = prev.as_ref().map(|p| (p.ex_score, p.bp, p.max_combo, p.clear_rank));
            if !crate::storage::play_result::slot_rule_passes(rule, prev_metrics, &candidate) {
                continue;
            }
            let record = crate::storage::score_db::CourseReplaySlotRecord {
                course_hash: course_hash.to_string(),
                rule_mode,
                slot,
                rule: rule.as_str().to_string(),
                course_score_id,
                played_at,
                ex_score,
                bp,
                max_combo,
                clear_rank,
            };
            match self.boot.score_db.upsert_course_replay_slot(&record) {
                Ok(()) => saved_slots[slot_index] = true,
                Err(error) => {
                    tracing::warn!(
                        %error,
                        course_hash,
                        slot,
                        "failed to upsert course_replay_slot"
                    );
                }
            }
        }
        saved_slots
    }
}
