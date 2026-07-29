use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use encoding_rs::SHIFT_JIS;
use serde_json::{Value as JsonValue, json};

use crate::{LoadedLuaSkinValue, SkinLoadDependencies, SkinLoadWarning, SkinLoadedFileDependency};

mod builder_assets;
mod builder_core;
mod builder_destination;
mod builder_play;
mod helpers;
mod processor;

use helpers::*;
use processor::Processor;

const LR2_OFFSET_LIFT: i32 = 3;
const LR2_OFFSET_JUDGE_1P: i32 = 32;
const LR2_REFERENCE_IMAGES: [i32; 5] = [100, 101, 102, 110, 111];

#[derive(Debug, Clone)]
struct CsvLine {
    command: String,
    fields: Vec<String>,
}

#[derive(Debug, Clone)]
struct CustomOption {
    name: String,
    base: i32,
    items: Vec<String>,
}

#[derive(Debug, Clone)]
struct CustomFile {
    name: String,
    path: String,
    default: String,
}

#[derive(Debug, Clone)]
struct CustomOffset {
    name: String,
    id: i32,
    flags: [bool; 6],
}

#[derive(Debug, Clone)]
struct Header {
    skin_type: i32,
    name: String,
    author: String,
    w: u32,
    h: u32,
    fadeout: i32,
    input: i32,
    scene: i32,
    close: i32,
    loadstart: i32,
    loadend: i32,
    playstart: i32,
    judgetimer: i32,
    finishmargin: i32,
    builtin_options: [bool; 4],
    options: Vec<CustomOption>,
    files: Vec<CustomFile>,
    offsets: Vec<CustomOffset>,
    selected_ops: HashMap<i32, bool>,
}

struct LoadedHeader {
    header: Header,
    dependencies: SkinLoadDependencies,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            skin_type: 0,
            name: String::new(),
            author: String::new(),
            w: 1280,
            h: 720,
            fadeout: 0,
            input: 0,
            scene: 0,
            close: 0,
            loadstart: 0,
            loadend: 0,
            playstart: 0,
            judgetimer: 1,
            finishmargin: 0,
            builtin_options: [true; 4],
            options: Vec::new(),
            files: Vec::new(),
            offsets: Vec::new(),
            selected_ops: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct SourceRegion {
    src: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    divx: i32,
    divy: i32,
    cycle: i32,
    timer: Option<i32>,
}

#[derive(Debug, Clone)]
struct CurrentObject {
    variants: Vec<CurrentObjectVariant>,
}

#[derive(Debug, Clone)]
struct CurrentObjectVariant {
    id: String,
    conditional_ops: Vec<i32>,
}

#[derive(Debug, Clone, Copy)]
enum NoteSlot {
    Note,
    LnStart,
    LnEnd,
    LnBody,
    LnBodyActive,
    HcnStart,
    HcnEnd,
    HcnBody,
    HcnActive,
    HcnDamage,
    HcnReactive,
    Mine,
}

struct CsvBuilder<'a> {
    skin_root: PathBuf,
    skin_file_dir: PathBuf,
    skin_file_dir_name: Option<String>,
    header: Header,
    files: &'a BTreeMap<String, String>,
    warnings: Vec<SkinLoadWarning>,
    sources: Vec<JsonValue>,
    source_paths: Vec<Option<String>>,
    fonts: Vec<JsonValue>,
    lr2font_ids: Vec<Option<String>>,
    images: Vec<JsonValue>,
    imagesets: Vec<JsonValue>,
    lr2_imagesets: Vec<Vec<SourceRegion>>,
    values: Vec<JsonValue>,
    texts: Vec<JsonValue>,
    sliders: Vec<JsonValue>,
    graphs: Vec<JsonValue>,
    judge_graphs: Vec<JsonValue>,
    bpm_graphs: Vec<JsonValue>,
    timing_visualizers: Vec<JsonValue>,
    special_destination_sizes: HashMap<String, (i32, i32)>,
    hidden_covers: Vec<JsonValue>,
    gauge: Option<JsonValue>,
    gauges: Vec<JsonValue>,
    note: NoteState,
    judges: Vec<JudgeState>,
    bga: Option<JsonValue>,
    destinations: Vec<JsonValue>,
    current: Option<CurrentObject>,
    conditional_ops: Vec<i32>,
    runtime_option_aliases: HashMap<i32, Vec<i32>>,
    stretch: i32,
    lr2_gauge_id: Option<String>,
    lr2_gauge_add_x: i32,
    lr2_gauge_add_y: i32,
    current_has_destination: bool,
    note_marker_inserted: bool,
    next_id: usize,
    remap_single_play_2p_lanes: bool,
    file_dependencies: BTreeSet<String>,
    loaded_file_dependencies: BTreeMap<PathBuf, SkinLoadedFileDependency>,
}

#[derive(Default)]
struct NoteState {
    note: Vec<String>,
    lnstart: Vec<String>,
    lnend: Vec<String>,
    lnbody: Vec<String>,
    lnbody_active: Vec<String>,
    hcnstart: Vec<String>,
    hcnend: Vec<String>,
    hcnbody: Vec<String>,
    hcnactive: Vec<String>,
    hcndamage: Vec<String>,
    hcnreactive: Vec<String>,
    mine: Vec<String>,
    size: Vec<i32>,
    dst: Vec<JsonValue>,
    dst2: Option<i32>,
    expansion_rate: Option<[i32; 2]>,
    line_sources: Vec<Option<String>>,
    line_destinations: Vec<Option<JsonValue>>,
    group: Vec<JsonValue>,
    bpm: Vec<JsonValue>,
    stop: Vec<JsonValue>,
    time: Vec<JsonValue>,
}

#[derive(Default, Clone)]
struct JudgeState {
    images: Vec<JsonValue>,
    numbers: Vec<JsonValue>,
    shift: bool,
    marker_inserted: bool,
    detail_inserted: bool,
}

