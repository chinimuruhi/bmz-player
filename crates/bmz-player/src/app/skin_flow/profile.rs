use super::*;

impl WinitApp {
    pub(super) fn sync_realtime_profile_settings(&mut self) {
        self.sync_active_play_realtime_profile_settings();
        if let Some(manager) = &self.audio.system_sound {
            let mix = self.boot.profile_config.audio_mix.clone();
            let preview_factor = select_preview_fade_factor(
                self.select.select_assets.preview_fade(),
                Instant::now(),
            );
            manager.refresh_volumes(|sound_type| {
                let volume = system_sound_volume_from_mix(&mix, sound_type);
                if sound_type == crate::system_sound::SoundType::Select {
                    volume * (1.0 - preview_factor).clamp(0.0, 1.0)
                } else {
                    volume
                }
            });
        }
        self.apply_select_preview_audio_mix();
    }

    pub(super) fn sync_active_play_lane_settings_from_profile(&mut self, before: &LaneViewConfig) {
        let speed_locked = self.play.active_course.as_ref().is_some_and(|course| {
            course.definition.constraints.speed == bmz_core::course::CourseSpeedConstraint::NoSpeed
        });
        let profile_lane = self.boot.profile_config.lane.clone();
        let Some(active_play) = &mut self.play.active_play else {
            return;
        };
        if apply_profile_lane_settings_to_session(
            &mut active_play.running.session,
            before,
            &profile_lane,
            speed_locked,
        ) {
            update_pre_ready_play_snapshot_options_for_session(
                self.play.play_ready_sound_started_at,
                &mut self.play.last_play_snapshot,
                &active_play.running.session,
                &active_play.running.applied_arrange,
            );
            tracing::info!(
                hispeed = active_play.running.session.hispeed,
                hispeed_mode = ?active_play.running.session.hispeed_mode,
                target_green_number = active_play.running.session.target_green_number,
                lane_cover = active_play.running.session.lane_cover,
                lift = active_play.running.session.lift,
                "applied egui lane settings to active play"
            );
        }
    }

    pub(super) fn sync_active_play_realtime_profile_settings(&mut self) {
        if let Some(active_play) = &mut self.play.active_play {
            let session = &mut active_play.running.session;
            let chart_normalization_gain = session.audio_mix.chart_normalization_gain;
            session.audio_mix = crate::config::play::audio_mix_from_profile_with_chart_gain(
                &self.boot.profile_config,
                chart_normalization_gain,
            );
            session.offsets =
                crate::config::play::play_offsets_from_profile(&self.boot.profile_config);
            session.input_offset_auto_adjust_enabled =
                self.boot.profile_config.judge.visual_offset_auto_adjust;
            let auto_adjust_available = session.replay_player.is_none()
                && !session.autoplay.as_ref().is_some_and(|autoplay| autoplay.is_full());
            if session.input_offset_auto_adjust_enabled && auto_adjust_available {
                session.input_offset_auto_adjust.get_or_insert_with(Default::default);
            } else {
                session.input_offset_auto_adjust = None;
            }
        }
    }

    pub(super) fn sync_profile_visual_offset_from_active_play(&mut self) {
        let Some((visual_offset_us, auto_adjust_active)) =
            self.play.active_play.as_ref().map(|active| {
                (
                    active.running.session.offsets.visual_offset_us,
                    active.running.session.input_offset_auto_adjust.is_some(),
                )
            })
        else {
            return;
        };
        sync_active_play_visual_offset_to_profile(
            &mut self.boot.profile_config,
            visual_offset_us,
            auto_adjust_active,
        );
    }

    pub(super) fn play_skin_defs_for_path(&mut self, path: &str) -> SceneSkinDefs {
        let key = path.trim().to_string();
        if let Some(defs) = self.skin.skin_defs_cache.get(&key) {
            return defs.clone();
        }
        let defs = play_skin_defs_from_path(&self.boot.app_paths, &key);
        self.skin.skin_defs_cache.insert(key, defs.clone());
        defs
    }

    pub(super) fn reset_skin_config_from_disk(&mut self) {
        match load_profile_config(&self.boot.profile_paths.profile_toml) {
            Ok(profile) => {
                replace_skin_config_from_loaded_profile(&mut self.boot.profile_config, profile);
                self.apply_profile_skin_offsets_to_active_play();
                self.reload_skins(SkinReloadRequest {
                    select: true,
                    decide: true,
                    result: true,
                    course_result: true,
                    play4: true,
                    play5: true,
                    play6: true,
                    play7: true,
                    play8: true,
                    play9: true,
                    play10: true,
                    play14: true,
                    offsets: true,
                });
                tracing::info!("skin config reset from profile.toml");
            }
            Err(error) => {
                tracing::error!(
                    path = %self.boot.profile_paths.profile_toml.display(),
                    %error,
                    "failed to reset skin config from profile.toml"
                );
            }
        }
    }

    pub(super) fn apply_profile_skin_offsets_to_active_play(&mut self) {
        let Some(key_mode) = self
            .play
            .active_play
            .as_ref()
            .map(|active_play| active_play.running.session.chart.metadata.key_mode)
        else {
            return;
        };
        let offsets = play_skin_selection_for_session(
            &self.boot.profile_config.skin,
            key_mode,
            self.select.session_mode,
        )
        .offsets
        .iter()
        .map(|offset| PlaySkinOffset {
            id: offset.id,
            x: offset.x,
            y: offset.y,
            w: offset.w,
            h: offset.h,
            r: offset.r,
            a: offset.a,
        })
        .collect();
        if let Some(active_play) = &mut self.play.active_play {
            active_play.running.session.skin_offsets = offsets;
        }
    }
}
