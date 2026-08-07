use bmz_chart::model::{LongNoteMode, PlayableChart};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LnPolicySetting {
    #[default]
    AutoLn,
    AutoCn,
    AutoHcn,
    ForceLn,
    ForceCn,
    ForceHcn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum LnScorePolicy {
    AutoLn,
    AutoCn,
    AutoHcn,
    ForceLn,
    ForceCn,
    ForceHcn,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChartLnProfile {
    pub has_undefined_ln: bool,
    pub has_defined_ln: bool,
    pub has_defined_cn: bool,
    pub has_defined_hcn: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChartLnCounts {
    pub undefined_ln_pairs: u32,
    pub defined_ln_pairs: u32,
    pub defined_cn_pairs: u32,
    pub defined_hcn_pairs: u32,
}

impl ChartLnCounts {
    pub fn from_chart(chart: &PlayableChart) -> Self {
        let mut counts = Self::default();
        for pair in &chart.long_notes {
            let count = match pair.mode {
                Some(LongNoteMode::Ln) => &mut counts.defined_ln_pairs,
                Some(LongNoteMode::Cn) => &mut counts.defined_cn_pairs,
                Some(LongNoteMode::Hcn) => &mut counts.defined_hcn_pairs,
                None => &mut counts.undefined_ln_pairs,
            };
            *count = count.saturating_add(1);
        }
        counts
    }

    pub const fn profile(self) -> ChartLnProfile {
        ChartLnProfile {
            has_undefined_ln: self.undefined_ln_pairs > 0,
            has_defined_ln: self.defined_ln_pairs > 0,
            has_defined_cn: self.defined_cn_pairs > 0,
            has_defined_hcn: self.defined_hcn_pairs > 0,
        }
    }

    pub const fn total_pairs(self) -> u32 {
        self.undefined_ln_pairs
            .saturating_add(self.defined_ln_pairs)
            .saturating_add(self.defined_cn_pairs)
            .saturating_add(self.defined_hcn_pairs)
    }

    /// Score-target note count for a resolved score policy.
    ///
    /// `base_total_notes` is Tap + LongStart. CN/HCN ends add one score target;
    /// LN ends replace their LongStart target and therefore add nothing here.
    pub const fn scored_total_notes(self, base_total_notes: u32, policy: LnScorePolicy) -> u32 {
        let extra_ends = match policy {
            LnScorePolicy::ForceLn => 0,
            LnScorePolicy::ForceCn | LnScorePolicy::ForceHcn => self.total_pairs(),
            LnScorePolicy::AutoLn => self.defined_cn_pairs.saturating_add(self.defined_hcn_pairs),
            LnScorePolicy::AutoCn | LnScorePolicy::AutoHcn => self
                .undefined_ln_pairs
                .saturating_add(self.defined_cn_pairs)
                .saturating_add(self.defined_hcn_pairs),
        };
        base_total_notes.saturating_add(extra_ends)
    }

    pub fn scored_total_notes_for_setting(
        self,
        base_total_notes: u32,
        setting: LnPolicySetting,
    ) -> u32 {
        self.scored_total_notes(base_total_notes, score_ln_policy(setting, self.profile()))
    }

    /// Score-target count defined by the chart itself, without profile fallback.
    pub const fn canonical_total_notes(self, base_total_notes: u32) -> u32 {
        base_total_notes
            .saturating_add(self.defined_cn_pairs)
            .saturating_add(self.defined_hcn_pairs)
    }
}

impl ChartLnProfile {
    pub fn from_chart(chart: &PlayableChart) -> Self {
        ChartLnCounts::from_chart(chart).profile()
    }

    pub fn has_any_ln(self) -> bool {
        self.has_undefined_ln || self.has_any_defined_ln()
    }

    pub fn has_any_defined_ln(self) -> bool {
        self.has_defined_ln || self.has_defined_cn || self.has_defined_hcn
    }

    fn single_defined_mode(self) -> Option<LongNoteMode> {
        match (self.has_defined_ln, self.has_defined_cn, self.has_defined_hcn) {
            (true, false, false) => Some(LongNoteMode::Ln),
            (false, true, false) => Some(LongNoteMode::Cn),
            (false, false, true) => Some(LongNoteMode::Hcn),
            _ => None,
        }
    }
}

pub const fn source_ln_mode(profile: ChartLnProfile) -> Option<LongNoteMode> {
    if profile.has_defined_hcn {
        Some(LongNoteMode::Hcn)
    } else if profile.has_defined_cn {
        Some(LongNoteMode::Cn)
    } else if profile.has_defined_ln || profile.has_undefined_ln {
        Some(LongNoteMode::Ln)
    } else {
        None
    }
}

pub const fn max_long_note_mode(
    left: Option<LongNoteMode>,
    right: Option<LongNoteMode>,
) -> Option<LongNoteMode> {
    match (left, right) {
        (Some(LongNoteMode::Hcn), _) | (_, Some(LongNoteMode::Hcn)) => Some(LongNoteMode::Hcn),
        (Some(LongNoteMode::Cn), _) | (_, Some(LongNoteMode::Cn)) => Some(LongNoteMode::Cn),
        (Some(LongNoteMode::Ln), _) | (_, Some(LongNoteMode::Ln)) => Some(LongNoteMode::Ln),
        (None, None) => None,
    }
}

/// 実際に降らせたLN種別。AUTOは定義済み種別を維持し、未定義LNへ
/// 適用したfallbackとの上位種を返す。FORCEは全LNを指定種別へ変換する。
pub fn played_ln_mode(profile: ChartLnProfile, policy: LnScorePolicy) -> Option<LongNoteMode> {
    if !profile.has_any_ln() {
        return None;
    }

    match policy {
        LnScorePolicy::ForceLn => Some(LongNoteMode::Ln),
        LnScorePolicy::ForceCn => Some(LongNoteMode::Cn),
        LnScorePolicy::ForceHcn => Some(LongNoteMode::Hcn),
        LnScorePolicy::AutoLn | LnScorePolicy::AutoCn | LnScorePolicy::AutoHcn => {
            let defined = if profile.has_defined_hcn {
                Some(LongNoteMode::Hcn)
            } else if profile.has_defined_cn {
                Some(LongNoteMode::Cn)
            } else if profile.has_defined_ln {
                Some(LongNoteMode::Ln)
            } else {
                None
            };
            let undefined = if profile.has_undefined_ln {
                Some(match policy {
                    LnScorePolicy::AutoLn => LongNoteMode::Ln,
                    LnScorePolicy::AutoCn => LongNoteMode::Cn,
                    LnScorePolicy::AutoHcn => LongNoteMode::Hcn,
                    LnScorePolicy::ForceLn | LnScorePolicy::ForceCn | LnScorePolicy::ForceHcn => {
                        unreachable!()
                    }
                })
            } else {
                None
            };
            max_long_note_mode(defined, undefined)
        }
    }
}

impl LnPolicySetting {
    pub const ORDER: [Self; 6] =
        [Self::AutoLn, Self::AutoCn, Self::AutoHcn, Self::ForceLn, Self::ForceCn, Self::ForceHcn];

    pub const fn is_force(self) -> bool {
        matches!(self, Self::ForceLn | Self::ForceCn | Self::ForceHcn)
    }

    pub const fn mode(self) -> LongNoteMode {
        match self {
            Self::AutoLn | Self::ForceLn => LongNoteMode::Ln,
            Self::AutoCn | Self::ForceCn => LongNoteMode::Cn,
            Self::AutoHcn | Self::ForceHcn => LongNoteMode::Hcn,
        }
    }

    pub const fn auto(mode: LongNoteMode) -> Self {
        match mode {
            LongNoteMode::Ln => Self::AutoLn,
            LongNoteMode::Cn => Self::AutoCn,
            LongNoteMode::Hcn => Self::AutoHcn,
        }
    }

    pub fn next(self) -> Self {
        cycle_ln_policy_setting(self, 1)
    }

    pub fn previous(self) -> Self {
        cycle_ln_policy_setting(self, -1)
    }

    pub const fn display_label(self) -> &'static str {
        match self {
            Self::AutoLn => "AUTO(LN)",
            Self::AutoCn => "AUTO(CN)",
            Self::AutoHcn => "AUTO(HCN)",
            Self::ForceLn => "FORCE(LN)",
            Self::ForceCn => "FORCE(CN)",
            Self::ForceHcn => "FORCE(HCN)",
        }
    }

    pub const fn as_ir_str(self) -> &'static str {
        match self {
            Self::AutoLn => "AutoLn",
            Self::AutoCn => "AutoCn",
            Self::AutoHcn => "AutoHcn",
            Self::ForceLn => "ForceLn",
            Self::ForceCn => "ForceCn",
            Self::ForceHcn => "ForceHcn",
        }
    }
}

fn cycle_ln_policy_setting(current: LnPolicySetting, direction: i32) -> LnPolicySetting {
    let index = LnPolicySetting::ORDER.iter().position(|value| *value == current).unwrap_or(0);
    let len = LnPolicySetting::ORDER.len();
    if direction >= 0 {
        LnPolicySetting::ORDER[(index + 1) % len]
    } else {
        LnPolicySetting::ORDER[(index + len - 1) % len]
    }
}

impl LnScorePolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AutoLn => "AutoLn",
            Self::AutoCn => "AutoCn",
            Self::AutoHcn => "AutoHcn",
            Self::ForceLn => "ForceLn",
            Self::ForceCn => "ForceCn",
            Self::ForceHcn => "ForceHcn",
        }
    }

    pub fn from_str_opt(value: &str) -> Option<Self> {
        match value {
            "AutoLn" => Some(Self::AutoLn),
            "AutoCn" => Some(Self::AutoCn),
            "AutoHcn" => Some(Self::AutoHcn),
            "ForceLn" => Some(Self::ForceLn),
            "ForceCn" => Some(Self::ForceCn),
            "ForceHcn" => Some(Self::ForceHcn),
            _ => None,
        }
    }

    pub const fn force(mode: LongNoteMode) -> Self {
        match mode {
            LongNoteMode::Ln => Self::ForceLn,
            LongNoteMode::Cn => Self::ForceCn,
            LongNoteMode::Hcn => Self::ForceHcn,
        }
    }

    pub const fn auto(mode: LongNoteMode) -> Self {
        match mode {
            LongNoteMode::Ln => Self::AutoLn,
            LongNoteMode::Cn => Self::AutoCn,
            LongNoteMode::Hcn => Self::AutoHcn,
        }
    }
}