pub fn load_lr2_csv_skin_value(
    path: &Path,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
) -> Result<LoadedLuaSkinValue> {
    let LoadedHeader { mut header, mut dependencies } = load_header(path, options)?;
    apply_default_play_header_items(&mut header);
    apply_selected_header_options(&mut header, options);
    let mut builder = CsvBuilder::new(path, header, files);
    let lines = read_csv_lines(path)?;
    let mut processor = Processor::new(builder.header.selected_ops.clone());
    processor.process_lines(&lines, path, &mut builder)?;
    dependencies.option_values.extend(processor.option_dependencies);
    dependencies.option_values.extend(builder.load_time_option_dependencies());
    dependencies.files.extend(builder.file_dependencies.iter().cloned());
    dependencies.loaded_files.extend(builder.loaded_file_dependencies.clone());
    let warnings = builder.warnings.clone();
    let internal_enabled_options = builder.internal_enabled_options();
    Ok(LoadedLuaSkinValue {
        value: builder.finish(),
        lua_runtime: None,
        runtime_draw_paths: Vec::new(),
        warnings,
        files: BTreeMap::new(),
        dependencies,
        internal_enabled_options,
    })
}

pub fn load_lr2_csv_skin_dependency_option_values(
    path: &Path,
    options: &BTreeMap<String, String>,
    option_ids: impl IntoIterator<Item = i32>,
) -> Result<BTreeMap<i32, bool>> {
    let LoadedHeader { mut header, .. } = load_header(path, options)?;
    apply_default_play_header_items(&mut header);
    apply_selected_header_options(&mut header, options);
    Ok(option_ids
        .into_iter()
        .map(|option_id| {
            let option_id = option_id.abs();
            (option_id, header.selected_ops.get(&option_id).copied().unwrap_or(false))
        })
        .collect())
}

