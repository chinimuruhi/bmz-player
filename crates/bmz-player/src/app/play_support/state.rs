#[derive(Debug, Clone, Copy)]
pub(in crate::app) struct ActiveLaneState {
    pub(in crate::app) lane_cover: f32,
    pub(in crate::app) lift: f32,
    pub(in crate::app) hispeed_mode: HispeedMode,
    pub(in crate::app) target_green_number: u32,
}

pub(in crate::app) fn profile_lane_settings_changed(
    before: &LaneViewConfig,
    after: &LaneViewConfig,
) -> bool {
    before.hispeed != after.hispeed
        || before.hispeed_mode != after.hispeed_mode
        || before.sudden != after.sudden
        || before.lift != after.lift
        || before.lift_enabled != after.lift_enabled
        || before.hispeed_auto_adjust != after.hispeed_auto_adjust
        || before.target_green_number != after.target_green_number
}

pub(in crate::app) fn apply_profile_lane_settings_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    before: &LaneViewConfig,
    profile: &LaneViewConfig,
    speed_locked: bool,
) -> bool {
    if !profile_lane_settings_changed(before, profile) {
        return false;
    }
    if speed_locked {
        return false;
    }

    let mode_changed = before.hispeed_mode != profile.hispeed_mode;
    let hispeed_changed = before.hispeed != profile.hispeed;
    let sudden_changed = before.sudden != profile.sudden;
    let lift_changed = before.lift != profile.lift;
    let lift_enabled_changed = before.lift_enabled != profile.lift_enabled;
    let auto_adjust_changed = before.hispeed_auto_adjust != profile.hispeed_auto_adjust;
    let target_green_changed = before.target_green_number != profile.target_green_number;
    let cover_changed = sudden_changed || lift_changed || lift_enabled_changed;

    if cover_changed {
        session.lift_enabled = profile.lift_enabled;
        session.lift = if profile.lift_enabled {
            crate::config::play::lane_unit_to_f32(profile.lift)
        } else {
            0.0
        };
        session.lane_cover = crate::config::play::clamp_lane_cover_for_lift(
            crate::config::play::lane_unit_to_f32(profile.sudden),
            session.lift,
        );
    }
    if auto_adjust_changed {
        session.hispeed_auto_adjust = profile.hispeed_auto_adjust;
    }

    let now = session.audio_clock.now();
    if mode_changed {
        session.hispeed_mode = match profile.hispeed_mode {
            HispeedModeConfig::Normal => HispeedMode::Normal,
            HispeedModeConfig::Floating => HispeedMode::Floating,
        };
        if session.hispeed_mode == HispeedMode::Floating
            && !target_green_changed
            && !hispeed_changed
        {
            session.target_green_number = current_green_number(session, now);
        }
    }

    if target_green_changed {
        session.target_green_number = profile.target_green_number.max(1);
    }

    let direct_hispeed_change = hispeed_changed;
    if direct_hispeed_change {
        session.hispeed = clamp_hispeed(profile.hispeed);
    }

    if session.hispeed_mode == HispeedMode::Floating {
        if direct_hispeed_change && !target_green_changed {
            // FHS の直接 HS 変更は、現在の見た目から緑数字ターゲットを更新する。
            session.target_green_number = current_green_number(session, now);
        } else if target_green_changed
            || cover_changed
            || auto_adjust_changed
            || (mode_changed && !direct_hispeed_change)
        {
            session.hispeed =
                hispeed_for_green_number(session, active_lane_cover_for_hispeed(session), now);
        }
    }

    true
}

pub(in crate::app) fn lane_state_for_profile_save(
    speed_locked: bool,
    hispeed: Option<f32>,
    lane_state: Option<ActiveLaneState>,
) -> (Option<f32>, Option<ActiveLaneState>) {
    if speed_locked { (None, None) } else { (hispeed, lane_state) }
}

