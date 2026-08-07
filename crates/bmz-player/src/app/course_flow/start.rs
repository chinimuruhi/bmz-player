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
        let course = self.play.active_course.as_ref()?;
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
        let Some(course) = self.play.active_course.as_ref() else {
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
        self.select.autoplay_folder = None;
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
        let course_metrics = match course_play_metrics_for_definition(
            &self.boot.library_db,
            &definition,
            &app_config,
            self.boot.profile_config.play.ln_mode_policy,
            self.boot.profile_config.play.rule_mode,
            &entry_start_options,
        ) {
            Ok(metrics) => metrics,
            Err(error) => {
                tracing::error!(%error, course_id, "failed to count course notes from source");
                return;
            }
        };
        self.play.active_course = Some(ActiveCourseSession {
            course_id,
            definition,
            course_total_notes: course_metrics.total_notes,
            course_ln_mode: course_metrics.ln_mode,
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
        let course_metrics = match course_play_metrics_for_definition(
            &self.boot.library_db,
            &definition,
            &app_config,
            self.boot.profile_config.play.ln_mode_policy,
            self.boot.profile_config.play.rule_mode,
            &entry_start_options,
        ) {
            Ok(metrics) => metrics,
            Err(error) => {
                tracing::error!(%error, course_id, "failed to count replay course notes from source");
                return;
            }
        };
        self.play.active_course = Some(ActiveCourseSession {
            course_id,
            definition,
            course_total_notes: course_metrics.total_notes,
            course_ln_mode: course_metrics.ln_mode,
            current_index: 0,
            entry_results: Vec::new(),
            entry_start_options,
            auto_advance_intermediate_results,
        });
        self.begin_course_decide_for_chart(first_chart_id, options, &course_title);
    }
}
