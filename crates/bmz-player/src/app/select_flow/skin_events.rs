use super::*;

impl WinitApp {
    pub(super) fn execute_select_skin_event(&mut self, event_id: i32, arg: i32) {
        match event_id {
            SKIN_EVENT_IR_SCOPE_GLOBAL => {
                self.select_select_ir_scope(
                    crate::screens::select_ir::SelectIrRankingScope::Global,
                );
            }
            SKIN_EVENT_IR_SCOPE_RIVAL => {
                self.select_select_ir_scope(
                    crate::screens::select_ir::SelectIrRankingScope::SelfAndRivals,
                );
            }
            SKIN_EVENT_IR_SCOPE_TOGGLE => {
                self.toggle_select_ir_scope();
            }
            SKIN_EVENT_DAILY_STATISTICS_RESET => self.reset_daily_statistics(),
            // beatoraja EventFactory: play / autoplay / practice.
            15 => {
                self.set_session_mode(SessionMode::Normal);
                self.enter_or_play_selected();
            }
            16 => {
                self.set_session_mode(SessionMode::Autoplay);
                self.enter_or_play_selected();
            }
            315 => {
                if self.select.course_builder.is_some() {
                    self.show_select_course_builder_chart_required();
                } else if let Some(chart_id) = self.currently_selected_chart_id() {
                    self.enter_practice(chart_id, PracticeCliOverrides::default());
                }
            }
            19 | 316 | 317 | 318 => {
                let slot = match event_id {
                    19 => 0,
                    316 => 1,
                    317 => 2,
                    318 => 3,
                    _ => unreachable!(),
                };
                if !self.start_replay_for_selected(slot) {
                    tracing::info!(slot, "select skin replay click ignored; slot is empty");
                }
            }
            11 => self.cycle_select_mode_filter(arg),
            12 => self.cycle_select_sort(arg),
            40 => self.cycle_select_gauge(arg),
            42 => self.cycle_select_arrange(arg),
            43 => self.cycle_select_arrange_2p(arg),
            54 => self.cycle_select_double_option(arg),
            55 => self.cycle_select_hs_fix(arg),
            72 => self.cycle_select_bga(arg),
            73 => self.cycle_select_bga_expand(arg),
            75 => {
                self.toggle_visual_offset_auto_adjust();
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            77 => self.cycle_select_target(arg),
            79 => self.cycle_active_rival(arg),
            78 => self.cycle_select_gauge_auto_shift(arg),
            89 => self.toggle_favorite_song_selected(),
            90 => self.toggle_favorite_chart_selected(),
            341 => self.cycle_select_bottom_shiftable_gauge(arg),
            340 => self.cycle_select_judge_algorithm(arg),
            308 => self.cycle_select_ln_mode(arg),
            301..=307 => {
                if self.boot.profile_config.play.assist.toggle_beatoraja_button(event_id) {
                    self.boot.profile_config.updated_at = now_unix_seconds();
                    self.invalidate_play_preload();
                    self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                }
            }
            312 => {
                // BMZ only exposes beatoraja's default sorter set for now.
                self.cycle_select_sort(arg);
            }
            321..=324 => self.cycle_replay_slot_rule(event_id, arg),
            330 => {
                if !self.begin_selected_play_mode_edit() {
                    return;
                }
                self.boot.profile_config.play.lane_effect =
                    toggled_select_sudden(self.boot.profile_config.play.lane_effect);
                self.finish_selected_play_mode_edit();
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            331 => {
                if !self.begin_selected_play_mode_edit() {
                    return;
                }
                self.boot.profile_config.lane.lift_enabled =
                    !self.boot.profile_config.lane.lift_enabled;
                self.finish_selected_play_mode_edit();
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            332 => {
                if !self.begin_selected_play_mode_edit() {
                    return;
                }
                self.boot.profile_config.play.lane_effect =
                    toggled_select_hidden(self.boot.profile_config.play.lane_effect);
                self.finish_selected_play_mode_edit();
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            342 => {
                if !self.begin_selected_play_mode_edit() {
                    return;
                }
                self.boot.profile_config.lane.hispeed_auto_adjust =
                    !self.boot.profile_config.lane.hispeed_auto_adjust;
                self.finish_selected_play_mode_edit();
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            344 => self.cycle_chart_replication_mode(arg),
            _ => {
                tracing::debug!(event_id, arg, "unsupported select skin event");
            }
        }
    }

    pub(super) fn reset_daily_statistics(&mut self) {
        match self.boot.score_db.reset_daily_statistics(now_unix_seconds()) {
            Ok(()) => {
                self.refresh_player_stats_snapshot();
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            Err(error) => tracing::warn!(%error, "failed to reset daily statistics"),
        }
    }

    pub(super) fn cycle_select_mode_filter(&mut self, arg: i32) {
        self.select.select_mode_filter = if arg >= 0 {
            self.select.select_mode_filter.next()
        } else {
            self.select.select_mode_filter.previous()
        };
        // reload_select_items 内で beatoraja 準拠の自動送りと profile config への
        // 永続化（退出 / プレイ後の save_current_play_options 用）を行う。
        let previous_len = self.select.select_items.len();
        self.reload_select_items();
        tracing::info!(
            mode = self.select.select_mode_filter.as_str(),
            previous_len,
            current_len = self.select.select_items.len(),
            "select mode filter changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_gauge(&mut self, arg: i32) {
        self.select.gauge_option = cycle_gauge_option_with_direction(self.select.gauge_option, arg);
        tracing::info!(gauge = ?self.select.gauge_option, "gauge option changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_arrange(&mut self, arg: i32) {
        self.select.arrange_option =
            cycle_arrange_option_with_direction(self.select.arrange_option, arg);
        tracing::info!(arrange = self.select.arrange_option.as_str(), "arrange option changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_arrange_2p(&mut self, arg: i32) {
        self.select.arrange_option_2p =
            cycle_arrange_option_with_direction(self.select.arrange_option_2p, arg);
        tracing::info!(arrange_2p = self.select.arrange_option_2p.as_str(), "2P arrange changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_double_option(&mut self, arg: i32) {
        self.select.double_option =
            cycle_double_option_with_direction(self.select.double_option, arg);
        tracing::info!(double_option = self.select.double_option.as_str(), "double option changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_hs_fix(&mut self, arg: i32) {
        if !self.begin_selected_play_mode_edit() {
            return;
        }
        self.select.hs_fix_option =
            cycle_hs_fix_option_with_direction(self.select.hs_fix_option, arg);
        self.boot.profile_config.play.hs_fix = hs_fix_config_from_option(self.select.hs_fix_option);
        self.finish_selected_play_mode_edit();
        tracing::info!(hs_fix = self.select.hs_fix_option.as_str(), "HS-FIX option changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_bga(&mut self, arg: i32) {
        self.boot.profile_config.play.bga =
            cycle_bga_option_with_direction(self.boot.profile_config.play.bga, arg);
        tracing::info!(
            bga = bga_mode_as_str(self.boot.profile_config.play.bga),
            "bga option changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_bga_expand(&mut self, arg: i32) {
        self.boot.profile_config.play.bga_expand =
            cycle_bga_expand_with_direction(self.boot.profile_config.play.bga_expand, arg);
        tracing::info!(
            bga_expand = ?self.boot.profile_config.play.bga_expand,
            "bga expand changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_target(&mut self, arg: i32) {
        let cycle = if arg >= 0 { TargetCycle::Next } else { TargetCycle::Previous };
        self.apply_target_option_cycle(cycle);
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_active_rival(&mut self, arg: i32) {
        let Some(provider) =
            crate::ir::provider_key::primary_provider_config(&self.boot.profile_config.ir)
        else {
            return;
        };
        if !crate::ir::rian_ir::is_rian_ir_provider(&provider.provider) {
            return;
        }
        let Some(provider_key) = crate::ir::provider_key::configured_provider_key(provider) else {
            return;
        };
        let ids: Vec<String> = self
            .boot
            .profile_config
            .rival
            .entries
            .iter()
            .filter(|entry| {
                matches!(entry.source, RivalSourceConfig::Ir)
                    && entry.ir_service == provider_key
                    && !entry.ir_user_id.is_empty()
            })
            .map(|entry| entry.id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }

        let current = self.boot.profile_config.rival.active_rival.clone();
        let position = if current.is_empty() {
            0
        } else {
            ids.iter().position(|id| id == &current).map(|index| index + 1).unwrap_or(0)
        };
        let count = ids.len() + 1;
        let next = if arg >= 0 { (position + 1) % count } else { (position + count - 1) % count };
        self.boot.profile_config.rival.active_rival =
            if next == 0 { String::new() } else { ids[next - 1].clone() };
        self.boot.profile_config.updated_at = now_unix_seconds();
        if let Err(error) =
            save_profile_config(&self.boot.profile_paths.profile_toml, &self.boot.profile_config)
        {
            self.boot.profile_config.rival.active_rival = current;
            tracing::error!(%error, "failed to save active rival");
            return;
        }
        let target = crate::screens::select_ir::SelectRivalFetchTarget::from_profile(
            &self.boot.profile_config,
        );
        self.select.select_ir.update_rival(target, &self.boot.profile_paths.root_dir);
        tracing::info!(
            rival = %self.boot.profile_config.rival.active_rival,
            "active rival changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_chart_replication_mode(&mut self, arg: i32) {
        let previous = self.boot.profile_config.rival.chart_replication_mode;
        self.boot.profile_config.rival.chart_replication_mode = previous.cycle(arg >= 0);
        self.boot.profile_config.updated_at = now_unix_seconds();
        if let Err(error) =
            save_profile_config(&self.boot.profile_paths.profile_toml, &self.boot.profile_config)
        {
            self.boot.profile_config.rival.chart_replication_mode = previous;
            tracing::error!(%error, "failed to save chart replication mode");
            return;
        }
        tracing::info!(
            mode = self.boot.profile_config.rival.chart_replication_mode.as_str(),
            "chart replication mode changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_gauge_auto_shift(&mut self, arg: i32) {
        self.select.gauge_auto_shift_option =
            cycle_gauge_auto_shift_option_with_direction(self.select.gauge_auto_shift_option, arg);
        tracing::info!(
            gauge_auto_shift = gauge_auto_shift_as_str(self.select.gauge_auto_shift_option),
            "gauge auto shift changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_bottom_shiftable_gauge(&mut self, arg: i32) {
        self.select.bottom_shiftable_gauge_option = cycle_bottom_shiftable_gauge_with_direction(
            self.select.bottom_shiftable_gauge_option,
            arg,
        );
        tracing::info!(
            bottom_shiftable_gauge =
                bottom_shiftable_gauge_as_str(self.select.bottom_shiftable_gauge_option),
            "bottom shiftable gauge changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_judge_algorithm(&mut self, arg: i32) {
        self.boot.profile_config.judge.judge_algorithm = cycle_judge_algorithm_with_direction(
            self.boot.profile_config.judge.judge_algorithm,
            arg,
        );
        self.boot.profile_config.updated_at = now_unix_seconds();
        self.sync_realtime_profile_settings();
        self.invalidate_play_preload();
        tracing::info!(
            judge_algorithm = self.boot.profile_config.judge.judge_algorithm.beatoraja_name(),
            "judge algorithm changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_sort(&mut self, arg: i32) {
        self.select.select_sort = if arg >= 0 {
            self.select.select_sort.next()
        } else {
            self.select.select_sort.previous()
        };
        // 退出 / プレイ後の save_current_play_options で永続化されるよう、
        // profile config をメモリ上で先に更新しておく。
        self.boot.profile_config.select.sort = self.select.select_sort.as_str().to_string();
        self.reload_select_items();
        tracing::info!(sort = self.select.select_sort.as_str(), "select sort changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_ln_mode(&mut self, arg: i32) {
        let score_context_before = SelectScoreContext::from_profile(&self.boot.profile_config);
        self.boot.profile_config.play.ln_mode_policy = if arg >= 0 {
            self.boot.profile_config.play.ln_mode_policy.next()
        } else {
            self.boot.profile_config.play.ln_mode_policy.previous()
        };
        self.sync_changed_select_score_context(score_context_before);
        tracing::info!(
            ln_mode = self.boot.profile_config.play.ln_mode_policy.display_label(),
            "select LN mode policy changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_replay_slot_rule(&mut self, event_id: i32, arg: i32) {
        let slot = (event_id - 321) as usize;
        if slot >= 4 {
            return;
        }
        let rule = &mut self.boot.profile_config.replay.slot_rules[slot];
        let next = rule.cycle(arg >= 0);
        if next == *rule {
            return;
        }
        *rule = next;
        tracing::info!(slot, ?next, "select replay autosave rule changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }
}