pub fn score_ln_policy(setting: LnPolicySetting, profile: ChartLnProfile) -> LnScorePolicy {
    if !profile.has_any_ln() {
        return LnScorePolicy::ForceLn;
    }

    if setting.is_force() {
        return LnScorePolicy::force(setting.mode());
    }

    if profile.has_undefined_ln && !profile.has_any_defined_ln() {
        return LnScorePolicy::force(setting.mode());
    }

    if !profile.has_undefined_ln {
        if let Some(mode) = profile.single_defined_mode() {
            return LnScorePolicy::force(mode);
        }
        return LnScorePolicy::AutoLn;
    }

    LnScorePolicy::auto(setting.mode())
}

/// Resolve the score policy used by a course chart.
///
/// BMZ's FORCE settings have the highest priority and convert every long note,
/// ignoring the course constraint. AUTO settings preserve explicitly typed
/// LN/CN/HCN notes; an explicit course constraint only replaces the fallback
/// used by undefined long notes, matching beatoraja's course behavior.
pub fn course_score_ln_policy(
    setting: LnPolicySetting,
    course_fallback: Option<LongNoteMode>,
    profile: ChartLnProfile,
) -> LnScorePolicy {
    if setting.is_force() {
        return score_ln_policy(setting, profile);
    }

    let fallback = course_fallback.unwrap_or_else(|| setting.mode());
    score_ln_policy(LnPolicySetting::auto(fallback), profile)
}

