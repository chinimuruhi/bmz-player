pub(super) fn profile_selection_label(
    profiles: &[crate::storage::profile::ProfileSummary],
    profile_id: &str,
) -> String {
    profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .map(|profile| format!("{} ({})", profile.id, profile.display_name))
        .unwrap_or_else(|| profile_id.to_string())
}

pub(super) fn trimmed_non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

pub(super) fn profile_id_text_edit(ui: &mut egui::Ui, value: &mut String) {
    if ui.text_edit_singleline(value).changed() {
        sanitize_profile_id_input(value);
    }
}

pub(super) fn sanitize_profile_id_input(value: &mut String) {
    value.retain(is_profile_id_char);
    if value.len() > 64 {
        value.truncate(64);
    }
}

pub(super) fn is_profile_id_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

pub(super) fn volume_slider(ui: &mut egui::Ui, value: &mut u32, label: &str) {
    ui.add(egui::Slider::new(value, 0..=100).text(label));
}

pub(super) fn lane_unit_slider(ui: &mut egui::Ui, value: &mut u32, label: &str) {
    lane_unit_slider_with_max(ui, value, label, 1000);
}

pub(super) fn lane_unit_slider_with_max(ui: &mut egui::Ui, value: &mut u32, label: &str, max: u32) {
    *value = (*value).min(max);
    ui.add(egui::Slider::new(value, 0..=max).text(label));
}

const OFFSET_SLIDER_MIN_MS: i64 = -500;
const OFFSET_SLIDER_MAX_MS: i64 = 500;
const OFFSET_SLIDER_STEP_MS: f64 = 1.0;

pub(super) fn offset_ms_slider(ui: &mut egui::Ui, value_us: &mut i64, label: &str) {
    let mut value_ms = (*value_us / 1_000).clamp(OFFSET_SLIDER_MIN_MS, OFFSET_SLIDER_MAX_MS);
    let response = ui.add(
        egui::Slider::new(&mut value_ms, OFFSET_SLIDER_MIN_MS..=OFFSET_SLIDER_MAX_MS)
            // smart_aim は 5/10/50 などの値へ吸着するため、オフセットでは使わない。
            .smart_aim(false)
            .step_by(OFFSET_SLIDER_STEP_MS)
            .text(format!("{label} (ms)")),
    );
    if response.changed() {
        *value_us = value_ms * 1_000;
    }
}

pub(super) fn judge_algorithm_label(value: JudgeAlgorithmConfig) -> &'static str {
    match value {
        JudgeAlgorithmConfig::Combo => "COMBO",
        JudgeAlgorithmConfig::Duration => "DURATION",
        JudgeAlgorithmConfig::Lowest => "LOWEST",
    }
}

pub(super) fn fast_slow_scope_label(text: Localizer, value: FastSlowDisplayScope) -> String {
    match value {
        FastSlowDisplayScope::Auto => tr!(text, "profile-fast-slow-auto"),
        FastSlowDisplayScope::ThresholdMs => tr!(text, "profile-fast-slow-threshold-mode"),
    }
}

pub(super) fn rule_mode_label(value: RuleMode) -> &'static str {
    match value {
        RuleMode::Beatoraja => "BEATORAJA",
        RuleMode::Lr2Oraja => "LR2ORAJA",
        RuleMode::Dx => "DX",
    }
}

pub(super) fn gauge_label(value: GaugeTypeConfig) -> &'static str {
    match value {
        GaugeTypeConfig::AssistEasy => "ASSIST EASY",
        GaugeTypeConfig::Easy => "EASY",
        GaugeTypeConfig::Normal => "NORMAL",
        GaugeTypeConfig::Hard => "HARD",
        GaugeTypeConfig::ExHard => "EX HARD",
        GaugeTypeConfig::AutoShift => "AUTO SHIFT",
        GaugeTypeConfig::Hazard => "HAZARD",
    }
}

pub(super) fn gauge_auto_shift_label(value: GaugeAutoShiftConfig) -> &'static str {
    match value {
        GaugeAutoShiftConfig::Off => "OFF",
        GaugeAutoShiftConfig::Continue => "CONTINUE",
        GaugeAutoShiftConfig::HardToGroove => "HARD->GROOVE",
        GaugeAutoShiftConfig::BestClear => "BEST CLEAR",
        GaugeAutoShiftConfig::SelectToUnder => "SELECT UNDER",
    }
}

pub(super) fn bottom_shiftable_gauge_label(value: BottomShiftableGaugeConfig) -> &'static str {
    match value {
        BottomShiftableGaugeConfig::AssistEasy => "ASSIST EASY",
        BottomShiftableGaugeConfig::Easy => "EASY",
        BottomShiftableGaugeConfig::Normal => "NORMAL",
    }
}

