//! Practice mode configuration (beatoraja `PracticeProperty` subset).

use std::path::PathBuf;

use anyhow::{Context, Result};
use bmz_chart::model::{JudgeRankKind, JudgeRankSpec, PlayableChart};
use bmz_chart::practice::apply_practice_section;
use bmz_core::clear::GaugeType;
use bmz_core::time::TimeUs;
use bmz_gameplay::gauge::{GaugeProperty, GaugeState};
use bmz_gameplay::judge::window::judge_rank_spec_to_percent_optional_for_keymode_and_rule_mode;
use bmz_gameplay::rule::RuleMode;
use bmz_render::snapshot::ResultGraphSnapshot;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::config::profile_config::GaugeTypeConfig;
use crate::paths::ProfilePaths;
use crate::select_options::ArrangeOption;

const PRACTICE_PROPERTY_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PracticeGaugeType {
    AssistEasy,
    Easy,
    #[default]
    Normal,
    Hard,
    ExHard,
    Hazard,
    Class,
    ExClass,
    ExHardClass,
    /// Compatibility with a practice file written before full gauge support.
    AutoShift,
}

impl PracticeGaugeType {
    pub const VALUES: [Self; 9] = [
        Self::AssistEasy,
        Self::Easy,
        Self::Normal,
        Self::Hard,
        Self::ExHard,
        Self::Hazard,
        Self::Class,
        Self::ExClass,
        Self::ExHardClass,
    ];

    pub const fn gauge_type(self) -> GaugeType {
        match self {
            Self::AssistEasy => GaugeType::AssistEasy,
            Self::Easy => GaugeType::Easy,
            Self::Normal => GaugeType::Normal,
            Self::Hard => GaugeType::Hard,
            Self::ExHard => GaugeType::ExHard,
            Self::Hazard => GaugeType::Hazard,
            Self::Class => GaugeType::Class,
            Self::ExClass => GaugeType::ExClass,
            Self::ExHardClass => GaugeType::ExHardClass,
            Self::AutoShift => GaugeType::ExHard,
        }
    }

    pub const fn scales_section_total(self) -> bool {
        matches!(self, Self::AssistEasy | Self::Easy | Self::Normal)
    }
}

