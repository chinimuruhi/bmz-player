//! bms-rs を使った BMS → [`IntermediateChart`] adapter。
//!
//! 内製 parser (`bms_adapter.rs`) を置き換えるのが目的。出力契約は据え置きで、
//! 下流 (`normalize.rs`) の入力としてそのまま流せる形に整える。
//!
//! 対応範囲:
//! - メタデータ: title / subtitle / artist / sub_artist / genre / play_level
//!   / difficulty / judge_rank / total / initial_bpm / stage_file / banner /
//!   back_bmp / preview_file / has_bga / key_mode
//! - 定義: WAV / BMP / BPM (#BPMxx) / STOP
//! - チャネル:
//!   - 01: BGM
//!   - 02: 小節長
//!   - 03/08: BPM 変更（インライン / ref）
//!   - 04/06/07/0A: BGA (Base/Poor/Overlay→Layer/Layer2)
//!   - 09: STOP
//!   - 1x/2x: Visible (P1/P2)
//!   - 3x/4x: Invisible (P1/P2)
//!   - 5x/6x: Long-channel (P1/P2)
//!   - Dx/Ex: Landmine (P1/P2)
//! - `#LNOBJ`: bms-rs には渡さず、BMZ 側で marker WAV を通常ノート列から LN ペアへ解決する。
//! - `.pms`: `KeyLayoutPms` / `KeyLayoutPmsBmeType` による 9K SP (18K は drop + warning)
//!
//! 未対応 (warning に流すか drop):
//! - JUDGE 変更イベント (#EXRANK / chA0)
//! - TEXT / OPTION / VIDEO / SEEK 等
//! - foot pedal / free zone
//! - PMS 18K (2P 側ノート)

use std::collections::BTreeMap;
use std::path::Path;

use bms_rs::bms::command::channel::mapper::{
    KeyLayoutBeat, KeyLayoutMapper, KeyLayoutPms, KeyLayoutPmsBmeType,
};
use bms_rs::bms::command::channel::{
    Channel, Key, NoteChannelId, NoteKind as BmsNoteKind, PlayerSide, read_channel,
};
use bms_rs::bms::command::time::ObjTime;
use bms_rs::bms::command::{JudgeLevel, LnMode, ObjId};
use bms_rs::bms::model::Bms;
use bms_rs::bms::model::obj::{
    BgaArgbObj, BgaKeyboundObj, BgaLayer, BgaObj, BgaOpacityObj, BgmVolumeObj, BpmChangeObj,
    JudgeObj, KeyVolumeObj, ScrollingFactorObj, SpeedObj, StopObj, TextObj, WavObj,
};
use bms_rs::bms::rng::JavaRandom;
use bms_rs::bms::{BmsOutput, BmsWarning, default_config_with_rng, parse_bms};
use bmz_core::chart::ChartIdentity;
use bmz_core::lane::{ChartKeyLayout, KeyMode, Lane, PmsKeyLayout};
use bmz_core::time::ChartTick;

use crate::hash::compute_chart_identity;
use crate::timing::TICKS_PER_MEASURE;

use super::BmsRandomSource;
use super::decode::decode_bms_text;
use super::error::{ImportError, ImportWarning};
use super::intermediate::{
    BmpDef, BpmDef, IntermediateBgaKind, IntermediateChart, IntermediateMetadata,
    IntermediateObject, IntermediateObjectKind, IntermediateResources, MeasureInfo, StopDef,
    WavDef,
};

use crate::model::{JudgeRankKind, JudgeRankSpec, LongNoteMode};

mod metadata;
mod objects;
mod random;
mod sparse;
mod timing;

use metadata::*;
use objects::*;
use random::*;
use sparse::*;
use timing::*;

pub(crate) const MAX_SUPPORTED_MEASURE: u32 = 100_000;
const SPARSE_BMS_MESSAGE_OBJECT_THRESHOLD: usize = 8_192;
const SPARSE_BMS_MARKER_HEADER: &str = "BMZSPARSE";

#[derive(Debug, Clone)]
struct SparseBmsMessage {
    id: usize,
    line_number: usize,
    measure: u64,
    channel: String,
    object_count: u64,
    objects: Vec<SparseBmsObject>,
}

#[derive(Debug, Clone)]
struct SparseBmsObject {
    index: u64,
    id: String,
}

#[derive(Debug, Clone)]
struct BgaMessage {
    line_number: usize,
    measure: u64,
    kind: IntermediateBgaKind,
    object_count: u64,
    objects: Vec<SparseBmsObject>,
}

pub fn import_bms_to_intermediate(
    source_path: &Path,
    random_seed: Option<u64>,
    warnings: &mut Vec<ImportWarning>,
) -> Result<IntermediateChart, ImportError> {
    import_bms_to_intermediate_with_random_source(
        source_path,
        &BmsRandomSource::Seed(random_seed),
        &mut Vec::new(),
        warnings,
    )
}