pub fn score_ln_policy_for_chart(setting: LnPolicySetting, chart: &PlayableChart) -> LnScorePolicy {
    score_ln_policy(setting, ChartLnProfile::from_chart(chart))
}

pub fn apply_ln_policy_to_chart(setting: LnPolicySetting, chart: &mut PlayableChart) {
    let policy = score_ln_policy(setting, ChartLnProfile::from_chart(chart));
    apply_score_ln_policy_to_chart(policy, chart);
}

pub fn apply_score_ln_policy_to_chart(policy: LnScorePolicy, chart: &mut PlayableChart) {
    let fallback_mode = match policy {
        LnScorePolicy::AutoLn | LnScorePolicy::ForceLn => LongNoteMode::Ln,
        LnScorePolicy::AutoCn | LnScorePolicy::ForceCn => LongNoteMode::Cn,
        LnScorePolicy::AutoHcn | LnScorePolicy::ForceHcn => LongNoteMode::Hcn,
    };
    chart.metadata.long_note_mode = fallback_mode;
    if matches!(policy, LnScorePolicy::ForceLn | LnScorePolicy::ForceCn | LnScorePolicy::ForceHcn) {
        for pair in &mut chart.long_notes {
            pair.mode = Some(fallback_mode);
        }
    }
}

pub fn effective_ln_mode(setting: LnPolicySetting, profile: ChartLnProfile) -> LongNoteMode {
    match score_ln_policy(setting, profile) {
        LnScorePolicy::AutoLn | LnScorePolicy::ForceLn => LongNoteMode::Ln,
        LnScorePolicy::AutoCn | LnScorePolicy::ForceCn => LongNoteMode::Cn,
        LnScorePolicy::AutoHcn | LnScorePolicy::ForceHcn => LongNoteMode::Hcn,
    }
}

