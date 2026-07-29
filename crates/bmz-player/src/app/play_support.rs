use super::*;

pub(super) fn cycle_bga_option_with_direction(
    current: BgaModeConfig,
    direction: i32,
) -> BgaModeConfig {
    const VALUES: [BgaModeConfig; 3] = [BgaModeConfig::On, BgaModeConfig::Auto, BgaModeConfig::Off];
    cycle_enum(VALUES, current, direction)
}

pub(super) fn cycle_bga_expand_with_direction(
    current: BgaExpandConfig,
    direction: i32,
) -> BgaExpandConfig {
    const VALUES: [BgaExpandConfig; 3] =
        [BgaExpandConfig::KeepAspect, BgaExpandConfig::Full, BgaExpandConfig::Off];
    cycle_enum(VALUES, current, direction)
}

pub(super) fn select_option_panel_for_holds(start_held: bool, select_held: bool) -> u8 {
    match (start_held, select_held) {
        (true, true) => 3,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 0,
    }
}

pub(super) fn select_option_panel_sound_for_transition(
    current_panel: u8,
    next_panel: u8,
) -> Option<crate::system_sound::SoundType> {
    use crate::system_sound::SoundType;

    match (current_panel == 0, next_panel == 0) {
        (true, false) => Some(SoundType::OptionOpen),
        (false, true) => Some(SoundType::OptionClose),
        _ => None,
    }
}

pub(super) fn transition_select_option_panel(
    current_panel: &mut u8,
    on_started_at: &mut Instant,
    off_started_at: &mut [Option<Instant>; 6],
    next_panel: u8,
    now: Instant,
) -> bool {
    if *current_panel == next_panel {
        return false;
    }
    if let Some(index) = current_panel.checked_sub(1).filter(|index| *index < 6) {
        off_started_at[index as usize] = Some(now);
    }
    if let Some(index) = next_panel.checked_sub(1).filter(|index| *index < 6) {
        off_started_at[index as usize] = None;
    }
    *current_panel = next_panel;
    *on_started_at = now;
    true
}

pub(super) fn select_hold_state_from_pressed_controls(
    pressed_controls: &HashSet<String>,
    bindings: &SelectKeyBindings,
) -> (bool, bool, HashSet<InputActionConfig>) {
    let start_held = pressed_controls.iter().any(|control| bindings.is_start(control));
    let select_held = pressed_controls
        .iter()
        .any(|control| control == "Select" || bindings.is_e2_action(control));
    let e_action_holds = pressed_controls
        .iter()
        .filter_map(|control| bindings.e_action_for_control(control))
        .collect();
    (start_held, select_held, e_action_holds)
}

pub(super) fn skin_logical_input_snapshot_from_pressed_controls(
    pressed_controls: &HashSet<String>,
    bindings: &SelectKeyBindings,
) -> SkinLogicalInputSnapshot {
    let mut held = [false; bmz_render::skin::SKIN_BMZ_INPUT_COUNT];
    for control in pressed_controls {
        held[0] |= bindings.is_start(control);
        held[1] |= control == "Select" || bindings.is_e2_action(control);
        match bindings.e_action_for_control(control) {
            Some(InputActionConfig::E1) => held[0] = true,
            Some(InputActionConfig::E2) => held[1] = true,
            Some(InputActionConfig::E3) => held[2] = true,
            Some(InputActionConfig::E4) => held[3] = true,
            _ => {}
        }
        match control.as_str() {
            "ArrowLeft" | "DPadLeft" => held[4] = true,
            "ArrowRight" | "DPadRight" => held[5] = true,
            "ArrowUp" | "DPadUp" => held[6] = true,
            "ArrowDown" | "DPadDown" => held[7] = true,
            _ => {}
        }
    }
    SkinLogicalInputSnapshot { held }
}

pub(super) fn apply_skin_logical_input_to_scene(
    scene: &mut AppSceneSnapshot,
    skin_input: SkinLogicalInputSnapshot,
) {
    match scene {
        AppSceneSnapshot::Select(snapshot) => snapshot.skin_input = skin_input,
        AppSceneSnapshot::Decide(snapshot) | AppSceneSnapshot::Play(snapshot) => {
            snapshot.skin_input = skin_input;
        }
        AppSceneSnapshot::Result(snapshot) => snapshot.skin_input = skin_input,
    }
}

pub(super) fn play_control_hold_state_from_pressed_inputs(
    pressed_inputs: &HashSet<(DeviceId, PhysicalControl)>,
    input: &PlayOptionInput,
) -> (bool, bool, bool) {
    let action_held = |action| {
        pressed_inputs.iter().any(|(device, control)| {
            !input.resolves_lane(*device, control) && input.is_action(*device, control, action)
        })
    };
    let e1_held = action_held(InputActionConfig::E1);
    let e2_held = action_held(InputActionConfig::E2);
    let e3_held = action_held(InputActionConfig::E3);
    (e1_held, e2_held, e3_held)
}

pub(super) fn play_ready_blocked_by_control_holds(e1_held: bool, e2_held: bool) -> bool {
    e1_held || e2_held
}

pub(super) fn play_ready_blocked_by_recent_control_hold(
    last_control_hold_at: Option<Instant>,
    now: Instant,
) -> bool {
    last_control_hold_at.is_some_and(|last_control_hold_at| {
        now.saturating_duration_since(last_control_hold_at) <= Duration::from_secs(1)
    })
}

pub(super) fn should_begin_play_fadeout_after_final_notes(
    control: &str,
    bindings: &SelectKeyBindings,
    ready_started: bool,
    play_ending_active: bool,
    play_state: bmz_gameplay::session::PlayState,
    final_notes_processed: bool,
) -> bool {
    ready_started
        && !play_ending_active
        && play_state == bmz_gameplay::session::PlayState::Playing
        && final_notes_processed
        && (play_fadeout_after_final_notes_control(control, bindings) || control == "Escape")
}