fn load_header(path: &Path, options: &BTreeMap<String, String>) -> Result<LoadedHeader> {
    let mut header = Header::default();
    let lines = read_csv_lines(path)?;
    let mut processor = Processor::new(HashMap::new());
    for line in &lines {
        if !processor.should_execute(line) {
            continue;
        }
        match line.command.as_str() {
            "RESOLUTION" => match parse_i32(line.fields.get(1)) {
                1 => {
                    header.w = 1280;
                    header.h = 720;
                }
                2 => {
                    header.w = 1920;
                    header.h = 1080;
                }
                3 => {
                    header.w = 3840;
                    header.h = 2160;
                }
                _ => {
                    header.w = 640;
                    header.h = 480;
                }
            },
            "INFORMATION" => {
                header.skin_type = parse_i32(line.fields.get(1));
                header.name = field(line, 2).to_string();
                header.author = field(line, 3).to_string();
            }
            "FADEOUT" => header.fadeout = parse_i32(line.fields.get(1)),
            "STARTINPUT" => header.input = parse_i32(line.fields.get(1)),
            "SCENETIME" => header.scene = parse_i32(line.fields.get(1)),
            "CLOSE" => header.close = parse_i32(line.fields.get(1)),
            "LOADSTART" => header.loadstart = parse_i32(line.fields.get(1)),
            "LOADEND" => header.loadend = parse_i32(line.fields.get(1)),
            "PLAYSTART" => header.playstart = parse_i32(line.fields.get(1)),
            "JUDGETIMER" => header.judgetimer = parse_i32(line.fields.get(1)),
            "FINISHMARGIN" => header.finishmargin = parse_i32(line.fields.get(1)),
            "CUSTOMOPTION_ADDITION_SETTING" => {
                for (index, enabled) in header.builtin_options.iter_mut().enumerate() {
                    if let Some(value) = line.fields.get(index + 1) {
                        *enabled = parse_i32(Some(value)) != 0;
                    }
                }
            }
            "CUSTOMOPTION" => {
                let name = field(line, 1).to_string();
                let base = parse_i32(line.fields.get(2));
                let items = line
                    .fields
                    .iter()
                    .skip(3)
                    .map(|item| item.trim())
                    .filter(|item| !item.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                if !name.is_empty() && !items.is_empty() {
                    header.options.push(CustomOption { name, base, items });
                }
            }
            "CUSTOMFILE" => {
                let name = field(line, 1).to_string();
                let path =
                    relative_to_skin_file_parent(path, &normalize_lr2_asset_path(field(line, 2)));
                let default = field(line, 3).to_string();
                if !name.is_empty() && !path.is_empty() {
                    header.files.push(CustomFile { name, path, default });
                }
            }
            "CUSTOMOFFSET" => {
                let mut flags = [true; 6];
                for (index, flag) in flags.iter_mut().enumerate() {
                    if line.fields.len() > index + 3 {
                        *flag = parse_i32(line.fields.get(index + 3)) > 0;
                    }
                }
                header.offsets.push(CustomOffset {
                    name: field(line, 1).to_string(),
                    id: parse_i32(line.fields.get(2)),
                    flags,
                });
            }
            _ => {}
        }
    }

    apply_selected_header_options(&mut header, options);
    apply_derived_play_options(&mut header);
    let dependencies = SkinLoadDependencies {
        number_values: BTreeMap::new(),
        text_values: BTreeMap::new(),
        option_values: processor.option_dependencies,
        event_index_values: BTreeMap::new(),
        offset_values: BTreeMap::new(),
        offset_id_values: BTreeMap::new(),
        files: BTreeSet::new(),
        loaded_files: BTreeMap::new(),
        virtual_io_files: BTreeMap::new(),
        opaque: false,
    };
    Ok(LoadedHeader { header, dependencies })
}

fn apply_selected_header_options(header: &mut Header, options: &BTreeMap<String, String>) {
    for option in &header.options {
        let selected_index = options
            .iter()
            .find(|(name, _)| lr2_option_text_matches(name, &option.name))
            .map(|(_, selected)| selected)
            .and_then(|selected| {
                option.items.iter().position(|item| lr2_option_text_matches(item, selected))
            })
            .unwrap_or(0);
        for (index, _) in option.items.iter().enumerate() {
            header.selected_ops.insert(option.base + index as i32, index == selected_index);
        }
    }
}

fn apply_default_play_header_items(header: &mut Header) {
    if !matches!(header.skin_type, 0 | 1 | 2 | 3 | 4 | 12 | 13) {
        return;
    }
    if header.builtin_options[0] {
        add_builtin_option(header, "BGA Size", 30, &["Normal", "Extend"]);
    }
    if header.builtin_options[1] {
        add_builtin_option(header, "Ghost", 34, &["Off", "Type A", "Type B", "Type C"]);
    }
    if header.builtin_options[2] {
        add_builtin_option(header, "Score Graph", 38, &["Off", "On"]);
    }
    if header.builtin_options[3] {
        add_builtin_option(header, "Judge Detail", 1997, &["Off", "EARLY/LATE", "+-ms"]);
    }
    add_builtin_offset(header, "All offset(%)", 1, [true, true, true, true, false, false]);
    add_builtin_offset(header, "Notes offset", 31, [false, false, false, true, false, false]);
    add_builtin_offset(header, "Judge offset", 32, [true, true, true, true, false, true]);
    add_builtin_offset(header, "Judge Detail offset", 33, [true, true, true, true, false, true]);
}

fn apply_derived_play_options(header: &mut Header) {
    if !matches!(header.skin_type, 0 | 1 | 2 | 3 | 4 | 12 | 13) {
        return;
    }

    for op in [160, 161, 162, 163, 164] {
        header.selected_ops.entry(op).or_insert(false);
    }
    let mode_op = match header.skin_type {
        0 | 12 | 13 => Some(160),
        1 => Some(161),
        2 => Some(162),
        3 => Some(163),
        4 => Some(164),
        _ => None,
    };
    if let Some(op) = mode_op {
        header.selected_ops.insert(op, true);
    }

    if header.selected_ops.get(&981).copied().unwrap_or(false) {
        header.selected_ops.entry(965).or_insert(true);
        header.selected_ops.entry(966).or_insert(false);
    }
}

fn add_builtin_option(header: &mut Header, name: &str, base: i32, items: &[&str]) {
    if header.options.iter().any(|option| option.name == name) {
        return;
    }
    header.options.push(CustomOption {
        name: name.to_string(),
        base,
        items: items.iter().map(|item| item.to_string()).collect(),
    });
    header.selected_ops.entry(base).or_insert(true);
    for index in 1..items.len() {
        header.selected_ops.entry(base + index as i32).or_insert(false);
    }
}

fn add_builtin_offset(header: &mut Header, name: &str, id: i32, flags: [bool; 6]) {
    if header.offsets.iter().any(|offset| offset.id == id) {
        return;
    }
    header.offsets.push(CustomOffset { name: name.to_string(), id, flags });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("{name}-{nanos}"))
    }

    #[test]
    fn lr2_asset_path_strips_theme_prefix() {
        assert_eq!(
            normalize_lr2_asset_path(r".\LR2files\Theme\WMII_FHD\play\parts\note\*.png"),
            "play/parts/note/*.png"
        );
    }

    #[test]
    fn lr2_destination_converts_top_origin_to_bottom_origin() {
        let mut values = [0; 22];
        values[2] = 100;
        values[3] = 10;
        values[4] = 20;
        values[5] = 30;
        values[6] = 40;
        let frame = destination_frame(&values, 1080);
        assert_eq!(frame["time"], 100);
        assert_eq!(frame["x"], 10);
        assert_eq!(frame["y"], 1020);
        assert_eq!(frame["w"], 30);
        assert_eq!(frame["h"], 40);
    }

    #[test]
    fn lr2_destination_preserves_angle_and_custom_offset_id() {
        let mut values = [0; 22];
        values[14] = -90;
        values[21] = 32;

        let destination = destination_def_with_default_offsets("image", &values, 1080, &[], &[]);

        assert_eq!(destination["dst"][0]["angle"], -90);
        assert_eq!(destination["offset"], 32);
    }

    #[test]
    fn lr2_dst_line_defaults_to_lift_offset() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-dst-line").join("play.lr2skin");
        let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
        builder.add_source("line.png");
        builder
            .execute(&parse_csv_line("#SRC_LINE,0,0,0,0,10,1,1,1,0,0").expect("valid SRC_LINE"))
            .unwrap();
        builder
            .execute(
                &parse_csv_line("#DST_LINE,0,0,10,20,40,2,0,255,255,255,255,0,0,0,0,0,0,0,0,0,")
                    .expect("valid DST_LINE"),
            )
            .unwrap();
        builder.complete_play_lines();

        let group = builder.note.group.first().expect("DST_LINE should produce note.group");

        assert_eq!(group["offset"], 0);
        assert_eq!(group["offsets"].as_array().unwrap(), &[json!(LR2_OFFSET_LIFT)]);
        for destinations in [&builder.note.bpm, &builder.note.stop, &builder.note.time] {
            assert_eq!(destinations.len(), 1);
            assert_eq!(destinations[0]["offsets"].as_array().unwrap(), &[json!(LR2_OFFSET_LIFT)]);
        }
        let bpm_frame = builder.note.bpm[0]["dst"].as_array().unwrap().first().unwrap();
        assert_eq!(bpm_frame["h"], json!(4));
        assert_eq!(
            (bpm_frame["r"].as_i64(), bpm_frame["g"].as_i64(), bpm_frame["b"].as_i64()),
            (Some(0), Some(192), Some(0))
        );
        let stop_frame = builder.note.stop[0]["dst"].as_array().unwrap().first().unwrap();
        assert_eq!(
            (stop_frame["r"].as_i64(), stop_frame["g"].as_i64(), stop_frame["b"].as_i64()),
            (Some(192), Some(192), Some(0))
        );
        let time_frame = builder.note.time[0]["dst"].as_array().unwrap().first().unwrap();
        assert_eq!(time_frame["h"], json!(2));
        assert_eq!(
            (time_frame["r"].as_i64(), time_frame["g"].as_i64(), time_frame["b"].as_i64()),
            (Some(64), Some(192), Some(192))
        );
    }

    #[test]
    fn lr2_nowjudge_indices_match_beatoraja_slots() {
        assert_eq!(lr2_judge_slot(5), 0);
        assert_eq!(lr2_judge_slot(4), 1);
        assert_eq!(lr2_judge_slot(3), 2);
        assert_eq!(lr2_judge_slot(2), 3);
        assert_eq!(lr2_judge_slot(1), 4);
        assert_eq!(lr2_judge_slot(0), 5);
        assert_eq!(lr2_judge_slot(6), 6);
    }

    #[test]
    fn lr2_number_ref_preserves_poor_plus_miss() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-number-ref").join("play.lr2skin");
        let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
        builder.add_source("numbers.png");
        builder
            .execute(
                &parse_csv_line("#SRC_NUMBER,0,0,0,0,10,20,1,10,0,0,426,0,4,0,1")
                    .expect("valid SRC_NUMBER"),
            )
            .unwrap();

        let value = builder.values.first().unwrap();
        assert_eq!(value["ref"], json!(426));
        assert_eq!(value["digit"], json!(4));
        assert_eq!(value["zeropadding"], json!(0));
        assert_eq!(value["space"], json!(1));
    }

    #[test]
    fn lr2_signed_number_reserves_sign_digit_and_defaults_to_blank_padding() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-signed-number").join("play.lr2skin");
        let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
        builder.add_source("numbers.png");
        builder
            .execute(
                &parse_csv_line("#SRC_NUMBER,0,0,0,0,168,30,12,2,0,0,12,0,2,,,,,,,,")
                    .expect("valid SRC_NUMBER"),
            )
            .unwrap();

        let value = builder.values.first().unwrap();
        assert_eq!(value["digit"], json!(3));
        assert_eq!(value["zeropadding"], json!(2));
        assert_eq!(value["space"], json!(0));
    }

    #[test]
    fn lr2_unsigned_number_ignores_explicit_zero_padding_like_beatoraja() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-unsigned-number").join("play.lr2skin");
        let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
        builder.add_source("numbers.png");
        builder
            .execute(
                &parse_csv_line("#SRC_NUMBER,0,0,0,0,100,20,10,1,0,0,100,0,4,2,0")
                    .expect("valid SRC_NUMBER"),
            )
            .unwrap();

        let value = builder.values.first().unwrap();
        assert_eq!(value["digit"], json!(4));
        assert_eq!(value["zeropadding"], json!(0));
    }

    #[test]
    fn lr2_button_keeps_state_reference_separate_from_clickability() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-button").join("play.lr2skin");
        let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
        builder.add_source("button.png");
        builder
            .execute(
                &parse_csv_line("#SRC_BUTTON,0,0,0,0,20,10,2,1,0,0,77,0,0,-1,3")
                    .expect("valid SRC_BUTTON"),
            )
            .unwrap();

        let image = builder.images.first().unwrap();
        assert_eq!(image["act"], json!(77));
        assert_eq!(image["clickable"], json!(false));
        assert_eq!(image["click"], json!(1));
        assert_eq!(image["len"], json!(3));
    }

    #[test]
    fn lr2_imageset_combines_registered_source_sets() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-imageset").join("play.lr2skin");
        let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
        builder.add_source("set.png");
        builder
            .execute(&parse_csv_line("#IMAGESET,0,0,0,0,20,10,2,1,0,0").expect("valid IMAGESET"))
            .unwrap();
        builder
            .execute(&parse_csv_line("#SRC_IMAGESET,100,0,88,1,0").expect("valid SRC_IMAGESET"))
            .unwrap();

        let imageset = builder.imagesets.first().unwrap();
        assert_eq!(imageset["ref"], json!(88));
        assert_eq!(imageset["images"].as_array().unwrap().len(), 1);
        assert_eq!(builder.images.last().unwrap()["cycle"], json!(100));
    }

    #[test]
    fn lr2_play_headers_and_stretch_are_preserved() {
        let path = Path::new("skin/play/test.lr2skin");
        let files = BTreeMap::new();
        let mut builder = CsvBuilder::new(path, Header::default(), &files);
        let lines = [
            parse_csv_line("#STARTINPUT,350").unwrap(),
            parse_csv_line("#SCENETIME,90000").unwrap(),
            parse_csv_line("#JUDGETIMER,3").unwrap(),
            parse_csv_line("#IMAGE,parts/frame.png").unwrap(),
            parse_csv_line("#SRC_IMAGE,0,0,0,0,10,10,1,1,0,0").unwrap(),
            parse_csv_line("#STRETCH,2").unwrap(),
            parse_csv_line("#DST_IMAGE,0,0,0,10,20,30,40,0,255,255,255,255,0,0,0,0,0,0,0,0,0")
                .unwrap(),
        ];
        let mut processor = Processor::new(HashMap::new());

        processor.process_lines(&lines, path, &mut builder).unwrap();

        assert_eq!(builder.header.input, 350);
        assert_eq!(builder.header.scene, 90_000);
        assert_eq!(builder.header.judgetimer, 3);
        assert_eq!(builder.destinations[0]["stretch"], json!(2));
    }

    #[test]
    fn lr2_bargraph_preserves_negative_fill_direction() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-negative-graph").join("play.lr2skin");
        let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
        builder.add_source("graph.png");
        builder
            .execute(
                &parse_csv_line("#SRC_BARGRAPH,0,0,0,0,100,10,1,1,0,0,0,0")
                    .expect("valid SRC_BARGRAPH"),
            )
            .unwrap();
        builder
            .execute(
                &parse_csv_line(
                    "#DST_BARGRAPH,0,0,50,20,-30,8,0,255,255,255,255,0,0,0,0,0,0,0,0,0",
                )
                .expect("valid DST_BARGRAPH"),
            )
            .unwrap();

        let frame = builder.destinations[0]["dst"].as_array().unwrap().first().unwrap();
        assert_eq!(frame["x"], json!(50));
        assert_eq!(frame["w"], json!(-30));
    }

    #[test]
    fn lr2_play_chart_sources_keep_beatoraja_fields_and_destination_size() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-play-chart").join("play.lr2skin");
        let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
        builder
            .execute(
                &parse_csv_line("#SRC_NOTECHART_1P,2,0,0,0,0,0,0,0,0,0,300,120,0,0,15,1,1,1,1")
                    .expect("valid SRC_NOTECHART_1P"),
            )
            .unwrap();
        builder
            .execute(
                &parse_csv_line(
                    "#DST_NOTECHART_1P,0,0,50,200,0,0,0,255,255,255,255,0,0,0,0,0,0,0,0,0",
                )
                .expect("valid DST_NOTECHART_1P"),
            )
            .unwrap();

        let graph = builder.judge_graphs.first().unwrap();
        assert_eq!(graph["type"], json!(2));
        assert_eq!(graph["delay"], json!(15));
        assert_eq!(graph["backTexOff"], json!(1));
        assert_eq!(graph["orderReverse"], json!(1));
        assert_eq!(graph["noGap"], json!(1));
        assert_eq!(graph["noGapX"], json!(1));
        let frame = builder.destinations[0]["dst"].as_array().unwrap().first().unwrap();
        assert_eq!(frame["x"], json!(50));
        assert_eq!(frame["y"], json!(520));
        assert_eq!(frame["w"], json!(300));
        assert_eq!(frame["h"], json!(120));
    }

    #[test]
    fn lr2_nowjudge_adds_beatoraja_judge_detail_objects() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-judge-detail").join("play.lr2skin");
        let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
        builder.add_source("judge.png");
        builder
            .execute(
                &parse_csv_line("#SRC_NOWJUDGE_1P,5,0,0,0,100,20,1,1,0,0,0")
                    .expect("valid SRC_NOWJUDGE_1P"),
            )
            .unwrap();
        builder
            .execute(
                &parse_csv_line(
                    "#DST_NOWJUDGE_1P,0,0,100,200,120,24,0,255,255,255,255,0,0,0,0,0,0,0,0,0",
                )
                .expect("valid DST_NOWJUDGE_1P"),
            )
            .unwrap();

        assert!(builder.sources.iter().any(|source| source["path"] == "bmz://lr2/judgedetail"));
        let detail_destinations = builder
            .destinations
            .iter()
            .filter(|destination| {
                destination["op"].as_array().is_some_and(|ops| {
                    ops.iter().any(|op| matches!(op.as_i64(), Some(1998 | 1999)))
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(detail_destinations.len(), 4);
        assert!(detail_destinations.iter().all(|destination| {
            destination["offsets"]
                .as_array()
                .is_some_and(|offsets| offsets == &[json!(33), json!(LR2_OFFSET_LIFT)])
        }));
    }

    #[test]
    fn lr2_text_defaults_to_shrink_overflow() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-text-shrink").join("play.lr2skin");
        let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
        builder.execute(&parse_csv_line("#SRC_TEXT,0,0,10,1,0").expect("valid SRC_TEXT")).unwrap();
        builder
            .execute(
                &parse_csv_line("#DST_TEXT,0,0,10,20,120,30,0,255,255,255,255,0,0,0,0,0,0,0,0,0,0")
                    .expect("valid DST_TEXT"),
            )
            .unwrap();

        let text = builder.texts.first().expect("SRC_TEXT should produce text");
        assert_eq!(text["overflow"], json!(1));

        let destination =
            builder.destinations.first().expect("DST_TEXT should produce destination");
        let frame = destination["dst"].as_array().unwrap().first().unwrap();
        assert_eq!(frame["w"], json!(120));
        assert_eq!(frame["h"], json!(30));
    }

    #[test]
    fn lr2_ln_body_keeps_animation_only_while_held() {
        let files = BTreeMap::new();

        for command in ["SRC_LN_BODY", "SRC_AUTO_LN_BODY"] {
            let skin_path = unique_test_dir("bmz-lr2-ln-body").join("play.lr2skin");
            let mut builder = CsvBuilder::new(&skin_path, Header::default(), &files);
            builder.execute(&parse_csv_line("#IMAGE,notes.png").expect("valid IMAGE")).unwrap();
            builder
                .execute(
                    &parse_csv_line(&format!("#{command},0,0,0,0,10,20,4,6,266,123"))
                        .expect("valid LN body source"),
                )
                .unwrap();

            let inactive = &builder.images[0];
            let active = &builder.images[1];
            assert_eq!(inactive["id"], json!(builder.note.lnbody[7]));
            assert_eq!(active["id"], json!(builder.note.lnbody_active[7]));
            assert_eq!(inactive["cycle"], json!(0), "{command} inactive body");
            assert!(inactive["timer"].is_null(), "{command} inactive body");
            assert_eq!(active["cycle"], json!(266), "{command} active body");
            assert_eq!(active["timer"], json!(123), "{command} active body");
        }
    }

    #[test]
    fn lr2_customfile_default_replaces_wildcard_once() {
        assert_eq!(
            substitute_wildcard_default("parts/note/*.png", "parts/note/*.png", "photon"),
            "parts/note/photon.png"
        );
    }

    #[test]
    fn lr2_customfile_selection_uses_existing_skin_file() {
        let root = unique_test_dir("bmz-lr2-customfile");
        let play_dir = root.join("play");
        std::fs::create_dir_all(play_dir.join("parts/gauge")).unwrap();
        std::fs::write(play_dir.join("parts/gauge/default.png"), []).unwrap();
        std::fs::write(play_dir.join("parts/gauge/blue.png"), []).unwrap();
        let skin_path = play_dir.join("FHDPLAY_AC.lr2skin");
        std::fs::write(&skin_path, []).unwrap();
        let mut header = Header::default();
        header.files.push(CustomFile {
            name: "GAUGE COLOR".to_string(),
            path: "parts/gauge/*.png".to_string(),
            default: "default".to_string(),
        });
        let files =
            BTreeMap::from([("GAUGE COLOR".to_string(), "parts/gauge/blue.png".to_string())]);
        let mut builder = CsvBuilder::new(&skin_path, header, &files);

        assert_eq!(
            builder.resolve_source_path(r".\LR2files\Theme\WMII_FHD\play\parts\gauge\*.png"),
            "parts/gauge/blue.png"
        );
    }

    #[test]
    fn lr2_customfile_selection_accepts_legacy_basename_selection() {
        let root = unique_test_dir("bmz-lr2-customfile-basename");
        let play_dir = root.join("play");
        std::fs::create_dir_all(play_dir.join("parts/gauge")).unwrap();
        std::fs::write(play_dir.join("parts/gauge/default.png"), []).unwrap();
        std::fs::write(play_dir.join("parts/gauge/blue.png"), []).unwrap();
        let skin_path = play_dir.join("FHDPLAY_AC.lr2skin");
        std::fs::write(&skin_path, []).unwrap();
        let mut header = Header::default();
        header.files.push(CustomFile {
            name: "GAUGE COLOR".to_string(),
            path: "parts/gauge/*.png".to_string(),
            default: "default".to_string(),
        });
        let files = BTreeMap::from([("GAUGE COLOR".to_string(), "blue.png".to_string())]);
        let mut builder = CsvBuilder::new(&skin_path, header, &files);

        assert_eq!(
            builder.resolve_source_path(r".\LR2files\Theme\WMII_FHD\play\parts\gauge\*.png"),
            "parts/gauge/blue.png"
        );
    }

    #[test]
    fn lr2_customfile_selection_falls_back_when_saved_file_is_missing() {
        let root = unique_test_dir("bmz-lr2-customfile-missing");
        let play_dir = root.join("play");
        std::fs::create_dir_all(play_dir.join("parts/gauge")).unwrap();
        std::fs::write(play_dir.join("parts/gauge/default.png"), []).unwrap();
        let skin_path = play_dir.join("FHDPLAY_AC.lr2skin");
        std::fs::write(&skin_path, []).unwrap();
        let mut header = Header::default();
        header.files.push(CustomFile {
            name: "GAUGE COLOR".to_string(),
            path: "parts/gauge/*.png".to_string(),
            default: "default".to_string(),
        });
        let files =
            BTreeMap::from([("GAUGE COLOR".to_string(), "parts/gauge/missing.png".to_string())]);
        let mut builder = CsvBuilder::new(&skin_path, header, &files);

        assert_eq!(
            builder.resolve_source_path(r".\LR2files\Theme\WMII_FHD\play\parts\gauge\*.png"),
            "parts/gauge/default.png"
        );
    }

    #[test]
    fn processor_selects_default_custom_option_branch() {
        let mut ops = HashMap::new();
        ops.insert(900, true);
        ops.insert(901, false);
        let mut processor = Processor::new(ops);
        assert!(!processor.should_execute(&CsvLine {
            command: "IF".into(),
            fields: vec!["#IF".into(), "900".into()],
        }));
        assert!(processor.active());
        assert!(
            !processor.should_execute(&CsvLine {
                command: "ENDIF".into(),
                fields: vec!["#ENDIF".into()],
            })
        );
        assert!(processor.active());
    }

    #[test]
    fn processor_keeps_outer_false_branch_inactive_inside_true_nested_if() {
        let mut ops = HashMap::new();
        ops.insert(900, false);
        ops.insert(901, true);
        let mut processor = Processor::new(ops);
        assert!(!processor.should_execute(&CsvLine {
            command: "IF".into(),
            fields: vec!["#IF".into(), "900".into()],
        }));
        assert!(!processor.active());
        assert!(!processor.should_execute(&CsvLine {
            command: "IF".into(),
            fields: vec!["#IF".into(), "901".into()],
        }));
        assert!(!processor.active());
        assert!(
            !processor.should_execute(&CsvLine {
                command: "ENDIF".into(),
                fields: vec!["#ENDIF".into()],
            })
        );
        assert!(!processor.active());
        assert!(
            !processor.should_execute(&CsvLine {
                command: "ENDIF".into(),
                fields: vec!["#ENDIF".into()],
            })
        );
        assert!(processor.active());
    }

    #[test]
    fn processor_keeps_autoplay_conditions_as_runtime_ops() {
        let ops = HashMap::from([(32, true), (33, false)]);
        let mut processor = Processor::new(ops);
        assert!(!processor.should_execute(&CsvLine {
            command: "IF".into(),
            fields: vec!["#IF".into(), "33".into()],
        }));

        assert!(processor.active());
        assert_eq!(processor.active_runtime_ops(), vec![33]);
    }

    #[test]
    fn processor_converts_runtime_else_to_negated_op() {
        let mut processor = Processor::new(HashMap::new());
        assert!(!processor.should_execute(&parse_csv_line("#IF,41").unwrap()));
        assert_eq!(processor.active_runtime_ops(), vec![41]);
        assert!(!processor.should_execute(&parse_csv_line("#ELSE").unwrap()));
        assert_eq!(processor.active_runtime_ops(), vec![-41]);
        assert!(!processor.should_execute(&parse_csv_line("#ENDIF").unwrap()));
        assert!(processor.active_runtime_ops().is_empty());
    }

    #[test]
    fn processor_expands_conditional_setoption_to_its_runtime_source() {
        let path = Path::new("skin/play/test.lr2skin");
        let files = BTreeMap::new();
        let mut builder = CsvBuilder::new(path, Header::default(), &files);
        let lines = [
            parse_csv_line("#IF,41").unwrap(),
            parse_csv_line("#SETOPTION,982,1").unwrap(),
            parse_csv_line("#ENDIF").unwrap(),
            parse_csv_line("#SRC_BGA").unwrap(),
            parse_csv_line("#IF,982").unwrap(),
            parse_csv_line("#DST_BGA,0,0,0,10,20,30,40,0,255,255,255,255,0,0,0,0,0,0,0,0,0")
                .unwrap(),
            parse_csv_line("#ENDIF").unwrap(),
        ];
        let mut processor = Processor::new(HashMap::new());

        processor.process_lines(&lines, path, &mut builder).unwrap();

        assert_eq!(builder.destinations[0]["op"].as_array().unwrap(), &[json!(41)]);
    }

    #[test]
    fn processor_does_not_leak_setoption_inside_runtime_if() {
        let path = Path::new("skin/play/test.lr2skin");
        let files = BTreeMap::new();
        let mut builder = CsvBuilder::new(path, Header::default(), &files);
        let lines = [
            parse_csv_line("#IF,33").unwrap(),
            parse_csv_line("#SETOPTION,985,1").unwrap(),
            parse_csv_line("#ENDIF").unwrap(),
        ];
        let mut processor = Processor::new(HashMap::new());

        processor.process_lines(&lines, path, &mut builder).unwrap();

        assert!(!processor.ops.contains_key(&985));
        assert!(!builder.header.selected_ops.contains_key(&985));
    }

    #[test]
    fn processor_attaches_autoplay_runtime_op_to_destination() {
        let path = Path::new("skin/play/test.lr2skin");
        let files = BTreeMap::new();
        let mut builder = CsvBuilder::new(path, Header::default(), &files);
        let lines = [
            parse_csv_line("#IMAGE,parts/frame.png").unwrap(),
            parse_csv_line("#SRC_IMAGE,0,0,0,0,10,10,1,1,0,0").unwrap(),
            parse_csv_line("#IF,33").unwrap(),
            parse_csv_line("#DST_IMAGE,0,0,0,10,20,30,40,0,255,255,255,255,0,0,0,0,0,0,0,0,0")
                .unwrap(),
            parse_csv_line("#ENDIF").unwrap(),
        ];
        let mut processor = Processor::new(HashMap::new());

        processor.process_lines(&lines, path, &mut builder).unwrap();

        let op = builder.destinations[0]["op"].as_array().unwrap();
        assert_eq!(op, &[json!(33)]);
    }

    #[test]
    fn processor_keeps_autoplay_off_on_score_graph_destinations() {
        let path = Path::new("skin/play/test.lr2skin");
        let files = BTreeMap::new();
        let mut builder = CsvBuilder::new(path, Header::default(), &files);
        let lines = [
            parse_csv_line("#IMAGE,parts/frame.png").unwrap(),
            parse_csv_line("#SRC_IMAGE,0,0,0,0,10,10,1,1,0,0").unwrap(),
            parse_csv_line("#IF,32").unwrap(),
            parse_csv_line("#DST_IMAGE,0,0,0,10,20,30,40,0,255,255,255,255,0,0,0,0,0,0,39,0,0")
                .unwrap(),
            parse_csv_line("#ENDIF").unwrap(),
        ];
        let mut processor = Processor::new(HashMap::new());

        processor.process_lines(&lines, path, &mut builder).unwrap();

        let op = builder.destinations[0]["op"].as_array().unwrap();
        assert_eq!(op, &[json!(32), json!(39)]);
    }

    #[test]
    fn consecutive_lr2_destinations_merge_into_keyframes() {
        let path = Path::new("skin/play/test.lr2skin");
        let files = BTreeMap::new();
        let mut builder = CsvBuilder::new(path, Header::default(), &files);
        builder
            .execute(&CsvLine {
                command: "IMAGE".into(),
                fields: vec!["#IMAGE".into(), "parts/frame.png".into()],
            })
            .unwrap();
        builder
            .execute(&CsvLine {
                command: "SRC_IMAGE".into(),
                fields: vec![
                    "#SRC_IMAGE".into(),
                    "0".into(),
                    "0".into(),
                    "0".into(),
                    "0".into(),
                    "10".into(),
                    "20".into(),
                    "1".into(),
                    "1".into(),
                    "0".into(),
                    "0".into(),
                ],
            })
            .unwrap();
        builder
            .execute(&CsvLine {
                command: "DST_IMAGE".into(),
                fields: vec![
                    "#DST_IMAGE".into(),
                    "0".into(),
                    "0".into(),
                    "10".into(),
                    "20".into(),
                    "30".into(),
                    "40".into(),
                    "0".into(),
                    "0".into(),
                    "255".into(),
                    "255".into(),
                    "255".into(),
                    "1".into(),
                    "1".into(),
                    "0".into(),
                    "0".into(),
                    "500".into(),
                    "0".into(),
                    "41".into(),
                    "30".into(),
                    "0".into(),
                ],
            })
            .unwrap();
        builder
            .execute(&CsvLine {
                command: "DST_IMAGE".into(),
                fields: vec![
                    "#DST_IMAGE".into(),
                    "0".into(),
                    "500".into(),
                    "10".into(),
                    "20".into(),
                    "30".into(),
                    "40".into(),
                    "0".into(),
                    "255".into(),
                    "255".into(),
                    "255".into(),
                    "255".into(),
                    "1".into(),
                    "1".into(),
                ],
            })
            .unwrap();

        assert_eq!(builder.destinations.len(), 1);
        let frames = builder.destinations[0].get("dst").and_then(JsonValue::as_array).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0]["a"], 0);
        assert_eq!(frames[1]["a"], 255);
        assert_eq!(builder.destinations[0]["loop"], 500);
    }

    #[test]
    fn lr2_note_destination_uses_lane_region_height() {
        let mut values = [0; 22];
        values[2] = 0;
        values[3] = 75;
        values[4] = 704;
        values[5] = 90;
        values[6] = 27;

        let frame = note_destination_frame(&values, 1080);

        assert_eq!(frame["x"], 75);
        assert_eq!(frame["y"], 349);
        assert_eq!(frame["w"], 90);
        assert_eq!(frame["h"], 731);
    }

    #[test]
    fn lr2_gauge_destination_uses_additive_part_span() {
        let mut values = [0; 22];
        values[2] = 1400;
        values[3] = 54;
        values[4] = 897;
        values[5] = 8;
        values[6] = 28;
        values[8] = 255;

        let destination = gauge_destination_def("gauge", &values, 1080, 9, 0, &[]);
        let frame = destination["dst"].as_array().unwrap().first().unwrap();

        assert_eq!(frame["x"], 54);
        assert_eq!(frame["y"], 155);
        assert_eq!(frame["w"], 450);
        assert_eq!(frame["h"], 28);
    }

    #[test]
    fn lr2_gauge_destination_preserves_negative_additive_direction() {
        let mut values = [0; 22];
        values[2] = 1400;
        values[3] = 54;
        values[4] = 897;
        values[5] = 8;
        values[6] = 28;
        values[8] = 255;

        let destination = gauge_destination_def("gauge", &values, 1080, -9, 0, &[]);
        let frame = destination["dst"].as_array().unwrap().first().unwrap();

        assert_eq!(frame["x"], 63);
        assert_eq!(frame["y"], 155);
        assert_eq!(frame["w"], -450);
        assert_eq!(frame["h"], 28);
    }

    #[test]
    fn lr2_gauge_omitted_parts_uses_beatoraja_animation_defaults() {
        let files = BTreeMap::new();
        let skin_path = unique_test_dir("bmz-lr2-gauge-defaults").join("play.lr2skin");
        let header = Header { skin_type: 2, ..Header::default() };
        let mut builder = CsvBuilder::new(&skin_path, header, &files);
        builder
            .execute(&parse_csv_line("#IMAGE,gauge.png").expect("valid IMAGE"))
            .expect("IMAGE should load");
        builder
            .execute(
                &parse_csv_line("#SRC_GROOVEGAUGE,0,0,0,0,32,28,4,1,0,0,9,0,,,,,,,,")
                    .expect("valid SRC_GROOVEGAUGE"),
            )
            .expect("SRC_GROOVEGAUGE should load");

        let gauge = builder.gauges.first().expect("gauge should be created");
        assert_eq!(gauge["parts"], json!(50));
        assert_eq!(gauge["type"], json!(0));
        assert_eq!(gauge["range"], json!(3));
        assert_eq!(gauge["cycle"], json!(33));
    }

    #[test]
    fn lr2_gauge_nodes_expand_standard_cells_to_beatoraja_slots() {
        let cells =
            ["red", "green", "back-red", "back-green"].map(|cell| cell.to_string()).to_vec();

        let nodes = lr2_gauge_nodes(&cells, 0, false);

        assert_eq!(nodes.len(), 36);
        assert_eq!(nodes[0], "red");
        assert_eq!(nodes[1], "green");
        assert_eq!(nodes[2], "back-red");
        assert_eq!(nodes[3], "back-green");
        assert_eq!(nodes[4], "red");
        assert_eq!(nodes[5], "green");
        assert_eq!(nodes[18], "red");
        assert_eq!(nodes[24], "red");
        assert_eq!(nodes[34], "red");
        assert_eq!(nodes[35], "green");
    }

    #[test]
    fn wmii_fhd_lr2skin_parse_has_no_unsupported_command_warnings_when_available() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !path.is_file() {
            return;
        }

        let loaded = load_lr2_csv_skin_value(&path, &BTreeMap::new(), &BTreeMap::new()).unwrap();
        assert!(
            loaded
                .warnings
                .iter()
                .all(|warning| !warning.message.contains("unsupported lr2 csv command")),
            "unexpected warnings: {:?}",
            loaded.warnings
        );
        assert!(
            loaded.warnings.iter().all(|warning| !warning.message.contains("source index 101")
                && !warning.message.contains("source index 110")
                && !warning.message.contains("source index 111")),
            "unexpected reference source warnings: {:?}",
            loaded.warnings
        );
        assert_eq!(loaded.value["name"], "WMII FHD play AC");
        assert!(loaded.value["destination"].as_array().unwrap().len() > 100);
        assert!(!loaded.value["note"]["group"].as_array().unwrap().is_empty());
    }

    #[test]
    fn wmii_fhd_lr2skin_dp_keeps_internal_setoption_ops_when_available() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC_DP.lr2skin");
        if !path.is_file() {
            return;
        }

        let options = BTreeMap::from([
            ("Displayjudge".to_string(), "ON".to_string()),
            ("GRAPH SIDE".to_string(), "RIGHT".to_string()),
            ("Score Graph".to_string(), "On".to_string()),
        ]);
        let loaded = load_lr2_csv_skin_value(&path, &options, &BTreeMap::new()).unwrap();

        assert!(
            loaded.internal_enabled_options.contains(&983),
            "expected WMII DP judge detail right-side op983 to be kept internally"
        );
        assert!(
            !loaded.internal_enabled_options.contains(&980),
            "custom property option 980 should remain user-selectable instead of internal"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_dp_uses_default_gauge_animation_when_available() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC_DP.lr2skin");
        if !path.is_file() {
            return;
        }

        let loaded = load_lr2_csv_skin_value(&path, &BTreeMap::new(), &BTreeMap::new()).unwrap();
        let gauges = loaded.value["gauges"].as_array().expect("gauges array");

        assert!(!gauges.is_empty(), "expected WMII DP gauge objects");
        for gauge in gauges {
            assert_eq!(gauge["type"], json!(0));
            assert_eq!(gauge["range"], json!(3));
            assert_eq!(gauge["cycle"], json!(33));
        }
    }

    #[test]
    fn wmii_fhd_lr2skin_keeps_gauge_sources_separate_when_available() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !path.is_file() {
            return;
        }

        let loaded = load_lr2_csv_skin_value(&path, &BTreeMap::new(), &BTreeMap::new()).unwrap();
        let gauges = loaded.value["gauges"].as_array().expect("gauges array");

        assert!(gauges.len() >= 4, "expected WMII gauge objects, got {gauges:?}");
        for gauge in gauges.iter().take(4) {
            let nodes = gauge["nodes"].as_array().unwrap();
            assert_eq!(nodes.len(), 36);
        }
        assert_ne!(gauges[0]["id"], gauges[1]["id"]);
    }
}