/// Score-target note count under `setting`, matching judge `affects_score` rules.
///
/// Base count is Tap + LongStart (`chart.total_notes`). Each long pair whose
/// effective mode is CN/HCN adds one more (the end note also scores). LN pairs
/// score once at the end, so they do not add beyond the base LongStart.
pub fn expected_scored_note_count(chart: &PlayableChart, setting: LnPolicySetting) -> u32 {
    ChartLnCounts::from_chart(chart).scored_total_notes_for_setting(chart.total_notes, setting)
}

/// Expected score-target note count for an already-resolved [`LnScorePolicy`].
///
/// `Force*` forces every long pair to that mode. `Auto*` keeps defined pair modes
/// and applies the policy fallback to undefined pairs.
pub fn expected_scored_note_count_for_policy(chart: &PlayableChart, policy: LnScorePolicy) -> u32 {
    ChartLnCounts::from_chart(chart).scored_total_notes(chart.total_notes, policy)
}

#[cfg(test)]
mod tests {
    use bmz_chart::model::{ChartMetadata, LongNotePair, LongNoteStyle};
    use bmz_core::chart::ChartIdentity;
    use bmz_core::ids::NoteId;
    use bmz_core::lane::Lane;
    use bmz_core::time::{ChartTick, TimeUs};

    use super::*;

    const NONE: ChartLnProfile = ChartLnProfile {
        has_undefined_ln: false,
        has_defined_ln: false,
        has_defined_cn: false,
        has_defined_hcn: false,
    };
    const UNDEFINED_ONLY: ChartLnProfile = ChartLnProfile { has_undefined_ln: true, ..NONE };
    const DEFINED_LN_ONLY: ChartLnProfile = ChartLnProfile { has_defined_ln: true, ..NONE };
    const DEFINED_CN_ONLY: ChartLnProfile = ChartLnProfile { has_defined_cn: true, ..NONE };
    const DEFINED_HCN_ONLY: ChartLnProfile = ChartLnProfile { has_defined_hcn: true, ..NONE };
    const DEFINED_MIXED: ChartLnProfile =
        ChartLnProfile { has_defined_ln: true, has_defined_cn: true, ..NONE };
    const UNDEFINED_AND_DEFINED: ChartLnProfile =
        ChartLnProfile { has_undefined_ln: true, has_defined_cn: true, ..NONE };

    #[test]
    fn policy_setting_ir_strings_use_score_policy_casing() {
        assert_eq!(LnPolicySetting::AutoLn.as_ir_str(), "AutoLn");
        assert_eq!(LnPolicySetting::AutoCn.as_ir_str(), "AutoCn");
        assert_eq!(LnPolicySetting::AutoHcn.as_ir_str(), "AutoHcn");
        assert_eq!(LnPolicySetting::ForceLn.as_ir_str(), "ForceLn");
        assert_eq!(LnPolicySetting::ForceCn.as_ir_str(), "ForceCn");
        assert_eq!(LnPolicySetting::ForceHcn.as_ir_str(), "ForceHcn");
    }