pub(super) fn should_play_retire_sound_for_failed_transition(
    previous: bmz_gameplay::session::PlayState,
    current: bmz_gameplay::session::PlayState,
) -> bool {
    previous == bmz_gameplay::session::PlayState::Playing
        && current == bmz_gameplay::session::PlayState::Failed
}

pub(super) fn play_fadeout_after_final_notes_control(
    control: &str,
    bindings: &SelectKeyBindings,
) -> bool {
    bindings.is_start(control) || bindings.is_e2_action(control)
}

pub(super) fn is_select_start_key(physical_key: PhysicalKey, bindings: &SelectKeyBindings) -> bool {
    physical_key_name(physical_key).is_some_and(|control| bindings.is_start(&control))
}

pub(super) fn is_select_modifier_key(
    physical_key: PhysicalKey,
    bindings: &SelectKeyBindings,
) -> bool {
    physical_key_name(physical_key).is_some_and(|control| bindings.is_e2_action(&control))
}

pub(super) fn should_toggle_select_gauge_auto_shift(
    control: &str,
    start_held: bool,
    select_held: bool,
    bindings: &SelectKeyBindings,
) -> bool {
    start_held && (select_held || bindings.is_e2_action(control)) && bindings.is_ui_key2(control)
}

pub(super) fn should_toggle_select_judge_auto_adjust(
    control: &str,
    start_held: bool,
    select_held: bool,
    bindings: &SelectKeyBindings,
) -> bool {
    start_held && (select_held || bindings.is_e2_action(control)) && bindings.is_ui_key3(control)
}

pub(super) fn arrange_option_from_profile(random: RandomOptionConfig) -> ArrangeOption {
    match random {
        RandomOptionConfig::Mirror => ArrangeOption::Mirror,
        RandomOptionConfig::Random => ArrangeOption::Random,
        RandomOptionConfig::RRandom => ArrangeOption::RRandom,
        RandomOptionConfig::SRandom => ArrangeOption::SRandom,
        RandomOptionConfig::Spiral => ArrangeOption::Spiral,
        RandomOptionConfig::HRandom => ArrangeOption::HRandom,
        RandomOptionConfig::AllScratch => ArrangeOption::AllScratch,
        RandomOptionConfig::RandomEx => ArrangeOption::RandomEx,
        RandomOptionConfig::SRandomEx => ArrangeOption::SRandomEx,
        RandomOptionConfig::FRandom => ArrangeOption::FRandom,
        RandomOptionConfig::MFRandom => ArrangeOption::MFRandom,
        RandomOptionConfig::Off => ArrangeOption::Normal,
    }
}

pub(super) fn random_config_from_arrange(arrange: ArrangeOption) -> RandomOptionConfig {
    match arrange {
        ArrangeOption::Normal => RandomOptionConfig::Off,
        ArrangeOption::Mirror => RandomOptionConfig::Mirror,
        ArrangeOption::Random => RandomOptionConfig::Random,
        ArrangeOption::RRandom => RandomOptionConfig::RRandom,
        ArrangeOption::SRandom => RandomOptionConfig::SRandom,
        ArrangeOption::Spiral => RandomOptionConfig::Spiral,
        ArrangeOption::HRandom => RandomOptionConfig::HRandom,
        ArrangeOption::AllScratch => RandomOptionConfig::AllScratch,
        ArrangeOption::RandomEx => RandomOptionConfig::RandomEx,
        ArrangeOption::SRandomEx => RandomOptionConfig::SRandomEx,
        ArrangeOption::FRandom => RandomOptionConfig::FRandom,
        ArrangeOption::MFRandom => RandomOptionConfig::MFRandom,
    }
}

pub(super) fn double_option_from_profile(double_option: DoubleOptionConfig) -> DoubleOption {
    match double_option {
        DoubleOptionConfig::Off => DoubleOption::Off,
        DoubleOptionConfig::Flip => DoubleOption::Flip,
        DoubleOptionConfig::Battle => DoubleOption::Battle,
        DoubleOptionConfig::BattleAutoScratch => DoubleOption::BattleAutoScratch,
    }
}

pub(super) fn double_config_from_option(double_option: DoubleOption) -> DoubleOptionConfig {
    match double_option {
        DoubleOption::Off => DoubleOptionConfig::Off,
        DoubleOption::Flip => DoubleOptionConfig::Flip,
        DoubleOption::Battle => DoubleOptionConfig::Battle,
        DoubleOption::BattleAutoScratch => DoubleOptionConfig::BattleAutoScratch,
    }
}

pub(super) fn play_skin_key_mode_for_options(
    chart_key_mode: KeyMode,
    double_option: DoubleOption,
    session_mode: SessionMode,
) -> KeyMode {
    if session_mode.is_battle() {
        return match chart_key_mode {
            KeyMode::K5 => KeyMode::K10,
            KeyMode::K7 => KeyMode::K14,
            _ => chart_key_mode,
        };
    }
    match double_option.normalize_for_key_mode(chart_key_mode) {
        DoubleOption::Battle | DoubleOption::BattleAutoScratch => match chart_key_mode {
            KeyMode::K5 => KeyMode::K10,
            KeyMode::K7 => KeyMode::K14,
            _ => chart_key_mode,
        },
        DoubleOption::Off | DoubleOption::Flip => chart_key_mode,
    }
}

pub(super) fn second_player_lane(lane: Lane) -> Option<Lane> {
    match lane {
        Lane::Scratch => Some(Lane::Scratch2),
        Lane::Key1 => Some(Lane::Key8),
        Lane::Key2 => Some(Lane::Key9),
        Lane::Key3 => Some(Lane::Key10),
        Lane::Key4 => Some(Lane::Key11),
        Lane::Key5 => Some(Lane::Key12),
        Lane::Key6 => Some(Lane::Key13),
        Lane::Key7 => Some(Lane::Key14),
        Lane::Key8
        | Lane::Key9
        | Lane::Key10
        | Lane::Key11
        | Lane::Key12
        | Lane::Key13
        | Lane::Key14
        | Lane::Scratch2 => None,
    }
}