pub(super) fn random_label(value: RandomOptionConfig) -> &'static str {
    match value {
        RandomOptionConfig::Off => "OFF",
        RandomOptionConfig::Mirror => "MIRROR",
        RandomOptionConfig::Random => "RANDOM",
        RandomOptionConfig::RRandom => "R-RANDOM",
        RandomOptionConfig::SRandom => "S-RANDOM",
        RandomOptionConfig::Spiral => "SPIRAL",
        RandomOptionConfig::HRandom => "H-RANDOM",
        RandomOptionConfig::AllScratch => "ALL-SCR",
        RandomOptionConfig::RandomEx => "RANDOM-EX",
        RandomOptionConfig::SRandomEx => "S-RANDOM-EX",
        RandomOptionConfig::FRandom => "F-RANDOM",
        RandomOptionConfig::MFRandom => "MF-RANDOM",
    }
}

pub(super) fn random_options() -> [(RandomOptionConfig, &'static str); 12] {
    [
        (RandomOptionConfig::Off, "OFF"),
        (RandomOptionConfig::Mirror, "MIRROR"),
        (RandomOptionConfig::Random, "RANDOM"),
        (RandomOptionConfig::RRandom, "R-RANDOM"),
        (RandomOptionConfig::SRandom, "S-RANDOM"),
        (RandomOptionConfig::Spiral, "SPIRAL"),
        (RandomOptionConfig::HRandom, "H-RANDOM"),
        (RandomOptionConfig::AllScratch, "ALL-SCR"),
        (RandomOptionConfig::RandomEx, "RANDOM-EX"),
        (RandomOptionConfig::SRandomEx, "S-RANDOM-EX"),
        (RandomOptionConfig::FRandom, "F-RANDOM"),
        (RandomOptionConfig::MFRandom, "MF-RANDOM"),
    ]
}

pub(super) fn double_option_label(value: DoubleOptionConfig) -> &'static str {
    match value {
        DoubleOptionConfig::Off => "OFF",
        DoubleOptionConfig::Flip => "FLIP",
        DoubleOptionConfig::Battle => "BATTLE",
        DoubleOptionConfig::BattleAutoScratch => "BATTLE AS",
    }
}

pub(super) fn hs_fix_label(value: HsFixConfig) -> &'static str {
    match value {
        HsFixConfig::Off => "OFF",
        HsFixConfig::StartBpm => "START BPM",
        HsFixConfig::MinBpm => "MIN BPM",
        HsFixConfig::MaxBpm => "MAX BPM",
        HsFixConfig::MainBpm => "MAIN BPM",
    }
}

pub(super) fn target_label(value: TargetOptionConfig) -> String {
    match value {
        TargetOptionConfig::None => "NONE".to_string(),
        TargetOptionConfig::RankA => "RANK_A".to_string(),
        TargetOptionConfig::RankAaMinus => "RANK_AA-".to_string(),
        TargetOptionConfig::RankAa => "RANK_AA".to_string(),
        TargetOptionConfig::RankAaaMinus => "RANK_AAA-".to_string(),
        TargetOptionConfig::RankAaa => "RANK_AAA".to_string(),
        TargetOptionConfig::RankMaxMinus => "RANK_MAX-".to_string(),
        TargetOptionConfig::Max => "MAX".to_string(),
        TargetOptionConfig::RankNext => "RANK_NEXT".to_string(),
        TargetOptionConfig::IrTop => "IR_TOP".to_string(),
        TargetOptionConfig::IrNext => "IR_NEXT".to_string(),
        TargetOptionConfig::RivalTop => "RIVAL TOP".to_string(),
        TargetOptionConfig::RivalNext => "RIVAL NEXT".to_string(),
        TargetOptionConfig::RivalIndex(index) => format!("RIVAL_{index}"),
    }
}

pub(super) fn grade_diff_display_label(value: ResultGradeDiffDisplay) -> &'static str {
    match value {
        ResultGradeDiffDisplay::Next => "NEXT",
        ResultGradeDiffDisplay::Nearest => "NEAREST",
    }
}

pub(super) fn lane_effect_label(value: LaneEffectConfig) -> &'static str {
    match value {
        LaneEffectConfig::Off => "OFF",
        LaneEffectConfig::Hidden => "HIDDEN",
        LaneEffectConfig::Sudden => "SUDDEN",
        LaneEffectConfig::HiddenSudden => "HIDDEN+SUDDEN",
    }
}

pub(super) fn bga_mode_label(value: BgaModeConfig) -> &'static str {
    match value {
        BgaModeConfig::On => "ON",
        BgaModeConfig::Auto => "AUTO",
        BgaModeConfig::Off => "OFF",
    }
}

pub(super) fn bga_expand_label(value: BgaExpandConfig) -> &'static str {
    match value {
        BgaExpandConfig::Full => "FULL",
        BgaExpandConfig::KeepAspect => "KEEP ASPECT",
        BgaExpandConfig::Off => "OFF",
    }
}

pub(super) fn hispeed_mode_label(value: HispeedModeConfig) -> &'static str {
    match value {
        HispeedModeConfig::Normal => "NORMAL",
        HispeedModeConfig::Floating => "FLOATING",
    }
}