    #[test]
    fn score_policy_canonicalizes_no_ln() {
        for setting in [
            LnPolicySetting::AutoLn,
            LnPolicySetting::AutoCn,
            LnPolicySetting::AutoHcn,
            LnPolicySetting::ForceLn,
            LnPolicySetting::ForceCn,
            LnPolicySetting::ForceHcn,
        ] {
            assert_eq!(score_ln_policy(setting, NONE), LnScorePolicy::ForceLn);
        }
    }

    #[test]
    fn score_policy_collapses_undefined_only_auto_to_force() {
        assert_eq!(
            score_ln_policy(LnPolicySetting::AutoLn, UNDEFINED_ONLY),
            LnScorePolicy::ForceLn
        );
        assert_eq!(
            score_ln_policy(LnPolicySetting::AutoCn, UNDEFINED_ONLY),
            LnScorePolicy::ForceCn
        );
        assert_eq!(
            score_ln_policy(LnPolicySetting::AutoHcn, UNDEFINED_ONLY),
            LnScorePolicy::ForceHcn
        );
    }

    #[test]
    fn score_policy_collapses_single_defined_mode_auto_to_force() {
        assert_eq!(
            score_ln_policy(LnPolicySetting::AutoCn, DEFINED_LN_ONLY),
            LnScorePolicy::ForceLn
        );
        assert_eq!(
            score_ln_policy(LnPolicySetting::AutoLn, DEFINED_CN_ONLY),
            LnScorePolicy::ForceCn
        );
        assert_eq!(
            score_ln_policy(LnPolicySetting::AutoLn, DEFINED_HCN_ONLY),
            LnScorePolicy::ForceHcn
        );
    }

    #[test]
    fn score_policy_keeps_auto_for_mixed_cases() {
        assert_eq!(score_ln_policy(LnPolicySetting::AutoCn, DEFINED_MIXED), LnScorePolicy::AutoLn);
        assert_eq!(
            score_ln_policy(LnPolicySetting::AutoCn, UNDEFINED_AND_DEFINED),
            LnScorePolicy::AutoCn
        );
        assert_eq!(
            score_ln_policy(LnPolicySetting::AutoHcn, UNDEFINED_AND_DEFINED),
            LnScorePolicy::AutoHcn
        );
    }

    #[test]
    fn force_setting_always_forces_when_chart_has_ln() {
        assert_eq!(
            score_ln_policy(LnPolicySetting::ForceLn, DEFINED_MIXED),
            LnScorePolicy::ForceLn
        );
        assert_eq!(
            score_ln_policy(LnPolicySetting::ForceCn, UNDEFINED_AND_DEFINED),
            LnScorePolicy::ForceCn
        );
        assert_eq!(
            score_ln_policy(LnPolicySetting::ForceHcn, DEFINED_CN_ONLY),
            LnScorePolicy::ForceHcn
        );
    }

    #[test]
    fn course_constraint_replaces_only_auto_fallback() {
        assert_eq!(
            course_score_ln_policy(
                LnPolicySetting::AutoLn,
                Some(LongNoteMode::Cn),
                UNDEFINED_AND_DEFINED,
            ),
            LnScorePolicy::AutoCn
        );
        assert_eq!(
            course_score_ln_policy(
                LnPolicySetting::AutoLn,
                Some(LongNoteMode::Hcn),
                DEFINED_CN_ONLY,
            ),
            LnScorePolicy::ForceCn
        );
    }

    #[test]
    fn force_setting_ignores_course_constraint() {
        assert_eq!(
            course_score_ln_policy(
                LnPolicySetting::ForceLn,
                Some(LongNoteMode::Hcn),
                UNDEFINED_AND_DEFINED,
            ),
            LnScorePolicy::ForceLn
        );

        let mut chart = chart_with_long_modes(&[None, Some(LongNoteMode::Hcn)]);
        let policy = course_score_ln_policy(
            LnPolicySetting::ForceLn,
            Some(LongNoteMode::Cn),
            ChartLnProfile::from_chart(&chart),
        );
        apply_score_ln_policy_to_chart(policy, &mut chart);
        assert!(chart.long_notes.iter().all(|pair| pair.mode == Some(LongNoteMode::Ln)));
    }

