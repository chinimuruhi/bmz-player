use super::*;

impl WinitApp {
    pub(super) fn route_keyboard_input(&mut self, event: &winit::event::KeyEvent) {
        if !event.repeat
            && let Some(device_event) = key_event_to_device_input(event)
            && self.filter_app_input_bounce(device_event).is_none()
        {
            return;
        }
        let control_event = ControlInputEvent::keyboard(event);
        self.input.track_control(&control_event);
        let play_control = control_event.name.as_deref();
        let play_physical_control = control_event.physical.as_ref();
        let has_play_control_context =
            self.play.active_play.is_some() || self.play.pending_play_start.is_some();
        if control_event.pressed
            && !control_event.repeat
            && let Some(control) = play_control
            && self.handle_quick_retry_control(control)
        {
            return;
        }
        if control_event.pressed
            && !control_event.repeat
            && let Some(control) = play_control
            && self.begin_play_fadeout_after_final_notes_control(control)
        {
            return;
        }
        if has_play_control_context && let Some(control) = play_physical_control.as_ref() {
            self.update_play_e1_control_state(
                W_KEYBOARD_DEVICE_ID,
                control,
                event.state == ElementState::Pressed,
            );
        }
        if has_play_control_context
            && let Some(control) = play_physical_control.as_ref()
            && self.update_play_exit_control_state(
                W_KEYBOARD_DEVICE_ID,
                control,
                event.state == ElementState::Pressed,
            )
        {
            return;
        }
        let window_keyboard_gameplay_enabled = self.window_keyboard_gameplay_enabled();
        self.input.track_window_keyboard(
            event.physical_key,
            event.state,
            event.repeat,
            window_keyboard_gameplay_enabled,
            has_play_control_context,
        );
        if has_play_control_context
            && window_keyboard_gameplay_enabled
            && let Some(device_event) = key_event_to_device_input(event)
        {
            self.route_play_device_input(device_event);
        }
        let play_option_lane_action = if event.state == ElementState::Pressed && !event.repeat {
            play_physical_control
                .as_ref()
                .and_then(|control| {
                    play_option_control_for_input(
                        W_KEYBOARD_DEVICE_ID,
                        control,
                        self.play.play_e1_held,
                        self.play.play_e2_held,
                        self.play.play_option_input.as_ref(),
                        &self.boot.profile_config.input,
                    )
                })
                .and_then(|action| lane_action_from_option(action, false))
        } else {
            None
        };
        let fixed_play_lane_action = keyboard_lane_action(&control_event);
        if self.play.active_play.is_some() {
            let lane_cover_changing = self
                .play
                .active_play
                .as_ref()
                .is_some_and(|play| play.running.session.lane_cover_changing);
            if lane_cover_changing && let Some(action) = play_option_lane_action {
                self.apply_play_lane_action(action);
                // E1+lane keys should still reach gameplay input so notes are judged
                // and key beams render while changing play options.
            }
            if let Some(action) = fixed_play_lane_action {
                self.apply_play_lane_action(action);
                return;
            }
            if event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                && event.state == ElementState::Pressed
                && !event.repeat
            {
                self.stop_active_play_like_escape("escape pressed during play");
                return;
            }
            // Start / E1 の2回連続押し → レーンカバー表示切替
            if control_event.pressed
                && !control_event.repeat
                && let Some(control) = play_control
                && self.select.select_keys.is_start(control)
            {
                self.handle_play_start_double_press();
                // Start キーはゲームプレイ入力としても通すのでフォールスルー
            }
            return;
        }

        if self.play.pending_decide.is_some() {
            if let Some(control) = control_event.name.as_deref()
                && !event.repeat
                && self.update_decide_cancel_control_state(
                    control,
                    event.state == ElementState::Pressed,
                )
            {
                return;
            }
            if let Some(action) = scene_decide_action(&control_event, &self.select.select_keys) {
                self.begin_decide_fadeout(matches!(action, DecideAction::Cancel));
            }
            return;
        }

        if self.play.pending_play_start.is_some() {
            if let Some(action) = fixed_play_lane_action {
                self.apply_play_lane_action(action);
                return;
            }
            if let Some(action) = play_option_lane_action {
                self.apply_play_lane_action(action);
                return;
            }
            if event.state == ElementState::Pressed
                && !event.repeat
                && let Some(control) = play_control
                && self.select.select_keys.is_start(control)
            {
                self.handle_play_start_double_press();
            }
            return;
        }

        // コース曲間の中間リザルト: リトライ無効、次の曲へ進むだけ。Key6 の
        // ゲージグラフ切替のみ単曲リザルト同様に許可する。retry を持つ単曲
        // リザルト分岐より先に評価し、R/Key5/Key7 等での誤 retry を防ぐ。
        if self.is_course_intermediate_result() {
            let pressed = event.state == ElementState::Pressed;
            if self.request_result_exit_skip_for_key(event.physical_key, event.state, event.repeat)
            {
                return;
            }
            if self.result.result_exit.is_none()
                && let Some(control) = physical_key_to_control(event.physical_key)
                && self.handle_course_intermediate_control(&control, pressed, event.repeat)
            {
                return;
            }
            if let Some(control) = physical_key_to_control(event.physical_key)
                && self.request_result_exit_skip_for_control(&control, pressed, event.repeat)
            {
                return;
            }
            if self.result.result_exit.is_none()
                && self.result_input_ready()
                && event.state == ElementState::Pressed
                && !event.repeat
                && let Some(slot) = digit_to_replay_slot(event.physical_key)
            {
                self.save_finished_play_replay_slot(slot);
                return;
            }
            if self.result.result_exit.is_none()
                && self.result_input_ready()
                && scene_result_action(&control_event).is_some()
            {
                // R / Enter / Escape いずれも次の曲へ進むだけ (retry/leave 区別なし)。
                self.begin_result_exit(self.course_intermediate_exit_action());
            }
            return;
        }

        if self.result.finished_play.is_some() && self.result.finished_course.is_none() {
            let pressed = event.state == ElementState::Pressed;
            if let Some(control) = physical_key_to_control(event.physical_key) {
                // フェードアウト中でも Key5/Key7 の押下状態は追跡し、
                // アニメーション終了時の retry arrange 判定に使う。
                self.track_result_lane_hold(&control, pressed);
                if self.request_result_exit_skip_for_key(
                    event.physical_key,
                    event.state,
                    event.repeat,
                ) || self.request_result_exit_skip_for_control(&control, pressed, event.repeat)
                {
                    return;
                }
                // 終了アニメーション中 (result_exit=Some) は held 追跡のみで、
                // 新しいアクションは受け付けない。
                if self.result.result_exit.is_none()
                    && self.handle_result_control(&control, pressed, event.repeat)
                {
                    return;
                }
            }
            if self.result.result_exit.is_none()
                && self.result_input_ready()
                && event.state == ElementState::Pressed
                && !event.repeat
                && let Some(slot) = digit_to_replay_slot(event.physical_key)
            {
                self.save_finished_play_replay_slot(slot);
                return;
            }
            if self.result.result_exit.is_none()
                && self.result_input_ready()
                && let Some(action) = scene_result_action(&control_event)
            {
                self.apply_result_action(action, false);
            }
            return;
        }

        // コース（段位）リザルト: Key5/Key7 はフェードアウト後の hold 状態で
        // retry arrange を決める。Key6 はゲージグラフ切替。
        if self.result.finished_course.is_some() {
            let pressed = event.state == ElementState::Pressed;
            if let Some(control) = physical_key_to_control(event.physical_key) {
                self.track_result_lane_hold(&control, pressed);
                if self.request_result_exit_skip_for_key(
                    event.physical_key,
                    event.state,
                    event.repeat,
                ) || self.request_result_exit_skip_for_control(&control, pressed, event.repeat)
                {
                    return;
                }
                if self.result.result_exit.is_none()
                    && self.handle_course_result_control(&control, pressed, event.repeat)
                {
                    return;
                }
            }
            if self.result.result_exit.is_none()
                && self.result_input_ready()
                && event.state == ElementState::Pressed
                && !event.repeat
                && let Some(slot) = digit_to_replay_slot(event.physical_key)
            {
                self.save_finished_course_replay_slot(slot);
                return;
            }
            if self.result.result_exit.is_none()
                && self.result_input_ready()
                && let Some(action) = scene_result_action(&control_event)
            {
                self.apply_result_action(action, true);
            }
            return;
        }

        if matches!(self.view_state(), AppViewState::Select)
            && event.physical_key == PhysicalKey::Code(KeyCode::F5)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            self.reload_from_select_context();
            return;
        }