pub fn import_bms_to_intermediate_with_random_source(
    source_path: &Path,
    random_source: &BmsRandomSource,
    bms_random_choices: &mut Vec<i32>,
    warnings: &mut Vec<ImportWarning>,
) -> Result<IntermediateChart, ImportError> {
    import_with_layout::<KeyLayoutBeat>(
        source_path,
        ChartKeyLayout::beat(),
        random_source,
        bms_random_choices,
        warnings,
    )
}

pub fn import_pms_to_intermediate(
    source_path: &Path,
    random_seed: Option<u64>,
    warnings: &mut Vec<ImportWarning>,
) -> Result<IntermediateChart, ImportError> {
    import_pms_to_intermediate_with_random_source(
        source_path,
        &BmsRandomSource::Seed(random_seed),
        &mut Vec::new(),
        warnings,
    )
}

pub fn import_pms_to_intermediate_with_random_source(
    source_path: &Path,
    random_source: &BmsRandomSource,
    bms_random_choices: &mut Vec<i32>,
    warnings: &mut Vec<ImportWarning>,
) -> Result<IntermediateChart, ImportError> {
    let bytes = std::fs::read(source_path)
        .map_err(|source| ImportError::Io { path: source_path.to_path_buf(), source })?;
    let text = decode_bms_text(&bytes, warnings);
    let (variant, conflict) = detect_pms_variant(&text);
    if conflict {
        warnings.push(ImportWarning::ParserDiagnostic {
            code: "PmsLayoutConflict".to_string(),
            message: "PMS standard (2P upper) and BME-type (1P 16-19) channels both used; \
                      using standard layout"
                .to_string(),
        });
    }
    match variant {
        PmsKeyLayout::Standard => import_with_layout::<KeyLayoutPms>(
            source_path,
            ChartKeyLayout::pms(PmsKeyLayout::Standard),
            random_source,
            bms_random_choices,
            warnings,
        ),
        PmsKeyLayout::BmeType => import_with_layout::<KeyLayoutPmsBmeType>(
            source_path,
            ChartKeyLayout::pms(PmsKeyLayout::BmeType),
            random_source,
            bms_random_choices,
            warnings,
        ),
    }
}

fn import_with_layout<T: KeyLayoutMapper>(
    source_path: &Path,
    layout: ChartKeyLayout,
    random_source: &BmsRandomSource,
    bms_random_choices: &mut Vec<i32>,
    warnings: &mut Vec<ImportWarning>,
) -> Result<IntermediateChart, ImportError> {
    let bytes = std::fs::read(source_path)
        .map_err(|source| ImportError::Io { path: source_path.to_path_buf(), source })?;
    let identity = compute_chart_identity(&bytes);
    let raw_text = decode_bms_text(&bytes, warnings);
    let has_bms_random = source_text_has_bms_random(&raw_text);
    let layout_text = if layout == ChartKeyLayout::pms(PmsKeyLayout::Standard) {
        strip_pms_bme_upper_channels(&raw_text)
    } else {
        raw_text.clone()
    };
    let text =
        apply_beatoraja_random_control(&layout_text, random_source, bms_random_choices, warnings);
    let metadata_text = strip_empty_metadata_commands(&text);
    let lnobj_parse_text = strip_lnobj_commands(&metadata_text);
    let bga_messages = extract_bga_message_lines(&lnobj_parse_text);
    let (parse_text, sparse_messages) =
        extract_sparse_bms_message_lines(&lnobj_parse_text, warnings);

    let BmsOutput { bms, warnings: bms_warnings } = parse_bms::<T, _, _, _>(
        &parse_text,
        default_config_with_rng(JavaRandom::new(random_source_seed(random_source) as i64))
            .key_mapper::<T>(),
    );
    for w in bms_warnings {
        if let Some(w) = map_bms_warning(&w) {
            warnings.push(w);
        }
    }
    let mut bms = bms.map_err(|err| ImportError::Parse {
        path: source_path.to_path_buf(),
        message: format!("{err:?}"),
    })?;
    inject_sparse_bms_messages::<T>(&mut bms, &sparse_messages, warnings);
    let bga_objects = bga_messages_to_intermediate_objects(
        &bga_messages,
        bms_uses_base62_obj_ids(&bms),
        warnings,
    );
    // bms-rs stores BGA changes in a single map keyed only by time, so simultaneous
    // Base/Poor/Layer changes overwrite each other. Use the source-derived events instead.
    bms.bmp.bga_changes.clear();

    let mut intermediate = build_intermediate_from_bms_with_extra_bga_objects::<T>(
        &bms,
        layout,
        &bga_objects,
        warnings,
    )?;
    intermediate.lnobj_wav_key =
        extract_lnobj_wav_key(&text, bms_uses_base62_obj_ids(&bms), warnings);
    let bms_headers = extract_bms_headers_from_text(&raw_text);
    intermediate.metadata.has_bms_random = has_bms_random;
    intermediate.metadata.bms_headers = bms_headers.clone();
    apply_raw_judge_rank_headers(&mut intermediate, &bms_headers);
    intermediate.metadata.source_url = bms
        .metadata
        .url
        .clone()
        .filter(|url| !url.is_empty())
        .or_else(|| bms_headers.get("URL").cloned())
        .unwrap_or_default();
    intermediate.metadata.append_url = append_url_from_headers(&bms_headers);
    intermediate.identity = identity;
    Ok(intermediate)
}

