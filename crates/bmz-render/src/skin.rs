use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, VecDeque, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use bmz_core::input::InputKind;
use bmz_core::judge::{Judge, TimingSide};
use bmz_core::lane::{KeyMode, LANE_COUNT, Lane};
use bmz_gameplay::session::{SkinRuntimeEvent, SkinRuntimeEventKind};
use serde::Deserialize;

use crate::assets::load_png_rgba;
use crate::plan::{
    Color, DrawCommand, PLAY_BACKBMP_TEXTURE, Point, Rect, RectBatchCache, RectBatchCacheKey,
    RectCommand, SELECT_BANNER_TEXTURE, SELECT_STAGE_TEXTURE, TextAlign, TextCaret, TextLayer,
    TextOutline, TextOverflow, TextShadow, TextStyle, TextureId, UvRect,
};
use crate::scene::{
    CourseConstraintFlags, CourseResultSkinSnapshot, DailyPlayerStatsSnapshot, PlayerStatsSnapshot,
    ResultGradeDiffDisplay, SelectRowKind, SelectRowSnapshot, SelectSnapshot,
};
use crate::skin_offset::{SKIN_OFFSET_BAR_LINE, SkinOffsetValues};
use crate::snapshot::{CourseStageMarker, DisplayJudgeCounts, LongBodyState};
use bmz_chart::model::LongNoteMode;

pub use bmz_skin_document::*;

mod condition;
mod document_render;
mod runtime;
mod select_state;
#[path = "skin/state_values/charts.rs"]
mod state_value_charts;
#[path = "skin/state_values/graph.rs"]
mod state_value_graph;
#[path = "skin/state_values/image.rs"]
mod state_value_image;
#[path = "skin/state_values/number.rs"]
mod state_value_number;
#[path = "skin/state_values/text.rs"]
mod state_value_text;
#[path = "skin/state_values/text_state.rs"]
mod state_value_text_state;
#[path = "skin/state_values/timer.rs"]
mod state_value_timer;

pub use condition::test_skin_ops;
use condition::*;
pub use document_render::SkinDocumentRenderExt;
use runtime::*;
pub use runtime::{
    DynamicTimerRuntime, JudgeRegionState, MAX_JUDGE_REGIONS, SkinClickHit, SkinClickTarget,
    SkinContext, SkinDrawState, SkinLuaDrawRuntime, SkinLuaRuntimeContext, SkinSliderHit,
    build_judge_region_state, lane_judge_region,
};
use select_state::*;
use state_value_charts::*;
use state_value_graph::*;
use state_value_image::*;
pub(crate) use state_value_number::result_grade_diff_label;
use state_value_number::*;
pub use state_value_number::{duration_to_green_number_ms, green_duration_to_duration_i32};
use state_value_text::*;
use state_value_text_state::*;
pub use state_value_text_state::{
    format_rm_skin_course_table_text, lua_main_state_event_index, lua_main_state_float,
    lua_main_state_number, lua_main_state_option, lua_main_state_timer,
};
pub use state_value_timer::skin_start_input_elapsed_ms;
use state_value_timer::*;

const OFFSET_ALL: i32 = 10;
const OFFSET_NOTES_1P: i32 = 30;
/// beatoraja の `SkinProperty.OFFSET_JUDGE_1P`。判定文字とコンボ数の destination が
/// `offsets: [32]` で参照する。コード本体では明示注入せず destination の `offsets`
/// 経由で適用する (テスト・ドキュメント用に定数だけ保持)。
#[allow(dead_code)]
const OFFSET_JUDGE_1P: i32 = 32;
const OFFSET_JUDGEDETAIL_1P: i32 = 33;