        if matches!(self.view_state(), AppViewState::Select)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            match event.physical_key {
                PhysicalKey::Code(KeyCode::F3) => self.handle_select_f3_action(),
                PhysicalKey::Code(KeyCode::F10) => self.start_autoplay_folder_selected(),
                PhysicalKey::Code(KeyCode::F11) => self.open_primary_ir_for_selected(),
                PhysicalKey::Code(KeyCode::Numpad9) => self.open_selected_chart_documents(),
                _ => {}
            }
            if matches!(
                event.physical_key,
                PhysicalKey::Code(KeyCode::F3 | KeyCode::F10 | KeyCode::F11 | KeyCode::Numpad9)
            ) {
                return;
            }
        }

        if matches!(self.view_state(), AppViewState::Select)
            && event.state == ElementState::Released
            && let Some(control) = physical_key_name(event.physical_key)
        {
            self.update_select_e_action_hold(&control, false);
        }

        // 検索モード中はテキスト入力を最優先で処理し、通常ナビゲーションは抑制する。
        // モード入りトリガ (`/`) も同じ select 画面チェックの直後に処理する。
        if matches!(self.view_state(), AppViewState::Select)
            && !in_settings_stack(&self.select.folder_stack)
            && self.handle_search_key(event)
        {
            return;
        }

        // Select 画面で ESC 長押し → アプリ終了 (実際の exit は redraw 時にチェック)。
        if event.physical_key == PhysicalKey::Code(KeyCode::Escape) {
            if in_settings_stack(&self.select.folder_stack)
                && event.state == ElementState::Pressed
                && !event.repeat
            {
                if self.select.key_config_edit.is_some() {
                    self.cancel_key_config_edit();
                    return;
                }
                if self.select.settings_edit.is_some() {
                    self.cancel_settings_edit();
                    return;
                }
            }
            match event.state {
                ElementState::Pressed => {
                    if self.select.select_exit_hold_started_at.is_none() {
                        self.select.select_exit_hold_started_at = Some(Instant::now());
                    }
                }
                ElementState::Released => {
                    self.select.select_exit_hold_started_at = None;
                }
            }
            return;
        }

        if in_settings_stack(&self.select.folder_stack) {
            if event.state == ElementState::Released
                && let Some(control_name) = physical_key_name(event.physical_key)
            {
                self.clear_select_hold_control(&control_name);
                return;
            }
            if self.select.key_config_edit.is_some()
                && event.state == ElementState::Pressed
                && !event.repeat
            {
                if event.physical_key == PhysicalKey::Code(KeyCode::Delete)
                    || event.physical_key == PhysicalKey::Code(KeyCode::Backspace)
                {
                    self.clear_key_config_binding();
                    return;
                }
                if let Some(control) = physical_key_name(event.physical_key) {
                    if control == "Escape" {
                        self.cancel_key_config_edit();
                    } else if control == "Delete" || control == "Backspace" {
                        self.clear_key_config_binding();
                    } else {
                        self.apply_key_config_control(&control);
                    }
                }
                return;
            }
            if !should_route_settings_key_event(
                event.state,
                event.repeat,
                self.select.settings_edit.is_some(),
            ) {
                return;
            }
            if let Some(control) = physical_key_name(event.physical_key) {
                self.route_settings_control(&control);
            } else {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowUp) => {
                        let _ = self.route_settings_control("ArrowUp");
                    }
                    PhysicalKey::Code(KeyCode::ArrowDown) => {
                        let _ = self.route_settings_control("ArrowDown");
                    }
                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        let _ = self.route_settings_control("ArrowLeft");
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                        let _ = self.route_settings_control("ArrowRight");
                    }
                    PhysicalKey::Code(KeyCode::Enter) => {
                        let _ = self.route_settings_control("Enter");
                    }
                    PhysicalKey::Code(KeyCode::Space) => {
                        let _ = self.route_settings_control("Space");
                    }
                    PhysicalKey::Code(KeyCode::Escape) => {
                        let _ = self.route_settings_control("Escape");
                    }
                    _ => {}
                }
            }
            return;
        }

        if let Some(control) = physical_key_name(event.physical_key) {
            self.update_select_e_action_hold(&control, event.state == ElementState::Pressed);
        }

        if event.state == ElementState::Pressed
            && !event.repeat
            && self.select.select_option_panel == 0
            && self.select_ir_scope_toggle_is_e3()
            && let Some(control) = physical_key_name(event.physical_key)
            && self.is_select_ir_scope_toggle_control(&control)
            && self.toggle_select_ir_scope()
        {
            return;
        }

        if is_select_start_key(event.physical_key, &self.select.select_keys) {
            self.set_start_held(event.state == ElementState::Pressed);
            return;
        }

        if event.state == ElementState::Pressed
            && !event.repeat
            && let Some(control) = physical_key_name(event.physical_key)
            && should_toggle_select_judge_auto_adjust(
                &control,
                self.input.start_held,
                self.input.select_held,
                &self.select.select_keys,
            )
        {
            self.toggle_visual_offset_auto_adjust();
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            if is_select_modifier_key(event.physical_key, &self.select.select_keys) {
                self.set_select_held(true);
            }
            return;
        }

        if event.state == ElementState::Pressed
            && !event.repeat
            && let Some(control) = physical_key_name(event.physical_key)
            && should_toggle_select_gauge_auto_shift(
                &control,
                self.input.start_held,
                self.input.select_held,
                &self.select.select_keys,
            )
        {
            self.toggle_gauge_auto_shift();
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            if is_select_modifier_key(event.physical_key, &self.select.select_keys) {
                self.set_select_held(true);
            }
            return;
        }

        if is_select_modifier_key(event.physical_key, &self.select.select_keys) {
            self.set_select_held(event.state == ElementState::Pressed);
            return;
        }

        if self.select.select_option_panel != 0 {
            if event.state == ElementState::Pressed
                && (!event.repeat
                    || (self.select.select_option_panel == 3
                        && physical_key_name(event.physical_key).is_some_and(|control| {
                            green_number_delta_control(&control, &self.select.select_keys).is_some()
                        })))
            {
                match self.select.select_option_panel {
                    1 => {
                        if let Some(slot) = digit_to_replay_slot(event.physical_key) {
                            if !self.start_replay_for_selected(slot) {
                                tracing::info!(slot, "Start+digit pressed but no replay available");
                            }
                            return;
                        }
                        if let Some(cycle) = target_cycle_from_key(event.physical_key) {
                            self.apply_target_option_cycle(cycle);
                            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                            return;
                        }
                        if let Some(control) = physical_key_name(event.physical_key)
                            && let Some(cycle) =
                                target_cycle_from_control(&control, &self.select.select_keys)
                        {
                            self.apply_target_option_cycle(cycle);
                            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                            return;
                        }
                        if let Some(control) = physical_key_name(event.physical_key)
                            && self.apply_play_option_control(&control)
                        {
                            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                        }
                    }
                    3 => {
                        if let Some(control) = physical_key_name(event.physical_key)
                            && self.apply_detail_option_control(&control)
                        {
                            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                        }
                    }
                    _ => {}
                }
            }
            return;
        }

        if matches!(self.view_state(), AppViewState::Select) {
            if let Some(action) = scene_select_action(&control_event, &self.select.select_keys) {
                self.apply_select_action(action, control_event.name.as_deref());
            } else if event.state == ElementState::Released
                && let Some(control_name) = control_event.name.as_deref()
            {
                self.clear_select_hold_control(control_name);
            }
        }
    }

    pub(super) fn poll_gamepad_events(&mut self) {
        let should_log_raw_input = self.should_log_gamepad_key_config_raw_input();
        let Some(gamepad) = &mut self.gamepad else { return };
        let backend_name = gamepad.name();
        let output = gamepad.poll();
        if self.input.should_discard_gamepad_output(self.ui.focused) {
            for event in output
                .buttons
                .iter()
                .filter(|event| should_route_gamepad_event_while_discarding(event.pressed))
            {
                self.route_gamepad_button_event(event);
            }
            self.reset_select_analog_scroll();
            self.reset_play_analog_scroll();
            return;
        }
        if should_log_raw_input {
            for event in &output.raw_events {
                log_gamepad_key_config_raw_event(backend_name, event);
            }
        }
        #[cfg(windows)]
        if let Some(diagnostics) = gamepad.gameinput_diagnostics()
            && diagnostics.reading_count > 0
        {
            tracing::trace!(
                reading_count = diagnostics.reading_count,
                oldest_reading_age_us = diagnostics.oldest_reading_age_us,
                "GameInput main-thread poll"
            );
        }
        for event in &output.buttons {
            self.route_gamepad_button_event(event);
        }
        for tick in &output.axis_ticks {
            // キーコンフィグ待ち受け中は合成 Press を待たず、生 tick から直接捕捉する。
            // 軸が active のままでも (押しっぱなし扱いで Press が出なくても) 確実に拾える。
            if self.select.key_config_edit.as_ref().is_some_and(|session| session.listening) {
                let control = format!("{}{}", tick.name, if tick.ticks > 0 { "+" } else { "-" });
                self.apply_key_config_gamepad(&control);
                continue;
            }
            self.route_gamepad_axis_ticks(&tick.name, tick.ticks);
        }
    }

    pub(super) fn route_gamepad_button_event(
        &mut self,
        event: &crate::input::gamepad::GamepadButtonEvent,
    ) {
        let mut device_event = crate::input::gamepad::to_device_input_event(event);
        if should_bypass_analog_scratch_bounce(
            event,
            self.play.play_option_input.as_ref().map(|input| &input.binding),
        ) {
            device_event.bounce_policy = InputBouncePolicy::Bypass;
        }
        let Some(device_event) = self.filter_app_input_bounce(device_event) else {
            return;
        };
        self.route_play_device_input(device_event);
        self.route_gamepad_button(event.device_id, &event.name, event.pressed);
    }

    pub(super) fn should_log_gamepad_key_config_raw_input(&self) -> bool {
        self.select
            .key_config_edit
            .as_ref()
            .is_some_and(|session| session.listening && session.target.slot().is_controller())
    }

    pub(super) fn route_gamepad_axis_ticks(&mut self, axis: &str, ticks: i32) {
        if self.apply_play_analog_option_ticks(axis, ticks) {
            return;
        }
        self.accumulate_select_analog_ticks(axis, ticks);
    }

    pub(super) fn apply_play_analog_option_ticks(&mut self, axis: &str, ticks: i32) -> bool {
        let Some(delta) = play_analog_lane_cover_delta(axis, ticks, &self.select.select_keys)
        else {
            return false;
        };
        let mode = match (self.play.play_e1_held, self.play.play_e2_held) {
            (true, false) => PlayAnalogOptionMode::LaneCover,
            (false, true) => PlayAnalogOptionMode::GreenNumber,
            _ => {
                self.reset_play_analog_scroll();
                return false;
            }
        };
        let lane_value_changing = self
            .play
            .active_play
            .as_ref()
            .is_some_and(|active_play| active_play.running.session.lane_cover_changing)
            || self
                .play
                .pending_play_start
                .as_ref()
                .is_some_and(|pending| pending.lane.lane_cover_changing);
        if !lane_value_changing {
            self.reset_play_analog_scroll();
            return false;
        }

        let now = Instant::now();
        let idle = self.play.play_analog_last_tick_at.is_none_or(|t| {
            now.duration_since(t) > Duration::from_millis(SELECT_ANALOG_SCROLL_TOLERANCE_MS)
        });
        self.play.play_analog_last_tick_at = Some(now);
        if idle {
            self.play.play_analog_scroll_buffer = 0;
        }
        self.play.play_analog_scroll_buffer += delta;

        let ticks_per_scroll = self.boot.profile_config.input.analog_ticks_per_scroll.max(1) as i32;
        let steps =
            take_analog_scroll_steps(&mut self.play.play_analog_scroll_buffer, ticks_per_scroll);
        if steps == 0 {
            return true;
        }

        let change = if steps > 0 { LaneCoverChange::Down } else { LaneCoverChange::Up };
        let action = match mode {
            PlayAnalogOptionMode::LaneCover => {
                PlayLaneAction::LaneCoverDelta(lane_cover_change_step(change) * steps.abs() as f32)
            }
            PlayAnalogOptionMode::GreenNumber => PlayLaneAction::GreenNumberDelta(
                green_number_change_step(green_number_change_from_analog_steps(steps))
                    * steps.abs(),
            ),
        };
        self.apply_play_lane_action(action);
        true
    }

    /// 選曲画面のアナログスクラッチ tick を蓄積する。回転量比例スクロール用。
    pub(super) fn accumulate_select_analog_ticks(&mut self, axis: &str, ticks: i32) {
        if !matches!(self.view_state(), AppViewState::Select)
            || self.play.active_play.is_some()
            || self.play.pending_decide.is_some()
            || self.play.pending_play_start.is_some()
            || self.select.key_config_edit.is_some()
            || (self.select.select_option_panel > 1 && self.select.settings_edit.is_none())
        {
            return;
        }
        let Some(delta) = select_analog_scroll_delta(axis, ticks, &self.select.select_keys) else {
            return;
        };
        let now = Instant::now();
        // tick が途切れていたら古い端数を捨てる (beatoraja の 200ms tolerance 相当)
        let idle = self.select.select_analog_last_tick_at.is_none_or(|t| {
            now.duration_since(t) > Duration::from_millis(SELECT_ANALOG_SCROLL_TOLERANCE_MS)
        });
        self.select.select_analog_last_tick_at = Some(now);
        update_analog_scroll_buffer(
            &mut self.select.select_analog_scroll_buffer,
            &mut self.select.select_analog_suppress_until_idle,
            idle,
            delta,
        );
    }

    /// キーコンフィグ確定/キャンセル後、回転中のスクラッチが止まるまで
    /// アナログスクロールを無効化する。
    pub(super) fn suppress_select_analog_until_idle(&mut self) {
        self.select.select_analog_suppress_until_idle = true;
        self.select.select_analog_scroll_buffer = 0;
        self.select.select_analog_last_tick_at = Some(Instant::now());
    }

    pub(super) fn reset_select_analog_scroll(&mut self) {
        self.select.select_analog_scroll_buffer = 0;
        self.select.select_analog_last_tick_at = None;
        self.select.select_analog_suppress_until_idle = false;
    }

    pub(super) fn reset_play_analog_scroll(&mut self) {
        self.play.play_analog_scroll_buffer = 0;
        self.play.play_analog_last_tick_at = None;
    }

    /// 蓄積したアナログ tick を analog_ticks_per_scroll ごとに 1 移動へ変換する。
    /// beatoraja MusicSelectInputProcessor の analogScrollBuffer と同じ仕組み。
    pub(super) fn advance_select_analog_scroll(&mut self) {
        if !self.ui.focused {
            self.reset_select_analog_scroll();
            return;
        }
        if !matches!(self.view_state(), AppViewState::Select) {
            self.reset_select_analog_scroll();
            return;
        }
        if self.select.key_config_edit.is_some() {
            self.reset_select_analog_scroll();
            return;
        }
        let ticks_per_scroll = self.boot.profile_config.input.analog_ticks_per_scroll.max(1) as i32;
        let mov = take_analog_scroll_steps(
            &mut self.select.select_analog_scroll_buffer,
            ticks_per_scroll,
        );
        if mov == 0 {
            return;
        }
        if self.select.settings_edit.is_some() {
            let direction = settings_edit_direction_from_analog_scroll(mov);
            for _ in 0..mov.abs() {
                self.adjust_settings_edit(direction);
            }
            return;
        }
        if self.select.select_option_panel > 1 {
            self.reset_select_analog_scroll();
            return;
        }
        if self.select.select_option_panel == 1 {
            let cycle = if mov > 0 { TargetCycle::Next } else { TargetCycle::Previous };
            for _ in 0..mov.abs() {
                self.apply_target_option_cycle(cycle);
            }
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
        } else {
            for _ in 0..mov.abs() {
                self.move_selection_with_duration(
                    if mov > 0 { SelectMove::Next } else { SelectMove::Previous },
                    select_analog_scroll_duration(mov),
                );
            }
        }
    }

    pub(super) fn route_gamepad_button(&mut self, device: DeviceId, button: &str, pressed: bool) {
        let control_event = ControlInputEvent::gamepad(device, button, pressed);
        self.input.track_control(&control_event);
        let physical_control =
            control_event.physical.as_ref().expect("gamepad control always has a physical value");
        let has_play_control_context =
            self.play.active_play.is_some() || self.play.pending_play_start.is_some();
        if pressed && self.handle_quick_retry_control(button) {
            return;
        }
        if pressed && self.begin_play_fadeout_after_final_notes_control(button) {
            return;
        }
        let play_e1_control = has_play_control_context
            && self.update_play_e1_control_state(device, physical_control, pressed);
        if has_play_control_context
            && self.update_play_exit_control_state(device, physical_control, pressed)
        {
            return;
        }
        let play_option_control = pressed.then(|| {
            play_option_control_for_input(
                device,
                physical_control,
                self.play.play_e1_held,
                self.play.play_e2_held,
                self.play.play_option_input.as_ref(),
                &self.boot.profile_config.input,
            )
        });
        let play_option_control = play_option_control.flatten();
        let play_option_lane_action = play_option_control
            .and_then(|action| lane_action_from_option(action, button.starts_with("Axis")));
        if pressed {
            let lane_cover_changing = self
                .play
                .active_play
                .as_ref()
                .is_some_and(|play| play.running.session.lane_cover_changing);
            if lane_cover_changing && play_option_control.is_some() {
                let Some(action) = play_option_lane_action else {
                    return;
                };
                self.apply_play_lane_action(action);
                // Gamepad play input was already queued in poll_gamepad_events.
            }
        }
        if !pressed {
            if in_settings_stack(&self.select.folder_stack) {
                self.clear_select_hold_control(button);
                return;
            }
            self.update_select_e_action_hold(button, false);
            if self.select.select_keys.is_start(button) {
                self.set_start_held(false);
            } else if self.select.select_keys.is_e2_action(button) || matches!(button, "Select") {
                self.set_select_held(false);
            }
            return;
        }

        self.update_select_e_action_hold(button, true);

        // プレイ中: Start / E1 の2回連続押しでレーンカバー表示切替。
        // プレイ入力自体は push_shared_event で処理済み。
        if self.play.active_play.is_some() {
            if play_e1_control {
                self.handle_play_start_double_press();
            }
            return;
        }

        if self.play.pending_decide.is_some() {
            if self.update_decide_cancel_control_state(button, pressed) {
                return;
            }
            if let Some(action) = scene_decide_action(&control_event, &self.select.select_keys) {
                self.begin_decide_fadeout(matches!(action, DecideAction::Cancel));
            }
            return;
        }

        if self.play.pending_play_start.is_some() {
            if play_option_control.is_some() {
                if let Some(action) = play_option_lane_action {
                    self.apply_play_lane_action(action);
                }
                return;
            }
            if play_e1_control {
                self.handle_play_start_double_press();
            }
            return;
        }

        // コース曲間の中間リザルト: リトライ無効、次の曲へ進むだけ。
        // retry を持つ単曲リザルト分岐より先に評価する。
        if self.is_course_intermediate_result() {
            let control = PhysicalControl::GamepadButton(button.to_string());
            if self.request_result_exit_skip_for_control(&control, pressed, false) {
                return;
            }
            if self.result.result_exit.is_none() {
                if self.handle_course_intermediate_control(&control, pressed, false) {
                    return;
                }
                if self.result_input_ready() && scene_result_action(&control_event).is_some() {
                    self.begin_result_exit(self.course_intermediate_exit_action());
                }
            }
            return;
        }

        // リザルト画面
        if self.result.finished_play.is_some() && self.result.finished_course.is_none() {
            let control = PhysicalControl::GamepadButton(button.to_string());
            // フェードアウト中でも Key5/Key7 の押下状態は追跡する。
            self.track_result_lane_hold(&control, pressed);
            if self.request_result_exit_skip_for_control(&control, pressed, false) {
                return;
            }
            // 終了アニメーション中 (result_exit=Some) は held 追跡のみ行う。
            if self.result.result_exit.is_none() {
                if self.handle_result_control(&control, pressed, false) {
                    return;
                }
                if self.result_input_ready()
                    && let Some(action) = scene_result_action(&control_event)
                {
                    self.apply_result_action(action, false);
                }
            }
            return;
        }

        // コース（段位）リザルト: Key5/Key7 はフェードアウト後の hold 状態で
        // retry arrange を決める。Button1/Start は同配置リトライ。
        if self.result.finished_course.is_some() {
            let control = PhysicalControl::GamepadButton(button.to_string());
            self.track_result_lane_hold(&control, pressed);
            if self.request_result_exit_skip_for_control(&control, pressed, false) {
                return;
            }
            if self.result.result_exit.is_none() {
                if self.handle_course_result_control(&control, pressed, false) {
                    return;
                }
                if self.result_input_ready()
                    && let Some(action) = scene_result_action(&control_event)
                {
                    self.apply_result_action(action, true);
                }
            }
            return;
        }

        if in_settings_stack(&self.select.folder_stack) {
            if self.select.key_config_edit.as_ref().is_some_and(|session| session.listening) {
                if pressed {
                    self.apply_key_config_gamepad(button);
                }
                return;
            }
            if pressed {
                let _ = self.route_settings_control(button);
            }
            return;
        }

        if self.select.select_option_panel == 0
            && self.select_ir_scope_toggle_is_e3()
            && self.is_select_ir_scope_toggle_control(button)
            && self.toggle_select_ir_scope()
        {
            return;
        }

        if should_toggle_select_gauge_auto_shift(
            button,
            self.input.start_held,
            self.input.select_held,
            &self.select.select_keys,
        ) {
            self.toggle_gauge_auto_shift();
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            if self.select.select_keys.is_e2_action(button) {
                self.set_select_held(true);
            }
            return;
        }

        if should_toggle_select_judge_auto_adjust(
            button,
            self.input.start_held,
            self.input.select_held,
            &self.select.select_keys,
        ) {
            self.toggle_visual_offset_auto_adjust();
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            if self.select.select_keys.is_e2_action(button) {
                self.set_select_held(true);
            }
            return;
        }

        if self.select.select_keys.is_start(button) {
            self.set_start_held(true);
            return;
        }

        if self.select.select_keys.is_e2_action(button) || matches!(button, "Select") {
            self.set_select_held(true);
            return;
        }

        if self.select.select_option_panel != 0 {
            if self.select.select_option_panel == 1
                && let Some(cycle) = target_cycle_from_control(button, &self.select.select_keys)
            {
                if button.starts_with("Axis") {
                    return;
                }
                self.apply_target_option_cycle(cycle);
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                return;
            }
            let option_changed = match self.select.select_option_panel {
                1 => self.apply_gamepad_play_option_control(device, button),
                3 => self.apply_detail_option_control(button),
                _ => false,
            };
            if option_changed {
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            return;
        }

        if matches!(self.view_state(), AppViewState::Select) {
            // アナログ軸にバインドされたスクラッチは tick 比例スクロール
            // (advance_select_analog_scroll) で処理する。beatoraja の isNonAnalogPressed 相当。
            if button.starts_with("Axis")
                && (self.select.select_keys.is_select_scratch_up(button)
                    || self.select.select_keys.is_select_scratch_down(button))
            {
                return;
            }
            if let Some(action) = scene_select_action(&control_event, &self.select.select_keys) {
                self.apply_select_action(action, Some(button));
            }
        }
    }

    pub(super) fn route_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        if let Some(change) = lane_cover_wheel_change(delta)
            && (self.play.active_play.is_some() || self.play.pending_play_start.is_some())
        {
            self.apply_play_lane_action(PlayLaneAction::LaneCoverDelta(lane_cover_change_step(
                change,
            )));
            return;
        }
        if !matches!(self.view_state(), AppViewState::Select) {
            return;
        }
        if in_settings_stack(&self.select.folder_stack) && self.select.settings_edit.is_some() {
            let direction = settings_edit_direction_from_mouse_wheel(delta);
            if direction != 0 {
                self.adjust_settings_edit(direction);
            }
            return;
        }
        if let Some(select_move) = select_wheel_move(delta) {
            self.move_selection(select_move);
        }
    }

    pub(super) fn route_mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if state == ElementState::Released {
            self.select.select_slider_dragging_type = None;
            return;
        }
        if state != ElementState::Pressed {
            return;
        }
        let Some((x, y)) = self.cursor_position_normalized() else {
            return;
        };
        if matches!(self.view_state(), AppViewState::Result) {
            self.select.select_slider_dragging_type = None;
            if button == MouseButton::Left && self.result.result_exit.is_none() {
                let AppSceneSnapshot::Result(snapshot) = self.scene_snapshot() else {
                    return;
                };
                if let Some(hit) = self.renderer.result_skin_slider_hit(&snapshot, x, y) {
                    self.select.select_slider_dragging_type = Some(hit.slider_type);
                    self.apply_result_slider_hit(hit);
                    return;
                }
                self.handle_result_skin_click(x, y);
            }
            return;
        }
        if !matches!(self.view_state(), AppViewState::Select) {
            self.select.select_slider_dragging_type = None;
            return;
        }
        if button == MouseButton::Left
            && !in_settings_stack(&self.select.folder_stack)
            && self.select_search_word_hit(x, y)
        {
            if !self.select.search.is_active() {
                self.set_search_mode(true);
                tracing::info!("entered song search mode from mouse click");
            } else {
                self.search_cursor_to_end();
            }
            return;
        }
        let snapshot = self.select_snapshot();
        if button == MouseButton::Left
            && let Some(hit) = self.renderer.select_skin_slider_hit(&snapshot, x, y)
        {
            self.select.select_slider_dragging_type = Some(hit.slider_type);
            self.apply_select_slider_hit(hit);
            return;
        }
        let Some(hit) = self.renderer.select_skin_click_hit(&snapshot, x, y) else {
            return;
        };
        self.handle_select_skin_click(hit, button, x, y);
    }

    pub(super) fn handle_result_skin_click(&mut self, x: f32, y: f32) {
        let AppSceneSnapshot::Result(snapshot) = self.scene_snapshot() else {
            return;
        };
        let Some(hit) = self.renderer.result_skin_click_hit(&snapshot, x, y) else {
            return;
        };
        let SkinClickTarget::Event { event_id, .. } = hit.target else {
            return;
        };
        match result_skin_click_action(event_id) {
            Some(ResultSkinClickAction::SetPanel(panel)) => {
                self.set_result_panel(panel);
            }
            Some(ResultSkinClickAction::SelectIrScope(tab)) => {
                self.select_result_ir_scope(tab);
            }
            Some(ResultSkinClickAction::ToggleIrScope) => {
                self.toggle_result_ir_scope();
            }
            Some(ResultSkinClickAction::ToggleFavoriteChart) => {
                self.toggle_favorite_chart_result();
            }
            Some(ResultSkinClickAction::SaveReplay(slot)) => {
                if self.result.finished_course.is_some() {
                    self.save_finished_course_replay_slot(slot);
                } else {
                    self.save_finished_play_replay_slot(slot);
                }
            }
            Some(ResultSkinClickAction::ResetDailyStatistics) => {
                self.reset_daily_statistics();
            }
            None => {
                let _ = self.renderer.dispatch_result_skin_runtime_event(event_id);
            }
        }
    }

    pub(super) fn route_select_slider_drag(&mut self) {
        if self.select.select_slider_dragging_type.is_none() {
            return;
        }
        let Some((x, y)) = self.cursor_position_normalized() else {
            return;
        };
        if matches!(self.view_state(), AppViewState::Result) {
            if self.result.result_exit.is_some() {
                return;
            }
            let AppSceneSnapshot::Result(snapshot) = self.scene_snapshot() else {
                return;
            };
            if let Some(hit) = self.renderer.result_skin_slider_hit(&snapshot, x, y) {
                self.apply_result_slider_hit(hit);
            }
            return;
        }
        if !matches!(self.view_state(), AppViewState::Select) {
            return;
        }
        let snapshot = self.select_snapshot();
        if let Some(hit) = self.renderer.select_skin_slider_hit(&snapshot, x, y) {
            self.apply_select_slider_hit(hit);
        }
    }

    pub(super) fn cursor_position_normalized(&self) -> Option<(f32, f32)> {
        let window = self.window.as_ref()?;
        let position = self.select.last_cursor_position?;
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return None;
        }
        Some((
            (position.x as f32 / size.width as f32).clamp(0.0, 1.0),
            (position.y as f32 / size.height as f32).clamp(0.0, 1.0),
        ))
    }

    pub(super) fn select_search_word_hit(&self, x: f32, y: f32) -> bool {
        let Some(document) = self.renderer.select_skin_document() else {
            return false;
        };
        let Some((rect_x, rect_y, rect_w, rect_h)) = document.text_destination_rect_for_ref(30)
        else {
            return false;
        };
        x >= rect_x && x <= rect_x + rect_w && y >= rect_y && y <= rect_y + rect_h
    }

    pub(super) fn search_cursor_to_end(&mut self) {
        self.select.search.cursor_to_end();
        self.update_search_ime_cursor_area();
    }

    pub(super) fn apply_select_slider_hit(&mut self, hit: SkinSliderHit) {
        match hit.slider_type {
            1 => self.apply_select_scroll_slider(hit.value),
            17..=19 => {
                let value = volume_f32_to_unit(hit.value);
                let mix = &mut self.boot.profile_config.audio_mix;
                match hit.slider_type {
                    17 if mix.master_volume != value => {
                        mix.master_volume = value;
                        self.sync_realtime_profile_settings();
                        tracing::info!(value, "select skin master volume changed");
                    }
                    18 if mix.key_volume != value => {
                        mix.key_volume = value;
                        self.sync_realtime_profile_settings();
                        tracing::info!(value, "select skin key volume changed");
                    }
                    19 if mix.bgm_volume != value => {
                        mix.bgm_volume = value;
                        self.sync_realtime_profile_settings();
                        tracing::info!(value, "select skin bgm volume changed");
                    }
                    _ => {}
                }
            }
            _ => {
                tracing::debug!(slider_type = hit.slider_type, "unsupported select skin slider");
            }
        }
    }

    pub(super) fn apply_result_slider_hit(&mut self, hit: SkinSliderHit) {
        if hit.slider_type == 8 {
            if let Some(result_ir) = &mut self.result.result_ir {
                result_ir.set_skin_scroll_rate(hit.value);
            }
        } else {
            tracing::debug!(slider_type = hit.slider_type, "unsupported result skin slider");
        }
    }

    pub(super) fn apply_select_scroll_slider(&mut self, value: f32) {
        let Some(next) = select_scroll_slider_index(value, self.select.select_items.len()) else {
            return;
        };
        if self.select.selected_index != next {
            self.select.selected_index = next;
            self.restart_select_bar_timer_without_scroll(Instant::now());
            self.play_system_sound(crate::system_sound::SoundType::Scratch);
        }
    }

    pub(super) fn handle_select_skin_click(
        &mut self,
        hit: SkinClickHit,
        button: MouseButton,
        x: f32,
        y: f32,
    ) {
        match hit.target {
            SkinClickTarget::SelectRow { row_index } => {
                self.handle_select_row_click(row_index, button);
            }
            SkinClickTarget::Event { event_id, click } => {
                let Some(arg) = select_click_event_arg(click, button, hit.rect, x, y) else {
                    return;
                };
                self.execute_select_skin_event(event_id, arg);
            }
        }
    }

    pub(super) fn handle_select_row_click(&mut self, row_index: u32, button: MouseButton) {
        if in_settings_stack(&self.select.folder_stack) && button == MouseButton::Left {
            if self.select.settings_edit.is_some() {
                self.commit_settings_edit();
                return;
            }
            if let Some(entry_id) =
                self.select.select_items.get(row_index as usize).and_then(|item| match item {
                    SelectItem::Config(row) => Some(row.entry_id),
                    _ => None,
                })
            {
                self.select.selected_index = row_index as usize;
                self.restart_select_bar_timer_without_scroll(Instant::now());
                self.begin_settings_edit(entry_id);
                return;
            }
        }
        match select_row_click_action(
            row_index,
            button,
            self.select.selected_index,
            self.select.select_items.len(),
            self.select.settings_edit.is_some(),
        ) {
            Some(SelectRowClickAction::Select(next)) => {
                self.select.selected_index = next;
                self.restart_select_bar_timer_without_scroll(Instant::now());
                self.play_system_sound(crate::system_sound::SoundType::Scratch);
            }
            Some(SelectRowClickAction::EnterOrPlay) => self.enter_or_play_selected(),
            Some(SelectRowClickAction::CancelSettingsEdit) => self.cancel_settings_edit(),
            Some(SelectRowClickAction::ExitFolder) => self.exit_folder(),
            None => {}
        }
    }
}