    #[test]
    fn auto_course_fallback_preserves_defined_modes() {
        let mut chart = chart_with_long_modes(&[None, Some(LongNoteMode::Hcn)]);
        let source_profile = ChartLnProfile::from_chart(&chart);
        let policy =
            course_score_ln_policy(LnPolicySetting::AutoLn, Some(LongNoteMode::Cn), source_profile);

        apply_score_ln_policy_to_chart(policy, &mut chart);

        assert_eq!(policy, LnScorePolicy::AutoCn);
        assert_eq!(chart.metadata.long_note_mode, LongNoteMode::Cn);
        assert_eq!(chart.long_notes[0].mode, None);
        assert_eq!(chart.long_notes[1].mode, Some(LongNoteMode::Hcn));
        assert_eq!(played_ln_mode(source_profile, policy), Some(LongNoteMode::Hcn));
    }

    #[test]
    fn max_long_note_mode_uses_canonical_priority() {
        assert_eq!(max_long_note_mode(None, None), None);
        assert_eq!(
            max_long_note_mode(Some(LongNoteMode::Ln), Some(LongNoteMode::Cn)),
            Some(LongNoteMode::Cn)
        );
        assert_eq!(
            max_long_note_mode(Some(LongNoteMode::Hcn), Some(LongNoteMode::Cn)),
            Some(LongNoteMode::Hcn)
        );
    }

    #[test]
    fn source_ln_mode_uses_highest_defined_mode_and_maps_undefined_to_ln() {
        assert_eq!(source_ln_mode(NONE), None);
        assert_eq!(source_ln_mode(UNDEFINED_ONLY), Some(LongNoteMode::Ln));
        assert_eq!(source_ln_mode(DEFINED_LN_ONLY), Some(LongNoteMode::Ln));
        assert_eq!(source_ln_mode(DEFINED_MIXED), Some(LongNoteMode::Cn));
        assert_eq!(
            source_ln_mode(ChartLnProfile { has_defined_hcn: true, ..DEFINED_MIXED }),
            Some(LongNoteMode::Hcn)
        );
    }

    #[test]
    fn played_ln_mode_distinguishes_auto_fallback_from_force_conversion() {
        assert_eq!(played_ln_mode(NONE, LnScorePolicy::ForceHcn), None);
        assert_eq!(played_ln_mode(DEFINED_CN_ONLY, LnScorePolicy::ForceLn), Some(LongNoteMode::Ln));
        assert_eq!(played_ln_mode(DEFINED_MIXED, LnScorePolicy::AutoLn), Some(LongNoteMode::Cn));
        assert_eq!(
            played_ln_mode(UNDEFINED_AND_DEFINED, LnScorePolicy::AutoHcn),
            Some(LongNoteMode::Hcn)
        );
        assert_eq!(
            played_ln_mode(
                ChartLnProfile { has_defined_hcn: true, ..DEFINED_MIXED },
                LnScorePolicy::AutoLn,
            ),
            Some(LongNoteMode::Hcn)
        );
    }

    #[test]
    fn expected_scored_notes_force_cn_adds_end_per_long_pair() {
        let chart = chart_with_long_modes(&[None, None]);
        // base total_notes = 2 (LongStarts); ForceCn scores start+end => 4
        assert_eq!(expected_scored_note_count(&chart, LnPolicySetting::ForceCn), 4);
        assert_eq!(expected_scored_note_count_for_policy(&chart, LnScorePolicy::ForceCn), 4);
        assert_eq!(expected_scored_note_count(&chart, LnPolicySetting::ForceLn), 2);
    }

    #[test]
    fn expected_scored_notes_auto_keeps_defined_and_applies_fallback() {
        let chart = chart_with_long_modes(&[None, Some(LongNoteMode::Ln)]);
        // AutoCn -> AutoCn policy: undefined->CN (+1), defined LN (+0) => base 2 + 1
        assert_eq!(expected_scored_note_count(&chart, LnPolicySetting::AutoCn), 3);
        // AutoLn on defined CN only collapses to ForceCn => both ends score
        let defined_cn = chart_with_long_modes(&[Some(LongNoteMode::Cn)]);
        assert_eq!(expected_scored_note_count(&defined_cn, LnPolicySetting::AutoLn), 2);
    }

