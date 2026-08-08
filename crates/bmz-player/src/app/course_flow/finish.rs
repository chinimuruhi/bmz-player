use super::course_flow_ir::EnqueueIrCourseJobRequest;
use super::*;

impl WinitApp {
    pub(super) fn finish_active_course(&mut self) {
        // 通常はPlay中に完了済み。極端に短い/早期Failedのコースでも
        // result denominatorを確定してからActiveCourseSessionを消費する。
        self.await_course_metrics();
        if self.play.pending_course_stage_launch.is_some() {
            self.invalidate_play_preload();
        }
        let Some(course) = self.play.active_course.take() else {
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
                    self.result.result_gauge_graph_type = last.summary.gauge_type as i32;
                    self.result.finished_play = Some(last);
                    self.result.result_key5_held = false;
                    self.result.result_key7_held = false;
                    self.result.result_scene_started_at = Instant::now();
                    self.ensure_result_skin_ready(ResultSkinSlot::Course);
                }
                let clear_type = self
                    .result
                    .finished_course
                    .as_ref()
                    .map(|course| course.final_clear_type)
                    .unwrap_or(bmz_core::clear::ClearType::Failed);
                self.play_course_result_entry_sound(clear_type);
                return;
            };
            let course_ln_policy = course_result.ln_policy;
            let course_rule_mode = course_result.rule_mode;
            course_result.previous_best_score = self
                .boot
                .score_db
                .best_course_score(&identity.course_hash, course_ln_policy, course_rule_mode)
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
                ln_policy: course_ln_policy,
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
                    self.enqueue_ir_course_job(EnqueueIrCourseJobRequest {
                        course_id,
                        course_score_id,
                        course_result: &course_result,
                        rule_mode: course_rule_mode,
                        device_type: last_finished.as_ref().map(|f| f.stored.device_type),
                        gauge: &insert.gauge_type,
                        played_at,
                        arrange: &insert.arrange,
                        random_seed: course_result
                            .entry_arranges
                            .first()
                            .and_then(|arrange| arrange.packed_beatoraja_seed_from_sides()),
                    });

                    course_result.course_score_id = Some(course_score_id);
                    course_result.course_played_at = Some(played_at);
                    // Update the four course replay slots that pass their
                    // configured rule.  Reuses the per-chart slot_rule_passes
                    // helper for identical semantics (Always overwrites
                    // unconditionally; Score / Bp / MaxCombo / Clear
                    // require strict improvement; empty slot always wins).
                    course_result.saved_replay_slots = self.update_course_replay_slots(
                        &identity.course_hash,
                        course_ln_policy,
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
                        .course_replay_slot_presence(
                            &identity.course_hash,
                            course_ln_policy,
                            course_rule_mode,
                        )
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
                .best_course_score(&identity.course_hash, course_ln_policy, course_rule_mode)
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
            self.result.result_gauge_graph_type = last.summary.gauge_type as i32;
            self.result.finished_play = Some(last);
            self.result.result_key5_held = false;
            self.result.result_key7_held = false;
            self.result.result_scene_started_at = Instant::now();
            self.ensure_result_skin_ready(ResultSkinSlot::Course);
        }
        let clear_type = self
            .result
            .finished_course
            .as_ref()
            .map(|course| course.final_clear_type)
            .unwrap_or(bmz_core::clear::ClearType::Failed);
        self.play_course_result_entry_sound(clear_type);
    }
}