pub(super) fn skin_reload_request_includes_key_mode(
    request: SkinReloadRequest,
    key_mode: KeyMode,
) -> bool {
    match key_mode {
        KeyMode::K4 => request.play4,
        KeyMode::K5 => request.play5,
        KeyMode::K6 => request.play6,
        KeyMode::K7 => request.play7,
        KeyMode::K8 => request.play8,
        KeyMode::K9 => request.play9,
        KeyMode::K10 => request.play10,
        KeyMode::K14 => request.play14,
    }
}

pub(super) fn hs_fix_option_from_profile(hs_fix: HsFixConfig) -> HsFixOption {
    match hs_fix {
        HsFixConfig::Off => HsFixOption::Off,
        HsFixConfig::StartBpm => HsFixOption::StartBpm,
        HsFixConfig::MinBpm => HsFixOption::MinBpm,
        HsFixConfig::MaxBpm => HsFixOption::MaxBpm,
        HsFixConfig::MainBpm => HsFixOption::MainBpm,
    }
}

pub(super) fn hs_fix_config_from_option(hs_fix: HsFixOption) -> HsFixConfig {
    match hs_fix {
        HsFixOption::Off => HsFixConfig::Off,
        HsFixOption::StartBpm => HsFixConfig::StartBpm,
        HsFixOption::MinBpm => HsFixConfig::MinBpm,
        HsFixOption::MaxBpm => HsFixConfig::MaxBpm,
        HsFixOption::MainBpm => HsFixConfig::MainBpm,
    }
}

pub(super) fn target_option_from_profile(target: TargetOptionConfig) -> TargetOption {
    match target {
        TargetOptionConfig::None => TargetOption::None,
        TargetOptionConfig::RankA => TargetOption::RankA,
        TargetOptionConfig::RankAaMinus => TargetOption::RankAaMinus,
        TargetOptionConfig::RankAa => TargetOption::RankAa,
        TargetOptionConfig::RankAaaMinus => TargetOption::RankAaaMinus,
        TargetOptionConfig::RankAaa => TargetOption::RankAaa,
        TargetOptionConfig::RankMaxMinus => TargetOption::RankMaxMinus,
        TargetOptionConfig::Max => TargetOption::Max,
        TargetOptionConfig::RankNext => TargetOption::RankNext,
        TargetOptionConfig::IrTop => TargetOption::IrTop,
        TargetOptionConfig::IrNext => TargetOption::IrNext,
        TargetOptionConfig::RivalTop => TargetOption::RivalTop,
        TargetOptionConfig::RivalNext => TargetOption::RivalNext,
        TargetOptionConfig::RivalIndex(index) => TargetOption::RivalIndex(index),
    }
}

