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