#[derive(Debug, Clone, PartialEq)]
pub struct SkinObject {
    pub id: SkinObjectId,
    pub source: SkinSource,
    pub placements: Vec<SkinPlacement>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkinDefinition {
    pub objects: Vec<SkinObject>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkinDocumentTexture {
    pub source_id: String,
    pub texture: SkinTextureId,
    pub source_size: SkinImageSize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinBgaFrame {
    pub texture: SkinTextureId,
    pub source_size: SkinImageSize,
    pub tint_r: f32,
    pub tint_g: f32,
    pub tint_b: f32,
    pub tint_a: f32,
    /// 動画 BGA フレームかどうか。Layer/Layer2 でも動画ならクロマキーを適用しない
    /// (beatoraja の `ffmpeg.frag` 相当)。
    pub is_video: bool,
}

impl SkinBgaFrame {
    pub fn opaque(texture: SkinTextureId, source_size: SkinImageSize) -> Self {
        Self {
            texture,
            source_size,
            tint_r: 1.0,
            tint_g: 1.0,
            tint_b: 1.0,
            tint_a: 1.0,
            is_video: false,
        }
    }
}

static DEFAULT_RESULT_IR_SNAPSHOT: crate::scene::ResultIrSnapshot =
    crate::scene::ResultIrSnapshot::EMPTY;

#[derive(Debug, Clone, PartialEq)]
pub struct SkinTextState<'a> {
    pub title: &'a str,
    /// 現在プロフィール名 (STRING_PLAYER=2)。
    pub player_name: &'a str,
    /// IR ライバル名 (STRING_RIVAL=1)。未取得なら空。
    pub rival: &'a str,
    pub subtitle: &'a str,
    pub artist: &'a str,
    pub subartist: &'a str,
    pub genre: &'a str,
    pub difficulty_name: &'a str,
    pub play_level: &'a str,
    pub grade_diff: &'a str,
    pub target: &'a str,
    pub select_arrange: &'a str,
    pub select_arrange_2p: &'a str,
    pub select_gauge: &'a str,
    pub select_gauge_auto_shift: &'a str,
    pub select_bottom_shiftable_gauge: &'a str,
    pub select_double_option: &'a str,
    pub select_hs_fix: &'a str,
    pub select_assist: &'a str,
    pub select_mode: &'a str,
    pub select_sort: &'a str,
    pub select_ln_mode: &'a str,
    pub select_bga: &'a str,
    pub select_judge_timing_auto_adjust: &'a str,
    pub current_folder: &'a str,
    pub bar_text: &'a str,
    pub table_level: &'a str,
    pub table_text_primary: &'a str,
    pub table_text_secondary: &'a str,
    pub table_text_fallback: &'a str,
    pub course_stage: Option<CourseStageMarker>,
    pub course_titles: [&'a str; 10],
    /// beatoraja `SkinProperty.STRING_SEARCHWORD` (`ref=30`). Current song search
    /// query as typed by the user.
    pub search_word: &'a str,
    /// Multiplier applied to the rendered alpha of the `ref=30` text element.
    /// `1.0` keeps the skin-defined alpha unchanged; values < 1.0 are used for
    /// placeholder / inactive states (beatoraja `messageFontColor=GRAY` 相当).
    pub search_word_alpha: f32,
    /// Optional caret position for `search_word`, expressed as a UTF-8 byte index.
    pub search_caret_byte_index: Option<usize>,
    pub ir_ranking: &'a crate::scene::ResultIrSnapshot,
}

impl<'a> Default for SkinTextState<'a> {
    fn default() -> Self {
        Self {
            title: "",
            player_name: "",
            subtitle: "",
            artist: "",
            subartist: "",
            genre: "",
            difficulty_name: "",
            play_level: "",
            grade_diff: "",
            target: "",
            select_arrange: "",
            select_arrange_2p: "",
            select_gauge: "",
            select_gauge_auto_shift: "",
            select_bottom_shiftable_gauge: "",
            select_double_option: "",
            select_hs_fix: "",
            select_assist: "",
            select_mode: "",
            select_sort: "",
            select_ln_mode: "",
            select_bga: "",
            select_judge_timing_auto_adjust: "",
            current_folder: "",
            bar_text: "",
            table_level: "",
            table_text_primary: "",
            table_text_secondary: "",
            table_text_fallback: "",
            course_stage: None,
            course_titles: [""; 10],
            search_word: "",
            rival: "",
            search_word_alpha: 1.0,
            search_caret_byte_index: None,
            ir_ranking: &DEFAULT_RESULT_IR_SNAPSHOT,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct SkinManifest {
    #[serde(default)]
    pub textures: Vec<SkinTextureManifest>,
    #[serde(default)]
    pub play: SkinPlayManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SkinTextureManifest {
    pub id: u32,
    pub path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct SkinPlayManifest {
    pub note: Option<SkinImageManifest>,
    pub ln_start: Option<SkinImageManifest>,
    pub ln_end: Option<SkinImageManifest>,
    pub receptor: Option<SkinImageManifest>,
    pub judge_line: Option<SkinImageManifest>,
    pub gauge_frame: Option<SkinImageManifest>,
    pub gauge_fill: Option<SkinImageManifest>,
    pub combo_panel: Option<SkinImageManifest>,
    pub combo_panel_inactive: Option<SkinImageManifest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct SkinImageManifest {
    pub texture: u32,
    pub key_even_texture: Option<u32>,
    pub scratch_texture: Option<u32>,
    pub source_size: Option<SkinImageSize>,
    #[serde(default)]
    pub uv: TextureRegion,
    #[serde(default)]
    pub scale: SkinImageScale,
    pub border: Option<SkinImageBorder>,
}

impl SkinImageManifest {
    pub fn texture_for_lane(self, lane: Lane) -> u32 {
        match lane {
            Lane::Scratch | Lane::Scratch2 => self.scratch_texture.unwrap_or(self.texture),
            Lane::Key2 | Lane::Key4 | Lane::Key6 | Lane::Key9 | Lane::Key11 | Lane::Key13 => {
                self.key_even_texture.unwrap_or(self.texture)
            }
            Lane::Key1
            | Lane::Key3
            | Lane::Key5
            | Lane::Key7
            | Lane::Key8
            | Lane::Key10
            | Lane::Key12
            | Lane::Key14 => self.texture,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct SkinImageSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkinImageScale {
    #[default]
    Stretch,
    NineSlice,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct SkinImageBorder {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
    #[serde(default)]
    pub unit: SkinImageBorderUnit,
}

impl SkinImageBorder {
    fn normalized(self, source_size: Option<SkinImageSize>) -> Option<Self> {
        match self.unit {
            SkinImageBorderUnit::Normalized => Some(self),
            SkinImageBorderUnit::Pixels => {
                let size = source_size?;
                if size.width <= 0.0 || size.height <= 0.0 {
                    return None;
                }
                Some(Self {
                    left: self.left / size.width,
                    right: self.right / size.width,
                    top: self.top / size.height,
                    bottom: self.bottom / size.height,
                    unit: SkinImageBorderUnit::Normalized,
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkinImageBorderUnit {
    #[default]
    Normalized,
    Pixels,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSkinTexture {
    pub id: TextureId,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkinRenderContext<'a> {
    pub phase: SkinPhase,
    pub elapsed_ms: i32,
    pub text: &'a [(TextSlot, String)],
    pub numbers: &'a [(NumberSlot, i64)],
}

#[derive(Debug, Clone, PartialEq)]
pub enum SkinSource {
    Image { texture: SkinTextureId, uv: TextureRegion },
    Text { slot: TextSlot, style: TextStyle },
    Number { slot: NumberSlot, style: TextStyle, digits: u8 },
    Rect { color: Color },
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct TextureRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for TextureRegion {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, width: 1.0, height: 1.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSlot {
    Title,
    Artist,
    Judge,
    ClearType,
    ReplayState,
    Custom(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberSlot {
    Score,
    ExScore,
    Combo,
    MaxCombo,
    Gauge,
    Hispeed,
    JudgeCount,
    Custom(u16),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkinPlacement {
    pub phase: SkinPhase,
    pub time_ms: i32,
    pub rect: Rect,
    pub alpha: f32,
    pub blend: BlendMode,
    pub animation: Animation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinPhase {
    Select,
    Play,
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Add,
    /// 透明な render target へ通常 alpha 合成済みの offscreen texture 用。
    /// RGB は premultiplied 済みなので、再合成時に source alpha を掛け直さない。
    Premultiplied,
    /// BGA Layer/Layer2 の黒クロマキー描画。
    /// beatoraja の `layer.frag` 相当: RGB(0,0,0) ピクセルを α=0 として描画する。
    LayerMask,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    pub keyframes: Vec<Keyframe>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe {
    pub time_ms: i32,
    pub rect: Rect,
    pub alpha: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SkinRenderItem {
    Image {
        texture: SkinTextureId,
        rect: Rect,
        uv: TextureRegion,
        tint: Color,
        blend: BlendMode,
        scale: SkinImageScale,
        border: Option<SkinImageBorder>,
        source_size: Option<SkinImageSize>,
        linear_filter: bool,
    },
    RotatedImage {
        texture: SkinTextureId,
        rect: Rect,
        uv: TextureRegion,
        tint: Color,
        blend: BlendMode,
        source_size: Option<SkinImageSize>,
        linear_filter: bool,
        angle_deg: f32,
        center: Point,
    },
    Text {
        origin: Point,
        text: String,
        style: TextStyle,
        caret: Option<TextCaret>,
        blend: BlendMode,
    },
    Rect {
        rect: Rect,
        color: Color,
        blend: BlendMode,
    },
    RectBatch {
        rects: Arc<[RectCommand]>,
        cache: Option<RectBatchCache>,
    },
}

impl SkinObject {
    pub fn resolve(
        &self,
        phase: SkinPhase,
        elapsed_ms: i32,
        text: impl Fn(TextSlot) -> String,
        number: impl Fn(NumberSlot) -> i64,
    ) -> Vec<SkinRenderItem> {
        self.placements
            .iter()
            .filter(|placement| placement.phase == phase)
            .map(|placement| {
                let resolved = placement.resolve(elapsed_ms);
                match &self.source {
                    SkinSource::Image { texture, uv } => SkinRenderItem::Image {
                        texture: *texture,
                        rect: resolved.rect,
                        uv: *uv,
                        tint: Color::rgba(1.0, 1.0, 1.0, resolved.alpha),
                        blend: resolved.blend,
                        scale: SkinImageScale::Stretch,
                        border: None,
                        source_size: None,
                        linear_filter: false,
                    },
                    SkinSource::Text { slot, style } => SkinRenderItem::Text {
                        origin: Point { x: resolved.rect.x, y: resolved.rect.y },
                        text: text(*slot),
                        style: style.clone().with_alpha(resolved.alpha),
                        caret: None,
                        blend: resolved.blend,
                    },
                    SkinSource::Number { slot, style, digits } => SkinRenderItem::Text {
                        origin: Point { x: resolved.rect.x, y: resolved.rect.y },
                        text: format_number(number(*slot), *digits),
                        style: style.clone().with_alpha(resolved.alpha),
                        caret: None,
                        blend: resolved.blend,
                    },
                    SkinSource::Rect { color } => SkinRenderItem::Rect {
                        rect: resolved.rect,
                        color: color.with_alpha(color.a * resolved.alpha),
                        blend: resolved.blend,
                    },
                }
            })
            .collect()
    }
}

impl SkinDefinition {
    pub fn resolve(&self, context: &SkinRenderContext<'_>) -> Vec<SkinRenderItem> {
        self.objects
            .iter()
            .flat_map(|object| {
                object.resolve(
                    context.phase,
                    context.elapsed_ms,
                    |slot| lookup_text(context.text, slot),
                    |slot| lookup_number(context.numbers, slot),
                )
            })
            .collect()
    }
}

/// beatoraja スキンの `note` 配列インデックスをキーモードに応じて返す。
/// スキン側の並び順: 1P [Key1..KeyN, Scratch], 2P [Key(N+1)..Key(2N), Scratch2]
fn beatoraja_note_index(lane: Lane, key_mode: KeyMode) -> usize {
    match key_mode {
        KeyMode::K5 => match lane {
            Lane::Key1 => 0,
            Lane::Key2 => 1,
            Lane::Key3 => 2,
            Lane::Key4 => 3,
            Lane::Key5 => 4,
            _ => 5, // Scratch
        },
        KeyMode::K7 => match lane {
            Lane::Key1 => 0,
            Lane::Key2 => 1,
            Lane::Key3 => 2,
            Lane::Key4 => 3,
            Lane::Key5 => 4,
            Lane::Key6 => 5,
            Lane::Key7 => 6,
            _ => 7, // Scratch
        },
        KeyMode::K6 => match lane {
            Lane::Key1 => 0,
            Lane::Key2 => 1,
            Lane::Key3 => 2,
            Lane::Key4 => 3,
            Lane::Key5 => 4,
            Lane::Key6 => 5,
            _ => 5,
        },
        KeyMode::K4 => match lane {
            Lane::Key1 => 0,
            Lane::Key2 => 1,
            Lane::Key3 => 2,
            Lane::Key4 => 3,
            _ => 3,
        },
        KeyMode::K10 => match lane {
            Lane::Key1 => 0,
            Lane::Key2 => 1,
            Lane::Key3 => 2,
            Lane::Key4 => 3,
            Lane::Key5 => 4,
            Lane::Scratch => 5,
            Lane::Key8 => 6,
            Lane::Key9 => 7,
            Lane::Key10 => 8,
            Lane::Key11 => 9,
            Lane::Key12 => 10,
            _ => 11, // Scratch2
        },
        KeyMode::K14 => match lane {
            Lane::Key1 => 0,
            Lane::Key2 => 1,
            Lane::Key3 => 2,
            Lane::Key4 => 3,
            Lane::Key5 => 4,
            Lane::Key6 => 5,
            Lane::Key7 => 6,
            Lane::Scratch => 7,
            Lane::Key8 => 8,
            Lane::Key9 => 9,
            Lane::Key10 => 10,
            Lane::Key11 => 11,
            Lane::Key12 => 12,
            Lane::Key13 => 13,
            Lane::Key14 => 14,
            _ => 15, // Scratch2
        },
        KeyMode::K9 => match lane {
            Lane::Key1 => 0,
            Lane::Key2 => 1,
            Lane::Key3 => 2,
            Lane::Key4 => 3,
            Lane::Key5 => 4,
            Lane::Key6 => 5,
            Lane::Key7 => 6,
            Lane::Key8 => 7,
            Lane::Key9 => 8,
            _ => 8,
        },
        KeyMode::K8 => match lane {
            Lane::Key1 => 0,
            Lane::Key2 => 1,
            Lane::Key3 => 2,
            Lane::Key4 => 3,
            Lane::Key5 => 4,
            Lane::Key6 => 5,
            Lane::Key7 => 6,
            Lane::Key8 => 7,
            _ => 0,
        },
    }
}

fn imageset_ref_lane(ref_id: i32) -> Option<Lane> {
    match ref_id {
        500 => Some(Lane::Scratch),
        501 => Some(Lane::Key1),
        502 => Some(Lane::Key2),
        503 => Some(Lane::Key3),
        504 => Some(Lane::Key4),
        505 => Some(Lane::Key5),
        506 => Some(Lane::Key6),
        507 => Some(Lane::Key7),
        _ => None,
    }
}

/// beatoraja `getImageIndexProperty` (IndexType) 相当。image / imageset の ref 専用で、
/// value の `skin_state_number` (ValueType) とは ID が重なっても別解決する。
fn skin_image_index_number(ref_id: i32, state: &SkinDrawState) -> Option<i64> {
    match ref_id {
        11 if state.select_screen => Some(state.select_mode_index as i64),
        12 if state.select_screen => Some(state.select_sort_index as i64),
        // beatoraja's `gaugetype_1p` image index is state-dependent: MusicSelector
        // reads the configured gauge, while Play/Result read the gauge that is
        // actually active after gauge auto shift has been applied.
        40 if state.select_screen => Some(state.select_gauge_index as i64),
        40 => Some(state.gauge_type.max(0) as i64),
        SKIN_REF_PLAY_GAUGE_TYPE => Some(state.gauge_type.max(0) as i64),
        // Target sprites use both the legacy ref=41 and BUTTON_TARGET=77.
        // They share beatoraja's 11-entry target-list index rather than BMZ's
        // compact target enumeration.
        41 | 77 => Some(state.select_target_index as i64),
        42 => Some(arrange_ref_index(state) as i64),
        43 => Some(arrange_2p_ref_index(state) as i64),
        54 => Some(state.select_double_option_index as i64),
        55 => Some(state.select_hs_fix_index as i64),
        72 => Some(state.select_bga_index as i64),
        75 => Some(i64::from(state.judge_timing_auto_adjust)),
        78 => Some(state.select_gauge_auto_shift_index as i64),
        89 if state.select_screen && select_chart_metadata_available(state) => {
            Some(i64::from(state.select_favorite_song))
        }
        90 if state.select_screen && select_chart_metadata_available(state) => {
            Some(i64::from(state.select_favorite_chart))
        }
        90 if state.result_favorite_chart.is_some() => {
            Some(i64::from(state.result_favorite_chart.unwrap_or(false)))
        }
        301..=307 => Some(0),
        308 if state.result_ln_mode_index.is_some() => {
            Some(state.result_ln_mode_index.unwrap_or_default() as i64)
        }
        308 => Some(state.select_ln_mode_index as i64),
        330 => Some(i64::from(state.lanecover_enabled)),
        331 => Some(i64::from(state.lift_enabled)),
        332 => Some(i64::from(state.hidden_enabled)),
        340 => Some(state.select_judge_algorithm_index as i64),
        341 => Some(state.select_bottom_shiftable_gauge_index as i64),
        342 => Some(i64::from(state.hispeed_auto_adjust)),
        344 => Some(extended_arrange_ref_index(state) as i64),
        345 => Some(extended_arrange_2p_ref_index(state) as i64),
        321..=324 => {
            let slot = (ref_id - 321) as usize;
            Some(state.select_replay_slot_rule_indices[slot])
        }
        // extranotedepth / minemode / scrollmode / longnotemode / seventonine / constant 等は
        // profile 連携前のため 0 固定。value ref 350-353/360-361/400 とは衝突しない。
        350..=353 | 360..=361 | 400 | 343 => Some(0),
        370 if state.select_screen || state.result_failed.is_some() => {
            Some(state.select_clear_index)
        }
        371 if state.result_failed.is_some() => result_mybest_clear_index_display(state),
        371 => state.target_clear_index,
        // beatoraja's image/index property table is separate from numeric values:
        // value 390..399 is ranking position, while image 390..399 is clear type.
        390..=399 => {
            ir_ranking_entry(&state.ir_ranking, ref_id - 390).and_then(|entry| entry.clear_index)
        }
        ref_id if random_lane_ref_slot(ref_id).is_some() => {
            skin_random_lane_ref_number(ref_id, state)
        }
        _ => None,
    }
}

fn skin_state_imageset_index(ref_id: i32, state: &SkinDrawState) -> Option<usize> {
    skin_image_index_number(ref_id, state).map(|value| value.max(0) as usize)
}

/// imageset の画像を判定インデックス (0=PGREAT..4=POOR,5=MISS) で選ぶ。
/// 2枚構成 (通常/PGREAT) は PGREAT 判定でのみ2枚目を使う。
fn imageset_image_for_index(
    imageset: &SkinImageSetDef,
    judge_index: Option<usize>,
) -> Option<String> {
    let len = imageset.images.len();
    if len == 0 {
        return None;
    }
    let index = if len == 2 {
        usize::from(judge_index == Some(0))
    } else {
        judge_index.unwrap_or(0).min(len - 1)
    };
    imageset.images.get(index).cloned()
}

pub(crate) fn judge_image_index(judge: &str) -> Option<usize> {
    let judge = judge.trim();
    if judge.starts_with("PGREAT") {
        Some(0)
    } else if judge.starts_with("GREAT") {
        Some(1)
    } else if judge.starts_with("GOOD") {
        Some(2)
    } else if judge.starts_with("BAD") {
        Some(3)
    } else if judge.starts_with("POOR") {
        Some(4)
    } else if judge.starts_with("EMPTY") {
        Some(5)
    } else {
        None
    }
}

pub(crate) fn judge_image_index_for_judge(judge: Judge) -> usize {
    match judge {
        Judge::PGreat => 0,
        Judge::Great => 1,
        Judge::Good => 2,
        Judge::Bad => 3,
        Judge::Poor => 4,
        Judge::EmptyPoor => 5,
    }
}

#[path = "skin/animation.rs"]
mod skin_animation;
#[path = "skin/gauge.rs"]
mod skin_gauge;
#[path = "skin/geometry.rs"]
mod skin_geometry;
#[path = "skin/interaction.rs"]
mod skin_interaction;
#[path = "skin/manifest.rs"]
mod skin_manifest;
#[path = "skin/value_helpers.rs"]
mod skin_value_helpers;

use skin_animation::*;
use skin_gauge::*;
use skin_geometry::*;
use skin_interaction::*;
pub use skin_manifest::*;
pub use skin_value_helpers::*;

#[cfg(test)]
#[path = "skin/tests.rs"]
mod tests;