impl From<GaugeTypeConfig> for PracticeGaugeType {
    fn from(value: GaugeTypeConfig) -> Self {
        match value {
            GaugeTypeConfig::AssistEasy => Self::AssistEasy,
            GaugeTypeConfig::Easy => Self::Easy,
            GaugeTypeConfig::Normal => Self::Normal,
            GaugeTypeConfig::Hard => Self::Hard,
            GaugeTypeConfig::ExHard => Self::ExHard,
            GaugeTypeConfig::Hazard => Self::Hazard,
            GaugeTypeConfig::AutoShift => Self::AutoShift,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticeGraphType {
    #[default]
    NoteType,
    Judge,
    EarlyLate,
}

/// Persisted / editable practice settings for one chart (SHA-256).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PracticeProperty {
    #[serde(default)]
    pub format_version: u32,
    pub start_time_ms: u32,
    pub end_time_ms: u32,
    pub gauge: PracticeGaugeType,
    #[serde(default)]
    pub gauge_category: Option<GaugeProperty>,
    pub start_gauge: u32,
    pub judgerank: i32,
    pub arrange: ArrangeOption,
    #[serde(default)]
    pub arrange_2p: ArrangeOption,
    #[serde(default)]
    pub dp_flip: bool,
    pub total: Option<f64>,
    #[serde(default = "default_playback_rate_percent")]
    pub playback_rate_percent: u16,
    #[serde(default)]
    pub graph_type: PracticeGraphType,
}

impl Default for PracticeProperty {
    fn default() -> Self {
        Self {
            format_version: PRACTICE_PROPERTY_FORMAT_VERSION,
            start_time_ms: 0,
            end_time_ms: 10_000,
            gauge: PracticeGaugeType::Normal,
            gauge_category: None,
            start_gauge: 20,
            judgerank: 100,
            arrange: ArrangeOption::Normal,
            arrange_2p: ArrangeOption::Normal,
            dp_flip: false,
            total: None,
            playback_rate_percent: 100,
            graph_type: PracticeGraphType::NoteType,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PracticePhase {
    /// Settings overlay; chart is preloaded but not playing.
    Config,
    /// Active section play.
    Playing,
}

#[derive(Debug, Clone)]
pub struct PracticeSession {
    pub chart_id: i64,
    pub chart_title: String,
    pub chart_sha256: [u8; 32],
    pub property: PracticeProperty,
    pub phase: PracticePhase,
    pub max_end_time_ms: u32,
    pub last_graph: Arc<ResultGraphSnapshot>,
    /// Absolute chart time represented by graph bucket zero.
    pub graph_start_time_ms: u32,
    pub is_double: bool,
    pub cursor: usize,
    /// `last_play_snapshot` に反映済みの設定中プレビュー時刻。
    pub preview_time_ms: Option<u32>,
}

/// CLI-only overrides applied when entering practice from the command line.
#[derive(Debug, Clone, Default)]
pub struct PracticeCliOverrides {
    pub start_time_ms: Option<u32>,
    pub end_time_ms: Option<u32>,
}

pub fn practice_property_path(profile_paths: &ProfilePaths, chart_sha256: &[u8; 32]) -> PathBuf {
    profile_paths.root_dir.join("practice").join(format!("{}.json", sha256_hex(chart_sha256)))
}

pub fn load_practice_property(
    profile_paths: &ProfilePaths,
    chart_sha256: &[u8; 32],
    chart: &PlayableChart,
    profile_gauge: GaugeTypeConfig,
    rule_mode: RuleMode,
    cli: &PracticeCliOverrides,
) -> Result<PracticeProperty> {
    let path = practice_property_path(profile_paths, chart_sha256);
    let loaded_from_file = path.is_file();
    let mut property = if loaded_from_file {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read practice config: {}", path.display()))?;
        serde_json::from_str(&text)
            .with_context(|| format!("parse practice config: {}", path.display()))?
    } else {
        PracticeProperty::default()
    };

    if !loaded_from_file {
        property.end_time_ms = default_end_time_ms(chart);
        property.judgerank = practice_judgerank_percent(chart, rule_mode);
        if profile_gauge != GaugeTypeConfig::AutoShift {
            property.gauge = profile_gauge.into();
        }
    } else {
        migrate_legacy_practice_property(&mut property, chart, rule_mode);
    }
    property
        .gauge_category
        .get_or_insert_with(|| GaugeProperty::from_keymode(chart.metadata.key_mode));
    if property.total.is_none() {
        property.total = chart.metadata.total;
    }

    if let Some(start) = cli.start_time_ms {
        property.start_time_ms = start;
    }
    if let Some(end) = cli.end_time_ms {
        property.end_time_ms = end;
    }
    clamp_practice_property(&mut property, chart);

    Ok(property)
}

fn practice_judgerank_percent(chart: &PlayableChart, rule_mode: RuleMode) -> i32 {
    judge_rank_spec_to_percent_optional_for_keymode_and_rule_mode(
        chart.metadata.judge_rank_spec,
        chart.metadata.key_mode,
        rule_mode,
    )
}

fn migrate_legacy_practice_property(
    property: &mut PracticeProperty,
    chart: &PlayableChart,
    rule_mode: RuleMode,
) {
    if property.format_version >= PRACTICE_PROPERTY_FORMAT_VERSION {
        return;
    }

    // BMZ の旧形式は初回値に BMS の生 #RANK / #DEFEXRANK を保存し、
    // 次回開始時に BMSON の倍率 (%) として再解釈していた。譜面由来の
    // 旧初期値と一致する場合だけ移行し、ユーザーが変更した値は保持する。
    if let Some(spec) = chart.metadata.judge_rank_spec
        && matches!(spec.kind, JudgeRankKind::BmsRank | JudgeRankKind::DefExRank)
        && property.judgerank == spec.value.clamp(1, 400)
    {
        property.judgerank = practice_judgerank_percent(chart, rule_mode);
    }
    property.format_version = PRACTICE_PROPERTY_FORMAT_VERSION;
}

pub fn save_practice_property(
    profile_paths: &ProfilePaths,
    chart_sha256: &[u8; 32],
    property: &PracticeProperty,
) -> Result<()> {
    let path = practice_property_path(profile_paths, chart_sha256);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create practice dir: {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(property).context("serialize practice property")?;
    std::fs::write(&path, text)
        .with_context(|| format!("write practice config: {}", path.display()))
}

pub fn apply_practice_property(chart: &mut PlayableChart, property: &PracticeProperty) {
    let start_us = TimeUs(i64::from(property.start_time_ms) * 1000);
    let end_ms = property.end_time_ms.max(property.start_time_ms.saturating_add(1000));
    let end_us = TimeUs(i64::from(end_ms) * 1000);
    let audio_start_us = TimeUs(start_us.0.saturating_sub(1_000_000).max(0));
    if let Some(total) = property.total {
        chart.metadata.total = Some(total);
    }
    apply_practice_section(chart, start_us, end_us);
    // beatoraja starts background keysounds at `starttime - 1s` and skips
    // earlier events. Retaining them would make the scheduler catch up every
    // sound before the practice range when entering midway through a chart.
    chart.bgm_events.retain(|event| event.time >= audio_start_us);
    chart.metadata.judge_rank = Some(property.judgerank);
    chart.metadata.judge_rank_spec =
        Some(JudgeRankSpec { value: property.judgerank, kind: JudgeRankKind::BmsonJudgeRank });
    if !property.gauge.scales_section_total()
        && let Some(total) = property.total
    {
        chart.metadata.total = Some(total);
    }
}

pub fn apply_practice_start_gauge(gauge: &mut GaugeState, start_gauge: u32) {
    let value = start_gauge.clamp(1, 100) as f32;
    gauge.set_initial_value(value);
}

pub fn practice_chart_zero_time(property: &PracticeProperty, skin_playstart_us: TimeUs) -> TimeUs {
    let lead_us = i64::from(property.start_time_ms.saturating_sub(1000)) * 1000;
    // `skin_playstart_us` is the normal negative READY offset. The audio clock
    // advances at the selected rate, so compensate the fixed wall-clock READY
    // duration and arrive at `lead_us` exactly when the play timer starts.
    let ready_wall_us = skin_playstart_us.0.saturating_neg().max(0);
    let ready_chart_us = ((i128::from(ready_wall_us) * i128::from(property.playback_rate_percent))
        / 100)
        .min(i128::from(i64::MAX)) as i64;
    TimeUs(lead_us.saturating_sub(ready_chart_us))
}

pub fn clamp_practice_property(property: &mut PracticeProperty, chart: &PlayableChart) {
    let max_end = default_end_time_ms(chart);
    property.start_time_ms = property.start_time_ms.min(max_end.saturating_sub(3000));
    property.end_time_ms =
        property.end_time_ms.clamp(property.start_time_ms.saturating_add(1000), max_end);
    property.judgerank = property.judgerank.clamp(1, 400);
    property.start_gauge = property.start_gauge.clamp(1, 100);
    property.playback_rate_percent =
        bmz_audio::clock::clamp_playback_rate_percent(property.playback_rate_percent);
    if let Some(total) = property.total.as_mut() {
        *total = total.clamp(10.0, 5000.0);
    }
}

pub fn practice_field_count(is_double: bool) -> usize {
    if is_double { 12 } else { 10 }
}

pub fn move_practice_cursor(cursor: &mut usize, is_double: bool, forward: bool) {
    let count = practice_field_count(is_double);
    *cursor = (*cursor + if forward { 1 } else { count - 1 }) % count;
}

pub fn adjust_practice_selected_field(
    property: &mut PracticeProperty,
    cursor: usize,
    is_double: bool,
    increment: bool,
    max_end_time_ms: u32,
) {
    let direction = if increment { 1_i32 } else { -1 };
    match cursor {
        0 => {
            adjust_u32(
                &mut property.start_time_ms,
                direction * 100,
                0,
                max_end_time_ms.saturating_sub(3000),
            );
            property.end_time_ms =
                property.end_time_ms.max(property.start_time_ms.saturating_add(1000));
        }
        1 => adjust_u32(
            &mut property.end_time_ms,
            direction * 100,
            property.start_time_ms.saturating_add(1000),
            max_end_time_ms,
        ),
        2 => cycle_gauge(&mut property.gauge, increment),
        3 => {
            cycle_gauge_category(&mut property.gauge_category, increment);
            property.start_gauge = practice_gauge_initial_value(
                property.gauge,
                property.gauge_category.unwrap_or_default(),
            );
        }
        4 => adjust_u32(&mut property.start_gauge, direction, 1, 100),
        5 => property.judgerank = (property.judgerank + direction).clamp(1, 400),
        6 => {
            if let Some(total) = property.total.as_mut() {
                *total = (*total + f64::from(direction) * 5.0).clamp(10.0, 5000.0);
            }
        }
        7 => {
            let value = i32::from(property.playback_rate_percent) + direction * 5;
            property.playback_rate_percent = value.clamp(50, 200) as u16;
        }
        8 => cycle_graph_type(&mut property.graph_type, increment),
        9 => cycle_arrange(&mut property.arrange, increment),
        10 if is_double => cycle_arrange(&mut property.arrange_2p, increment),
        11 if is_double => property.dp_flip = !property.dp_flip,
        _ => {}
    }
}

fn adjust_u32(value: &mut u32, delta: i32, min: u32, max: u32) {
    *value = (i64::from(*value) + i64::from(delta)).clamp(i64::from(min), i64::from(max)) as u32;
}

fn cycle_gauge(value: &mut PracticeGaugeType, increment: bool) {
    let index = PracticeGaugeType::VALUES.iter().position(|item| item == value).unwrap_or(0);
    let len = PracticeGaugeType::VALUES.len();
    *value = PracticeGaugeType::VALUES[(index + if increment { 1 } else { len - 1 }) % len];
}

fn cycle_gauge_category(value: &mut Option<GaugeProperty>, increment: bool) {
    let values =
        [GaugeProperty::FiveKeys, GaugeProperty::SevenKeys, GaugeProperty::Pms, GaugeProperty::Lr2];
    let current = value.unwrap_or(GaugeProperty::SevenKeys);
    let index = values.iter().position(|item| *item == current).unwrap_or(0);
    *value = Some(values[(index + if increment { 1 } else { values.len() - 1 }) % values.len()]);
}

pub fn practice_gauge_initial_value(gauge: PracticeGaugeType, property: GaugeProperty) -> u32 {
    bmz_gameplay::gauge::gauge_definitions_for(property)
        .into_iter()
        .find(|definition| definition.gauge_type == gauge.gauge_type())
        .map(|definition| definition.init.round().clamp(1.0, 100.0) as u32)
        .unwrap_or(20)
}

fn cycle_graph_type(value: &mut PracticeGraphType, increment: bool) {
    let values =
        [PracticeGraphType::NoteType, PracticeGraphType::Judge, PracticeGraphType::EarlyLate];
    let index = values.iter().position(|item| item == value).unwrap_or(0);
    *value = values[(index + if increment { 1 } else { values.len() - 1 }) % values.len()];
}

fn cycle_arrange(value: &mut ArrangeOption, increment: bool) {
    *value = if increment { value.cycle() } else { value.cycle_prev() };
}

const fn default_playback_rate_percent() -> u16 {
    100
}

pub fn default_end_time_ms(chart: &PlayableChart) -> u32 {
    let end_ms = (chart.end_time.0 / 1000).max(0);
    u32::try_from(end_ms).unwrap_or(u32::MAX).saturating_add(1000)
}

fn sha256_hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use bmz_core::chart::ChartIdentity;

    use super::*;
    use bmz_chart::model::ChartMetadata;

    fn empty_chart(end_ms: i64) -> PlayableChart {
        PlayableChart {
            identity: ChartIdentity { file_md5: [0; 16], file_sha256: [1; 32] },
            metadata: ChartMetadata {
                judge_rank: Some(150),
                judge_rank_spec: Some(JudgeRankSpec {
                    value: 150,
                    kind: JudgeRankKind::BmsonJudgeRank,
                }),
                total: Some(250.0),
                ..Default::default()
            },
            lane_notes: std::array::from_fn(|_| Vec::new()),
            long_notes: Vec::new(),
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
            bga_asset_by_bmp_key: Default::default(),
            bar_lines: Vec::new(),
            sounds: Vec::new(),
            bga_assets: Vec::new(),
            total_notes: 0,
            end_time: TimeUs(end_ms * 1000),
        }
    }

    #[test]
    fn load_practice_property_uses_chart_defaults() {
        let root = std::env::temp_dir().join(format!(
            "bmz-practice-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let paths = ProfilePaths {
            root_dir: root.clone(),
            profile_toml: root.join("profile.toml"),
            collection_db: root.join("collection.db"),
            score_db: root.join("score.db"),
            network_db: root.join("network.db"),
            replay_dir: root.join("replay"),
        };
        let chart = empty_chart(120_000);
        let property = load_practice_property(
            &paths,
            &chart.identity.file_sha256,
            &chart,
            GaugeTypeConfig::Hard,
            RuleMode::Beatoraja,
            &PracticeCliOverrides { start_time_ms: Some(5000), end_time_ms: None },
        )
        .unwrap();
        assert_eq!(property.start_time_ms, 5000);
        assert_eq!(property.end_time_ms, 121_000);
        assert_eq!(property.judgerank, 150);
        assert_eq!(property.gauge, PracticeGaugeType::Hard);
        assert_eq!(property.total, Some(250.0));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn practice_judgerank_normalizes_source_rank_kinds() {
        let mut chart = empty_chart(120_000);
        for (key_mode, spec, expected) in [
            (
                bmz_core::lane::KeyMode::K7,
                JudgeRankSpec { value: 3, kind: JudgeRankKind::BmsRank },
                100,
            ),
            (
                bmz_core::lane::KeyMode::K7,
                JudgeRankSpec { value: 2, kind: JudgeRankKind::BmsRank },
                75,
            ),
            (
                bmz_core::lane::KeyMode::K9,
                JudgeRankSpec { value: 2, kind: JudgeRankKind::BmsRank },
                70,
            ),
            (
                bmz_core::lane::KeyMode::K7,
                JudgeRankSpec { value: 125, kind: JudgeRankKind::DefExRank },
                93,
            ),
            (
                bmz_core::lane::KeyMode::K7,
                JudgeRankSpec { value: 125, kind: JudgeRankKind::BmsonJudgeRank },
                125,
            ),
        ] {
            chart.metadata.key_mode = key_mode;
            chart.metadata.judge_rank_spec = Some(spec);
            assert_eq!(practice_judgerank_percent(&chart, RuleMode::Beatoraja), expected);
        }
    }

    #[test]
    fn legacy_practice_judgerank_migrates_only_unchanged_source_default() {
        let mut chart = empty_chart(120_000);
        chart.metadata.judge_rank_spec =
            Some(JudgeRankSpec { value: 3, kind: JudgeRankKind::BmsRank });

        let mut legacy_default =
            PracticeProperty { format_version: 0, judgerank: 3, ..Default::default() };
        migrate_legacy_practice_property(&mut legacy_default, &chart, RuleMode::Beatoraja);
        assert_eq!(legacy_default.judgerank, 100);
        assert_eq!(legacy_default.format_version, PRACTICE_PROPERTY_FORMAT_VERSION);

        let mut customized =
            PracticeProperty { format_version: 0, judgerank: 2, ..Default::default() };
        migrate_legacy_practice_property(&mut customized, &chart, RuleMode::Beatoraja);
        assert_eq!(customized.judgerank, 2);
        assert_eq!(customized.format_version, PRACTICE_PROPERTY_FORMAT_VERSION);
    }
}