    #[test]
    fn chart_ln_counts_classify_pairs_and_compute_policy_totals() {
        let chart = chart_with_long_modes(&[
            None,
            Some(LongNoteMode::Ln),
            Some(LongNoteMode::Cn),
            Some(LongNoteMode::Hcn),
        ]);
        let counts = ChartLnCounts::from_chart(&chart);

        assert_eq!(
            counts,
            ChartLnCounts {
                undefined_ln_pairs: 1,
                defined_ln_pairs: 1,
                defined_cn_pairs: 1,
                defined_hcn_pairs: 1,
            }
        );
        assert_eq!(counts.canonical_total_notes(chart.total_notes), 6);
        assert_eq!(counts.scored_total_notes(chart.total_notes, LnScorePolicy::ForceLn), 4);
        assert_eq!(counts.scored_total_notes(chart.total_notes, LnScorePolicy::ForceCn), 8);
        assert_eq!(counts.scored_total_notes(chart.total_notes, LnScorePolicy::AutoLn), 6);
        assert_eq!(counts.scored_total_notes(chart.total_notes, LnScorePolicy::AutoCn), 7);
        assert_eq!(
            counts.scored_total_notes_for_setting(chart.total_notes, LnPolicySetting::AutoHcn),
            7
        );
    }

    #[test]
    fn auto_policy_keeps_defined_modes_and_sets_undefined_fallback() {
        let mut chart = chart_with_long_modes(&[None, Some(LongNoteMode::Hcn)]);

        apply_ln_policy_to_chart(LnPolicySetting::AutoCn, &mut chart);

        assert_eq!(chart.metadata.long_note_mode, LongNoteMode::Cn);
        assert_eq!(chart.long_notes[0].mode, None);
        assert_eq!(chart.long_notes[1].mode, Some(LongNoteMode::Hcn));
    }

    #[test]
    fn force_policy_overwrites_defined_and_undefined_modes() {
        let mut chart = chart_with_long_modes(&[None, Some(LongNoteMode::Ln)]);

        apply_ln_policy_to_chart(LnPolicySetting::ForceHcn, &mut chart);

        assert_eq!(chart.metadata.long_note_mode, LongNoteMode::Hcn);
        assert!(chart.long_notes.iter().all(|pair| pair.mode == Some(LongNoteMode::Hcn)));
    }

    fn chart_with_long_modes(modes: &[Option<LongNoteMode>]) -> PlayableChart {
        PlayableChart {
            identity: ChartIdentity { file_md5: [0; 16], file_sha256: [0; 32] },
            metadata: ChartMetadata::default(),
            lane_notes: std::array::from_fn(|_| Vec::new()),
            long_notes: modes
                .iter()
                .enumerate()
                .map(|(index, mode)| LongNotePair {
                    lane: Lane::Key1,
                    style: LongNoteStyle::ChannelPair,
                    mode: *mode,
                    start_note_id: NoteId((index * 2 + 1) as u32),
                    end_note_id: NoteId((index * 2 + 2) as u32),
                    start_tick: ChartTick(0),
                    end_tick: ChartTick(192),
                    start_time: TimeUs(0),
                    end_time: TimeUs(1_000_000),
                    sound: None,
                })
                .collect(),
            bgm_events: Vec::new(),
            bga_events: Vec::new(),
            timing_events: Vec::new(),
            scroll_events: Vec::new(),
            speed_events: Vec::new(),
            judge_rank_events: Vec::new(),
            bgm_volume_events: Vec::new(),
            key_volume_events: Vec::new(),
            text_events: Vec::new(),
            bga_opacity_events: Vec::new(),
            bga_argb_events: Vec::new(),
            swbga_definitions: Vec::new(),
            bga_keybound_events: Vec::new(),
            bga_asset_by_bmp_key: std::collections::HashMap::new(),
            bar_lines: Vec::new(),
            sounds: Vec::new(),
            bga_assets: Vec::new(),
            total_notes: modes.len() as u32,
            end_time: TimeUs(1_000_000),
        }
    }
}
