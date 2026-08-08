use super::*;

impl WinitApp {
    pub(super) fn try_start_replay_from_file(&mut self, path: &std::path::Path) -> bool {
        let replay_file = match crate::storage::replay::load_replay(path) {
            Ok(file) => file,
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "replay file load failed");
                return false;
            }
        };
        let Ok(sha) = crate::storage::common::hex_to_hash::<32>(&replay_file.chart_sha256) else {
            tracing::warn!(sha = %replay_file.chart_sha256, "replay file has invalid chart sha256");
            return false;
        };
        let Some(chart_id) = self.boot.library_db.chart_id_by_sha256(sha).ok().flatten() else {
            tracing::warn!(
                sha = %replay_file.chart_sha256,
                "replay chart is not in the library; load the song first"
            );
            return false;
        };
        let player = bmz_gameplay::replay::ReplayPlayer {
            events: replay_file.events.clone(),
            next_index: 0,
        };
        let options = PlayStartOptions {
            session_mode: SessionMode::Normal,
            autoplay: false,
            practice_mode: false,
            replay_player: Some(player),
            chart_zero_time: TimeUs(0),
            gauge: Some(self.select.gauge_option),
            gauge_auto_shift: self.select.gauge_auto_shift_option,
            bottom_shiftable_gauge: self.select.bottom_shiftable_gauge_option,
            arrange: replay_file.arrange_option(),
            arrange_2p: replay_file.arrange_2p_option(),
            double_option: replay_file.double_option(),
            hs_fix: self.select.hs_fix_option,
            target: self.select.target_option,
            arrange_seed: replay_file.arrange_seed,
            arrange_seed_2p: replay_file.arrange_seed_2p,
            random_trainer_seed: None,
            legacy_arrange_seed: replay_file.uses_legacy_seed_scheme(),
            bms_random_seed: None,
            bms_random_choices: replay_file.bms_random_choices.clone(),
            arrange_pattern: replay_file.lane_shuffle_pattern.clone(),
            initial_gauge_value: None,
            initial_gauge_values: None,
            initial_course_combo: None,
            judge_constraint: bmz_core::course::CourseJudgeConstraint::Normal,
            ln_mode_override: None,
            course_gauge_override: None,
            course_gauge_property_override: None,
        };
        self.start_chart_with_options(chart_id, options);
        true
    }

    pub(super) fn start_replay_chart_with_options(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
        show_decide: bool,
    ) {
        if show_decide {
            self.begin_decide_for_chart(chart_id, options);
        } else {
            self.start_chart_with_options(chart_id, options);
        }
    }

    pub(super) fn try_start_replay_for_chart(
        &mut self,
        chart_id: i64,
        slot: u8,
        show_decide: bool,
    ) -> bool {
        let chart = match crate::screens::play_session::load_source_chart_for_chart(
            &self.boot.library_db,
            chart_id,
            None,
        ) {
            Ok(chart) => chart,
            Err(error) => {
                tracing::warn!(chart_id, %error, "replay start failed: source chart load failed");
                return false;
            }
        };
        let sha = chart.identity.file_sha256;
        let key_mode = chart.metadata.key_mode;
        let key = crate::storage::score_db::ScoreKey::with_options(
            sha,
            crate::ln_policy::score_ln_policy_for_chart(
                self.boot.profile_config.play.ln_mode_policy,
                &chart,
            ),
            self.select.double_option.normalize_for_key_mode(key_mode).score_bucket(),
            self.boot.profile_config.play.rule_mode,
        );
        let Some(slot_record) = self.boot.score_db.replay_slot(key, slot).ok().flatten() else {
            tracing::info!(slot, "no replay saved for slot");
            return false;
        };
        let abs_path = self.boot.profile_paths.root_dir.join(&slot_record.replay_path);
        let replay_file = match load_replay_for_chart_policy_and_double_option(
            &abs_path,
            sha,
            slot_record.ln_policy,
            slot_record.double_option,
        ) {
            Ok(file) => file,
            Err(error) => {
                tracing::warn!(%error, path = %abs_path.display(), "replay load failed");
                return false;
            }
        };
        let player = bmz_gameplay::replay::ReplayPlayer {
            events: replay_file.events.clone(),
            next_index: 0,
        };
        let options = PlayStartOptions {
            session_mode: SessionMode::Normal,
            autoplay: false,
            practice_mode: false,
            replay_player: Some(player),
            chart_zero_time: TimeUs(0),
            gauge: Some(self.select.gauge_option),
            gauge_auto_shift: self.select.gauge_auto_shift_option,
            bottom_shiftable_gauge: self.select.bottom_shiftable_gauge_option,
            arrange: replay_file.arrange_option(),
            arrange_2p: replay_file.arrange_2p_option(),
            double_option: replay_file.double_option(),
            hs_fix: self.select.hs_fix_option,
            target: self.select.target_option,
            arrange_seed: replay_file.arrange_seed,
            arrange_seed_2p: replay_file.arrange_seed_2p,
            random_trainer_seed: None,
            legacy_arrange_seed: replay_file.uses_legacy_seed_scheme(),
            bms_random_seed: None,
            bms_random_choices: replay_file.bms_random_choices.clone(),
            arrange_pattern: replay_file.lane_shuffle_pattern.clone(),
            initial_gauge_value: None,
            initial_gauge_values: None,
            initial_course_combo: None,
            judge_constraint: bmz_core::course::CourseJudgeConstraint::Normal,
            ln_mode_override: None,
            course_gauge_override: None,
            course_gauge_property_override: None,
        };
        self.start_replay_chart_with_options(chart_id, options, show_decide);
        true
    }

    pub(super) fn start_replay_for_selected(&mut self, slot: u8) -> bool {
        if let Some(chart_id) = self.currently_selected_chart_id() {
            return self.try_start_replay_for_chart(chart_id, slot, true);
        }
        if let Some(course_id) = self.currently_selected_course_id() {
            return self.try_start_course_replay_for_slot(course_id, slot);
        }
        false
    }

    pub(super) fn currently_selected_chart_id(&self) -> Option<i64> {
        match self.select.select_items.get(self.select.selected_index)? {
            SelectItem::Chart(row) => row.chart.as_ref().map(|chart| chart.chart_id),
            SelectItem::Folder { .. }
            | SelectItem::Course(_)
            | SelectItem::Executable(_)
            | SelectItem::Config(_)
            | SelectItem::KeyBinding(_)
            | SelectItem::SettingsBack
            | SelectItem::SettingsClose
            | SelectItem::AdvancedSettings => None,
        }
    }

    pub(super) fn currently_selected_course_id(&self) -> Option<i64> {
        match self.select.select_items.get(self.select.selected_index)? {
            SelectItem::Course(row) => Some(row.course_id),
            SelectItem::Chart(_)
            | SelectItem::Folder { .. }
            | SelectItem::Executable(_)
            | SelectItem::Config(_)
            | SelectItem::KeyBinding(_)
            | SelectItem::SettingsBack
            | SelectItem::SettingsClose
            | SelectItem::AdvancedSettings => None,
        }
    }

    pub(super) fn try_start_course_replay_for_slot(&mut self, course_id: i64, slot: u8) -> bool {
        let Some((stored, identity)) = self.course_identity_with_stored(course_id) else {
            tracing::warn!(course_id, slot, "course identity unavailable for replay slot");
            return false;
        };
        let ln_policy =
            match crate::screens::select_model::normalized_course_ln_policy_for_definition(
                &self.boot.library_db,
                &stored.definition,
                self.boot.profile_config.play.ln_mode_policy,
            ) {
                Ok(policy) => policy,
                Err(error) => {
                    tracing::warn!(%error, course_id, slot, "course LN policy unavailable for replay slot");
                    return false;
                }
            };
        let rule_mode = self.boot.profile_config.play.rule_mode;
        match self.boot.score_db.course_replay_slot(
            &identity.course_hash,
            ln_policy,
            rule_mode,
            slot,
        ) {
            Ok(Some(record)) => {
                tracing::info!(
                    course_id,
                    course_hash = %identity.course_hash,
                    rule_mode = rule_mode.as_str(),
                    course_score_id = record.course_score_id,
                    slot,
                    "starting course replay from select"
                );
                self.start_course_replay(course_id, record.course_score_id);
                true
            }
            Ok(None) => {
                tracing::info!(
                    course_id,
                    course_hash = %identity.course_hash,
                    rule_mode = rule_mode.as_str(),
                    slot,
                    "no saved course attempt in this replay slot"
                );
                false
            }
            Err(error) => {
                tracing::error!(
                    %error,
                    course_id,
                    course_hash = %identity.course_hash,
                    rule_mode = rule_mode.as_str(),
                    slot,
                    "failed to look up course_replay_slot"
                );
                false
            }
        }
    }
}