pub(super) fn replay_slot_rule_label(value: ReplaySlotRule) -> &'static str {
    match value {
        ReplaySlotRule::Disabled => "DISABLED",
        ReplaySlotRule::Always => "ALWAYS",
        ReplaySlotRule::ScoreUpdate => "SCORE UPDATE",
        ReplaySlotRule::BpUpdate => "BP UPDATE",
        ReplaySlotRule::MaxComboUpdate => "MAX COMBO UPDATE",
        ReplaySlotRule::ClearUpdate => "CLEAR UPDATE",
    }
}

pub(super) fn system_sound_path_row(
    ui: &mut egui::Ui,
    text: Localizer,
    label: &str,
    value: &mut String,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(value).desired_width(260.0));
        if ui.button(tr!(text, "common-choose-folder")).clicked()
            && let Some(folder) = rfd::FileDialog::new().pick_folder()
        {
            *value = folder.to_string_lossy().into_owned();
        }
    });
}

pub(super) fn ir_provider_text_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(value);
    });
}

pub(super) fn ir_provider_family(provider: &str) -> &'static str {
    if crate::ir::rian_ir::is_rian_ir_provider(provider) {
        crate::ir::rian_ir::RIAN_IR_PROVIDER
    } else {
        crate::ir::bmz_official::BMZ_IR_PROVIDER
    }
}

pub(super) fn normalized_ir_base_url(url: &str) -> Option<String> {
    let mut parsed = reqwest::Url::parse(url.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    parsed.set_fragment(None);
    parsed.set_query(None);
    let path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(if path.is_empty() { "/" } else { &path });
    Some(parsed.to_string().trim_end_matches('/').to_ascii_lowercase())
}

pub(super) fn classify_ir_provider_preset(provider: &IrProviderConfig) -> IrProviderPreset {
    let normalized = normalized_ir_base_url(&provider.base_url);
    let family = ir_provider_family(&provider.provider);
    let bmz_url = normalized_ir_base_url(crate::ir::bmz_official::BMZ_IR_DEFAULT_BASE_URL);
    let rian_public = normalized_ir_base_url(crate::ir::rian_ir::RIAN_IR_PUBLIC_BASE_URL);
    let rian_api = normalized_ir_base_url(crate::ir::rian_ir::RIAN_IR_DEFAULT_BASE_URL);

    if family == crate::ir::bmz_official::BMZ_IR_PROVIDER && normalized == bmz_url {
        IrProviderPreset::BmzIr
    } else if family == crate::ir::rian_ir::RIAN_IR_PROVIDER
        && (normalized == rian_public || normalized == rian_api)
    {
        IrProviderPreset::RianIr
    } else {
        IrProviderPreset::Other
    }
}

pub(super) fn apply_ir_provider_preset(provider: &mut IrProviderConfig, preset: IrProviderPreset) {
    match preset {
        IrProviderPreset::BmzIr => {
            provider.provider = crate::ir::bmz_official::BMZ_IR_PROVIDER.to_string();
            provider.base_url = crate::ir::bmz_official::BMZ_IR_DEFAULT_BASE_URL.to_string();
        }
        IrProviderPreset::RianIr => {
            provider.provider = crate::ir::rian_ir::RIAN_IR_PROVIDER.to_string();
            provider.base_url = crate::ir::rian_ir::RIAN_IR_PUBLIC_BASE_URL.to_string();
        }
        IrProviderPreset::Other => {}
    }
}

pub(super) fn ir_provider_preset_label(text: Localizer, preset: IrProviderPreset) -> String {
    match preset {
        IrProviderPreset::BmzIr => tr!(text, "profile-ir-provider-bmz"),
        IrProviderPreset::RianIr => tr!(text, "profile-ir-provider-rian"),
        IrProviderPreset::Other => tr!(text, "profile-ir-provider-other"),
    }
}

pub(super) fn ir_send_policy_label(value: IrSendPolicyConfig) -> &'static str {
    match value {
        IrSendPolicyConfig::UpdateScore => "UPDATE SCORE",
        IrSendPolicyConfig::Always => "ALWAYS",
        IrSendPolicyConfig::CompleteSong => "COMPLETE SONG",
    }
}

pub(super) fn ir_primary_provider_label(provider: &IrProviderConfig, provider_key: &str) -> String {
    let account = provider.account_display_name.trim();
    if account.is_empty() {
        format!("{provider_key} ({})", provider.base_url)
    } else {
        format!("{provider_key} - {account} ({})", provider.base_url)
    }
}

pub(super) fn sync_ir_provider_roles(ir_config: &mut IrConfig) -> bool {
    let primary_provider = ir_config.primary_provider.trim();
    let mut changed = false;
    for provider in &mut ir_config.providers {
        let next_role = if !primary_provider.is_empty()
            && crate::ir::provider_key::configured_provider_key(provider)
                .is_some_and(|provider_key| provider_key == primary_provider)
        {
            IrProviderRoleConfig::Primary
        } else {
            IrProviderRoleConfig::SubmitOnly
        };
        if provider.role != next_role {
            provider.role = next_role;
            changed = true;
        }
    }
    changed
}

pub(super) fn format_optional_timestamp(value: Option<i64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string())
}