pub(crate) fn build_intermediate_from_bms<T: KeyLayoutMapper>(
    bms: &Bms,
    layout: ChartKeyLayout,
    warnings: &mut Vec<ImportWarning>,
) -> Result<IntermediateChart, ImportError> {
    build_intermediate_from_bms_with_extra_bga_objects::<T>(bms, layout, &[], warnings)
}

fn build_intermediate_from_bms_with_extra_bga_objects<T: KeyLayoutMapper>(
    bms: &Bms,
    layout: ChartKeyLayout,
    extra_bga_objects: &[IntermediateObject],
    warnings: &mut Vec<ImportWarning>,
) -> Result<IntermediateChart, ImportError> {
    let metadata = build_metadata(bms);
    let mut resources = build_resources(bms);
    let mut objects = Vec::new();

    push_note_objects::<T>(bms, layout, &mut objects, warnings);
    push_bgm_objects::<T>(bms, &mut objects);
    push_bga_objects(bms, &mut objects);
    objects.extend_from_slice(extra_bga_objects);
    push_bpm_change_objects(bms, &mut objects);
    push_stop_objects(bms, &mut objects, &mut resources);
    push_scroll_objects(bms, &mut objects);
    push_speed_objects(bms, &mut objects);
    push_judge_rank_objects(bms, &mut objects);
    push_volume_objects(bms, &mut objects);
    push_text_objects(bms, &mut objects);
    push_bga_opacity_objects(bms, &mut objects);
    push_bga_argb_objects(bms, &mut objects);
    push_bga_keybound_objects(bms, &mut objects);

    let max_measure = compute_max_measure(bms, &objects)?;
    let measures = build_measures(max_measure, bms);

    let mut intermediate = IntermediateChart {
        identity: ChartIdentity { file_md5: [0; 16], file_sha256: [0; 32] },
        metadata,
        resources,
        measures,
        objects,
        lnobj_wav_key: None, // bms-rs 側で吸収済み
    };

    intermediate.metadata.has_bga = intermediate
        .objects
        .iter()
        .any(|object| matches!(object.kind, IntermediateObjectKind::Bga { .. }));

    let lane_key_mode = KeyMode::detect_from_lanes_with_layout(
        layout,
        intermediate.objects.iter().filter_map(|o| match o.kind {
            IntermediateObjectKind::VisibleNote { lane, .. }
            | IntermediateObjectKind::InvisibleNote { lane, .. }
            | IntermediateObjectKind::LongChannelNote { lane, .. }
            | IntermediateObjectKind::MineNote { lane, .. } => Some(lane),
            _ => None,
        }),
    );
    intermediate.metadata.key_mode =
        detect_key_mode_from_bms_headers(bms, layout).unwrap_or(lane_key_mode);
    normalize_qwilight_lanes(&mut intermediate.objects, intermediate.metadata.key_mode);

    Ok(intermediate)
}

fn normalize_qwilight_lanes(objects: &mut [IntermediateObject], key_mode: KeyMode) {
    if !matches!(key_mode, KeyMode::K4 | KeyMode::K6 | KeyMode::K8) {
        return;
    }

    for object in objects {
        let lane = match &mut object.kind {
            IntermediateObjectKind::VisibleNote { lane, .. }
            | IntermediateObjectKind::InvisibleNote { lane, .. }
            | IntermediateObjectKind::LongChannelNote { lane, .. }
            | IntermediateObjectKind::MineNote { lane, .. } => lane,
            _ => continue,
        };
        *lane = match key_mode {
            KeyMode::K4 => match *lane {
                Lane::Key4 => Lane::Key3,
                Lane::Key5 => Lane::Key4,
                lane => lane,
            },
            KeyMode::K6 => match *lane {
                Lane::Key5 => Lane::Key4,
                Lane::Key6 => Lane::Key5,
                Lane::Key7 => Lane::Key6,
                lane => lane,
            },
            KeyMode::K8 => match *lane {
                Lane::Scratch => Lane::Key1,
                Lane::Key1 => Lane::Key2,
                Lane::Key2 => Lane::Key3,
                Lane::Key3 => Lane::Key4,
                Lane::Key4 => Lane::Key5,
                Lane::Key5 => Lane::Key6,
                Lane::Key6 => Lane::Key7,
                Lane::Key7 => Lane::Key8,
                lane => lane,
            },
            _ => *lane,
        };
    }
}

