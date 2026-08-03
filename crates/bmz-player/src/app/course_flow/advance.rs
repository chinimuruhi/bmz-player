use super::*;

impl WinitApp {
    /// コース曲間の中間リザルト状態かどうか。active_course を保持したまま
    /// finished_play だけが立ち、finished_course はまだ無い状態を指す。
    pub(super) fn is_course_intermediate_result(&self) -> bool {
        is_course_intermediate_result(
            self.play.active_course.is_some(),
            self.result.finished_course.is_some(),
            self.result.finished_play.is_some(),
        )
    }

    pub(super) fn course_intermediate_auto_advance_enabled(&self) -> bool {
        self.play
            .active_course
            .as_ref()
            .is_some_and(|course| course.auto_advance_intermediate_results)
    }

    /// コース曲間の中間リザルト画面を表示する。直前に終わった曲の結果を
    /// finished_play に入れて Result スキンを出すが、active_course は保持し
    /// finished_course は立てないので「中間リザルト」状態になる。
    pub(super) fn show_course_intermediate_result(&mut self) {
        let last = self
            .play
            .active_course
            .as_ref()
            .and_then(|course| course.entry_results.last())
            .map(|entry| entry.finished.clone());
        let Some(last) = last else {
            // 直前結果が無い異常系では中間リザルトを出さず、次の曲へ進む。
            self.start_next_course_chart();
            return;
        };
        self.result.result_gauge_graph_type = last.summary.gauge_type as i32;
        self.result.finished_play = Some(last);
        self.result.result_exit = None;
        self.result.result_key5_held = false;
        self.result.result_key7_held = false;
        self.result.result_scene_started_at = Instant::now();
        self.ensure_result_skin_ready(ResultSkinSlot::Normal);
    }

    /// 中間リザルトを閉じて次の曲へ進む。finished_play をクリアして中間リザルト
    /// 状態を抜け、active_course はそのまま次の曲を開始する。
    pub(super) fn advance_to_next_course_chart(&mut self) {
        self.result.finished_play = None;
        self.result.result_exit = None;
        self.result.result_key5_held = false;
        self.result.result_key7_held = false;
        self.start_next_course_chart();
    }

    pub(super) fn autoplay_folder_has_next(&self) -> bool {
        self.select
            .autoplay_folder
            .as_ref()
            .is_some_and(|session| session.next_index < session.chart_ids.len())
    }

    pub(super) fn advance_autoplay_folder(&mut self) {
        let Some(session) = self.select.autoplay_folder.as_mut() else {
            self.leave_result();
            return;
        };
        let Some(&chart_id) = session.chart_ids.get(session.next_index) else {
            self.select.autoplay_folder = None;
            self.leave_result();
            return;
        };
        session.next_index += 1;
        self.result.finished_play = None;
        self.result.result_exit = None;
        self.result.result_key5_held = false;
        self.result.result_key7_held = false;
        let mut options = self.play_start_options();
        options.session_mode = SessionMode::Autoplay;
        options.autoplay = true;
        self.start_chart_with_options(chart_id, options);
        tracing::info!(chart_id, "advanced folder autoplay");
    }

    pub(super) fn finish_course_after_intermediate_result(&mut self) {
        self.result.finished_play = None;
        self.result.result_exit = None;
        self.result.result_key5_held = false;
        self.result.result_key7_held = false;
        self.finish_active_course();
    }

    /// コースの (current_index が指す) 次の曲を開始する。ゲージ持ち越しや
    /// replay / 同配置 arrange の適用は元の advance_course_after_finish と同じ。
    pub(super) fn start_next_course_chart(&mut self) {
        let Some(course) = &self.play.active_course else {
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
        let Some(course) = self.play.active_course.as_ref() else {
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
        if self.handle_result_ir_scroll_control(control, pressed, repeat) {
            return true;
        }
        if pressed
            && !repeat
            && self.result_input_ready()
            && self.result.result_panel == 1
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
        let Some(course) = &mut self.play.active_course else {
            return;
        };
        let chart_id = self.play.last_started_chart_id.unwrap_or(0);
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
}