pub(super) fn target_config_from_option(target: TargetOption) -> TargetOptionConfig {
    match target {
        TargetOption::None => TargetOptionConfig::None,
        TargetOption::RankA => TargetOptionConfig::RankA,
        TargetOption::RankAaMinus => TargetOptionConfig::RankAaMinus,
        TargetOption::RankAa => TargetOptionConfig::RankAa,
        TargetOption::RankAaaMinus => TargetOptionConfig::RankAaaMinus,
        TargetOption::RankAaa => TargetOptionConfig::RankAaa,
        TargetOption::RankMaxMinus => TargetOptionConfig::RankMaxMinus,
        TargetOption::Max => TargetOptionConfig::Max,
        TargetOption::RankNext => TargetOptionConfig::RankNext,
        TargetOption::IrTop => TargetOptionConfig::IrTop,
        TargetOption::IrNext => TargetOptionConfig::IrNext,
        TargetOption::RivalTop => TargetOptionConfig::RivalTop,
        TargetOption::RivalNext => TargetOptionConfig::RivalNext,
        TargetOption::RivalIndex(index) => TargetOptionConfig::RivalIndex(index),
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ActiveLaneState {
    pub(super) lane_cover: f32,
    pub(super) lift: f32,
    pub(super) hispeed_mode: HispeedMode,
    pub(super) target_green_number: u32,
}

pub(super) fn profile_lane_settings_changed(
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

pub(super) fn apply_profile_lane_settings_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    before: &LaneViewConfig,
    profile: &LaneViewConfig,
    speed_locked: bool,
) -> bool {
    if !profile_lane_settings_changed(before, profile) {
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

    if speed_locked {
        return true;
    }

    if target_green_changed {
        session.target_green_number = profile.target_green_number.max(1);
    }

    let direct_hispeed_change = hispeed_changed;
    if direct_hispeed_change {
        session.hispeed = profile.hispeed.clamp(0.5, 10.0);
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

pub(super) fn active_lane_state_for_session(
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
pub(super) struct CurrentPlayOptions {
    pub(super) arrange: ArrangeOption,
    pub(super) arrange_2p: ArrangeOption,
    pub(super) target: TargetOption,
    pub(super) gauge: GaugeTypeConfig,
    pub(super) gauge_auto_shift: GaugeAutoShiftConfig,
    pub(super) bottom_shiftable_gauge: BottomShiftableGaugeConfig,
    pub(super) double_option: DoubleOption,
    pub(super) hs_fix: HsFixOption,
    pub(super) session_mode: SessionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SelectScoreContext {
    pub(super) rule_mode: RuleMode,
    pub(super) ln_mode_policy: LnPolicySetting,
}

impl SelectScoreContext {
    pub(super) fn from_profile(profile: &ProfileConfig) -> Self {
        Self::from_play(&profile.play)
    }

    pub(super) fn from_play(play: &PlayDefaultsConfig) -> Self {
        Self { rule_mode: play.rule_mode, ln_mode_policy: play.ln_mode_policy }
    }
}

pub(super) fn session_mode_from_profile(play: &PlayDefaultsConfig) -> SessionMode {
    play.session_mode.unwrap_or(if play.auto_play {
        SessionMode::Autoplay
    } else {
        SessionMode::Normal
    })
}

pub(super) fn normalize_session_mode_for_course(options: &mut PlayStartOptions) {
    options.session_mode = match options.session_mode {
        SessionMode::Autoplay | SessionMode::AutoplayBattle => SessionMode::Autoplay,
        SessionMode::Normal | SessionMode::GhostBattle => SessionMode::Normal,
    };
    options.autoplay = options.session_mode.primary_autoplay();
    options.replay_player = None;
}

pub(super) fn select_play_options_from_profile(play: &PlayDefaultsConfig) -> CurrentPlayOptions {
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
pub(super) fn replace_skin_config_from_loaded_profile(
    current: &mut ProfileConfig,
    loaded: ProfileConfig,
) {
    current.skin = loaded.skin;
}

pub(super) fn merge_changed_select_play_options_from_profile(
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

pub(super) fn apply_current_play_options_to_profile(
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

pub(super) fn apply_lane_state_to_profile(
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

pub(super) fn clamp_hispeed_for_profile(hispeed: f32, mode: HispeedModeConfig, step: f32) -> f32 {
    let clamped = hispeed.clamp(0.5, 10.0);
    if mode == HispeedModeConfig::Normal
        && (normalize_hispeed_step(step, default_hispeed_step_nhs()) - 0.25).abs() < f32::EPSILON
    {
        (clamped * 4.0).round().clamp(2.0, 40.0) / 4.0
    } else {
        clamped
    }
}

pub(super) fn hispeed_mode_to_config(mode: HispeedMode) -> HispeedModeConfig {
    match mode {
        HispeedMode::Normal => HispeedModeConfig::Normal,
        HispeedMode::Floating => HispeedModeConfig::Floating,
    }
}

pub(super) fn update_pre_ready_play_snapshot_options_for_session(
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

pub(super) fn update_play_exit_hold_started_at(
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

pub(super) fn play_exit_hold_elapsed(
    started_at: Option<Instant>,
    now: Instant,
    duration: Duration,
) -> bool {
    started_at.is_some_and(|started_at| now.duration_since(started_at) >= duration)
}

pub(super) fn select_click_event_arg(
    click_type: i32,
    button: MouseButton,
    rect: Rect,
    x: f32,
    y: f32,
) -> Option<i32> {
    let button_arg = match button {
        MouseButton::Left => 1,
        MouseButton::Right => -1,
        MouseButton::Middle => 1,
        _ => return None,
    };
    match click_type {
        0 => Some(button_arg),
        1 => Some(-button_arg),
        2 => Some(if x >= rect.x + rect.width * 0.5 { 1 } else { -1 }),
        3 => Some(if y <= rect.y + rect.height * 0.5 { 1 } else { -1 }),
        _ => None,
    }
}

pub(super) fn chart_snapshot_metadata_for_chart(
    select_items: &[SelectItem],
    chart_id: i64,
    fallback: impl FnOnce(i64) -> Option<ChartListItem>,
) -> Option<(ChartListItem, Option<u32>)> {
    select_items
        .iter()
        .find_map(|item| match item {
            SelectItem::Chart(row) => row.chart.as_ref().and_then(|chart| {
                (chart.chart_id == chart_id)
                    .then(|| (chart.clone(), row.best_score.as_ref().map(|best| best.ex_score)))
            }),
            _ => None,
        })
        .or_else(|| fallback(chart_id).map(|chart| (chart, None)))
}

pub(super) fn apply_chart_metadata_to_snapshot(
    snapshot: &mut RenderSnapshot,
    chart: &ChartListItem,
    total_notes: u32,
    best_ex_score: Option<u32>,
) {
    snapshot.title.clone_from(&chart.title);
    snapshot.subtitle.clone_from(&chart.subtitle);
    snapshot.artist.clone_from(&chart.artist);
    snapshot.subartist.clone_from(&chart.subartist);
    snapshot.genre.clone_from(&chart.genre);
    snapshot.difficulty_name.clone_from(&chart.difficulty_name);
    snapshot.play_level.clone_from(&chart.play_level);
    snapshot.judge_rank = chart.judge_rank;
    snapshot.total_notes = total_notes;
    snapshot.duration = TimeUs(chart.length_ms.saturating_mul(1_000));
    snapshot.min_bpm = chart.min_bpm as f32;
    snapshot.max_bpm = chart.max_bpm as f32;
    snapshot.now_bpm = chart.initial_bpm as f32;
    // PACEMAKER の MyBest 表示。projected (ghost 進行値) は進捗 0 なので 0。
    snapshot.best_ex_score = best_ex_score;
    snapshot.projected_best_ex_score = best_ex_score.map(|_| 0);
}

pub(super) fn course_titles_from_entries<'a>(
    entries: impl IntoIterator<Item = (&'a str, bool)>,
) -> [String; 10] {
    let mut titles: [String; 10] = Default::default();
    for (index, (title, resolved)) in entries.into_iter().take(10).enumerate() {
        titles[index] = if resolved {
            title.to_string()
        } else {
            format!("(no song) {}", if title.is_empty() { "----" } else { title })
        };
    }
    titles
}

pub(super) fn course_constraint_flags(
    constraints: &bmz_core::course::CourseConstraints,
) -> bmz_render::scene::CourseConstraintFlags {
    use bmz_core::course::{
        CourseClassConstraint, CourseGaugeConstraint, CourseJudgeConstraint, CourseLnConstraint,
        CourseSpeedConstraint,
    };

    bmz_render::scene::CourseConstraintFlags {
        class: constraints.class == CourseClassConstraint::Grade,
        mirror: constraints.class == CourseClassConstraint::GradeMirrorAllowed,
        random: constraints.class == CourseClassConstraint::GradeRandomAllowed,
        no_speed: constraints.speed == CourseSpeedConstraint::NoSpeed,
        no_good: constraints.judge == CourseJudgeConstraint::NoGood,
        no_great: constraints.judge == CourseJudgeConstraint::NoGreat,
        gauge_lr2: constraints.gauge == CourseGaugeConstraint::Lr2,
        gauge_5k: constraints.gauge == CourseGaugeConstraint::Keys5,
        gauge_7k: constraints.gauge == CourseGaugeConstraint::Keys7,
        gauge_9k: constraints.gauge == CourseGaugeConstraint::Keys9,
        gauge_24k: constraints.gauge == CourseGaugeConstraint::Keys24,
        ln: constraints.ln == CourseLnConstraint::Ln,
        cn: constraints.ln == CourseLnConstraint::Cn,
        hcn: constraints.ln == CourseLnConstraint::Hcn,
    }
}

pub(super) fn moved_select_index(
    current_index: usize,
    row_count: usize,
    select_move: SelectMove,
) -> usize {
    if row_count == 0 {
        return 0;
    }

    match select_move {
        SelectMove::Previous => (current_index + row_count - 1) % row_count,
        SelectMove::Next => (current_index + 1) % row_count,
        SelectMove::PagePrevious => (current_index + row_count - (7 % row_count)) % row_count,
        SelectMove::PageNext => (current_index + 7) % row_count,
        SelectMove::First => 0,
        SelectMove::Last => row_count - 1,
    }
}

pub(super) fn select_move_scroll_direction(select_move: SelectMove) -> i32 {
    match select_move {
        SelectMove::Previous | SelectMove::PagePrevious => -1,
        SelectMove::Next | SelectMove::PageNext => 1,
        SelectMove::First | SelectMove::Last => 0,
    }
}

#[cfg(test)]
pub(super) fn hispeed_action(
    physical_key: PhysicalKey,
    state: ElementState,
    repeat: bool,
) -> Option<HispeedChange> {
    match keyboard_lane_action(&ControlInputEvent::keyboard_parts(physical_key, state, repeat)) {
        Some(PlayLaneAction::Hispeed(change)) => Some(change),
        _ => None,
    }
}

pub(super) fn play_option_control_for_input(
    device: DeviceId,
    control: &PhysicalControl,
    e1_held: bool,
    e2_held: bool,
    play_input: Option<&PlayOptionInput>,
    profile_input: &ProfileInputConfig,
) -> Option<PlayOptionControl> {
    let play_input = play_input?;
    let resolved = play_input.resolve_entry(device, control);
    if e1_held && resolved.is_none() && play_input.is_action(device, control, InputActionConfig::E2)
    {
        return Some(PlayOptionControl::ToggleHispeedMode);
    }
    if e1_held == e2_held {
        return None;
    }

    let resolved = resolved?;
    if let Some(direction) = crate::config::play_input::hispeed_direction_for_lane(
        profile_input,
        play_input.key_mode,
        resolved.lane,
    ) {
        return match (e1_held, direction) {
            (true, HispeedDirectionConfig::Down) => {
                Some(PlayOptionControl::Hispeed(HispeedChange::Down))
            }
            (true, HispeedDirectionConfig::Up) => {
                Some(PlayOptionControl::Hispeed(HispeedChange::Up))
            }
            (false, HispeedDirectionConfig::Down) => {
                Some(PlayOptionControl::GreenNumber(GreenNumberChange::Down))
            }
            (false, HispeedDirectionConfig::Up) => {
                Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up))
            }
        };
    }

    let control_name = physical_control_name(control);
    let scratch_direction = resolved.scratch_direction.or_else(|| {
        control_name.and_then(|control| {
            if is_scratch_up_control(control) {
                Some(ScratchDirection::Up)
            } else if is_scratch_down_control(control) {
                Some(ScratchDirection::Down)
            } else {
                None
            }
        })
    })?;
    match (e1_held, scratch_direction) {
        (true, ScratchDirection::Up) => Some(PlayOptionControl::LaneCover(LaneCoverChange::Up)),
        (true, ScratchDirection::Down) => Some(PlayOptionControl::LaneCover(LaneCoverChange::Down)),
        (false, ScratchDirection::Up) => {
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up))
        }
        (false, ScratchDirection::Down) => {
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Down))
        }
    }
}

pub(super) fn visual_offset_delta_control(
    control: &str,
    bindings: &SelectKeyBindings,
) -> Option<i32> {
    if bindings.is_ui_key5(control) {
        Some(-1)
    } else if bindings.is_ui_key7(control) {
        Some(1)
    } else {
        None
    }
}

pub(super) fn green_number_delta_control(
    control: &str,
    bindings: &SelectKeyBindings,
) -> Option<i32> {
    if bindings.is_ui_key4(control) {
        Some(-1)
    } else if bindings.is_ui_key6(control) {
        Some(1)
    } else {
        None
    }
}

#[cfg(test)]
pub(super) fn lane_cover_step(
    physical_key: PhysicalKey,
    state: ElementState,
    repeat: bool,
) -> Option<f32> {
    match keyboard_lane_action(&ControlInputEvent::keyboard_parts(physical_key, state, repeat)) {
        Some(PlayLaneAction::LaneCoverDelta(delta)) => Some(delta),
        _ => None,
    }
}

pub(super) fn lane_cover_change_step(change: LaneCoverChange) -> f32 {
    match change {
        LaneCoverChange::Up => LANE_COVER_STEP,
        LaneCoverChange::Down => -LANE_COVER_STEP,
    }
}

/// アナログスクラッチによる緑数字操作は、レーンカバー操作とは増減方向が逆。
/// 正の step (Scratch Down) で緑数字を上げ、負の step (Scratch Up) で下げる。
pub(super) fn green_number_change_from_analog_steps(steps: i32) -> GreenNumberChange {
    if steps > 0 { GreenNumberChange::Up } else { GreenNumberChange::Down }
}

pub(super) fn green_number_change_step(change: GreenNumberChange) -> i32 {
    match change {
        GreenNumberChange::Up => 1,
        GreenNumberChange::Down => -1,
    }
}

pub(super) fn hispeed_step_for_profile(profile: &ProfileConfig, mode: HispeedMode) -> f32 {
    match mode {
        HispeedMode::Normal => {
            normalize_hispeed_step(profile.lane.hispeed_step_nhs, default_hispeed_step_nhs())
        }
        HispeedMode::Floating => {
            normalize_hispeed_step(profile.lane.hispeed_step_fhs, default_hispeed_step_fhs())
        }
    }
}

pub(super) fn adjusted_hispeed(current: f32, change: HispeedChange, step: f32) -> f32 {
    let delta = match change {
        HispeedChange::Down => -step,
        HispeedChange::Up => step,
    };
    (current + delta).clamp(0.5, 10.0)
}

pub(super) fn apply_pending_play_lane_action_to_state(
    lane: &mut PendingPlayLaneState,
    action: PlayLaneAction,
    profile: &ProfileConfig,
    now_bpm: f32,
    speed_locked: bool,
) -> bool {
    match action {
        PlayLaneAction::ToggleHispeedMode => match lane.hispeed_mode {
            HispeedMode::Normal => {
                lane.target_green_number = lane.current_green_number(now_bpm);
                lane.hispeed_mode = HispeedMode::Floating;
            }
            HispeedMode::Floating => {
                lane.hispeed = lane.hispeed.clamp(0.5, 10.0);
                lane.hispeed_mode = HispeedMode::Normal;
            }
        },
        PlayLaneAction::Hispeed(change) => {
            if speed_locked {
                return false;
            }
            let step = hispeed_step_for_profile(profile, lane.hispeed_mode);
            lane.hispeed = adjusted_hispeed(lane.hispeed, change, step);
        }
        PlayLaneAction::LaneCoverDelta(delta) => {
            if lane.lane_cover_visible {
                lane.lane_cover = (lane.lane_cover - delta)
                    .clamp(0.0, crate::config::play::lane_cover_max_for_lift(lane.lift));
                lane.refresh_cover_hispeed(now_bpm, speed_locked);
            } else {
                lane.lift = (lane.lift + delta).clamp(0.0, (1.0 - lane.lane_cover).clamp(0.0, 1.0));
                if lane.hispeed_auto_adjust {
                    lane.refresh_floating_hispeed(now_bpm, speed_locked);
                }
            }
        }
        PlayLaneAction::GreenNumberDelta(delta) => {
            if speed_locked {
                return false;
            }
            let current = match lane.hispeed_mode {
                HispeedMode::Normal => lane.current_green_number(now_bpm),
                HispeedMode::Floating => lane.target_green_number,
            };
            lane.target_green_number = adjusted_green_number(current, delta);
            lane.hispeed_mode = HispeedMode::Floating;
            lane.refresh_floating_hispeed(now_bpm, speed_locked);
        }
        PlayLaneAction::ToggleLaneCoverVisibility => {
            let was_visible = lane.lane_cover_visible;
            lane.lane_cover_visible = !lane.lane_cover_visible;
            if !was_visible && lane.lane_cover_visible {
                lane.refresh_cover_hispeed(now_bpm, speed_locked);
            }
        }
    }
    true
}

pub(super) fn sync_active_play_visual_offset_to_profile(
    profile: &mut ProfileConfig,
    visual_offset_us: i64,
    auto_adjust_active: bool,
) {
    if !auto_adjust_active || profile.judge.visual_offset_us == visual_offset_us {
        return;
    }
    profile.judge.visual_offset_us = visual_offset_us;
    profile.updated_at = now_unix_seconds();
}

pub(super) fn apply_hispeed_change_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    change: HispeedChange,
    step: f32,
) {
    session.hispeed = adjusted_hispeed(session.hispeed, change, step);
}

pub(super) fn apply_play_lane_action_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    action: PlayLaneAction,
    speed_locked: bool,
    hispeed_step: f32,
) -> bool {
    match action {
        PlayLaneAction::ToggleHispeedMode => {
            match session.hispeed_mode {
                HispeedMode::Normal => {
                    let now = session.audio_clock.now();
                    session.target_green_number = current_green_number(session, now);
                    session.hispeed_mode = HispeedMode::Floating;
                }
                HispeedMode::Floating => {
                    session.hispeed = session.hispeed.clamp(0.5, 10.0);
                    session.hispeed_mode = HispeedMode::Normal;
                }
            }
            true
        }
        PlayLaneAction::Hispeed(change) => {
            if speed_locked {
                return false;
            }
            apply_hispeed_change_to_session(session, change, hispeed_step);
            true
        }
        PlayLaneAction::LaneCoverDelta(delta) => {
            apply_lane_cover_step_to_session(session, delta, speed_locked)
        }
        PlayLaneAction::GreenNumberDelta(delta) => {
            apply_green_number_step_to_session(session, delta, speed_locked)
        }
        PlayLaneAction::ToggleLaneCoverVisibility => {
            toggle_lane_cover_visibility(session, speed_locked);
            true
        }
    }
}

#[cfg(test)]
pub(super) fn apply_play_option_control_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    action: PlayOptionControl,
    speed_locked: bool,
    hispeed_step: f32,
) -> bool {
    apply_play_lane_action_to_session(
        session,
        lane_action_from_option(action, false).expect("button option always maps to a lane action"),
        speed_locked,
        hispeed_step,
    )
}

pub(super) fn replay_pending_play_lane_actions(
    session: &mut bmz_gameplay::session::GameSession,
    actions: &[PlayLaneAction],
    profile: &ProfileConfig,
    speed_locked: bool,
) {
    for &action in actions {
        let step = hispeed_step_for_profile(profile, session.hispeed_mode);
        let _ = apply_play_lane_action_to_session(session, action, speed_locked, step);
    }
}

pub(super) fn handoff_pending_play_visual_input(
    session: &mut bmz_gameplay::session::GameSession,
    input: &SharedInputBackend,
    visual_input: &PendingPlayVisualInput,
) {
    let mut input = input.clone();
    let _ = input.drain_events();
    visual_input.clone().apply_to_session(session);
}

pub(super) fn apply_green_number_step_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    delta: i32,
    speed_locked: bool,
) -> bool {
    if speed_locked {
        return false;
    }
    let current = match session.hispeed_mode {
        HispeedMode::Normal => current_green_number(session, session.audio_clock.now()),
        HispeedMode::Floating => session.target_green_number,
    };
    session.target_green_number = adjusted_green_number(current, delta);
    session.hispeed_mode = HispeedMode::Floating;
    let now = session.audio_clock.now();
    session.hispeed =
        hispeed_for_green_number(session, active_lane_cover_for_hispeed(session), now);
    true
}

pub(super) fn apply_lane_cover_step_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    delta: f32,
    speed_locked: bool,
) -> bool {
    if session.lane_cover_visible {
        session.lane_cover = (session.lane_cover - delta)
            .clamp(0.0, crate::config::play::lane_cover_max_for_lift(session.lift));
        if session.hispeed_mode == HispeedMode::Floating && !speed_locked {
            let now = session.audio_clock.now();
            session.hispeed = if session.hispeed_auto_adjust {
                hispeed_for_green_number(session, session.lane_cover, now)
            } else {
                hispeed_for_green_number_at_bpm(
                    session,
                    session.lane_cover,
                    now,
                    session.hsfix_base_bpm,
                )
            };
        }
    } else {
        session.lift =
            (session.lift + delta).clamp(0.0, (1.0 - session.lane_cover).clamp(0.0, 1.0));
        if session.hispeed_auto_adjust
            && session.hispeed_mode == HispeedMode::Floating
            && !speed_locked
        {
            let now = session.audio_clock.now();
            session.hispeed = hispeed_for_green_number(session, 0.0, now);
        }
    }
    true
}

pub(super) fn reset_floating_hispeed_if_enabled(
    session: &mut bmz_gameplay::session::GameSession,
    speed_locked: bool,
) {
    if session.hispeed_mode == HispeedMode::Floating && !speed_locked {
        let now = session.audio_clock.now();
        let lane_cover = active_lane_cover_for_hispeed(session);
        session.hispeed = if session.hispeed_auto_adjust {
            hispeed_for_green_number(session, lane_cover, now)
        } else {
            hispeed_for_green_number_at_bpm(session, lane_cover, now, session.hsfix_base_bpm)
        };
    }
}

/// Start / E1 の連続押し間隔を判定する。2回目なら true を返しタイムスタンプをクリアする。
pub(super) fn register_play_start_double_press(
    last_press_at: &mut Option<Instant>,
    now: Instant,
) -> bool {
    let is_double = last_press_at
        .is_some_and(|prev| now.duration_since(prev) <= PLAY_START_DOUBLE_PRESS_WINDOW);
    if is_double {
        *last_press_at = None;
        true
    } else {
        *last_press_at = Some(now);
        false
    }
}

pub(super) fn toggle_lane_cover_visibility(
    session: &mut bmz_gameplay::session::GameSession,
    speed_locked: bool,
) {
    let was_visible = session.lane_cover_visible;
    session.lane_cover_visible = !session.lane_cover_visible;
    if !was_visible && session.lane_cover_visible {
        reset_floating_hispeed_if_enabled(session, speed_locked);
    }
}

pub(super) fn active_lane_cover_for_hispeed(session: &bmz_gameplay::session::GameSession) -> f32 {
    if session.lane_cover_visible {
        crate::config::play::clamp_lane_cover_for_lift(session.lane_cover, session.lift)
    } else {
        0.0
    }
}

pub(super) fn current_green_number(
    session: &bmz_gameplay::session::GameSession,
    now: TimeUs,
) -> u32 {
    let total = note_display_duration_ms_for_hispeed(
        session,
        session.hispeed,
        active_lane_cover_for_hispeed(session),
        now,
    );
    green_number_from_display_duration(total)
}

pub(super) fn adjusted_green_number(current: u32, delta: i32) -> u32 {
    let next = current as i64 + delta as i64;
    next.clamp(TARGET_GREEN_NUMBER_MIN as i64, TARGET_GREEN_NUMBER_MAX as i64) as u32
}

pub(super) fn green_number_from_display_duration(duration_ms: f32) -> u32 {
    let displayed_duration_ms = duration_ms.round().clamp(0.0, i32::MAX as f32) as i32;
    bmz_render::skin::duration_to_green_number_ms(displayed_duration_ms)
        .clamp(TARGET_GREEN_NUMBER_MIN as i32, TARGET_GREEN_NUMBER_MAX as i32) as u32
}

pub(super) fn instant_elapsed_us_u64(start: Instant) -> u64 {
    start.elapsed().as_micros().min(u64::MAX as u128) as u64
}

pub(super) fn instant_duration_us_u64(start: Instant, end: Instant) -> u64 {
    end.saturating_duration_since(start).as_micros().min(u64::MAX as u128) as u64
}

pub(super) fn duration_us_u64(duration: Duration) -> u64 {
    duration.as_micros().min(u64::MAX as u128) as u64
}

pub(super) fn count_smoke_play_frame(rendered_frames: u32, exit_after_frames: u32) -> (u32, bool) {
    let frames = rendered_frames.saturating_add(1);
    (frames, frames >= exit_after_frames)
}

pub(super) fn note_display_duration_ms_for_hispeed(
    session: &bmz_gameplay::session::GameSession,
    hispeed: f32,
    lane_cover: f32,
    now: TimeUs,
) -> f32 {
    let now_bpm = floating_hispeed_target_bpm(session, now);
    let scroll_multiplier = crate::screens::play_snapshot::current_scroll_multiplier(
        &session.chart,
        &session.timing_map,
        now,
    );
    crate::screens::play_snapshot::display_duration_ms_for_bpm_hispeed(
        now_bpm as f32,
        hispeed,
        lane_cover,
        session.lift,
        scroll_multiplier,
    )
}

pub(super) fn hispeed_for_green_number(
    session: &bmz_gameplay::session::GameSession,
    lane_cover: f32,
    now: TimeUs,
) -> f32 {
    hispeed_for_green_number_at_bpm(
        session,
        lane_cover,
        now,
        floating_hispeed_target_bpm(session, now),
    )
}

pub(super) fn hispeed_for_green_number_at_bpm(
    session: &bmz_gameplay::session::GameSession,
    lane_cover: f32,
    now: TimeUs,
    target_bpm: f64,
) -> f32 {
    let target_green = session.target_green_number.max(1) as f32;
    let visible_max = crate::config::play::visible_lane_fraction(lane_cover, session.lift);
    let scroll_multiplier = crate::screens::play_snapshot::current_scroll_multiplier(
        &session.chart,
        &session.timing_map,
        now,
    );
    let hispeed = hispeed_for_green_number_values(
        target_green,
        visible_max,
        target_bpm.max(1.0),
        scroll_multiplier,
    );
    hispeed.clamp(0.5, 10.0)
}

pub(super) fn floating_hispeed_target_bpm(
    session: &bmz_gameplay::session::GameSession,
    now: TimeUs,
) -> f64 {
    if session.audio_clock.running && now.0 >= 0 {
        session.timing_map.bpm_at_time(now).max(1.0)
    } else {
        session.hsfix_base_bpm.max(1.0)
    }
}

pub(super) fn chart_started_for_system_sound(session: &bmz_gameplay::session::GameSession) -> bool {
    session.audio_clock.running && session.audio_clock.now().0 >= 0
}

pub(super) fn hispeed_for_green_number_values(
    target_green: f32,
    visible_max: f32,
    now_bpm: f64,
    scroll_multiplier: f32,
) -> f32 {
    crate::screens::play_snapshot::hispeed_for_green_number_values(
        target_green,
        visible_max,
        now_bpm,
        scroll_multiplier,
    )
}

#[cfg(test)]
pub(super) fn result_action(
    physical_key: PhysicalKey,
    state: ElementState,
    repeat: bool,
) -> Option<ResultAction> {
    scene_result_action(&ControlInputEvent::keyboard_parts(physical_key, state, repeat))
}

pub(super) fn result_exit_skip_key(
    physical_key: PhysicalKey,
    state: ElementState,
    repeat: bool,
) -> bool {
    if state != ElementState::Pressed || repeat {
        return false;
    }
    matches!(physical_key, PhysicalKey::Code(KeyCode::Enter | KeyCode::Escape))
}

pub(super) fn result_exit_transition_ready(
    elapsed: Duration,
    fadeout: Duration,
    animation_duration: Duration,
    skip_requested: bool,
    skip_final_frame_held: bool,
) -> bool {
    let required_duration = if skip_requested { animation_duration } else { fadeout };
    elapsed >= required_duration && (!skip_requested || skip_final_frame_held)
}

#[cfg(test)]
pub(super) fn decide_control_action(
    control: &str,
    bindings: &SelectKeyBindings,
) -> Option<DecideAction> {
    scene_decide_action(&ControlInputEvent::gamepad(DeviceId(1), control, true), bindings)
}

pub(super) fn decide_cancel_chord_pressed(e1_held: bool, e2_held: bool, e3_held: bool) -> bool {
    e2_held && (e1_held || e3_held)
}

pub(super) fn elapsed_since(started_at: Instant) -> TimeUs {
    TimeUs(started_at.elapsed().as_micros().min(i64::MAX as u128) as i64)
}

pub(super) fn elapsed_since_ms(started_at: Instant) -> i32 {
    (started_at.elapsed().as_millis().min(i32::MAX as u128)) as i32
}

pub(super) fn apply_operating_time_ms_to_scene(
    scene: &mut AppSceneSnapshot,
    operating_time_ms: i32,
) {
    match scene {
        AppSceneSnapshot::Select(snapshot) => {
            snapshot.operating_time_ms = operating_time_ms;
        }
        AppSceneSnapshot::Decide(snapshot) | AppSceneSnapshot::Play(snapshot) => {
            snapshot.operating_time_ms = operating_time_ms;
        }
        AppSceneSnapshot::Result(_) => {}
    }
}

pub(super) fn apply_skin_runtime_info_to_scene(
    scene: &mut AppSceneSnapshot,
    player_name: &str,
    current_fps: u32,
) {
    match scene {
        AppSceneSnapshot::Select(snapshot) => {
            snapshot.player_name = player_name.to_string();
            snapshot.current_fps = current_fps;
        }
        AppSceneSnapshot::Decide(snapshot) | AppSceneSnapshot::Play(snapshot) => {
            snapshot.player_name = player_name.to_string();
            snapshot.current_fps = current_fps;
        }
        AppSceneSnapshot::Result(snapshot) => {
            snapshot.player_name = player_name.to_string();
            snapshot.current_fps = current_fps;
        }
    }
}

pub(super) fn preloaded_matches_start(
    preloaded: &PreloadedInputPlaySession,
    chart_id: i64,
    options: &PlayStartOptions,
) -> bool {
    preloaded.chart_id == chart_id
        && preloaded.session_options.autoplay == options.autoplay
        && preloaded.session_options.practice_mode == options.practice_mode
        && preloaded.session_options.arrange == options.arrange
        && preloaded.session_options.arrange_2p == options.arrange_2p
        && preloaded.session_options.double_option == options.double_option
        && preloaded.session_options.hs_fix == options.hs_fix
        && preloaded.session_options.arrange_seed == options.arrange_seed
        && preloaded.session_options.arrange_seed_2p == options.arrange_seed_2p
        && preloaded.session_options.random_trainer_seed == options.random_trainer_seed
        && preloaded.session_options.legacy_arrange_seed == options.legacy_arrange_seed
        && preloaded.session_options.bms_random_seed == options.bms_random_seed
        && preloaded.session_options.bms_random_choices == options.bms_random_choices
        && preloaded.session_options.arrange_pattern == options.arrange_pattern
        && preloaded.session_options.initial_gauge_value == options.initial_gauge_value
        && preloaded.session_options.initial_gauge_values == options.initial_gauge_values
        && preloaded.session_options.initial_course_combo == options.initial_course_combo
}