pub(in crate::app) fn active_lane_state_for_session(
    session: &bmz_gameplay::session::GameSession,
) -> ActiveLaneState {
    ActiveLaneState {
        lane_cover: session.lane_cover,
        lift: session.lift,
        hispeed_mode: session.hispeed_mode,
        // NHS の現在表示は曲終了時に変動するため保存しない。target は NHS→FHS
        // の明示切替時に session 側で更新された値を引き継ぐ。
        target_green_number: session.target_green_number,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct CurrentPlayOptions {
    pub(in crate::app) arrange: ArrangeOption,
    pub(in crate::app) arrange_2p: ArrangeOption,
    pub(in crate::app) target: TargetOption,
    pub(in crate::app) gauge: GaugeTypeConfig,
    pub(in crate::app) gauge_auto_shift: GaugeAutoShiftConfig,
    pub(in crate::app) bottom_shiftable_gauge: BottomShiftableGaugeConfig,
    pub(in crate::app) double_option: DoubleOption,
    pub(in crate::app) hs_fix: HsFixOption,
    pub(in crate::app) session_mode: SessionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct SelectScoreContext {
    pub(in crate::app) rule_mode: RuleMode,
    pub(in crate::app) ln_mode_policy: LnPolicySetting,
}

impl SelectScoreContext {
    pub(in crate::app) fn from_profile(profile: &ProfileConfig) -> Self {
        Self::from_play(&profile.play)
    }

    pub(in crate::app) fn from_play(play: &PlayDefaultsConfig) -> Self {
        Self { rule_mode: play.rule_mode, ln_mode_policy: play.ln_mode_policy }
    }
}

pub(in crate::app) fn session_mode_from_profile(play: &PlayDefaultsConfig) -> SessionMode {
    play.session_mode.unwrap_or(if play.auto_play {
        SessionMode::Autoplay
    } else {
        SessionMode::Normal
    })
}

pub(in crate::app) fn normalize_session_mode_for_course(options: &mut PlayStartOptions) {
    options.session_mode = match options.session_mode {
        SessionMode::Autoplay | SessionMode::AutoplayBattle => SessionMode::Autoplay,
        SessionMode::Normal | SessionMode::GhostBattle => SessionMode::Normal,
    };
    options.autoplay = options.session_mode.primary_autoplay();
    options.replay_player = None;
}

pub(in crate::app) fn select_play_options_from_profile(
    play: &PlayDefaultsConfig,
) -> CurrentPlayOptions {
    let (gauge, gauge_auto_shift) = if play.gauge == GaugeTypeConfig::AutoShift {
        (GaugeTypeConfig::ExHard, GaugeAutoShiftConfig::BestClear)
    } else {
        (play.gauge, play.gauge_auto_shift)
    };
    CurrentPlayOptions {
        arrange: arrange_option_from_profile(play.random),
        arrange_2p: arrange_option_from_profile(play.random2),
        target: target_option_from_profile(play.target),
        gauge,
        gauge_auto_shift,
        bottom_shiftable_gauge: play.bottom_shiftable_gauge,
        double_option: double_option_from_profile(play.double_option),
        hs_fix: hs_fix_option_from_profile(play.hs_fix),
        session_mode: session_mode_from_profile(play),
    }
}

/// `profile.toml` の保存済みスキン設定だけを現在のprofileへ戻す。
///
/// スキンUIのリセットで、同じファイルに保存されているプレイ・入力・UI設定まで
/// 巻き戻さないため、`ProfileConfig` 全体は差し替えない。
pub(in crate::app) fn replace_skin_config_from_loaded_profile(
    current: &mut ProfileConfig,
    loaded: ProfileConfig,
) {
    current.skin = loaded.skin;
}

pub(in crate::app) fn merge_changed_select_play_options_from_profile(
    mut current: CurrentPlayOptions,
    before: &PlayDefaultsConfig,
    after: &PlayDefaultsConfig,
) -> CurrentPlayOptions {
    let profile = select_play_options_from_profile(after);
    if before.random != after.random {
        current.arrange = profile.arrange;
    }
    if before.random2 != after.random2 {
        current.arrange_2p = profile.arrange_2p;
    }
    if before.target != after.target {
        current.target = profile.target;
    }
    if before.gauge != after.gauge || before.gauge_auto_shift != after.gauge_auto_shift {
        current.gauge = profile.gauge;
        current.gauge_auto_shift = profile.gauge_auto_shift;
    }
    if before.bottom_shiftable_gauge != after.bottom_shiftable_gauge {
        current.bottom_shiftable_gauge = profile.bottom_shiftable_gauge;
    }
    if before.double_option != after.double_option {
        current.double_option = profile.double_option;
    }
    if before.hs_fix != after.hs_fix {
        current.hs_fix = profile.hs_fix;
    }
    if before.session_mode != after.session_mode {
        current.session_mode = profile.session_mode;
    } else if before.auto_play != after.auto_play {
        current.session_mode =
            if after.auto_play { SessionMode::Autoplay } else { SessionMode::Normal };
    }
    current
}

pub(in crate::app) fn apply_current_play_options_to_profile(
    profile: &mut ProfileConfig,
    hispeed: Option<f32>,
    lane_state: Option<ActiveLaneState>,
    options: CurrentPlayOptions,
    updated_at: i64,
) {
    apply_lane_state_to_profile(profile, hispeed, lane_state);
    profile.play.random = random_config_from_arrange(options.arrange);
    profile.play.random2 = random_config_from_arrange(options.arrange_2p);
    profile.play.target = target_config_from_option(options.target);
    profile.play.gauge = options.gauge;
    profile.play.gauge_auto_shift = options.gauge_auto_shift;
    profile.play.bottom_shiftable_gauge = options.bottom_shiftable_gauge;
    profile.play.double_option = double_config_from_option(options.double_option);
    profile.play.hs_fix = hs_fix_config_from_option(options.hs_fix);
    profile.play.session_mode = Some(options.session_mode);
    profile.play.auto_play = options.session_mode.primary_autoplay();
    profile.play.assist = AssistOptionConfig::None;
    profile.updated_at = updated_at;
}

pub(in crate::app) fn apply_lane_state_to_profile(
    profile: &mut ProfileConfig,
    hispeed: Option<f32>,
    lane_state: Option<ActiveLaneState>,
) {
    let saved_hispeed_mode = lane_state
        .map(|state| hispeed_mode_to_config(state.hispeed_mode))
        .unwrap_or(profile.lane.hispeed_mode);
    if let Some(hispeed) = hispeed {
        let step = match saved_hispeed_mode {
            HispeedModeConfig::Normal => profile.lane.hispeed_step_nhs,
            HispeedModeConfig::Floating => profile.lane.hispeed_step_fhs,
        };
        profile.lane.hispeed = clamp_hispeed_for_profile(hispeed, saved_hispeed_mode, step);
    }
    if let Some(state) = lane_state {
        profile.lane.sudden = crate::config::play::lane_f32_to_unit(state.lane_cover);
        if profile.lane.lift_enabled {
            profile.lane.lift = crate::config::play::lane_f32_to_unit(state.lift);
        }
        profile.lane.hispeed_mode = hispeed_mode_to_config(state.hispeed_mode);
        profile.lane.target_green_number = state.target_green_number.max(1);
    }
}

pub(in crate::app) fn clamp_hispeed_for_profile(
    hispeed: f32,
    mode: HispeedModeConfig,
    step: f32,
) -> f32 {
    let clamped = clamp_hispeed(hispeed);
    if mode == HispeedModeConfig::Normal
        && (normalize_hispeed_step(step, default_hispeed_step_nhs()) - 0.25).abs() < f32::EPSILON
    {
        clamp_hispeed((clamped * 4.0).round() / 4.0)
    } else {
        clamped
    }
}

pub(in crate::app) fn hispeed_mode_to_config(mode: HispeedMode) -> HispeedModeConfig {
    match mode {
        HispeedMode::Normal => HispeedModeConfig::Normal,
        HispeedMode::Floating => HispeedModeConfig::Floating,
    }
}

pub(in crate::app) fn update_pre_ready_play_snapshot_options_for_session(
    ready_sound_started_at: Option<Instant>,
    last_play_snapshot: &mut Option<RenderSnapshot>,
    session: &bmz_gameplay::session::GameSession,
    applied_arrange: &AppliedArrange,
) {
    if ready_sound_started_at.is_some() {
        return;
    }
    let Some(snapshot) = last_play_snapshot else {
        return;
    };
    crate::screens::play_snapshot::update_render_snapshot_play_options(
        snapshot,
        session,
        snapshot.time,
    );
    apply_play_arrange_to_snapshot(snapshot, applied_arrange);
}

pub(in crate::app) fn update_play_exit_hold_started_at(
    started_at: &mut Option<Instant>,
    e1_held: bool,
    e2_held: bool,
    now: Instant,
) {
    if e1_held && e2_held {
        started_at.get_or_insert(now);
    } else {
        *started_at = None;
    }
}

pub(in crate::app) fn play_exit_hold_elapsed(
    started_at: Option<Instant>,
    now: Instant,
    duration: Duration,
) -> bool {
    started_at.is_some_and(|started_at| now.duration_since(started_at) >= duration)
}
use super::*;