/// Qwilight / BMSE 拡張ヘッダ (`#4K`, `#6K`, `#8K`) からキーモードを読む。
///
/// bms-rs はこれらを構造化しないため `repr.raw_command_lines` を走査する。
/// 複数行ある場合は後勝ち（EXPANSION FIELD の宣言を優先）。
pub(crate) fn detect_key_mode_from_bms_headers(
    bms: &Bms,
    layout: ChartKeyLayout,
) -> Option<KeyMode> {
    if layout.is_pms() {
        return None;
    }

    let mut mode = None;
    for line in &bms.repr.raw_command_lines {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("#4K") {
            mode = Some(KeyMode::K4);
        } else if trimmed.eq_ignore_ascii_case("#6K") {
            mode = Some(KeyMode::K6);
        } else if trimmed.eq_ignore_ascii_case("#8K") {
            mode = Some(KeyMode::K8);
        }
    }
    mode
}

/// `.pms` テキストから Standard / BME-type を判定する。
pub(crate) fn detect_pms_variant(source: &str) -> (PmsKeyLayout, bool) {
    let mut has_standard_upper = false;
    let mut has_bme_upper = false;

    for line in source.lines() {
        let Some(channel) = message_channel_bytes(line) else {
            continue;
        };
        if pms_standard_upper_channel(channel) {
            has_standard_upper = true;
        }
        if pms_bme_upper_channel(channel) {
            has_bme_upper = true;
        }
    }

    let variant = if has_standard_upper {
        PmsKeyLayout::Standard
    } else if has_bme_upper {
        PmsKeyLayout::BmeType
    } else {
        PmsKeyLayout::Standard
    };
    (variant, has_standard_upper && has_bme_upper)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use bmz_core::lane::KeyMode;

    use super::*;

    const PMS_HEADER: &str = "\
#TITLE PMS Test
#ARTIST Tester
#BPM 120
#WAV01 key.wav
";

    fn pms_note_lines_standard() -> String {
        let mut lines = String::from(PMS_HEADER);
        for (i, channel) in
            ["11", "12", "13", "14", "15", "22", "23", "24", "25"].into_iter().enumerate()
        {
            let measure = i + 1;
            lines.push_str(&format!("#{measure:03}{channel}:01\n"));
        }
        lines
    }

    fn pms_note_lines_bme() -> String {
        let mut lines = String::from(PMS_HEADER);
        for (i, channel) in
            ["11", "12", "13", "14", "15", "16", "17", "18", "19"].into_iter().enumerate()
        {
            let measure = i + 1;
            lines.push_str(&format!("#{measure:03}{channel}:01\n"));
        }
        lines
    }

    fn import_pms_text(text: &str) -> IntermediateChart {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.pms");
        std::fs::write(&path, text).unwrap();
        std::fs::write(dir.path().join("key.wav"), b"wav").unwrap();
        let mut warnings = Vec::new();
        import_pms_to_intermediate(&path, None, &mut warnings).unwrap()
    }

    fn note_lanes(chart: &IntermediateChart) -> Vec<Lane> {
        chart
            .objects
            .iter()
            .filter_map(|object| match object.kind {
                IntermediateObjectKind::VisibleNote { lane, .. } => Some(lane),
                _ => None,
            })
            .collect()
    }

    fn playable_lane_counts(chart: &IntermediateChart) -> [usize; bmz_core::lane::LANE_COUNT] {
        let mut counts = [0; bmz_core::lane::LANE_COUNT];
        for object in &chart.objects {
            let lane = match object.kind {
                IntermediateObjectKind::VisibleNote { lane, .. }
                | IntermediateObjectKind::InvisibleNote { lane, .. }
                | IntermediateObjectKind::LongChannelNote { lane, .. }
                | IntermediateObjectKind::MineNote { lane, .. } => lane,
                _ => continue,
            };
            counts[lane.index()] += 1;
        }
        counts
    }

    #[test]
    fn detect_pms_variant_standard_from_p2_upper_channels() {
        let (variant, conflict) = detect_pms_variant(&pms_note_lines_standard());
        assert_eq!(variant, PmsKeyLayout::Standard);
        assert!(!conflict);
    }

    #[test]
    fn detect_pms_variant_ignores_non_message_headers_with_colons() {
        let text = "\
#TITLE 赤 (原曲: 天衣無縫) [9K NORMAL]
#BPM 120
";
        let (variant, conflict) = detect_pms_variant(text);
        assert_eq!(variant, PmsKeyLayout::Standard);
        assert!(!conflict);
    }

    #[test]
    fn detect_pms_variant_bme_from_p1_upper_channels() {
        let (variant, conflict) = detect_pms_variant(&pms_note_lines_bme());
        assert_eq!(variant, PmsKeyLayout::BmeType);
        assert!(!conflict);
    }

    #[test]
    fn pms_standard_9k_maps_key1_through_key9() {
        let chart = import_pms_text(&pms_note_lines_standard());
        assert_eq!(chart.metadata.key_mode, KeyMode::K9);
        let lanes = note_lanes(&chart);
        assert_eq!(lanes.len(), 9);
        for (expected, actual) in [
            Lane::Key1,
            Lane::Key2,
            Lane::Key3,
            Lane::Key4,
            Lane::Key5,
            Lane::Key6,
            Lane::Key7,
            Lane::Key8,
            Lane::Key9,
        ]
        .into_iter()
        .zip(lanes)
        {
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn pms_standard_drops_conflicting_bme_upper_channels() {
        let mut text = pms_note_lines_standard();
        text.push_str("#01018:01\n");

        let chart = import_pms_text(&text);

        assert_eq!(note_lanes(&chart).len(), 9);
        assert_eq!(playable_lane_counts(&chart)[Lane::Key8.index()], 1);
    }

    #[test]
    fn pms_bme_9k_maps_key1_through_key9() {
        let chart = import_pms_text(&pms_note_lines_bme());
        assert_eq!(chart.metadata.key_mode, KeyMode::K9);
        let lanes = note_lanes(&chart);
        assert_eq!(lanes.len(), 9);
        assert!(lanes.contains(&Lane::Key9));
    }

    #[test]
    fn pms_5k_still_reports_k9_key_mode() {
        let mut text = String::from(PMS_HEADER);
        for (i, channel) in ["11", "12", "13", "14", "15"].into_iter().enumerate() {
            let measure = i + 1;
            text.push_str(&format!("#{measure:03}{channel}:01\n"));
        }
        let chart = import_pms_text(&text);
        assert_eq!(chart.metadata.key_mode, KeyMode::K9);
        assert_eq!(note_lanes(&chart).len(), 5);
    }

    fn import_bms_text(text: &str) -> IntermediateChart {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bms");
        std::fs::write(&path, text).unwrap();
        std::fs::write(dir.path().join("key.wav"), b"wav").unwrap();
        let mut warnings = Vec::new();
        import_bms_to_intermediate(&path, None, &mut warnings).unwrap()
    }

    fn import_bms_text_with_warnings(text: &str) -> (IntermediateChart, Vec<ImportWarning>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bms");
        std::fs::write(&path, text).unwrap();
        std::fs::write(dir.path().join("key.wav"), b"wav").unwrap();
        let mut warnings = Vec::new();
        let chart = import_bms_to_intermediate(&path, None, &mut warnings).unwrap();
        (chart, warnings)
    }

    const BMS_HEADER: &str = "\
#TITLE BMS Test
#ARTIST Tester
#BPM 120
#WAV01 key.wav
";

    fn ue_8k_note_lines() -> String {
        let mut lines = String::from(BMS_HEADER);
        for (i, channel) in ["16", "11", "12", "13", "14", "15", "18", "19"].into_iter().enumerate()
        {
            let measure = i + 1;
            lines.push_str(&format!("#{measure:03}{channel}:01\n"));
        }
        lines
    }

    #[test]
    fn detect_key_mode_from_headers_parses_qwilight_tags() {
        use bms_rs::bms::command::channel::mapper::KeyLayoutBeat;
        use bms_rs::bms::{default_config, parse_bms};

        let parse =
            |text: &str| parse_bms::<KeyLayoutBeat, _, _, _>(text, default_config()).bms.unwrap();

        assert_eq!(
            detect_key_mode_from_bms_headers(&parse("#4K\n"), ChartKeyLayout::beat()),
            Some(KeyMode::K4),
        );
        assert_eq!(
            detect_key_mode_from_bms_headers(&parse("#6K\n"), ChartKeyLayout::beat()),
            Some(KeyMode::K6),
        );
        assert_eq!(
            detect_key_mode_from_bms_headers(&parse("#8K\n"), ChartKeyLayout::beat()),
            Some(KeyMode::K8),
        );
        assert_eq!(
            detect_key_mode_from_bms_headers(
                &parse("* EXPANSION\n#6K\n#8K\n"),
                ChartKeyLayout::beat(),
            ),
            Some(KeyMode::K8),
        );
        assert_eq!(
            detect_key_mode_from_bms_headers(&parse("#TITLE x\n"), ChartKeyLayout::beat()),
            None,
        );
        assert_eq!(
            detect_key_mode_from_bms_headers(
                &parse("#8K\n"),
                ChartKeyLayout::pms(PmsKeyLayout::Standard),
            ),
            None,
        );
    }

    #[test]
    fn bms_8k_header_overrides_lane_detected_k7() {
        let mut text = ue_8k_note_lines();
        text.push_str("#8K\n");
        let chart = import_bms_text(&text);
        assert_eq!(chart.metadata.key_mode, KeyMode::K8);
    }

    #[test]
    fn bms_8k_header_maps_ue_channels_to_eight_key_lanes() {
        let mut text = ue_8k_note_lines();
        text.push_str("#8K\n");

        let chart = import_bms_text(&text);

        assert_eq!(chart.metadata.key_mode, KeyMode::K8);
        assert_eq!(
            note_lanes(&chart),
            vec![
                Lane::Key1,
                Lane::Key2,
                Lane::Key3,
                Lane::Key4,
                Lane::Key5,
                Lane::Key6,
                Lane::Key7,
                Lane::Key8,
            ],
        );
    }

    #[test]
    fn bms_without_qwilight_header_uses_lane_detect() {
        let chart = import_bms_text(&ue_8k_note_lines());
        assert_eq!(chart.metadata.key_mode, KeyMode::K7);
    }

    #[test]
    fn bms_4k_and_6k_headers_set_key_mode() {
        let mut text = ue_8k_note_lines();
        text.push_str("#4K\n");
        assert_eq!(import_bms_text(&text).metadata.key_mode, KeyMode::K4);

        let mut text = ue_8k_note_lines();
        text.push_str("#6K\n");
        assert_eq!(import_bms_text(&text).metadata.key_mode, KeyMode::K6);
    }

    #[test]
    fn bms_4k_header_maps_ue_channels_to_four_key_lanes() {
        let mut text = String::from(BMS_HEADER);
        text.push_str("#4K\n");
        for (i, channel) in ["11", "12", "14", "15"].into_iter().enumerate() {
            let measure = i + 1;
            text.push_str(&format!("#{measure:03}{channel}:01\n"));
        }

        let chart = import_bms_text(&text);

        assert_eq!(chart.metadata.key_mode, KeyMode::K4);
        assert_eq!(note_lanes(&chart), vec![Lane::Key1, Lane::Key2, Lane::Key3, Lane::Key4],);
    }

    #[test]
    fn bms_6k_header_maps_ue_channels_to_six_key_lanes() {
        let mut text = String::from(BMS_HEADER);
        text.push_str("#6K\n");
        for (i, channel) in ["11", "12", "13", "15", "18", "19"].into_iter().enumerate() {
            let measure = i + 1;
            text.push_str(&format!("#{measure:03}{channel}:01\n"));
        }

        let chart = import_bms_text(&text);

        assert_eq!(chart.metadata.key_mode, KeyMode::K6);
        assert_eq!(
            note_lanes(&chart),
            vec![Lane::Key1, Lane::Key2, Lane::Key3, Lane::Key4, Lane::Key5, Lane::Key6],
        );
    }

    #[test]
    #[ignore = "requires local 6K U_E FULL PACK sample data"]
    fn bms_6k_full_pack_sample_uses_six_active_lanes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../data/songs/6K U_E FULL PACK 3.1/234 [HAPPY HARDCORE] Blue-White Crazybits/crazybits6bit.bms",
        );
        assert!(path.exists(), "missing sample chart: {}", path.display());

        let mut warnings = Vec::new();
        let chart = import_bms_to_intermediate(&path, None, &mut warnings).unwrap();
        let counts = playable_lane_counts(&chart);

        assert_eq!(chart.metadata.key_mode, KeyMode::K6);
        for lane in [Lane::Key1, Lane::Key2, Lane::Key3, Lane::Key4, Lane::Key5, Lane::Key6] {
            assert!(counts[lane.index()] > 0, "{lane:?} has no playable objects");
        }
        assert_eq!(counts[Lane::Scratch.index()], 0);
        assert_eq!(counts[Lane::Key7.index()], 0);
    }

    #[test]
    #[ignore = "requires local 4K U_E FULL PACK sample data"]
    fn bms_4k_full_pack_sample_uses_four_active_lanes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/songs/4K U_E FULL PACK 2.1/[kozato] Marion/_Marion_4Pursuit.bml");
        assert!(path.exists(), "missing sample chart: {}", path.display());

        let mut warnings = Vec::new();
        let chart = import_bms_to_intermediate(&path, None, &mut warnings).unwrap();
        let counts = playable_lane_counts(&chart);

        assert_eq!(chart.metadata.key_mode, KeyMode::K4);
        for lane in [Lane::Key1, Lane::Key2, Lane::Key3, Lane::Key4] {
            assert!(counts[lane.index()] > 0, "{lane:?} has no playable objects");
        }
        assert_eq!(counts[Lane::Scratch.index()], 0);
        assert_eq!(counts[Lane::Key5.index()], 0);
    }

    #[test]
    fn bms_random_zero_is_clamped_to_one_for_beatoraja_compatibility() {
        let (chart, warnings) = import_bms_text_with_warnings(
            "\
#TITLE Random Zero
#BPM 120
#WAV01 key.wav
#RANDOM 0
#IF 1
#00111:01
#ENDIF
#ENDRANDOM
",
        );

        assert_eq!(note_lanes(&chart), vec![Lane::Key1]);
        assert!(warnings.iter().any(|warning| matches!(
            warning,
            ImportWarning::ParserDiagnostic { code, .. } if code == "RandomZeroClamped"
        )));
    }

    #[test]
    fn bms_random_control_is_flattened_like_beatoraja() {
        let (chart, _warnings) = import_bms_text_with_warnings(
            "\
#TITLE Random Flatten
#BPM 120
#WAV01 key.wav
#RANDOM 1
#IF 2
#00111:01
#ENDIF
#IF 1
#00212:01
#ENDIF
",
        );

        assert_eq!(note_lanes(&chart), vec![Lane::Key2]);
    }

    #[test]
    fn bms_random_else_after_matched_if_is_included_like_beatoraja() {
        // beatoraja (jbms-parser BMSDecoder) は #ELSE を予約語として扱わない。
        // #IF が一致した場合、#ELSE 以降のブロックもそのまま取り込まれる。
        let (chart, warnings) = import_bms_text_with_warnings(
            "\
#TITLE Else Matched
#BPM 120
#WAV01 key.wav
#RANDOM 1
#IF 1
#00111:01
#ELSE
#00212:01
#ENDIF
",
        );

        assert_eq!(note_lanes(&chart), vec![Lane::Key1, Lane::Key2]);
        assert!(warnings.iter().any(|warning| matches!(
            warning,
            ImportWarning::ParserDiagnostic { code, .. }
                if code == "BeatorajaRandomUnsupportedElse"
        )));
    }

    #[test]
    fn bms_random_else_after_unmatched_if_stays_skipped_like_beatoraja() {
        // #IF が不一致の場合、#ELSE は skip 状態を反転させないため
        // #ELSE 以降のブロックも beatoraja と同じく skip される。
        let (chart, _warnings) = import_bms_text_with_warnings(
            "\
#TITLE Else Unmatched
#BPM 120
#WAV01 key.wav
#RANDOM 1
#IF 2
#00111:01
#ELSE
#00212:01
#ENDIF
#00313:01
",
        );

        assert_eq!(note_lanes(&chart), vec![Lane::Key3]);
    }

    #[test]
    fn bms_random_elseif_is_ignored_like_beatoraja() {
        // #ELSEIF も同様に無視され、直前の #IF の skip 状態が継続する。
        let (chart, warnings) = import_bms_text_with_warnings(
            "\
#TITLE ElseIf Ignored
#BPM 120
#WAV01 key.wav
#RANDOM 1
#IF 1
#00111:01
#ELSEIF 2
#00212:01
#ENDIF
",
        );

        assert_eq!(note_lanes(&chart), vec![Lane::Key1, Lane::Key2]);
        assert!(warnings.iter().any(|warning| matches!(
            warning,
            ImportWarning::ParserDiagnostic { code, .. }
                if code == "BeatorajaRandomUnsupportedElse"
        )));
    }

    #[test]
    fn bms_random_sections_set_has_bms_random_metadata() {
        let (with_random, _) = import_bms_text_with_warnings(
            "\
#TITLE Random Song
#BPM 120
#WAV01 key.wav
#RANDOM 1
#IF 1
#00111:01
#ENDIF
",
        );
        let (without_random, _) = import_bms_text_with_warnings(
            "\
#TITLE Plain Song
#BPM 120
#WAV01 key.wav
#00111:01
",
        );

        assert!(with_random.metadata.has_bms_random);
        assert!(!without_random.metadata.has_bms_random);
    }

    #[test]
    fn bms_headers_capture_url_and_metadata_commands() {
        let (chart, _) = import_bms_text_with_warnings(
            "\
#TITLE Example Song
#ARTIST Alice
#URL http://example.com/bms
#URL-WAV http://example.com/append
#BPM 120
#WAV01 key.wav
#00111:01
",
        );

        assert_eq!(chart.metadata.source_url, "http://example.com/bms");
        assert_eq!(chart.metadata.append_url, "http://example.com/append");
        assert_eq!(chart.metadata.bms_headers.get("TITLE"), Some(&"Example Song".to_string()));
        assert_eq!(
            chart.metadata.bms_headers.get("URL"),
            Some(&"http://example.com/bms".to_string())
        );
        assert_eq!(
            chart.metadata.bms_headers.get("URL-WAV"),
            Some(&"http://example.com/append".to_string())
        );
        assert!(!chart.metadata.bms_headers.contains_key("00111"));
    }

    #[test]
    fn bms_headers_exclude_base62_channel_commands() {
        let headers = extract_bms_headers_from_text("#002D9:000102\n#TITLE Example");

        assert!(!headers.contains_key("002D9"));
        assert_eq!(headers.get("TITLE"), Some(&"Example".to_string()));
    }

    #[test]
    fn empty_trailing_metadata_does_not_clear_previous_values() {
        let (chart, _) = import_bms_text_with_warnings(
            "\
#TITLE Sakura Fubuki
#ARTIST Street
#GENRE Drumstep
#BPM 175
#PLAYLEVEL 12
#TOTAL 440
#STAGEFILE
#WAV01 key.wav
#00111:01
#GENRE
#TITLE
#ARTIST
#TOTAL
",
        );

        assert_eq!(chart.metadata.title, "Sakura Fubuki");
        assert_eq!(chart.metadata.artist, "Street");
        assert_eq!(chart.metadata.genre, "Drumstep");
        assert_eq!(chart.metadata.play_level, "12");
        assert_eq!(chart.metadata.initial_bpm, 175.0);
        assert_eq!(chart.metadata.total, Some(440.0));
        assert_eq!(chart.metadata.stage_file, "");
        assert_eq!(chart.metadata.bms_headers.get("TITLE"), Some(&"Sakura Fubuki".to_string()));
        assert_eq!(chart.metadata.bms_headers.get("TOTAL"), Some(&"440".to_string()));
    }

    #[test]
    fn bms_random_orphan_if_warns_and_continues_like_beatoraja() {
        let (chart, warnings) = import_bms_text_with_warnings(
            "\
#TITLE Orphan If
#BPM 120
#WAV01 key.wav
#IF 1
#00111:01
#ENDIF
",
        );

        assert_eq!(note_lanes(&chart), vec![Lane::Key1]);
        assert!(warnings.iter().any(|warning| matches!(
            warning,
            ImportWarning::ParserDiagnostic { code, .. }
                if code == "BeatorajaRandomIfWithoutRandom"
        )));
        assert!(warnings.iter().any(|warning| matches!(
            warning,
            ImportWarning::ParserDiagnostic { code, .. }
                if code == "BeatorajaRandomEndifWithoutIf"
        )));
    }

    #[test]
    fn bms_end_if_typo_is_ignored_like_beatoraja() {
        let (chart, warnings) = import_bms_text_with_warnings(
            "\
#TITLE End If Typo
#BPM 120
#WAV01 key.wav
#SETRANDOM 2
#IF 1
#00111:01
#end if
#IF 2
#00212:01
#end if
",
        );

        assert_eq!(note_lanes(&chart), vec![Lane::Key2]);
        assert!(warnings.iter().any(|warning| matches!(
            warning,
            ImportWarning::ParserDiagnostic { code, .. }
                if code == "BeatorajaRandomIgnoredTypoControl"
        )));
    }

    #[test]
    fn bms_setrandom_is_flattened_with_fixed_condition() {
        let (chart, _warnings) = import_bms_text_with_warnings(
            "\
#TITLE SetRandom
#BPM 120
#WAV01 key.wav
#SETRANDOM 2
#IF 1
#00111:01
#ENDIF
#IF 2
#00212:01
#ENDIF
#ENDRANDOM
",
        );

        assert_eq!(note_lanes(&chart), vec![Lane::Key2]);
    }

    #[test]
    fn bms_8k_ue_sample_reports_k8_when_present() {
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/songs/8K U_E FULL PACK 1.1/[r] Baby/_baby_8K_Hard.bms"
        ));
        if !path.exists() {
            return;
        }
        let mut warnings = Vec::new();
        let chart = import_bms_to_intermediate(path, None, &mut warnings).unwrap();
        let counts = playable_lane_counts(&chart);
        assert_eq!(chart.metadata.key_mode, KeyMode::K8);
        for lane in [
            Lane::Key1,
            Lane::Key2,
            Lane::Key3,
            Lane::Key4,
            Lane::Key5,
            Lane::Key6,
            Lane::Key7,
            Lane::Key8,
        ] {
            assert!(counts[lane.index()] > 0, "{lane:?} has no playable objects");
        }
        assert_eq!(counts[Lane::Scratch.index()], 0);
    }

    #[test]
    fn pms_18k_player2_notes_are_dropped_with_warning() {
        let mut text = String::from(PMS_HEADER);
        text.push_str("#00121:01\n");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.pms");
        std::fs::write(&path, &text).unwrap();
        std::fs::write(dir.path().join("key.wav"), b"wav").unwrap();
        let mut warnings = Vec::new();
        let chart = import_pms_to_intermediate(&path, None, &mut warnings).unwrap();
        assert!(note_lanes(&chart).is_empty());
        assert!(
            warnings.iter().any(|warning| matches!(
                warning,
                ImportWarning::UnsupportedPmsPlayerSide { side: 2 }
            ))
        );
    }
}
