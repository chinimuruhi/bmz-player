use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use anyhow::{Context, Result};
use bmz_audio::ffmpeg_loader::FfmpegSampleLoader;
use bmz_audio::loader::SampleLoader;
use bmz_audio::sample::DecodedSample;
use bmz_core::lane::KeyMode;
use bmz_render::assets::{RgbaImageAsset, load_static_rgba_image};
use bmz_render::bitmap_font::{BitmapFont, load_bitmap_font};
use bmz_render::plan::TextureId;
use bmz_render::renderer::{GpuUploader, PreparedTexture, Renderer};
use bmz_render::skin::{
    DestinationListEntry, SkinContext, SkinDocument, SkinDocumentTexture, SkinDrawState,
    SkinFilepathDef, SkinImageSize, SkinLuaDrawRuntime, SkinManifest, SkinTextureId,
    default_skin_manifest_for_root, lua_main_state_event_index, lua_main_state_float,
    lua_main_state_number, lua_main_state_option, lua_main_state_timer,
};
use bmz_skin::{
    LuaLoadRuntimeState, LuaMainState, LuaSkinRuntime, SkinKind as DecodeSkinKind,
    SkinLoadDependencies, SkinLoadedFileDependency,
};
use rayon::prelude::*;

use crate::config::profile_config::{SkinConfig, SkinOffsetConfig};
use crate::paths::{AppPaths, resolve_app_paths};
use crate::select_options::SessionMode;

/// `SkinConfig` から key_mode に対応するプレイスキン path / options / files / offsets を借用する。
pub struct PlaySkinSelection<'a> {
    pub key_mode: KeyMode,
    pub path: &'a str,
    pub options: &'a BTreeMap<String, String>,
    pub files: &'a BTreeMap<String, String>,
    pub offsets: &'a [SkinOffsetConfig],
}

/// `SkinConfig` から key_mode に応じたプレイスキン設定の参照を取り出す。
pub fn play_skin_selection_for(skin: &SkinConfig, key_mode: KeyMode) -> PlaySkinSelection<'_> {
    match key_mode {
        KeyMode::K5 => PlaySkinSelection {
            key_mode,
            path: skin.play5.as_str(),
            options: &skin.play5_options,
            files: &skin.play5_files,
            offsets: &skin.play5_offsets,
        },
        KeyMode::K4 => PlaySkinSelection {
            key_mode,
            path: skin.play4.as_str(),
            options: &skin.play4_options,
            files: &skin.play4_files,
            offsets: &skin.play4_offsets,
        },
        KeyMode::K6 => PlaySkinSelection {
            key_mode,
            path: skin.play6.as_str(),
            options: &skin.play6_options,
            files: &skin.play6_files,
            offsets: &skin.play6_offsets,
        },
        KeyMode::K7 => PlaySkinSelection {
            key_mode,
            path: skin.play7.as_str(),
            options: &skin.play7_options,
            files: &skin.play7_files,
            offsets: &skin.play7_offsets,
        },
        KeyMode::K8 => PlaySkinSelection {
            key_mode,
            path: skin.play8.as_str(),
            options: &skin.play8_options,
            files: &skin.play8_files,
            offsets: &skin.play8_offsets,
        },
        KeyMode::K10 => PlaySkinSelection {
            key_mode,
            path: skin.play10.as_str(),
            options: &skin.play10_options,
            files: &skin.play10_files,
            offsets: &skin.play10_offsets,
        },
        KeyMode::K14 => PlaySkinSelection {
            key_mode,
            path: skin.play14.as_str(),
            options: &skin.play14_options,
            files: &skin.play14_files,
            offsets: &skin.play14_offsets,
        },
        KeyMode::K9 => PlaySkinSelection {
            key_mode,
            path: skin.play9.as_str(),
            options: &skin.play9_options,
            files: &skin.play9_files,
            offsets: &skin.play9_offsets,
        },
    }
}

/// Battle SessionMode 専用スロットを優先し、未設定なら従来の10K/14Kスロットへ
/// フォールバックする。
pub fn play_skin_selection_for_session(
    skin: &SkinConfig,
    key_mode: KeyMode,
    session_mode: SessionMode,
) -> PlaySkinSelection<'_> {
    if !session_mode.is_battle() {
        return play_skin_selection_for(skin, key_mode);
    }
    match key_mode {
        KeyMode::K10 if !skin.battle5.trim().is_empty() => PlaySkinSelection {
            key_mode,
            path: skin.battle5.as_str(),
            options: &skin.battle5_options,
            files: &skin.battle5_files,
            offsets: &skin.battle5_offsets,
        },
        KeyMode::K14 if !skin.battle7.trim().is_empty() => PlaySkinSelection {
            key_mode,
            path: skin.battle7.as_str(),
            options: &skin.battle7_options,
            files: &skin.battle7_files,
            offsets: &skin.battle7_offsets,
        },
        _ => play_skin_selection_for(skin, key_mode),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkinKind {
    Play,
    Select,
    Decide,
    Result,
}

impl SkinKind {
    fn first_texture_id(self) -> u32 {
        match self {
            SkinKind::Play => 10_000,
            SkinKind::Select => 20_000,
            SkinKind::Decide => 25_000,
            SkinKind::Result => 30_000,
        }
    }

    fn warn_missing_required_sources(self) -> bool {
        matches!(self, SkinKind::Play)
    }

    fn font_namespace(self) -> &'static str {
        match self {
            SkinKind::Play => "play",
            SkinKind::Select => "select",
            SkinKind::Decide => "decide",
            SkinKind::Result => "result",
        }
    }
}

pub fn default_skin_document_path_from_paths(app_paths: &AppPaths, kind: SkinKind) -> PathBuf {
    let file_name = match kind {
        SkinKind::Play => "play7.json",
        SkinKind::Select => "select.json",
        SkinKind::Decide => "decide.json",
        SkinKind::Result => "result.json",
    };
    default_skin_root_from_paths(app_paths).join(file_name)
}

pub fn default_play_skin_document_path_from_paths(
    app_paths: &AppPaths,
    key_mode: KeyMode,
) -> PathBuf {
    let file_name = match key_mode {
        KeyMode::K4 => "play4.json",
        KeyMode::K5 => "play5.json",
        KeyMode::K6 => "play6.json",
        KeyMode::K7 => "play7.json",
        KeyMode::K8 => "play8.json",
        KeyMode::K9 => "play9.json",
        KeyMode::K10 => "play10.json",
        KeyMode::K14 => "play14.json",
    };
    default_skin_root_from_paths(app_paths).join(file_name)
}

/// バックグラウンドスレッドでデコード可能な 1 スキンぶんの中間データ。
/// Renderer に触らず Send-safe な値だけを保持する。
pub struct DecodedSkin {
    pub kind: SkinKind,
    pub document: SkinDocument,
    pub lua_runtime: Option<LuaSkinRuntime>,
    pub fonts: Vec<DecodedFont>,
    pub sources: Vec<DecodedSource>,
    pub audio_assets: Vec<DecodedSkinAudio>,
    pub stats: SkinDecodeStats,
}

pub struct DecodedSkinAudio {
    pub path: String,
    pub sample: DecodedSample,
}

#[derive(Debug, Clone, Default)]
pub struct SkinDecodeStats {
    pub document_us: u64,
    pub document_cache_hits: usize,
    pub document_cache_misses: usize,
    pub document_cache_uncacheable: usize,
    pub document_cache_disabled: usize,
    pub font_count: usize,
    pub font_decode_us: u64,
    pub font_payload_skipped: usize,
    pub font_cache_hits: usize,
    pub font_cache_misses: usize,
    pub font_cache_uncacheable: usize,
    pub font_cache_disabled: usize,
    pub source_task_count: usize,
    pub source_decode_us: u64,
    pub builtin_source_count: usize,
    pub image_source_count: usize,
    pub video_source_count: usize,
    pub source_cache_hits: usize,
    pub source_cache_misses: usize,
    pub source_cache_uncacheable: usize,
    pub source_cache_disabled: usize,
    pub video_source_cache_hits: usize,
    pub video_source_cache_misses: usize,
    pub video_source_cache_uncacheable: usize,
    pub video_source_cache_disabled: usize,
    pub source_texture_cache_hits: usize,
    pub video_source_texture_cache_hits: usize,
    pub source_texture_cache_hit_bytes: usize,
    pub video_source_texture_cache_hit_bytes: usize,
    pub decoded_source_count: usize,
    pub decoded_source_bytes: usize,
}

pub struct DecodedFont {
    pub stored_id: String,
    pub path: PathBuf,
    pub data: Option<DecodedFontData>,
    pub cache_key: Option<SkinFontCacheKey>,
}

#[derive(Clone)]
pub enum DecodedFontData {
    Vector(Vec<u8>),
    Bitmap(BitmapFont),
}

pub struct DecodedSource {
    pub source_id: String,
    pub path: PathBuf,
    pub texture: SkinTextureId,
    pub asset: Option<RgbaImageAsset>,
    pub size: SkinImageSize,
    pub cache_key: Option<SkinSourceAssetCacheKey>,
    pub is_video: bool,
}

pub type SharedSkinSourceAssetCache = Arc<Mutex<SkinSourceAssetCache>>;
pub type SharedSkinDocumentCache = Arc<Mutex<SkinDocumentCache>>;
pub type SharedSkinFontCache = Arc<Mutex<SkinFontCache>>;
pub type SharedSkinGpuTextureCache = Arc<Mutex<SkinGpuTextureCache>>;

mod cache;
mod decode;
mod install;
mod path;

pub use cache::{
    CachedSkinGpuTexture, SkinDocumentCache, SkinFontCache, SkinFontCacheKey, SkinGpuTextureCache,
    SkinSourceAssetCache, SkinSourceAssetCacheKey,
};
pub(crate) use decode::enabled_options_from_selections;
pub use decode::{
    apply_beatoraja_decide_json_skin, apply_beatoraja_json_skin, apply_beatoraja_result_json_skin,
    apply_beatoraja_select_json_skin, apply_default_skin, apply_default_skin_from_paths,
    apply_skin_from_config, decode_beatoraja_skin, decode_beatoraja_skin_with_options,
    decode_beatoraja_skin_with_options_and_runtime_state,
    decode_beatoraja_skin_with_options_and_runtime_state_and_caches,
    decode_beatoraja_skin_with_options_and_runtime_state_and_source_cache, default_skin_root,
    default_skin_root_from_paths, is_decodable_skin_path, is_json_skin_path, is_lr2_skin_path,
    is_lua_skin_path, load_default_skin_into_renderer, load_default_skin_into_renderer_from_paths,
};
pub use install::{
    PreparedSource, SkinUploadStats, UploadedSkin, install_decoded_font, install_decoded_skin,
    install_decoded_source, set_decoded_skin_context, upload_decoded_skin,
    upload_decoded_skin_with_texture_cache,
};
pub(crate) use path::RANDOM_FILE_SELECTION;

use cache::*;
use decode::*;
use install::*;
use path::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::hint::black_box;
    use std::time::Instant;

    use bmz_core::time::TimeUs;
    use bmz_render::plan::{DrawCommand, DrawPlan};
    use bmz_render::renderer::Renderer;
    use bmz_render::scene::{AppSceneSnapshot, SelectRowSnapshot, SelectSnapshot};
    use bmz_render::skin::{
        DestinationListEntry, DynamicTimerRuntime, SkinContext, SkinDocumentRenderExt,
        SkinDocumentTexture, SkinDrawState, SkinImageSize, SkinManifest, SkinRenderItem,
        SkinTextState,
    };

    fn test_app_paths() -> AppPaths {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        AppPaths::from_dirs(data.clone(), data.clone(), data.join("cache"), data.join("logs"))
    }

    #[test]
    fn default_skin_root_contains_json_documents() {
        let root = default_skin_root();
        for file_name in ["select.json", "decide.json", "result.json", "play7.json"] {
            assert!(root.join(file_name).is_file(), "missing bundled default {file_name}");
        }
    }

    #[test]
    fn render_lua_main_state_reads_current_frame_skin_offset() {
        let mut state = SkinDrawState::default();
        state.skin_offsets.set(
            45,
            bmz_render::skin_offset::SkinOffsetValue { x: 1, y: 2, w: 3, h: 4, r: 5, a: -6 },
        );
        let text_values = BTreeMap::new();
        let provider =
            RenderLuaMainState { state: &state, enabled_options: &[], text_values: &text_values };

        assert_eq!(
            provider.offset(45),
            bmz_skin::LuaSkinOffsetValue { x: 1, y: 2, w: 3, h: 4, r: 5, a: -6 }
        );
        assert_eq!(provider.offset(46), bmz_skin::LuaSkinOffsetValue::default());
    }

    #[test]
    fn skin_audio_path_stays_inside_skin_root() {
        let root = unique_test_dir("bmz-skin-audio-path");
        fs::create_dir_all(root.join("parts")).unwrap();
        fs::write(root.join("parts/bgm.ogg"), []).unwrap();

        let resolved = resolve_skin_audio_path(&root, "parts/bgm.ogg").unwrap();
        assert_eq!(resolved.file_name().and_then(|name| name.to_str()), Some("bgm.ogg"));
        assert!(resolve_skin_audio_path(&root, "../outside.ogg").is_none());
        assert!(
            resolve_skin_audio_path(&root, root.join("parts/bgm.ogg").to_string_lossy().as_ref())
                .is_none()
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bundled_default_json_skin_documents_decode() {
        let app_paths = test_app_paths();
        for (kind, expected_type) in
            [(SkinKind::Select, 5), (SkinKind::Decide, 6), (SkinKind::Result, 7)]
        {
            let path = default_skin_document_path_from_paths(&app_paths, kind);
            let decoded = decode_beatoraja_skin(&path, kind)
                .unwrap_or_else(|error| panic!("failed to decode {}: {error:#}", path.display()));
            assert_eq!(decoded.document.skin_type, expected_type);
            assert!(!decoded.sources.is_empty(), "{} has no image sources", path.display());
        }

        for (key_mode, expected_type) in [
            (KeyMode::K4, 22),
            (KeyMode::K5, 1),
            (KeyMode::K6, 23),
            (KeyMode::K7, 0),
            (KeyMode::K8, 24),
            (KeyMode::K9, 4),
            (KeyMode::K10, 3),
            (KeyMode::K14, 2),
        ] {
            let path = default_play_skin_document_path_from_paths(&app_paths, key_mode);
            let decoded = decode_beatoraja_skin(&path, SkinKind::Play)
                .unwrap_or_else(|error| panic!("failed to decode {}: {error:#}", path.display()));
            assert_eq!(decoded.document.skin_type, expected_type);
            assert!(decoded.document.note.is_some(), "{} has no note definition", path.display());
            assert!(
                decoded.document.note.as_ref().is_some_and(|note| !note.group.is_empty()),
                "{} has no bar line group",
                path.display()
            );
            assert!(
                destination_ids(&decoded.document).contains("keybeam_img"),
                "{} has no keybeam destination",
                path.display()
            );
            assert!(!decoded.sources.is_empty(), "{} has no image sources", path.display());
        }
    }

    #[test]
    fn bundled_default_play_and_result_display_extended_arrange_labels() {
        let app_paths = test_app_paths();
        for (path, kind, ids) in [
            (
                default_play_skin_document_path_from_paths(&app_paths, KeyMode::K7),
                SkinKind::Play,
                [
                    ("play_arrange_1p_f", "1P F-RANDOM", "event_index(344) == 10"),
                    ("play_arrange_1p_mf", "1P MF-RANDOM", "event_index(344) == 11"),
                    ("play_arrange_2p_f", "2P F-RANDOM", "event_index(345) == 10"),
                    ("play_arrange_2p_mf", "2P MF-RANDOM", "event_index(345) == 11"),
                ],
            ),
            (
                default_skin_document_path_from_paths(&app_paths, SkinKind::Result),
                SkinKind::Result,
                [
                    ("result_arrange_1p_f", "1P F-RANDOM", "event_index(344) == 10"),
                    ("result_arrange_1p_mf", "1P MF-RANDOM", "event_index(344) == 11"),
                    ("result_arrange_2p_f", "2P F-RANDOM", "event_index(345) == 10"),
                    ("result_arrange_2p_mf", "2P MF-RANDOM", "event_index(345) == 11"),
                ],
            ),
        ] {
            let decoded = decode_beatoraja_skin(&path, kind)
                .unwrap_or_else(|error| panic!("failed to decode {}: {error:#}", path.display()));
            for (id, label, draw) in ids {
                assert!(
                    decoded
                        .document
                        .text
                        .iter()
                        .any(|text| text.id == id && text.constant_text == label),
                    "{} should decode {id} text",
                    path.display()
                );
                assert!(decoded.document.destination.iter().any(|entry| matches!(
                    entry,
                    DestinationListEntry::Single(destination)
                        if destination.id == id && destination.draw == draw
                )));
            }
        }
    }

    #[test]
    fn lua_compat_virtual_io_contains_only_sanitized_beatoraja_config() {
        let files = lua_compat_virtual_io_files();
        assert_eq!(files.len(), 2);

        let system: serde_json::Value =
            serde_json::from_str(&files["config_sys.json"]).expect("system config should be JSON");
        assert_eq!(system, serde_json::json!({ "playername": "bmz" }));

        let player: serde_json::Value =
            serde_json::from_str(&files["player/bmz/config_player.json"])
                .expect("player config should be JSON");
        let player = player.as_object().expect("player config should be an object");
        assert_eq!(
            player.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "mode5",
                "mode7",
                "mode9",
                "mode10",
                "mode14",
                "mode24",
                "mode24double"
            ])
        );
        for mode in player.values() {
            assert_eq!(mode["keyboard"], serde_json::json!({}));
            assert_eq!(mode["controller"], serde_json::json!([]));
            assert_eq!(mode["midi"], serde_json::json!({}));
        }
    }

    #[test]
    fn wmii_result_decodes_with_virtual_io_and_graph_default() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/result/result.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let options =
            BTreeMap::from([("Expand Panel".to_string(), "ON - GRAPH DEFAULT".to_string())]);
        let runtime_state = LuaLoadRuntimeState {
            number_values: BTreeMap::new(),
            text_values: BTreeMap::new(),
            option_values: BTreeMap::from([(51, true), (160, true)]),
            ..LuaLoadRuntimeState::default()
        };
        let loaded = load_skin_document_uncached(
            &skin_path,
            SkinKind::Result,
            &options,
            &BTreeMap::new(),
            &runtime_state,
        )
        .expect("unmodified WMII result should decode through the BMZ loader");

        assert_eq!(loaded.document.result_panel_default, Some(2));
        assert_eq!(
            loaded
                .document
                .image
                .iter()
                .find(|image| image.id == "BtnGraphData")
                .and_then(|image| image.act),
            Some(bmz_render::skin::SKIN_EVENT_RESULT_PANEL_GRAPH)
        );
        assert_eq!(
            loaded
                .document
                .image
                .iter()
                .find(|image| image.id == "BtnIrData")
                .and_then(|image| image.act),
            Some(bmz_render::skin::SKIN_EVENT_RESULT_PANEL_IR)
        );
        let favorite = loaded
            .document
            .image
            .iter()
            .find(|image| image.id == "favorite")
            .expect("WMII result favorite button should decode");
        assert_eq!(favorite.ref_id, 90);
        assert_eq!(favorite.act, Some(90));
        assert_eq!(favorite.divy, 3);
        assert!(loaded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination)
                if destination.draw.contains("result_panel(1)")
        )));
        assert!(loaded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination)
                if destination.draw.contains("result_panel(2)")
        )));
        let destinations = loaded
            .document
            .destination
            .iter()
            .filter_map(|entry| match entry {
                DestinationListEntry::Single(destination) => Some(destination),
                DestinationListEntry::Conditional { .. } => None,
            })
            .collect::<Vec<_>>();
        assert!(destinations.iter().any(|destination| destination.id == "randomButton1p"));
        let random_key = destinations
            .iter()
            .find(|destination| destination.id == "randomKeySet1P_1")
            .expect("7K Result should retain the RANDOM lane placement destinations");
        assert!(random_key.draw.contains("event_index(42)"));
        let rank_aaa = destinations
            .iter()
            .find(|destination| {
                destination.id == "rankBig_AAA" && destination.loop_time == Some(100)
            })
            .expect("rankBig_AAA should survive malformed op repair");
        assert_eq!(rank_aaa.op, [300, 920]);
        assert_eq!(rank_aaa.loop_time, Some(100));
        assert_eq!(rank_aaa.filter, 1);
        assert_eq!(rank_aaa.dst.len(), 2);
        for (id, rank) in [("AAA_BG", 300), ("AA_BG", 301), ("A_BG", 302)] {
            let backgrounds = destinations
                .iter()
                .filter(|destination| {
                    destination.id == id && matches!(destination.loop_time, Some(500 | 600 | 700))
                })
                .collect::<Vec<_>>();
            assert_eq!(backgrounds.len(), 3, "expected three {id} animations");
            assert!(backgrounds.iter().all(|destination| destination.op == [90, rank]));
        }
        let clear_backgrounds = destinations
            .iter()
            .filter(|destination| {
                destination.id == "clearBG"
                    && matches!(destination.loop_time, Some(500 | 600 | 700))
            })
            .collect::<Vec<_>>();
        assert_eq!(clear_backgrounds.len(), 3);
        assert!(clear_backgrounds.iter().all(|destination| destination.op == [90]));
        let expanded_timing_values = destinations
            .iter()
            .filter(|destination| {
                matches!(
                    destination.id.as_str(),
                    "timingAvg"
                        | "timingAvgAdot"
                        | "timingDotMS"
                        | "durationAvg"
                        | "durationAvgAdot"
                        | "stddav"
                        | "stddaAdot"
                ) && destination.dst.first().is_some_and(|entry| {
                    matches!(
                        entry,
                        bmz_render::skin::SkinDstEntry::Frame(frame)
                            if frame.x.is_some_and(|x| x >= 1_000)
                    )
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(expanded_timing_values.len(), 12);
        assert!(
            expanded_timing_values.iter().all(|destination| {
                destination.draw.contains("result_panel(2)")
                    && !destination.draw.contains("result_panel(0)")
                    && !destination.draw.contains("result_panel(1)")
            }),
            "expanded timing values must stay hidden on the IR panel: {:?}",
            expanded_timing_values
                .iter()
                .map(|destination| (destination.id.as_str(), destination.draw.as_str()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            loaded.dependencies.virtual_io_files.get("config_sys.json"),
            Some(&Some("{\"playername\":\"bmz\"}".to_string()))
        );
        assert!(loaded.dependencies.virtual_io_files.contains_key("player/bmz/config_player.json"));
    }

    #[test]
    fn wmii_course_result_uses_native_stage_titles_and_result_data() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/result/courseResult.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let runtime_state = LuaLoadRuntimeState {
            text_values: BTreeMap::from([
                (150, "Stage One".to_string()),
                (151, "Stage Two".to_string()),
                (152, "Stage Three".to_string()),
                (153, "Stage Four".to_string()),
            ]),
            option_values: BTreeMap::from([(160, true), (290, true)]),
            virtual_io_files: BTreeMap::from([(
                "skin/WMII_FHD/result/courseData.json".to_string(),
                serde_json::json!({
                    "songs": [
                        { "stage": 1, "score": 1000, "gauge": 80, "miss": 10, "rate": 0.5 },
                        { "stage": 2, "score": 2000, "gauge": 81, "miss": 11, "rate": 0.6 },
                        { "stage": 3, "score": 3000, "gauge": 82, "miss": 12, "rate": 0.7 },
                        { "stage": 4, "score": 3456, "gauge": 88, "miss": 13, "rate": 0.75 }
                    ]
                })
                .to_string(),
            )]),
            ..LuaLoadRuntimeState::default()
        };
        let loaded = load_skin_document_uncached(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &runtime_state,
        )
        .expect("unmodified WMII course result should decode with native stage data");

        for (id, expected) in
            [("stage_gauge4", "88"), ("stage_score4", "3456"), ("stage_miss4", "13")]
        {
            let value = loaded
                .document
                .value
                .iter()
                .find(|value| value.id == id)
                .unwrap_or_else(|| panic!("missing {id}"));
            assert_eq!(value.value_expr, expected, "unexpected {id} expression");
        }
        let graph = loaded
            .document
            .graph
            .iter()
            .find(|graph| graph.id == "stage_scoreGraph4")
            .expect("missing stage 4 score-rate graph");
        assert_eq!(graph.value_expr, "0.75");
        assert!(loaded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination) if destination.id == "courseTitle4"
        )));
        assert_eq!(
            loaded
                .document
                .value
                .iter()
                .find(|value| value.id == "courseClearRate")
                .map(|value| value.value_expr.as_str()),
            Some(bmz_render::skin::SKIN_EXPR_COURSE_CLEAR_RATE)
        );
        assert_eq!(
            loaded.dependencies.virtual_io_files.get("skin/WMII_FHD/result/courseData.json"),
            runtime_state
                .virtual_io_files
                .get("skin/WMII_FHD/result/courseData.json")
                .cloned()
                .map(Some)
                .as_ref()
        );
    }

    #[test]
    fn modern_chic_result_bakes_runtime_song_label_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/ModernChic/result.luaskin");
        if !skin_path.is_file() {
            return;
        }
        let runtime_state = LuaLoadRuntimeState {
            text_values: BTreeMap::from([
                (10, "Song".to_string()),
                (11, "Subtitle".to_string()),
                (12, "Song Subtitle".to_string()),
                (13, "Genre".to_string()),
                (14, "Artist".to_string()),
                (1003, "Table ★12".to_string()),
            ]),
            ..LuaLoadRuntimeState::default()
        };
        let loaded = load_skin_document_uncached(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &runtime_state,
        )
        .expect("unmodified ModernChic result should decode with runtime song text");
        let bottom = loaded
            .document
            .text
            .iter()
            .find(|text| text.id == "bottomResult")
            .expect("ModernChic bottomResult text");
        assert_eq!(bottom.constant_text, "Song Subtitle / Artist / Genre / Table ★12");
    }

    #[test]
    fn luxe_flat_result_decodes_local_panel_state_and_tab_actions() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Luxez-Flat/result/result.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let runtime_state = LuaLoadRuntimeState {
            option_values: BTreeMap::from([(50, false), (51, true)]),
            ..LuaLoadRuntimeState::default()
        };
        let loaded = load_skin_document_uncached(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &runtime_state,
        )
        .expect("unmodified Luxe Flat result should decode through the BMZ loader");

        assert_eq!(loaded.document.result_panel_default, Some(2));
        assert_eq!(
            loaded
                .document
                .image
                .iter()
                .find(|image| image.id == "result_modeselect_graph_data_off")
                .and_then(|image| image.act),
            Some(bmz_render::skin::SKIN_EVENT_RESULT_PANEL_GRAPH)
        );
        assert_eq!(
            loaded
                .document
                .image
                .iter()
                .find(|image| image.id == "result_modeselect_ir_ranking_off")
                .and_then(|image| image.act),
            Some(bmz_render::skin::SKIN_EVENT_RESULT_PANEL_IR)
        );
        assert!(loaded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination)
                if destination.draw.contains("result_panel(1)")
        )));
        assert!(loaded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination)
                if destination.draw.contains("result_panel(2)")
        )));
        assert_eq!(
            loaded
                .document
                .value
                .iter()
                .find(|value| value.id == "rank_diff_count")
                .map(|value| value.value_expr.as_str()),
            Some("bmz:nearest_rank_diff_abs")
        );
        assert_eq!(
            loaded
                .document
                .value
                .iter()
                .find(|value| value.id == "ir_scorerate1")
                .map(|value| value.value_expr.as_str()),
            Some("bmz:ir_score_rate_integer:1")
        );
        assert_eq!(
            loaded
                .document
                .value
                .iter()
                .find(|value| value.id == "ir_scorerate_dot1")
                .map(|value| value.value_expr.as_str()),
            Some("bmz:ir_score_rate_fraction:1")
        );
        assert!(loaded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination)
                if destination.id == "rank_diff_aaa_plus"
                    && destination.draw.contains("nearest_rank(AAA,plus)")
        )));
    }

    #[test]
    fn luxe_flat_result_displays_extended_arrange_labels_and_lane_pattern() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Luxez-Flat/result/result.luaskin");
        if !skin_path.is_file() {
            return;
        }
        let runtime_state = LuaLoadRuntimeState {
            event_index_values: BTreeMap::from([(42, 2), (43, 2), (344, 10), (345, 11)]),
            option_values: BTreeMap::from([(163, true)]),
            ..LuaLoadRuntimeState::default()
        };
        let loaded = load_skin_document_uncached(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &runtime_state,
        )
        .expect("Luxe Flat result should decode extended arrange labels");

        assert_eq!(
            loaded
                .document
                .text
                .iter()
                .find(|text| text.id == "lane_option")
                .map(|text| text.constant_text.as_str()),
            Some("F-RANDOM / MF-RANDOM")
        );
        assert!(loaded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination)
                if destination.id == "1key"
                    && destination.draw.contains("event_index(450) == 1")
        )));

        let course_skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Luxez-Flat/result/courseresult.luaskin");
        let course_loaded = load_skin_document_uncached(
            &course_skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &runtime_state,
        )
        .expect("Luxe Flat course result should decode extended arrange labels");
        assert_eq!(
            course_loaded
                .document
                .text
                .iter()
                .find(|text| text.id == "lane_option")
                .map(|text| text.constant_text.as_str()),
            Some("F-RANDOM / MF-RANDOM")
        );
    }

    #[test]
    fn wmii_result_renders_bmz_player_version_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/result/result.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::from([("Display Version".to_string(), "ON".to_string())]),
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(
            decoded.document.text.iter().any(|text| text.id == "version" && text.ref_id == 1010),
            "WMII version text should retain STRING_VERSION ref 1010"
        );
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let items = decoded.document.static_render_items(
            &sources,
            &SkinDrawState { elapsed_ms: 2_000, ..SkinDrawState::default() },
            &SkinTextState::default(),
        );

        assert!(items.iter().any(|item| matches!(
            item,
            SkinRenderItem::Text { text, .. }
                if text == &format!("bmz-player {}", env!("CARGO_PKG_VERSION"))
        )));
    }

    #[test]
    fn wmii_result_uses_runtime_combo_break_for_clear_animation() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/result/result.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let options =
            BTreeMap::from([("Expand Panel".to_string(), "ON - GRAPH DEFAULT".to_string())]);
        let load = |combo_break: i32| {
            load_skin_document_uncached(
                &skin_path,
                SkinKind::Result,
                &options,
                &BTreeMap::new(),
                &LuaLoadRuntimeState {
                    number_values: BTreeMap::from([(425, combo_break)]),
                    text_values: BTreeMap::new(),
                    option_values: BTreeMap::from([(51, true), (160, true)]),
                    ..LuaLoadRuntimeState::default()
                },
            )
            .expect("unmodified WMII result should decode")
        };
        let destination_ids = |loaded: &LoadedSkinDocumentWithDependencies| {
            loaded
                .document
                .destination
                .iter()
                .filter_map(|entry| match entry {
                    DestinationListEntry::Single(destination) => Some(destination.id.clone()),
                    DestinationListEntry::Conditional { .. } => None,
                })
                .collect::<Vec<_>>()
        };

        let full_combo = load(0);
        let full_combo_ids = destination_ids(&full_combo);
        assert!(full_combo_ids.iter().any(|id| id == "result_FULL"));
        assert!(full_combo_ids.iter().any(|id| id == "result_COMBO"));
        assert!(!full_combo_ids.iter().any(|id| id == "result_CLEAR"));

        let normal_clear = load(1);
        let normal_clear_ids = destination_ids(&normal_clear);
        assert!(normal_clear_ids.iter().any(|id| id == "result_CLEAR"));
        assert!(!normal_clear_ids.iter().any(|id| id == "result_FULL"));
        assert!(!normal_clear_ids.iter().any(|id| id == "result_COMBO"));
    }

    fn filepath_def(name: &str, path: &str, def: &str) -> SkinFilepathDef {
        SkinFilepathDef {
            category: String::new(),
            name: name.to_string(),
            path: path.to_string(),
            def: def.to_string(),
        }
    }

    #[test]
    fn substitute_filepath_choice_replaces_wildcard_in_asset_path() {
        let filepaths = vec![filepath_def("レーザー", "custom/laser/*", "default")];
        let mut files = BTreeMap::new();
        files.insert("レーザー".to_string(), "veryshort".to_string());

        let result = substitute_filepath_choice("custom/laser/*/main.png", &filepaths, &files);
        assert_eq!(result.as_deref(), Some("custom/laser/veryshort/main.png"));
    }

    #[test]
    fn substitute_filepath_choice_strips_def_suffix_from_selection() {
        let filepaths = vec![filepath_def("icon", "icon-*.png", "")];
        let mut files = BTreeMap::new();
        files.insert("icon".to_string(), "icon-blue.png".to_string());

        let result = substitute_filepath_choice("icon-*.png", &filepaths, &files);
        assert_eq!(result.as_deref(), Some("icon-blue.png"));
    }

    #[test]
    fn resolve_skin_source_accepts_beatoraja_filename_selection() {
        let root = unique_test_dir("bmz-json-source-filename");
        std::fs::create_dir_all(root.join("parts")).unwrap();
        std::fs::write(root.join("parts/default.png"), []).unwrap();
        std::fs::write(root.join("parts/blue.png"), []).unwrap();
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "filepath": [
                    { "name": "Parts", "path": "parts/*.png", "def": "blue" }
                ]
            }
            "#,
        )
        .unwrap();
        let files = BTreeMap::from([("Parts".to_string(), "default.png".to_string())]);

        let resolved =
            resolve_json_skin_source_path(&root, "parts/*.png", &document, &files).unwrap();

        assert_eq!(resolved.file_name().and_then(|name| name.to_str()), Some("default.png"));
    }

    #[test]
    fn resolve_skin_source_still_accepts_legacy_relative_selection() {
        let root = unique_test_dir("bmz-json-source-relative");
        std::fs::create_dir_all(root.join("parts")).unwrap();
        std::fs::write(root.join("parts/default.png"), []).unwrap();
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "filepath": [
                    { "name": "Parts", "path": "parts/*.png" }
                ]
            }
            "#,
        )
        .unwrap();
        let files = BTreeMap::from([("Parts".to_string(), "parts/default.png".to_string())]);

        let resolved =
            resolve_json_skin_source_path(&root, "parts/*.png", &document, &files).unwrap();

        assert_eq!(resolved.file_name().and_then(|name| name.to_str()), Some("default.png"));
    }

    #[test]
    fn substitute_filepath_choice_returns_none_when_prefix_mismatch() {
        let filepaths = vec![filepath_def("レーザー", "custom/laser/*", "default")];
        let mut files = BTreeMap::new();
        files.insert("レーザー".to_string(), "custom/laser/veryshort".to_string());

        // asset の prefix が定義と一致しない
        let result = substitute_filepath_choice("other/path/*.png", &filepaths, &files);
        assert_eq!(result, None);
    }

    #[test]
    fn enabled_options_includes_unselected_property_default_for_real_skin() {
        // 実際の Starseeker play7.luaskin で「スコアグラフ=On」のみ選択した時、
        // 未選択の「プレーサイド」のデフォルト (1P=920) と「スコアグラフ=On」(901)
        // の両方が enabled_options に入ることを確認する。
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Starseeker/play/play7.luaskin");
        if !skin_path.is_file() {
            eprintln!("skipping: skin not present at {}", skin_path.display());
            return;
        }
        let mut selections = BTreeMap::new();
        selections.insert("スコアグラフ".to_string(), "On".to_string());

        let loaded = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &selections,
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            None,
        )
        .expect("load skin document");
        let ops = enabled_options_from_selections(&loaded.document, &selections);
        assert!(ops.contains(&901), "expected 901 in ops, got {ops:?}");
        assert!(ops.contains(&920), "expected 920 (1P default) in ops, got {ops:?}");
    }

    #[test]
    fn enabled_options_rejects_stale_numeric_selection() {
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "property": [
                    {
                        "name": "Graph",
                        "def": "AC",
                        "item": [
                            { "name": "AC", "op": 922 },
                            { "name": "TYPE-M", "op": 923 }
                        ]
                    }
                ]
            }
            "#,
        )
        .unwrap();
        let selections = BTreeMap::from([("Graph".to_string(), "999".to_string())]);

        assert_eq!(enabled_options_from_selections(&document, &selections), vec![922]);
    }

    #[test]
    fn substitute_filepath_choice_returns_none_when_no_selection() {
        let filepaths = vec![filepath_def("レーザー", "custom/laser/*", "default")];
        let files: BTreeMap<String, String> = BTreeMap::new();

        let result = substitute_filepath_choice("custom/laser/*/main.png", &filepaths, &files);
        assert_eq!(result, None);
    }

    #[test]
    fn default_skin_can_be_applied_to_renderer() {
        let mut renderer = Renderer::default();

        apply_default_skin(&mut renderer).unwrap();
    }

    #[test]
    fn beatoraja_default_json_skin_can_be_applied_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/beatoraja/skin/default/play7.json");
        if !skin_path.is_file() {
            return;
        }
        let mut renderer = Renderer::default();

        apply_beatoraja_json_skin(&mut renderer, &skin_path).unwrap();
    }

    #[test]
    fn beatoraja_default_select_json_skin_can_be_applied_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/beatoraja/skin/default/select.json");
        if !skin_path.is_file() {
            return;
        }
        let mut renderer = Renderer::default();

        apply_beatoraja_select_json_skin(&mut renderer, &skin_path).unwrap();
    }

    #[test]
    fn ecfn_play7_1p_json_skin_can_be_applied_when_available() {
        let skin_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play7-1p.json");
        if !skin_path.is_file() {
            return;
        }
        let mut renderer = Renderer::default();

        apply_beatoraja_json_skin(&mut renderer, &skin_path).unwrap();
    }

    #[test]
    fn beatoraja_default_result_json_skin_can_be_applied_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/beatoraja/skin/default/result.json");
        if !skin_path.is_file() {
            return;
        }
        let mut renderer = Renderer::default();

        apply_beatoraja_result_json_skin(&mut renderer, &skin_path).unwrap();
    }

    #[test]
    fn ecfn_result_json_skin_can_be_applied_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/ECFN/RESULT/result-converted.json");
        if !skin_path.is_file() {
            return;
        }
        let mut renderer = Renderer::default();

        apply_beatoraja_result_json_skin(&mut renderer, &skin_path).unwrap();
    }

    #[test]
    fn ecfn_select_json_skin_can_be_applied_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/ECFN/select/select-converted.json");
        if !skin_path.is_file() {
            return;
        }
        let mut renderer = Renderer::default();

        apply_beatoraja_select_json_skin(&mut renderer, &skin_path).unwrap();
    }

    #[test]
    fn ecfn_select_lua_skin_decodes_movie_source_first_frame_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/ECFN/select/select.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Select,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let mv = decoded.sources.iter().find(|source| source.source_id == "mv").unwrap();

        let mv_path = mv.path.to_string_lossy().replace('\\', "/");
        assert!(mv_path.ends_with("mv/default.mp4"));
        let asset = mv.asset.as_ref().expect("movie first frame should decode");
        assert!(asset.width > 0);
        assert!(asset.height > 0);
        assert_eq!(asset.pixels.len(), asset.width as usize * asset.height as usize * 4);
    }

    #[test]
    #[ignore = "manual select skin profiling helper"]
    fn profile_ecfn_select_plan_generation() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/ECFN/select/select.luaskin");
        if !skin_path.is_file() {
            eprintln!("skip: {} is missing", skin_path.display());
            return;
        }

        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Select,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let document_textures = decoded.sources.iter().map(|source| SkinDocumentTexture {
            source_id: source.source_id.clone(),
            texture: source.texture,
            source_size: SkinImageSize { width: source.size.width, height: source.size.height },
        });
        let skin = SkinContext::from_manifest_and_document(
            bmz_render::skin::default_skin_manifest(),
            decoded.document,
            document_textures,
        );
        let rows = (0..25)
            .map(|index| SelectRowSnapshot {
                index,
                title: format!("War in the Mirrorworld[{index:02}]"),
                artist: "Aoi".to_string(),
                difficulty_name: "ANOTHER".to_string(),
                play_level: "12".to_string(),
                total_notes: 2253,
                chart_normal_notes: 2167,
                chart_scratch_notes: 86,
                chart_density: 19.0,
                chart_peak_density: 38.0,
                chart_end_density: 25.0,
                min_bpm: 171.0,
                max_bpm: 171.0,
                chart_main_bpm: 171.0,
                initial_bpm: 171.0,
                length_ms: 115_000,
                ..SelectRowSnapshot::default()
            })
            .collect();
        let mut runtime = DynamicTimerRuntime::default();
        let mut snapshot = SelectSnapshot {
            time: TimeUs(0),
            selection_time: TimeUs(0),
            chart_count: 1_000,
            selected_index: 12,
            rows,
            stage_background: true,
            banner_image: true,
            ..SelectSnapshot::default()
        };

        for frame in 0..30 {
            snapshot.time = TimeUs(frame * 16_666);
            black_box(DrawPlan::from_scene_with_skin(
                &AppSceneSnapshot::Select(snapshot.clone()),
                &skin,
                &mut runtime,
            ));
        }

        let frames = 300;
        let start = Instant::now();
        let mut commands = 0_usize;
        for frame in 0..frames {
            snapshot.time = TimeUs((frame + 30) * 16_666);
            let plan = DrawPlan::from_scene_with_skin(
                &AppSceneSnapshot::Select(snapshot.clone()),
                &skin,
                &mut runtime,
            );
            commands += plan.commands.len();
            black_box(plan);
        }
        let elapsed = start.elapsed();
        println!(
            "profile_ecfn_select_plan_generation frames={frames} avg_plan_ms={:.3} avg_commands={}",
            elapsed.as_secs_f64() * 1000.0 / frames as f64,
            commands / frames as usize
        );
    }

    #[test]
    #[ignore = "manual select skin profiling helper"]
    fn profile_rgba_frame_clone_cost() {
        let width = 1920_usize;
        let height = 1080_usize;
        let rgba = vec![127_u8; width * height * 4];
        let frames = 240;

        let clone_start = Instant::now();
        let mut cloned_len = 0_usize;
        for _ in 0..frames {
            let cloned = black_box(rgba.clone());
            cloned_len += black_box(cloned.len());
        }
        let clone_elapsed = clone_start.elapsed();

        let borrow_start = Instant::now();
        let mut borrowed_len = 0_usize;
        for _ in 0..frames {
            borrowed_len += black_box(rgba.as_slice()).len();
        }
        let borrow_elapsed = borrow_start.elapsed();

        assert_eq!(cloned_len, borrowed_len);
        println!(
            "profile_rgba_frame_clone_cost frames={frames} bytes={} avg_clone_ms={:.3} avg_borrow_ms={:.6}",
            rgba.len(),
            clone_elapsed.as_secs_f64() * 1000.0 / frames as f64,
            borrow_elapsed.as_secs_f64() * 1000.0 / frames as f64
        );
    }

    #[test]
    fn m_select_lua_select_skin_renders_items_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/mz-select/music_select.luaskin");
        if !skin_path.is_file() {
            return;
        }
        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Select,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(decoded.document.text.iter().any(|text| {
            text.id == "defaultNotesProcessingCounter_notes"
                || text.id == "defaultNotesProcessingCounter_stroke"
        }));
        let document_textures =
            decoded.sources.iter().map(|source| bmz_render::skin::SkinDocumentTexture {
                source_id: source.source_id.clone(),
                texture: source.texture,
                source_size: bmz_render::skin::SkinImageSize {
                    width: source.size.width,
                    height: source.size.height,
                },
            });
        let context = bmz_render::skin::SkinContext::from_manifest_and_document(
            bmz_render::skin::default_skin_manifest(),
            decoded.document,
            document_textures,
        );
        assert!(context.document().is_some_and(|document| document.skin_type == 5));
        let snapshot = bmz_render::scene::SelectSnapshot {
            arrange: "F-RANDOM".to_string(),
            arrange_2p: "MF-RANDOM".to_string(),
            gauge: "EX-HARD".to_string(),
            double_option: "BATTLE".to_string(),
            hs_fix: "CONSTANT".to_string(),
            rows: vec![bmz_render::scene::SelectRowSnapshot {
                title: "Song".to_string(),
                ..Default::default()
            }],
            chart_count: 1,
            ..Default::default()
        };
        let items = context.select_document_items_with_dynamic_timers(&snapshot, None);
        assert!(!items.is_empty(), "m_select select skin should produce render items");
        assert!(
            items
                .iter()
                .any(|item| matches!(item, bmz_render::skin::SkinRenderItem::Text { text, .. } if text == "Song")),
            "m_select select skin should render the song title text"
        );
        for label in ["F-RANDOM", "MF-RANDOM", "EX-HARD", "BATTLE", "CONSTANT"] {
            assert!(
                items.iter().any(
                    |item| matches!(item, bmz_render::skin::SkinRenderItem::Text { text, .. } if text == label)
                ),
                "m_select should render the dynamic option label {label}"
            );
        }
        for x in [503.0, 586.0] {
            let hit = context
                .select_click_hit(&snapshot, x / 1920.0, 0.98)
                .expect("m_select arrange cell should remain clickable across its full width");
            assert_eq!(
                hit.target,
                bmz_render::skin::SkinClickTarget::Event { event_id: 42, click: 2 }
            );
            assert!((hit.rect.x - 462.0 / 1920.0).abs() < f32::EPSILON);
            assert!((hit.rect.width - 166.0 / 1920.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn antique_play_skin_shows_random_lane_pattern_before_ready_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/mz-select/play/antique/system/play7main.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let load_context = |options: &BTreeMap<String, String>| {
            let decoded = decode_beatoraja_skin_with_options(
                &skin_path,
                SkinKind::Play,
                options,
                &BTreeMap::new(),
            )
            .unwrap();
            let number_texture = decoded
                .sources
                .iter()
                .find(|source| source.source_id == "src_number_lane")
                .expect("antique number lane source")
                .texture;
            let document_textures = decoded.sources.iter().map(|source| SkinDocumentTexture {
                source_id: source.source_id.clone(),
                texture: source.texture,
                source_size: SkinImageSize { width: source.size.width, height: source.size.height },
            });
            (
                SkinContext::from_manifest_and_document(
                    SkinManifest::default(),
                    decoded.document,
                    document_textures,
                ),
                number_texture,
            )
        };
        let (default_context, default_number_texture) = load_context(&BTreeMap::new());
        let default_document = default_context.document().expect("antique play document");
        assert!(default_document.enabled_options().contains(&916));
        assert!(!default_document.enabled_options().contains(&917));
        assert!(
            default_document
                .property
                .iter()
                .any(|property| { property.name == "RANDOM配置表示" && property.def == "OFF" })
        );
        let options = BTreeMap::from([("RANDOM配置表示".to_string(), "ON".to_string())]);
        let (context, number_texture) = load_context(&options);
        let document = context.document().expect("antique play document");
        assert!(document.enabled_options().contains(&917));
        assert!(!document.enabled_options().contains(&916));
        assert!(
            document
                .all_destinations(&document.enabled_options())
                .iter()
                .filter(|destination| destination.id.starts_with("num_random_"))
                .all(|destination| !destination.draw.starts_with("bmz:lua_draw_callback:")),
            "RANDOM digit color predicates should compile without per-frame Lua callbacks"
        );
        let displayed_values = [2_u8, 3, 4, 5, 6, 7, 1];
        let mut pattern = (0..bmz_core::lane::LANE_COUNT as u8).collect::<Vec<_>>();
        for (destination, source) in (1..=7).zip(displayed_values) {
            pattern[destination] = source;
        }
        let applied_arrange = crate::screens::play_session::AppliedArrange {
            arrange: crate::select_options::ArrangeOption::Random,
            pattern: Some(pattern.clone()),
            ..crate::screens::play_session::AppliedArrange::default()
        };
        let mut pre_ready = bmz_render::snapshot::RenderSnapshot {
            key_mode: KeyMode::K7,
            ready_elapsed_time: None,
            ..Default::default()
        };
        crate::screens::play_loop::apply_play_arrange_to_snapshot(&mut pre_ready, &applied_arrange);
        assert_eq!(pre_ready.lane_shuffle_pattern, pattern);

        let render = |context: &SkinContext, snapshot| {
            bmz_render::plan::DrawPlan::from_scene_with_skin(
                &bmz_render::scene::AppSceneSnapshot::Play(snapshot),
                context,
                &mut bmz_render::skin::DynamicTimerRuntime::default(),
            )
        };
        let random_digits = |plan: &bmz_render::plan::DrawPlan, number_texture: SkinTextureId| {
            let mut digits = plan
                .commands
                .iter()
                .filter_map(|command| match command {
                    bmz_render::plan::DrawCommand::Image { texture, rect, tint, .. }
                        if texture.0 == number_texture.0 && (0.69..0.72).contains(&rect.y) =>
                    {
                        Some((rect.x, *tint))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            digits.sort_by(|left, right| left.0.total_cmp(&right.0));
            digits
        };

        let digits = random_digits(&render(&context, pre_ready.clone()), number_texture);
        assert_eq!(digits.len(), 7, "expected seven pre-READY RANDOM digits");
        for (index, (_, tint)) in digits.into_iter().enumerate() {
            let value = displayed_values[index];
            let expected = if matches!(value, 1 | 3 | 5 | 7) {
                (1.0, 1.0, 1.0)
            } else {
                (64.0 / 255.0, 160.0 / 255.0, 1.0)
            };
            assert!((tint.r - expected.0).abs() < 0.01);
            assert!((tint.g - expected.1).abs() < 0.01);
            assert!((tint.b - expected.2).abs() < 0.01);
        }

        let mut ready = pre_ready.clone();
        ready.ready_elapsed_time = Some(TimeUs(0));
        let ready_digits = random_digits(&render(&context, ready.clone()), number_texture);
        assert_eq!(ready_digits.len(), 7, "READY should start at full opacity");
        assert!(ready_digits.iter().all(|(_, tint)| (tint.a - 1.0).abs() < 0.01));

        ready.ready_elapsed_time = Some(TimeUs(250_000));
        let fading_digits = random_digits(&render(&context, ready.clone()), number_texture);
        assert_eq!(fading_digits.len(), 7, "RANDOM digits should fade for 500 ms");
        assert!(fading_digits.iter().all(|(_, tint)| (tint.a - 0.5).abs() < 0.02));

        ready.ready_elapsed_time = Some(TimeUs(501_000));
        assert!(
            random_digits(&render(&context, ready), number_texture).is_empty(),
            "RANDOM digits should disappear after the BACKBMP 500 ms fade"
        );

        let mut no_pattern = pre_ready.clone();
        crate::screens::play_loop::apply_play_arrange_to_snapshot(
            &mut no_pattern,
            &crate::screens::play_session::AppliedArrange {
                arrange: crate::select_options::ArrangeOption::Random,
                ..crate::screens::play_session::AppliedArrange::default()
            },
        );
        assert!(random_digits(&render(&context, no_pattern), number_texture).is_empty());

        assert!(
            random_digits(&render(&default_context, pre_ready), default_number_texture).is_empty(),
            "RANDOM display should default to OFF"
        );
    }

    #[test]
    fn luxe_flat_lua_select_skin_keeps_operating_time_refs_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Luxez-Flat/music_select.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Select).unwrap();
        for ref_id in 27..=29 {
            assert!(
                decoded.document.value.iter().any(|value| value.ref_id == ref_id),
                "Luxe Flat should retain operating-time ref {ref_id}"
            );
        }
        for (id, center_x) in [
            ("bmz_select_arrange", 138),
            ("bmz_select_gauge", 302),
            ("bmz_select_double_option", 446),
            ("bmz_select_hs_fix", 613),
            ("bmz_select_arrange_2p", 790),
        ] {
            assert!(
                decoded.document.text.iter().any(|text| text.id == id),
                "Luxe Flat should decode dynamic {id} text"
            );
            assert!(decoded.document.destination.iter().any(|entry| matches!(
                entry,
                DestinationListEntry::Single(destination)
                    if destination.id == id
                        && destination.act.is_none()
                        && matches!(
                            destination.dst.first(),
                            Some(bmz_render::skin::SkinDstEntry::Frame(frame))
                                if frame.x == Some(center_x)
                )
            )));
        }
        assert!(
            decoded
                .document
                .panel
                .iter()
                .any(|panel| panel.id == "bmz_select_option_hit" && panel.color == "00000000")
        );
        for (act, left_x, width) in
            [(42, 69, 138), (40, 254, 96), (54, 381, 129), (55, 550, 126), (43, 721, 138)]
        {
            assert!(decoded.document.destination.iter().any(|entry| matches!(
                entry,
                DestinationListEntry::Single(destination)
                    if destination.id == "bmz_select_option_hit"
                        && destination.act == Some(act)
                        && matches!(
                            destination.dst.first(),
                            Some(bmz_render::skin::SkinDstEntry::Frame(frame))
                                if frame.x == Some(left_x) && frame.w == Some(width)
                )
            )));
        }
        let document_textures =
            decoded.sources.iter().map(|source| bmz_render::skin::SkinDocumentTexture {
                source_id: source.source_id.clone(),
                texture: source.texture,
                source_size: bmz_render::skin::SkinImageSize {
                    width: source.size.width,
                    height: source.size.height,
                },
            });
        let context = bmz_render::skin::SkinContext::from_manifest_and_document(
            bmz_render::skin::default_skin_manifest(),
            decoded.document,
            document_textures,
        );
        let hit = context
            .select_click_hit(&bmz_render::scene::SelectSnapshot::default(), 100.0 / 1920.0, 0.98)
            .expect("Luxe Flat arrange cell should be clickable from its left half");
        assert_eq!(hit.target, bmz_render::skin::SkinClickTarget::Event { event_id: 42, click: 0 });
        assert!((hit.rect.x - 69.0 / 1920.0).abs() < f32::EPSILON);
        assert!((hit.rect.width - 138.0 / 1920.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rm_skin_play_lua_skins_can_be_decoded_when_available() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/Rm-skin");
        let cases = [
            (root.join("play5main.luaskin"), SkinKind::Play),
            (root.join("play7main.luaskin"), SkinKind::Play),
            (root.join("play9main.luaskin"), SkinKind::Play),
        ];
        for (skin_path, kind) in cases {
            if !skin_path.is_file() {
                continue;
            }
            let decoded = decode_beatoraja_skin(&skin_path, kind).unwrap();
            assert!(!decoded.document.destination.is_empty(), "{}", skin_path.display());
        }
    }

    #[test]
    fn ecfn_lua_skins_can_be_decoded_when_available() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN");
        let cases = [
            (root.join("select/select.luaskin"), SkinKind::Select),
            (root.join("play/play7.luaskin"), SkinKind::Play),
            (root.join("RESULT/result.luaskin"), SkinKind::Result),
        ];
        for (skin_path, kind) in cases {
            if !skin_path.is_file() {
                continue;
            }
            let decoded = decode_beatoraja_skin(&skin_path, kind).unwrap();
            assert!(!decoded.document.destination.is_empty());
        }
    }

    #[test]
    fn ecfn_play7_uses_default_filepaths_when_defs_are_missing() {
        let skin_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play7.luaskin");
        if !skin_path.is_file() {
            return;
        }
        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

        for (source_id, suffix) in [
            ("6", "laser/default.png"),
            ("7", "notes/default.png"),
            ("12", "lanecover/default.png"),
        ] {
            let source = decoded
                .sources
                .iter()
                .find(|source| source.source_id == source_id)
                .unwrap_or_else(|| panic!("ECFN source {source_id} should decode"));
            let path = source.path.to_string_lossy().replace('\\', "/");
            assert!(
                path.ends_with(suffix),
                "ECFN source {source_id} should resolve to {suffix}, got {path}"
            );
        }
    }

    #[test]
    fn luxe_flat_lua_select_skin_keeps_score_availability_guards_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Luxez-Flat/music_select.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Select).unwrap();
        let clear_state = decoded
            .document
            .destination
            .iter()
            .find_map(|entry| match entry {
                DestinationListEntry::Single(destination)
                    if destination.id == "default_playerdata_state_clear" =>
                {
                    Some(destination)
                }
                DestinationListEntry::Single(_) | DestinationListEntry::Conditional { .. } => None,
            })
            .expect("Luxe Flat should retain the player clear-state destination");
        assert_eq!(clear_state.draw, "select_score_available()");
    }

    #[test]
    fn mz_select_lua_select_skin_keeps_local_score_availability_guards() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/mz-select/music_select.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Select).unwrap();
        let guarded = decoded
            .document
            .destination
            .iter()
            .filter_map(|entry| match entry {
                DestinationListEntry::Single(destination)
                    if destination.id.starts_with("default_playerdata_")
                        && destination.draw == "select_score_available()" =>
                {
                    Some(destination.id.as_str())
                }
                DestinationListEntry::Single(_) | DestinationListEntry::Conditional { .. } => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(guarded.len(), 21, "mz-select player-data score guards: {guarded:?}");
        assert!(guarded.contains(&"default_playerdata_state_clear"));
        assert!(guarded.contains(&"default_playerdata_score_count"));
        assert!(guarded.contains(&"default_playerdata_scorerate_dot_count"));
    }

    #[test]
    fn mz_select_result_uses_runtime_decisions_and_draws_note_graphs() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/mz-select/result/result.luaskin");
        if !skin_path.is_file() {
            return;
        }
        let runtime_state = LuaLoadRuntimeState {
            number_values: BTreeMap::from([
                (74, 100),
                (153, 354),
                (370, 7),
                (371, 5),
                (374, -12),
                (375, -50),
                (410, 20),
                (411, 10),
                (412, 8),
                (413, 4),
                (414, 3),
                (415, 2),
                (416, 1),
                (417, 1),
                (418, 1),
                (419, 1),
                (421, 1),
                (422, 1),
            ]),
            ..LuaLoadRuntimeState::default()
        };
        let decoded = decode_beatoraja_skin_with_options_and_runtime_state(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &runtime_state,
        )
        .expect("decode mz-select result skin with runtime result values");

        let timing = decoded
            .document
            .text
            .iter()
            .find(|text| text.id == "timing")
            .expect("mz-select timing text");
        assert_eq!(timing.constant_text, "平均12.5ms遅い");
        for (id, label, draw) in [
            ("arrange_f_random", "F-RANDOM", "event_index(344) == 10"),
            ("arrange_mf_random", "MF-RANDOM", "event_index(344) == 11"),
            ("arrange_f_random_2p", "2P F-RANDOM", "event_index(345) == 10"),
            ("arrange_mf_random_2p", "2P MF-RANDOM", "event_index(345) == 11"),
        ] {
            assert!(
                decoded
                    .document
                    .text
                    .iter()
                    .any(|text| text.id == id && text.constant_text == label),
                "mz-select result should decode {id} text"
            );
            assert!(decoded.document.destination.iter().any(|entry| matches!(
                entry,
                DestinationListEntry::Single(destination)
                    if destination.id == id && destination.draw == draw
            )));
        }
        let clear_state = decoded
            .document
            .image
            .iter()
            .find(|image| image.id == "clear_state")
            .expect("mz-select clear update image");
        assert_eq!(clear_state.x, 0, "current clear above previous should use UP image");
        assert!(decoded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination) if destination.id == "win"
        )));
        assert!(!decoded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination) if destination.id == "draw"
        )));

        let document_textures = decoded.sources.iter().map(|source| SkinDocumentTexture {
            source_id: source.source_id.clone(),
            texture: source.texture,
            source_size: source.size,
        });
        let context = SkinContext::from_manifest_and_document(
            bmz_render::skin::default_skin_manifest(),
            decoded.document,
            document_textures,
        );
        let graph = std::sync::Arc::new(bmz_render::snapshot::ResultGraphSnapshot {
            judge_graph_buckets: vec![
                bmz_render::snapshot::ResultJudgeGraphBucket { values: [0, 10, 5, 2, 1, 1] },
                bmz_render::snapshot::ResultJudgeGraphBucket { values: [0, 8, 4, 2, 1, 0] },
            ],
            early_late_graph_buckets: vec![
                bmz_render::snapshot::ResultEarlyLateGraphBucket {
                    values: [0, 10, 4, 2, 1, 0, 3, 2, 1, 0],
                },
                bmz_render::snapshot::ResultEarlyLateGraphBucket {
                    values: [0, 8, 3, 2, 1, 0, 4, 2, 1, 0],
                },
            ],
            judge_graph_density: vec![12, 18],
            ..bmz_render::snapshot::ResultGraphSnapshot::default()
        });
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 500,
            result_failed: Some(false),
            total_notes: 100,
            key_mode: KeyMode::K7,
            ..bmz_render::skin::SkinDrawState::default()
        };
        let items = context.static_document_items_for_result_state_and_text(
            &graph,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );
        let populated_batches = items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::RectBatch { rects, .. } if !rects.is_empty()
                )
            })
            .count();
        assert_eq!(populated_batches, 2, "JUDGE and FAST/SLOW graph batches should render");
        assert!(
            !items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Rect {
                    color,
                    blend: bmz_render::skin::BlendMode::Add,
                    ..
                } if color.r == 0.0 && color.g == 0.0 && color.b == 0.0
            )),
            "additive black gauge backgrounds must not cover the two note graphs"
        );
    }

    #[test]
    fn ecfn_play7_judge_combo_x_matches_beatoraja_layout_when_available() {
        use std::collections::HashMap;

        use bmz_render::skin::{SkinDocumentTexture, SkinImageSize, SkinRenderItem, SkinTextureId};

        let skin_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play7.luaskin");
        if !skin_path.is_file() {
            return;
        }
        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let mock_texture = SkinDocumentTexture {
            source_id: "mock".to_string(),
            texture: SkinTextureId(1),
            source_size: SkinImageSize { width: 1920.0, height: 1080.0 },
        };
        let sources: HashMap<String, SkinDocumentTexture> = decoded
            .document
            .source
            .iter()
            .map(|source| (source.id.clone(), mock_texture.clone()))
            .chain(
                decoded
                    .document
                    .value
                    .iter()
                    .map(|value| (value.src.clone(), mock_texture.clone())),
            )
            .chain(
                decoded
                    .document
                    .image
                    .iter()
                    .map(|image| (image.src.clone(), mock_texture.clone())),
            )
            .collect();
        let items =
            decoded.document.judge_render_items("PGREAT", 42, 100, &sources).expect("judge items");
        let digit_xs: Vec<f32> = items
            .iter()
            .skip(1)
            .filter_map(|item| match item {
                SkinRenderItem::Image { rect, .. } => Some(rect.x),
                _ => None,
            })
            .collect();
        assert_eq!(digit_xs.len(), 2);
        let expected_first = 334.0 / 1920.0;
        let expected_second = 392.0 / 1920.0;
        assert!(
            (digit_xs[0] - expected_first).abs() < 0.001,
            "first digit x={} expected {expected_first}",
            digit_xs[0]
        );
        assert!(
            (digit_xs[1] - expected_second).abs() < 0.001,
            "second digit x={} expected {expected_second}",
            digit_xs[1]
        );
    }

    #[test]
    fn ecfn_play7_pre_notes_judge_line_renders_in_front_when_available() {
        use std::collections::HashMap;

        use bmz_render::skin::{
            SkinDocumentTexture, SkinDrawState, SkinImageSize, SkinRenderItem, SkinTextState,
        };

        let skin_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play7.luaskin");
        if !skin_path.is_file() {
            return;
        }
        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let image_15 = decoded
            .document
            .image
            .iter()
            .find(|image| image.id == "15")
            .expect("ECFN id=15 image should decode");
        assert_eq!((image_15.src.as_str(), image_15.x, image_15.y), ("0", 16, 0));
        let image_15_map = decoded.document.image_map();
        let mapped_15 = image_15_map.get("15").expect("ECFN id=15 image should map");
        assert_eq!((mapped_15.src.as_str(), mapped_15.x, mapped_15.y), ("0", 16, 0));
        let system_texture = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "0")
            .map(|source| source.texture)
            .expect("ECFN source 0 should decode");
        let system_size = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "0")
            .map(|source| SkinImageSize { width: source.size.width, height: source.size.height })
            .expect("ECFN source 0 should decode");
        let sources: HashMap<String, SkinDocumentTexture> = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect();

        let (behind, front, _) = decoded.document.static_render_items_split(
            &sources,
            &SkinDrawState::default(),
            &SkinTextState::default(),
        );

        assert!(
            behind.iter().all(|item| !matches!(
                item,
                SkinRenderItem::Image {
                    texture,
                    rect,
                    ..
                } if *texture == system_texture
                    && (rect.y - 715.0 / 1080.0).abs() < 0.001
                    && (rect.height - 8.0 / 1080.0).abs() < 0.001
            )),
            "ECFN judge line should not remain behind notes"
        );
        assert!(
            front.iter().any(|item| matches!(
                item,
                SkinRenderItem::Image {
                    texture,
                    rect,
                    uv,
                    ..
                } if *texture == system_texture
                    && (rect.y - 715.0 / 1080.0).abs() < 0.001
                    && (rect.height - 8.0 / 1080.0).abs() < 0.001
                    && (uv.x - 16.0 / system_size.width).abs() < 0.001
                    && uv.y.abs() < 0.001
            )),
            "expected ECFN id=15 judge line in front items; got {front:?}"
        );
    }

    #[test]
    fn ecfn_play14_judge1_combo_is_right_of_judge_when_available() {
        use std::collections::HashMap;

        use bmz_core::lane::Lane;
        use bmz_render::skin::{
            MAX_JUDGE_REGIONS, SkinDocumentTexture, SkinDrawState, SkinImageSize, SkinRenderItem,
            SkinTextureId,
        };

        let skin_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play14.luaskin");
        if !skin_path.is_file() {
            return;
        }
        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play)
            .expect("ECFN play14 should decode with default options");
        let judge0 =
            decoded.document.judge.iter().find(|judge| judge.id == "judge").expect("judge");
        let judge1 =
            decoded.document.judge.iter().find(|judge| judge.id == "judge1").expect("judge1");
        assert_eq!(judge0.index, 0);
        assert_eq!(judge1.index, 1);

        let mock_texture = SkinDocumentTexture {
            source_id: "mock".to_string(),
            texture: SkinTextureId(1),
            source_size: SkinImageSize { width: 1920.0, height: 1080.0 },
        };
        let sources: HashMap<String, SkinDocumentTexture> = decoded
            .document
            .source
            .iter()
            .map(|source| (source.id.clone(), mock_texture.clone()))
            .chain(
                decoded
                    .document
                    .value
                    .iter()
                    .map(|value| (value.src.clone(), mock_texture.clone())),
            )
            .chain(
                decoded
                    .document
                    .image
                    .iter()
                    .map(|image| (image.src.clone(), mock_texture.clone())),
            )
            .collect();

        let mut judge_ms = [None; MAX_JUDGE_REGIONS];
        let mut judge_index = [None; MAX_JUDGE_REGIONS];
        judge_ms[0] = Some(100);
        judge_ms[1] = Some(100);
        judge_index[0] = Some(0);
        judge_index[1] = Some(0);
        let state = SkinDrawState { judge_ms, judge_index, combo: 42, ..SkinDrawState::default() };

        let left_items = decoded
            .document
            .judge_render_items_for_def(judge0, 0, 42, 100, &sources, &state)
            .expect("left judge");
        let right_items = decoded
            .document
            .judge_render_items_for_def(judge1, 0, 42, 100, &sources, &state)
            .expect("right judge");
        let left_digit = left_items
            .iter()
            .skip(1)
            .find_map(|item| match item {
                SkinRenderItem::Image { rect, .. } => Some(rect.x),
                _ => None,
            })
            .expect("left combo digit");
        let right_digit = right_items
            .iter()
            .skip(1)
            .find_map(|item| match item {
                SkinRenderItem::Image { rect, .. } => Some(rect.x),
                _ => None,
            })
            .expect("right combo digit");
        assert!(
            right_digit > left_digit,
            "judge1 digit x={right_digit} should be right of judge x={left_digit}"
        );

        let region = bmz_render::skin::lane_judge_region(
            Lane::Key8.index(),
            bmz_core::lane::LANE_COUNT,
            decoded.document.judge_region_count(),
        );
        assert_eq!(region, 1);
    }

    #[test]
    fn starseeker_play_lua_skin_can_be_decoded_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Starseeker/play/play7.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

        assert!(!decoded.document.destination.is_empty());
    }

    #[test]
    fn starseeker_frame_filepath_selection_merges_frame_destinations_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Starseeker/play/play7.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let mut files = BTreeMap::new();
        files.insert("フレーム".to_string(), "custom/frame/AC_SP/starseeker".to_string());

        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &files,
        )
        .expect("decode starseeker frame skin");

        assert!(
            decoded.document.source.iter().any(|source| source.id == "main_frame"),
            "expected main_frame source from starseeker frameL.lua"
        );
        assert!(
            decoded
                .document
                .all_destinations(&[])
                .iter()
                .any(|destination| destination.id == "base_L" || destination.id == "base_R"),
            "expected frame panel destinations from starseeker frameL.lua"
        );
    }

    #[test]
    fn starseeker_default_frame_uses_same_directory_for_lua_parts_and_sources_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Starseeker/play/play7.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play)
            .expect("decode starseeker default frame skin");
        let main_frame = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "main_frame")
            .expect("main_frame source should be decoded from selected frame");

        assert!(
            main_frame.path.components().any(|component| component.as_os_str() == "TM_default"),
            "expected default frame source under TM_default, got {}",
            main_frame.path.display()
        );
    }

    #[test]
    fn starseeker_result_lua_skin_renders_stat_details_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Starseeker/result/result.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([
            ("F/Sリスト".to_string(), "Default".to_string()),
            ("逆サイド詳細フレーム".to_string(), "ON".to_string()),
            ("プレーサイド".to_string(), "1P".to_string()),
        ]);
        let files = BTreeMap::from([
            ("使用テーマ".to_string(), "Theme/starseeker".to_string()),
            ("フォント".to_string(), "_font/TYPE-M".to_string()),
            ("シャッター".to_string(), "Shutter/TYPE-M".to_string()),
        ]);
        let decoded =
            decode_beatoraja_skin_with_options(&skin_path, SkinKind::Result, &options, &files)
                .expect("decode starseeker result skin");
        let destinations = decoded.document.all_destinations(&[]);
        let slow_judgement_timing = destinations
            .iter()
            .find(|destination| destination.id == "judge_adv_s")
            .expect("starseeker result should keep SLOW timing label destination");
        let fast_judgement_timing = destinations
            .iter()
            .find(|destination| destination.id == "judge_adv_f")
            .expect("starseeker result should keep FAST timing label destination");
        assert_eq!(slow_judgement_timing.draw, "number(374) < 0 or number(375) < 0");
        assert_eq!(fast_judgement_timing.draw, "number(374) > 0 or number(375) > 0");
        assert!(
            decoded.document.all_destinations(&[]).iter().any(|destination| {
                matches!(
                    destination.id.as_str(),
                    "judge_detail" | "judgegraph" | "fsgraph" | "timingGraph"
                )
            }),
            "starseeker result stat destinations should survive lua conversion"
        );
        assert!(
            decoded.document.source.iter().any(|source| source.id == "jud_detail_main"),
            "starseeker result document should keep jud_detail_main source; sources: {:?}",
            decoded.document.source.iter().map(|source| source.id.as_str()).collect::<Vec<_>>()
        );
        let stat_texture = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "jud_detail_main")
            .map(|source| source.texture)
            .expect("starseeker result should load jud_detail_main source");
        let document_textures =
            decoded.sources.iter().map(|source| bmz_render::skin::SkinDocumentTexture {
                source_id: source.source_id.clone(),
                texture: source.texture,
                source_size: bmz_render::skin::SkinImageSize {
                    width: source.size.width,
                    height: source.size.height,
                },
            });
        let context = bmz_render::skin::SkinContext::from_manifest_and_document(
            bmz_render::skin::default_skin_manifest(),
            decoded.document,
            document_textures,
        );
        let bmz_render::scene::AppSceneSnapshot::Result(mut snapshot) =
            bmz_render::sample::sample_result_scene()
        else {
            panic!("sample result scene");
        };
        snapshot.elapsed_time = bmz_core::time::TimeUs(1_000_000);
        snapshot.judge_counts = bmz_render::snapshot::DisplayJudgeCounts {
            pgreat: 120,
            great: 40,
            good: 12,
            bad: 4,
            poor: 3,
            empty_poor: 2,
        };
        snapshot.fast_slow_counts = bmz_render::snapshot::FastSlowJudgeCounts {
            fast_pgreat: 80,
            slow_pgreat: 40,
            fast_great: 12,
            slow_great: 28,
            fast_good: 4,
            slow_good: 8,
            fast_bad: 1,
            slow_bad: 3,
            fast_poor: 1,
            slow_poor: 2,
            fast_empty_poor: 1,
            slow_empty_poor: 1,
        };
        let graph = std::sync::Arc::make_mut(&mut snapshot.graph);
        graph.judge_graph_density = vec![1, 3, 2, 4];
        graph.timing_points = vec![
            bmz_render::snapshot::ResultTimingPoint {
                time_ms: 100,
                delta_us: -12_000,
                judge: bmz_core::judge::Judge::Great,
            },
            bmz_render::snapshot::ResultTimingPoint {
                time_ms: 200,
                delta_us: 8_000,
                judge: bmz_core::judge::Judge::PGreat,
            },
        ];

        let plan = bmz_render::plan::DrawPlan::from_scene_with_skin(
            &bmz_render::scene::AppSceneSnapshot::Result(snapshot),
            &context,
            &mut bmz_render::skin::DynamicTimerRuntime::default(),
        );

        assert!(plan.commands.iter().any(|command| matches!(
            command,
            bmz_render::plan::DrawCommand::Image { texture, .. }
                if *texture == bmz_render::plan::TextureId(stat_texture.0)
        )));
        assert!(plan.commands.iter().any(|command| matches!(
            command,
            bmz_render::plan::DrawCommand::Rect { rect, .. }
                if rect.x > 0.70 && rect.y > 0.20 && rect.y < 0.55
        )));
    }

    #[test]
    fn milliondollar_result_runtime_events_toggle_observe_timers_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/MILLIONDOLLAR/result.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect("decode MILLIONDOLLAR result skin");
        let cim_sources = decoded
            .sources
            .iter()
            .filter(|source| {
                source
                    .path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("cim"))
            })
            .collect::<Vec<_>>();
        assert_eq!(cim_sources.len(), 11, "all MILLIONDOLLAR CIM atlases must decode");
        assert!(
            cim_sources.iter().all(|source| source.asset.is_some()),
            "MILLIONDOLLAR CIM atlases must provide RGBA assets before GPU upload"
        );
        let document = &decoded.document;
        let circle_destinations = document
            .destination
            .iter()
            .filter_map(|entry| match entry {
                DestinationListEntry::Single(destination)
                    if destination.id == "Graph_Circle_Meter"
                        || destination.id == "Graph_Circle_Frame" =>
                {
                    Some(destination)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(circle_destinations.len(), 724);
        let circle_timers = circle_destinations
            .iter()
            .filter_map(|destination| destination.timer)
            .collect::<BTreeSet<_>>();
        assert_eq!(circle_timers.len(), 1, "the shared circle visibility edge needs one timer");
        assert!(circle_timers.iter().all(|timer| {
            ((*timer - bmz_render::skin::SKIN_DYNAMIC_TIMER_BASE) as usize)
                < bmz_render::skin::SKIN_DYNAMIC_TIMER_COUNT
        }));

        let source_12 = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "12")
            .expect("MILLIONDOLLAR parts atlas");
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: source.size,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let mut circle_runtime = DynamicTimerRuntime::default();
        circle_runtime.reset_for_document(Some(document));
        let mut circle_state = SkinDrawState::default();
        circle_runtime.advance(document, &mut circle_state, 100);
        let circle_items =
            document.static_render_items(&sources, &circle_state, &SkinTextState::default());
        let rendered_segments = circle_items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    SkinRenderItem::RotatedImage { texture, .. }
                        if *texture == source_12.texture
                )
            })
            .count();
        assert!(
            rendered_segments >= 700,
            "MILLIONDOLLAR circle graph segments must render, got {rendered_segments}"
        );
        let circle_angles = circle_items
            .iter()
            .filter_map(|item| match item {
                SkinRenderItem::RotatedImage { texture, angle_deg, center, .. }
                    if *texture == source_12.texture =>
                {
                    assert!((center.x - 0.5).abs() < f32::EPSILON);
                    assert!((center.y - 0.5).abs() < f32::EPSILON);
                    Some(*angle_deg)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(circle_angles.iter().all(|angle| *angle >= 0.0));
        assert!(circle_angles.iter().any(|angle| *angle >= 359.0));

        let event = document.runtime_events.first().expect("runtime toggle event");
        let initial_true = event
            .toggle_flags
            .iter()
            .find(|flag_id| {
                document.runtime_flags.iter().any(|flag| flag.id == **flag_id && flag.initial)
            })
            .copied()
            .expect("initially visible flag");
        let initial_false = event
            .toggle_flags
            .iter()
            .find(|flag_id| {
                document.runtime_flags.iter().any(|flag| flag.id == **flag_id && !flag.initial)
            })
            .copied()
            .expect("initially hidden flag");
        let timer_index = |flag_id: i32| {
            let observe = format!("runtime_flag({flag_id})");
            let timer = document
                .dynamic_timers
                .iter()
                .find(|timer| timer.observe == observe)
                .expect("timer observing runtime flag");
            usize::try_from(timer.id - bmz_render::skin::SKIN_DYNAMIC_TIMER_BASE).unwrap()
        };
        let true_timer = timer_index(initial_true);
        let false_timer = timer_index(initial_false);
        let mut runtime = DynamicTimerRuntime::default();
        runtime.reset_for_document(Some(document));
        let mut state = SkinDrawState::default();

        runtime.advance(document, &mut state, 100);
        assert_eq!(state.dynamic_timer_ms[true_timer], Some(0));
        assert_eq!(state.dynamic_timer_ms[false_timer], None);

        assert!(runtime.dispatch_runtime_event(document, event.id));
        runtime.advance(document, &mut state, 150);
        assert_eq!(state.dynamic_timer_ms[true_timer], None);
        assert_eq!(state.dynamic_timer_ms[false_timer], Some(0));

        assert!(runtime.dispatch_runtime_event(document, event.id));
        runtime.advance(document, &mut state, 200);
        assert_eq!(state.dynamic_timer_ms[true_timer], Some(0));
        assert_eq!(state.dynamic_timer_ms[false_timer], None);
    }

    #[test]
    fn milliondollar_result_song_info_uses_runtime_judge_rank_and_ln_mode_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/MILLIONDOLLAR/result.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin_with_options_and_runtime_state(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState {
                text_values: BTreeMap::from([(1, "RANK AAA".to_string())]),
                option_values: BTreeMap::from([
                    (180, false),
                    (181, true),
                    (182, false),
                    (183, false),
                    (184, false),
                ]),
                event_index_values: BTreeMap::from([(42, 2), (43, 0), (54, 0), (308, 2)]),
                ..LuaLoadRuntimeState::default()
            },
        )
        .expect("decode MILLIONDOLLAR result skin with chart metadata");

        let judge_rank = decoded
            .document
            .image
            .iter()
            .find(|image| image.id == "Parts_Text_Info_Judgerank")
            .expect("MILLIONDOLLAR judge-rank label");
        let ln_type = decoded
            .document
            .image
            .iter()
            .find(|image| image.id == "Parts_Text_Info_Lntype")
            .expect("MILLIONDOLLAR LN-type label");
        let arrange = decoded
            .document
            .image
            .iter()
            .find(|image| image.id == "Parts_Texts_Useoption_SP")
            .expect("MILLIONDOLLAR SP arrange label");
        let target_rank = decoded
            .document
            .image
            .iter()
            .find(|image| image.id == "Parts_Texts_Target_Rank")
            .expect("MILLIONDOLLAR fixed target rank label");
        assert_eq!(judge_rank.y, 310, "HARD must select atlas row 3");
        assert_eq!(ln_type.y, 291, "HCN must select atlas row 2");
        assert_eq!(arrange.y, 48, "RANDOM must select atlas row 2");
        assert_eq!(target_rank.y, 16, "RANK AAA must select the AAA target row");
        assert!(decoded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination)
                if destination.id == "Parts_Texts_Useoption_SP"
        )));
    }

    #[test]
    fn milliondollar_result_uses_integer_only_gauge_layout_at_one_hundred_percent_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/MILLIONDOLLAR/result.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin_with_options_and_runtime_state(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState {
                number_values: BTreeMap::from([(107, 100)]),
                ..LuaLoadRuntimeState::default()
            },
        )
        .expect("decode MILLIONDOLLAR result skin with full gauge");

        let draw_for = |id: &str| {
            decoded.document.destination.iter().find_map(|entry| match entry {
                DestinationListEntry::Single(destination) if destination.id == id => {
                    Some(destination.draw.as_str())
                }
                _ => None,
            })
        };
        assert_eq!(draw_for("Number_Remaingauge_Max_1"), Some("number(107) == 100"));
        assert_eq!(draw_for("Number_Remaingauge_Max_00"), Some("number(107) == 100"));
        assert_eq!(draw_for("Number_Remaingauge_Normal"), Some("number(107) < 100"));
        assert_eq!(draw_for("Parts_Text_Remaingauge_Dot"), Some("number(107) < 100"));
        assert_eq!(draw_for("Number_Remaingauge_Afterdot"), Some("number(107) < 100"));
    }

    #[test]
    fn milliondollar_result_rank_diff_uses_load_time_result_scores_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/MILLIONDOLLAR/result.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin_with_options_and_runtime_state(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState {
                number_values: BTreeMap::from([
                    (71, 2_877),
                    (74, 1_550),
                    (151, 2_756),
                    (170, 3_021),
                    (171, 2_877),
                ]),
                ..LuaLoadRuntimeState::default()
            },
        )
        .expect("decode MILLIONDOLLAR result skin with result scores");

        let best_rank = decoded
            .document
            .image
            .iter()
            .find(|image| image.id == "Parts_Rank_Middle_Best")
            .expect("MILLIONDOLLAR best DJ level");
        let next_rank = decoded
            .document
            .image
            .iter()
            .find(|image| image.id == "Parts_Rank_Nextrank")
            .expect("MILLIONDOLLAR next-rank label");
        let next_rank_diff = decoded
            .document
            .value
            .iter()
            .find(|value| value.id == "Number_Nextrank_Diff")
            .expect("MILLIONDOLLAR next-rank difference");

        assert_eq!(best_rank.y, 0, "3021/3100 must select the AAA row");
        assert_eq!(next_rank.x, 951, "positive rank difference must select the plus label");
        assert_eq!(next_rank.y, 18, "AAA+ must select the plus row");
        assert_eq!(next_rank_diff.value_expr, "121");
    }

    #[test]
    fn starseeker_result_misscount_diff_uses_runtime_number_color_block() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Starseeker/result/result.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([
            ("F/Sリスト".to_string(), "Default".to_string()),
            ("逆サイド詳細フレーム".to_string(), "ON".to_string()),
            ("プレーサイド".to_string(), "1P".to_string()),
        ]);
        let files = BTreeMap::from([
            ("使用テーマ".to_string(), "Theme/starseeker".to_string()),
            ("フォント".to_string(), "_font/TYPE-M".to_string()),
            ("シャッター".to_string(), "Shutter/TYPE-M".to_string()),
        ]);
        let runtime_state = LuaLoadRuntimeState {
            number_values: BTreeMap::from([(178, -1)]),
            text_values: BTreeMap::new(),
            option_values: BTreeMap::new(),
            ..LuaLoadRuntimeState::default()
        };
        let decoded = decode_beatoraja_skin_with_options_and_runtime_state(
            &skin_path,
            SkinKind::Result,
            &options,
            &files,
            &runtime_state,
        )
        .expect("decode starseeker result skin with misscount diff");

        let diff_misscount = decoded
            .document
            .value
            .iter()
            .find(|value| value.id == "Diff_Misscount")
            .expect("starseeker result should define Diff_Misscount");

        assert_eq!(diff_misscount.ref_id, 178);
        assert_eq!(diff_misscount.y, 345);
    }

    #[test]
    fn play_skin_selection_for_returns_per_mode_fields() {
        let mut skin = SkinConfig {
            play4: "skin4.json".to_string(),
            play5: "skin5.json".to_string(),
            play6: "skin6.json".to_string(),
            play7: "skin7.json".to_string(),
            play8: "skin8.json".to_string(),
            play9: "skin9.json".to_string(),
            play10: "skin10.json".to_string(),
            play14: "skin14.json".to_string(),
            battle5: "battle5.json".to_string(),
            battle7: "battle7.json".to_string(),
            ..SkinConfig::default()
        };
        skin.play4_options.insert("g".to_string(), "r".to_string());
        skin.play5_options.insert("a".to_string(), "x".to_string());
        skin.play6_options.insert("f".to_string(), "q".to_string());
        skin.play7_options.insert("b".to_string(), "y".to_string());
        skin.play8_options.insert("h".to_string(), "n".to_string());
        skin.play9_options.insert("e".to_string(), "p".to_string());
        skin.play10_files.insert("c".to_string(), "z.png".to_string());
        skin.play14_files.insert("d".to_string(), "w.png".to_string());
        skin.play7_offsets.push(SkinOffsetConfig { id: 30, h: 7, ..Default::default() });
        skin.play14_offsets.push(SkinOffsetConfig { id: 30, h: 14, ..Default::default() });

        let s4 = play_skin_selection_for(&skin, KeyMode::K4);
        assert_eq!(s4.path, "skin4.json");
        assert!(s4.options.contains_key("g"));

        let s5 = play_skin_selection_for(&skin, KeyMode::K5);
        assert_eq!(s5.path, "skin5.json");
        assert!(s5.options.contains_key("a"));

        let s6 = play_skin_selection_for(&skin, KeyMode::K6);
        assert_eq!(s6.path, "skin6.json");
        assert!(s6.options.contains_key("f"));

        let s7 = play_skin_selection_for(&skin, KeyMode::K7);
        assert_eq!(s7.path, "skin7.json");
        assert!(s7.options.contains_key("b"));
        assert_eq!(s7.offsets[0].h, 7);

        let s8 = play_skin_selection_for(&skin, KeyMode::K8);
        assert_eq!(s8.path, "skin8.json");
        assert!(s8.options.contains_key("h"));

        let s9 = play_skin_selection_for(&skin, KeyMode::K9);
        assert_eq!(s9.path, "skin9.json");
        assert!(s9.options.contains_key("e"));

        let s10 = play_skin_selection_for(&skin, KeyMode::K10);
        assert_eq!(s10.path, "skin10.json");
        assert!(s10.files.contains_key("c"));

        let s14 = play_skin_selection_for(&skin, KeyMode::K14);
        assert_eq!(s14.path, "skin14.json");
        assert!(s14.files.contains_key("d"));
        assert_eq!(s14.offsets[0].h, 14);

        let battle5 =
            play_skin_selection_for_session(&skin, KeyMode::K10, SessionMode::AutoplayBattle);
        assert_eq!(battle5.path, "battle5.json");
        let battle7 =
            play_skin_selection_for_session(&skin, KeyMode::K14, SessionMode::GhostBattle);
        assert_eq!(battle7.path, "battle7.json");
        assert_eq!(
            play_skin_selection_for_session(&skin, KeyMode::K14, SessionMode::Normal).path,
            "skin14.json"
        );
    }

    #[test]
    fn apply_skin_from_config_empty_path_uses_default_skin() {
        let mut renderer = Renderer::default();
        let app_paths = test_app_paths();

        apply_skin_from_config(&mut renderer, &app_paths, "").unwrap();
    }

    #[test]
    fn apply_skin_from_config_rejects_toml_skin_directory() {
        let mut renderer = Renderer::default();
        let app_paths = test_app_paths();
        let path = default_skin_root();

        let error = apply_skin_from_config(&mut renderer, &app_paths, path.to_str().unwrap())
            .unwrap_err()
            .to_string();

        assert!(error.contains("BMZ TOML skin directories are no longer supported"), "{error}");
    }

    #[test]
    fn apply_skin_from_config_json_path_loads_beatoraja_skin_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/beatoraja/skin/default/play7.json");
        if !skin_path.is_file() {
            return;
        }
        let mut renderer = Renderer::default();
        let app_paths = test_app_paths();

        apply_skin_from_config(&mut renderer, &app_paths, skin_path.to_str().unwrap()).unwrap();
    }

    #[test]
    fn apply_skin_from_config_lua_path_loads_beatoraja_skin_when_available() {
        let skin_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play7.luaskin");
        if !skin_path.is_file() {
            return;
        }
        let mut renderer = Renderer::default();
        let app_paths = test_app_paths();

        apply_skin_from_config(&mut renderer, &app_paths, skin_path.to_str().unwrap()).unwrap();
    }

    #[test]
    fn rmz_play8_lua_skin_decodes_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Rmz-skin/play8main.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

        assert_eq!(decoded.document.skin_type, 24);
        let note = decoded.document.note.as_ref().expect("play8 skin should define notes");
        assert_eq!(note.note.len(), 8);
        assert_eq!(note.dst.len(), 8);
    }

    #[test]
    fn antique_play_lua_bakes_configured_keybeam_height_offset_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/mz-select/play/antique/system/play7main.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let load = |height| {
            load_skin_document(
                &skin_path,
                SkinKind::Play,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &LuaLoadRuntimeState {
                    offset_values: BTreeMap::from([(
                        "キービームの長さ".to_string(),
                        bmz_skin::LuaSkinOffsetValue { h: height, ..Default::default() },
                    )]),
                    offset_id_values: BTreeMap::from([(
                        53,
                        bmz_skin::LuaSkinOffsetValue { h: height, ..Default::default() },
                    )]),
                    ..Default::default()
                },
                None,
            )
            .expect("decode Antique play skin")
            .document
        };
        let keybeam_height = |document: &SkinDocument| {
            document.destination.iter().find_map(|entry| match entry {
                DestinationListEntry::Single(destination)
                    if destination.id == "imgset_keybeam1" && destination.timer == Some(101) =>
                {
                    destination.dst.first().and_then(|entry| match entry {
                        bmz_render::skin::SkinDstEntry::Frame(frame) => frame.h,
                        bmz_render::skin::SkinDstEntry::Conditional { .. } => None,
                    })
                }
                _ => None,
            })
        };

        assert_eq!(keybeam_height(&load(0)), Some(564));
        assert_eq!(keybeam_height(&load(37)), Some(601));
    }

    #[test]
    fn simple_play_lua_bakes_configured_note_height_offset_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/simple-play/play7.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let load_note_sizes = |height| {
            load_skin_document(
                &skin_path,
                SkinKind::Play,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &LuaLoadRuntimeState {
                    offset_values: BTreeMap::from([(
                        "ノーツオフセット Notes Offset".to_string(),
                        bmz_skin::LuaSkinOffsetValue { h: height, ..Default::default() },
                    )]),
                    ..Default::default()
                },
                None,
            )
            .expect("decode simple-play skin")
            .document
            .note
            .expect("simple-play note definition")
            .size
        };
        let baseline = load_note_sizes(0);
        let configured = load_note_sizes(7);

        assert_eq!(baseline.len(), configured.len());
        assert!(
            baseline.iter().zip(configured).all(|(before, after)| after == before + 7),
            "simple-play note heights did not receive the configured offset"
        );
    }

    #[test]
    fn rmz_play7_keeps_runtime_stagefile_loading_destinations_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Rmz-skin/play7main.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let destinations = decoded.document.all_destinations(&decoded.document.enabled_options());
        let stagefile_destinations =
            destinations.iter().filter(|destination| destination.id == "-100").collect::<Vec<_>>();

        assert!(stagefile_destinations.iter().any(|destination| {
            destination.timer.is_none()
                && destination.op.contains(&80)
                && destination.op.contains(&191)
        }));
        assert!(stagefile_destinations.iter().any(|destination| {
            destination.timer == Some(40)
                && destination.op.contains(&81)
                && destination.op.contains(&191)
        }));
    }

    #[test]
    fn rmz_play7_lanecover_green_renders_green_number_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/Rmz-skin/play7main.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let lanecover_green_value = decoded
            .document
            .value
            .iter()
            .find(|value| value.id == "lanecover-green")
            .expect("Rmz lanecover green value should decode");
        assert_eq!(
            lanecover_green_value.value_expr, "0.6*number(312)",
            "decoded value: {lanecover_green_value:?}"
        );
        let source = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "play_system_src")
            .expect("Rmz play system source should decode");
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            total_duration_ms: 500,
            duration_green_ms: Some(300),
            lane_cover_changing: true,
            lanecover_enabled: true,
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );
        assert!(
            !items.iter().any(
                |item| matches!(item, bmz_render::skin::SkinRenderItem::Text { text, .. } if text == "FHS")
            ),
            "FHS mark should stay hidden while NHS is active"
        );
        let digit_width = 20.0;
        let source_candidates = items
            .iter()
            .filter_map(|item| {
                if let bmz_render::skin::SkinRenderItem::Image { texture, rect, uv, .. } = item
                    && *texture == source.texture
                {
                    Some((
                        (rect.x * 1920.0).round() as i32,
                        (rect.y * 1080.0).round() as i32,
                        (uv.x * source.size.width / digit_width).round() as i32,
                        (uv.y * source.size.height).round() as i32,
                    ))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut digits = items
            .iter()
            .filter_map(|item| {
                if let bmz_render::skin::SkinRenderItem::Image { texture, rect, uv, .. } = item
                    && *texture == source.texture
                    && (rect.y * 1080.0 - 10.0).abs() < 2.0
                    && (rect.x * 1920.0 - 849.0).abs() < 80.0
                {
                    let digit = (uv.x * source.size.width / digit_width).round() as i32;
                    Some(((rect.x * 1920.0).round() as i32, digit))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        digits.sort_by_key(|(x, _)| *x);
        let digits = digits.into_iter().map(|(_, digit)| digit).collect::<Vec<_>>();

        assert_eq!(digits, vec![3, 0, 0], "source candidates: {source_candidates:?}");

        let fhs_state = bmz_render::skin::SkinDrawState { hispeed_mode_index: 1, ..state.clone() };
        let fhs_items = decoded.document.static_render_items(
            &sources,
            &fhs_state,
            &bmz_render::skin::SkinTextState::default(),
        );
        assert!(
            fhs_items.iter().any(
                |item| matches!(item, bmz_render::skin::SkinRenderItem::Text { text, .. } if text == "FHS")
            ),
            "FHS mark should render while FHS is active"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_decodes_play_document_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        assert_eq!(decoded.document.name, "WMII FHD play AC");
        assert!(decoded.document.w >= 1920);
        assert!(decoded.document.source.len() >= 10);
        assert!(decoded.document.image.len() >= 100);
        assert!(
            decoded.document.source.iter().any(|source| source.id == "110")
                && decoded.document.source.iter().any(|source| source.id == "111"),
            "expected LR2 black/white reference sources"
        );
        let note = decoded.document.note.as_ref().expect("lr2 play skin should define notes");
        assert!(!note.group.is_empty());
        assert!(decoded.document.gauge.is_some());
        assert!(decoded.document.bga.is_some());
        assert!(
            decoded.sources.len() >= 10,
            "expected WMII sources to decode, got {}; source paths: {:?}; decoded: {:?}",
            decoded.sources.len(),
            decoded.document.source.iter().map(|source| source.path.as_str()).collect::<Vec<_>>(),
            decoded.sources.iter().map(|source| source.path.clone()).collect::<Vec<_>>()
        );
        let black = decoded.sources.iter().find(|source| source.source_id == "110").unwrap();
        let white = decoded.sources.iter().find(|source| source.source_id == "111").unwrap();
        assert_eq!(black.asset.as_ref().unwrap().pixels, vec![0, 0, 0, 255]);
        assert_eq!(white.asset.as_ref().unwrap().pixels, vec![255, 255, 255, 255]);
    }

    #[test]
    fn wmii_fhd_lr2skin_can_be_applied_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }
        let mut renderer = Renderer::default();

        apply_beatoraja_json_skin(&mut renderer, &skin_path).unwrap();
    }

    #[test]
    fn wmii_fhd_lr2skin_produces_static_play_items_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([
            ("GRAPH SIDE".to_string(), "LEFT".to_string()),
            ("Score Graph".to_string(), "On".to_string()),
        ]);
        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            &options,
            &BTreeMap::new(),
        )
        .unwrap();
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            ready_timer_ms: Some(2_000),
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );
        assert!(!items.is_empty());
    }

    #[test]
    fn wmii_fhd_lr2skin_renders_play_fadeout_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let black_texture = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "110")
            .map(|source| source.texture)
            .expect("WMII black reference source should decode");
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let state = bmz_render::skin::SkinDrawState { fadeout_ms: Some(500), ..Default::default() };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );

        assert!(
            items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                    if *texture == black_texture
                        && rect.width >= 0.99
                        && rect.height >= 0.99
                        && tint.a > 0.99
            )),
            "expected WMII timer=2 fadeout to draw an opaque fullscreen black image"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_decodes_auto_judge_button_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([("Displayjudge".to_string(), "ON".to_string())]);
        let decoded = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &options,
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            None,
        )
        .unwrap()
        .document;
        let candidates = decoded
            .image
            .iter()
            .filter(|image| image.divx == 1 && image.divy >= 2 && image.h > 0)
            .map(|image| {
                format!(
                    "src={} x={} y={} w={} h={} divy={} ref={} act={:?}",
                    image.src,
                    image.x,
                    image.y,
                    image.w,
                    image.h,
                    image.divy,
                    image.ref_id,
                    image.act
                )
            })
            .collect::<Vec<_>>();
        let auto_judge = decoded
            .image
            .iter()
            .find(|image| image.act == Some(75) && image.divx == 1 && image.divy >= 2)
            .unwrap_or_else(|| {
                panic!(
                    "WMII auto judge button should decode; candidates: {}",
                    candidates.join(" | ")
                )
            });

        assert_eq!(auto_judge.ref_id, 0);
        assert_eq!(auto_judge.click, 2);
        assert_eq!(auto_judge.clickable, Some(false));
        assert!(
            auto_judge.h > 0,
            "WMII auto judge button should keep a positive source height: {auto_judge:?}"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_renders_ac_bga_frame_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let frame_image = decoded
            .document
            .image
            .iter()
            .find(|image| image.src == "2" && image.x == 1016 && image.y == 1276 && image.w == 389)
            .expect("WMII AC frame image should decode");
        let mut destinations = Vec::new();
        for entry in &decoded.document.destination {
            match entry {
                bmz_render::skin::DestinationListEntry::Single(destination) => {
                    destinations.push(destination);
                }
                bmz_render::skin::DestinationListEntry::Conditional {
                    destinations: nested,
                    ..
                } => {
                    destinations.extend(nested.iter());
                }
            }
        }
        let frame_destination = destinations
            .into_iter()
            .find(|destination| {
                destination.id == frame_image.id
                    && destination.op.contains(&33)
                    && destination.op.contains(&41)
                    && destination.op.contains(&30)
            })
            .expect("WMII AC frame destination should decode");
        assert!(
            frame_destination.dst.len() >= 2,
            "expected WMII AC frame destination keyframes, got {:?}",
            frame_destination.dst
        );
        let frame_texture = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "2")
            .expect("WMII AC frame source should load")
            .texture;
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            ready_timer_ms: Some(2_000),
            has_bga: true,
            bga_enabled: true,
            autoplay: true,
            skin_loaded: true,
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );
        assert!(
            items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                    if *texture == frame_texture
                        && (rect.width - 389.0 / 1920.0).abs() < 0.001
                        && tint.a > 0.5
            )),
            "expected WMII AC BGA frame item from source 2; got {items:?}"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_uses_full_note_lane_region_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let area = decoded
            .document
            .note_lane_area(
                bmz_core::lane::Lane::Scratch,
                bmz_core::lane::KeyMode::K7,
                &decoded.document.enabled_options(),
            )
            .expect("WMII scratch lane area should decode");

        assert!((area.x - 75.0 / 1920.0).abs() < 0.001);
        assert!(
            area.height > 0.65,
            "expected LR2 note.dst to define the full scroll lane height, got {area:?}"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_maps_note_sources_by_lr2_lane_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let note = decoded.document.note.as_ref().expect("WMII notes should decode");
        let images = decoded.document.image_map();
        let scratch =
            images.get(note.note[7].as_str()).expect("WMII scratch note image should resolve");
        let key1 = images.get(note.note[0].as_str()).expect("WMII key1 note image should resolve");
        let key2 = images.get(note.note[1].as_str()).expect("WMII key2 note image should resolve");

        assert_eq!((scratch.x, scratch.w), (94, 90));
        assert_eq!((key1.x, key1.w), (187, 52));
        assert_eq!((key2.x, key2.w), (241, 40));
    }

    #[test]
    fn wmii_fhd_lr2skin_inserts_notes_marker_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        assert!(
            decoded
                .document
                .all_destinations(&decoded.document.enabled_options())
                .iter()
                .any(|destination| destination.id == "notes"),
            "LR2 play skins should insert the notes marker at the first DST_NOTE command"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_renders_groove_gauge_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let gauge_texture = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "19")
            .expect("WMII gauge source should load")
            .texture;
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        for gauge_type in [
            bmz_core::clear::GaugeType::AssistEasy,
            bmz_core::clear::GaugeType::Normal,
            bmz_core::clear::GaugeType::Hard,
        ] {
            let state = bmz_render::skin::SkinDrawState {
                elapsed_ms: 2_000,
                play_timer_ms: Some(2_000),
                gauge: 80.0,
                gauge_max: 100.0,
                gauge_border: 80.0,
                gauge_type: gauge_type as i32,
                ..Default::default()
            };

            let items = decoded.document.static_render_items(
                &sources,
                &state,
                &bmz_render::skin::SkinTextState::default(),
            );
            assert!(
                items.iter().any(|item| matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                        if *texture == gauge_texture
                            && (rect.x - 54.0 / 1920.0).abs() < 0.001
                            && rect.width > 0.004
                            && tint.a > 0.5
                )),
                "expected WMII groove gauge item from source 19 for {gauge_type:?}; got {items:?}"
            );
        }
    }

    #[test]
    fn wmii_fhd_lr2skin_renders_lift_cover_when_lifted() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        assert!(
            decoded.document.hidden_cover.iter().any(|cover| cover.id.contains("liftcover")
                && cover.disappear_line == 357
                && !cover.is_disappear_line_link_lift),
            "expected LR2 SRC_LIFT to decode as a liftcover hiddenCover"
        );
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let lift_cover = decoded
            .document
            .hidden_cover
            .iter()
            .find(|cover| cover.id.contains("liftcover"))
            .expect("WMII lift cover hiddenCover should decode");
        let lift_texture = decoded
            .sources
            .iter()
            .find(|source| source.source_id == lift_cover.src)
            .map(|source| source.texture)
            .expect("WMII lift source should decode");
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            offset_lift_px: 0,
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );

        assert!(
            !items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, tint, .. }
                    if *texture == lift_texture && tint.a > 0.5
            )),
            "expected WMII LIFT cover to stay hidden while lift offset is zero"
        );

        let lifted_items = decoded.document.static_render_items(
            &sources,
            &bmz_render::skin::SkinDrawState {
                elapsed_ms: 2_000,
                play_timer_ms: Some(2_000),
                offset_lift_px: 200,
                lift: 200.0 / 1080.0,
                lift_enabled: true,
                ..Default::default()
            },
            &bmz_render::skin::SkinTextState::default(),
        );
        assert!(
            lifted_items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                    if *texture == lift_texture && rect.height < 0.25 && tint.a > 0.5
            )),
            "expected WMII LIFT cover to render clipped once lift offset is active; got {lifted_items:?}"
        );
    }

    #[test]
    fn wmii_fhd_luaskin_renders_lift_cover_when_lifted() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/play7wide.luaskin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let lift_cover = decoded
            .document
            .lift_cover
            .iter()
            .find(|cover| cover.id.eq_ignore_ascii_case("lift"))
            .unwrap_or_else(|| {
                panic!(
                    "WMII Lua lift cover should decode; got {:?}",
                    decoded
                        .document
                        .lift_cover
                        .iter()
                        .map(|cover| (&cover.id, &cover.src))
                        .collect::<Vec<_>>()
                )
            });
        let lift_texture = decoded
            .sources
            .iter()
            .find(|source| source.source_id == lift_cover.src)
            .map(|source| source.texture)
            .expect("WMII Lua lift source should decode");
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();

        let lifted_items = decoded.document.static_render_items(
            &sources,
            &bmz_render::skin::SkinDrawState {
                elapsed_ms: 2_000,
                play_timer_ms: Some(2_000),
                offset_lift_px: 200,
                lift: 200.0 / 1080.0,
                lift_enabled: true,
                ..Default::default()
            },
            &bmz_render::skin::SkinTextState::default(),
        );

        assert!(
            lifted_items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, tint, .. }
                    if *texture == lift_texture && tint.a > 0.5
            )),
            "expected WMII Lua LIFT cover to render once lift offset is active; got {lifted_items:?}"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_moves_judge_line_with_lift_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let judge_line_ids = decoded
            .document
            .image
            .iter()
            .filter(|image| image.src == "1" && image.x == 1231 && image.y == 0)
            .map(|image| image.id.as_str())
            .collect::<Vec<_>>();
        assert!(!judge_line_ids.is_empty(), "expected WMII judge line source image");

        assert!(
            decoded
                .document
                .all_destinations(&decoded.document.enabled_options())
                .iter()
                .any(|destination| judge_line_ids.contains(&destination.id.as_str())
                    && destination.offsets.contains(&3)),
            "expected WMII DST_JUDGELINE to include beatoraja default OFFSET_LIFT"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_renders_score_graph_bars_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::from([("Score Graph".to_string(), "On".to_string())]),
            &BTreeMap::new(),
        )
        .unwrap();
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            total_notes: 1_000,
            past_notes: 500,
            ex_score: 1_000,
            best_ex_score: Some(1_300),
            projected_best_ex_score: Some(650),
            target_ex_score: Some(1_500),
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );

        assert!(
            items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                    if (rect.x - 546.0 / 1920.0).abs() < 0.01
                        && (rect.width - 277.0 / 1920.0).abs() < 0.01
                        && (rect.height - 798.0 / 1080.0).abs() < 0.01
                        && tint.a > 0.5
            )),
            "expected WMII score graph frame/background to render on the left side"
        );
        assert!(
            items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { rect, .. }
                    if (rect.x - 670.0 / 1920.0).abs() < 0.01
                        && rect.width > 0.0
                        && rect.height > 0.05
            )),
            "expected WMII score graph bars to render in the graph area"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_hides_score_graph_and_extends_bga_on_autoplay_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([
            ("BGA Size".to_string(), "Extend".to_string()),
            ("Score Graph".to_string(), "On".to_string()),
        ]);
        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            &options,
            &BTreeMap::new(),
        )
        .unwrap();
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            ready_timer_ms: Some(2_000),
            has_bga: true,
            bga_enabled: true,
            autoplay: true,
            skin_loaded: true,
            total_notes: 1_000,
            past_notes: 500,
            ex_score: 1_000,
            best_ex_score: Some(1_300),
            target_ex_score: Some(1_500),
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );

        assert!(
            items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                    if (rect.x - 726.0 / 1920.0).abs() < 0.01
                        && (rect.width - 1027.0 / 1920.0).abs() < 0.01
                        && tint.a > 0.5
            )),
            "expected WMII autoplay extended BGA frame to render; got {items:?}"
        );
        assert!(
            !items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                    if (rect.x - 546.0 / 1920.0).abs() < 0.01
                        && (rect.width - 277.0 / 1920.0).abs() < 0.01
                        && (rect.height - 798.0 / 1080.0).abs() < 0.01
                        && tint.a > 0.5
            )),
            "WMII score graph frame must stay hidden during autoplay"
        );
        assert!(
            !items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                    if (rect.x - 551.0 / 1920.0).abs() < 0.01
                        && (rect.width - 267.0 / 1920.0).abs() < 0.01
                        && tint.a > 0.5
            )),
            "WMII score graph target labels must stay hidden during autoplay"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_renders_lane_cover_and_lift_numbers_when_adjusting() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let source1 = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "1")
            .expect("WMII number source should decode");
        let number_uv_y = 883.0 / source1.size.height;
        let number_uv_h = 20.0 / source1.size.height;
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            lane_cover: 0.290,
            lift: 0.222,
            total_duration_ms: 517,
            offset_lift_px: (0.222_f32 * 723.0).round() as i32,
            offset_lanecover_px: -(723.0_f32 * 0.290).round() as i32,
            lane_cover_changing: true,
            lanecover_enabled: true,
            lift_enabled: true,
            now_bpm: 88.0,
            main_bpm: 88.0,
            min_bpm: 38.0,
            max_bpm: 156.0,
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );

        let number_digits = items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { texture, uv, .. }
                        if *texture == source1.texture
                            && (uv.y - number_uv_y).abs() < 0.001
                            && (uv.height - number_uv_h).abs() < 0.001
                )
            })
            .collect::<Vec<_>>();
        let white_digits = number_digits
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { tint, .. }
                        if tint.r > 0.95 && tint.g > 0.95 && tint.b > 0.95 && tint.a > 0.5
                )
            })
            .count();
        let green_digits = number_digits
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { tint, .. }
                        if tint.r < 0.4 && tint.g > 0.75 && tint.b < 0.5 && tint.a > 0.5
                )
            })
            .count();
        let green_bpm_cover_digits = number_digits
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { tint, rect, .. }
                        if tint.r < 0.4
                            && tint.g > 0.75
                            && tint.b < 0.5
                            && tint.a > 0.5
                            && (rect.y * 1080.0 - 165.0).abs() < 2.0
                )
            })
            .count();
        let green_bpm_no_cover_digits = number_digits
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { tint, rect, .. }
                        if tint.r < 0.4
                            && tint.g > 0.75
                            && tint.b < 0.5
                            && tint.a > 0.5
                            && (rect.y * 1080.0 - 203.0).abs() < 2.0
                )
            })
            .count();
        let green_digit_ys = number_digits
            .iter()
            .filter_map(|item| {
                if let bmz_render::skin::SkinRenderItem::Image { tint, rect, .. } = item
                    && tint.r < 0.4
                    && tint.g > 0.75
                    && tint.b < 0.5
                    && tint.a > 0.5
                {
                    Some((rect.y * 1080.0).round() as i32)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        assert!(
            white_digits >= 6,
            "expected WMII SUDDEN and LIFT white number digits to render; got {white_digits}"
        );
        assert!(
            green_digits >= 6,
            "expected WMII upper and lower green number digits to render; got {green_digits}"
        );
        assert!(
            green_bpm_cover_digits >= 9,
            "expected WMII BPM green digits to use lanecover-on layout; got {green_bpm_cover_digits}; green ys {green_digit_ys:?}"
        );
        assert_eq!(
            green_bpm_no_cover_digits, 0,
            "expected WMII BPM green digits not to use lanecover-off layout when op271 is active"
        );

        let zero_lift_state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            lane_cover: 0.290,
            lift: 0.0,
            total_duration_ms: 517,
            offset_lift_px: 0,
            offset_lanecover_px: -(723.0_f32 * 0.290).round() as i32,
            lane_cover_changing: true,
            lanecover_enabled: true,
            lift_enabled: true,
            now_bpm: 88.0,
            main_bpm: 88.0,
            min_bpm: 38.0,
            max_bpm: 156.0,
            ..Default::default()
        };
        let zero_lift_items = decoded.document.static_render_items(
            &sources,
            &zero_lift_state,
            &bmz_render::skin::SkinTextState::default(),
        );
        let zero_lift_digits = zero_lift_items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { texture, uv, rect, .. }
                        if *texture == source1.texture
                            && (uv.y - number_uv_y).abs() < 0.001
                            && (uv.height - number_uv_h).abs() < 0.001
                            && (rect.y * 1080.0 - 724.0).abs() < 2.0
                )
            })
            .collect::<Vec<_>>();
        let zero_lift_white_digits = zero_lift_digits
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { tint, .. }
                        if tint.r > 0.95 && tint.g > 0.95 && tint.b > 0.95 && tint.a > 0.5
                )
            })
            .count();
        let zero_lift_green_digits = zero_lift_digits
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { tint, .. }
                        if tint.r < 0.4 && tint.g > 0.75 && tint.b < 0.5 && tint.a > 0.5
                )
            })
            .count();
        assert!(
            zero_lift_white_digits > 0,
            "expected WMII LIFT white digits to render even when LIFT is zero"
        );
        assert!(
            zero_lift_green_digits > 0,
            "expected WMII LIFT green digits to render even when LIFT is zero"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_renders_runtime_difficulty_badge_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            difficulty: 4,
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );

        assert!(
            items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                    if (rect.x - 617.0 / 1920.0).abs() < 0.01
                        && (rect.width - 187.0 / 1920.0).abs() < 0.01
                        && tint.a > 0.1
            )),
            "expected WMII ANOTHER difficulty badge to render for difficulty op154"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_renders_judge_and_combo_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([("Displayjudge".to_string(), "ON".to_string())]);
        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            &options,
            &BTreeMap::new(),
        )
        .unwrap();
        let judge_texture = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "13")
            .expect("WMII judge source should load")
            .texture;
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let mut judge_ms = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
        judge_ms[0] = Some(100);
        let mut judge_index = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
        judge_index[0] = Some(0);
        let mut judge_combo = [0; bmz_render::skin::MAX_JUDGE_REGIONS];
        judge_combo[0] = 123;
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            judge_ms,
            judge_index,
            judge_combo,
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );
        let judge_items = items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                        if *texture == judge_texture
                            && rect.height > 0.01
                            && tint.a > 0.5
                )
            })
            .count();

        assert!(
            judge_items >= 2,
            "expected WMII judge text and combo digits from source 13; got {items:?}"
        );
        assert!(
            items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, rect, uv, tint, .. }
                    if *texture == judge_texture
                        && rect.height > 0.05
                        && uv.y < 0.001
                        && tint.a > 0.5
            )),
            "expected PGREAT judge image to use the top WMII judge source row; got {items:?}"
        );

        for (judge_index, label) in ["PGREAT", "GREAT", "GOOD", "BAD", "POOR"].iter().enumerate() {
            let mut judge_ms = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
            judge_ms[0] = Some(100);
            let mut judge_indices = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
            judge_indices[0] = Some(judge_index);
            let mut judge_combo = [0; bmz_render::skin::MAX_JUDGE_REGIONS];
            judge_combo[0] = 123;
            let state = bmz_render::skin::SkinDrawState {
                elapsed_ms: 2_000,
                play_timer_ms: Some(2_000),
                judge_ms,
                judge_index: judge_indices,
                judge_combo,
                ..Default::default()
            };
            let items = decoded.document.static_render_items(
                &sources,
                &state,
                &bmz_render::skin::SkinTextState::default(),
            );
            assert!(
                items.iter().any(|item| matches!(
                    item,
                    bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                        if *texture == judge_texture
                            && rect.height > 0.05
                            && tint.a > 0.5
                )),
                "expected WMII {label} judge image to render; got {items:?}"
            );
        }
    }

    #[test]
    fn wmii_fhd_lr2skin_dp_renders_judge_detail_panel_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC_DP.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([
            ("Displayjudge".to_string(), "ON".to_string()),
            ("GRAPH SIDE".to_string(), "RIGHT".to_string()),
            ("Score Graph".to_string(), "On".to_string()),
        ]);
        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            &options,
            &BTreeMap::new(),
        )
        .unwrap();

        assert!(
            decoded.document.enabled_options().contains(&983),
            "expected WMII DP judge detail panel op983 to stay enabled"
        );

        let frame_texture = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "1")
            .expect("WMII frame source should load")
            .texture;
        let sources = decoded
            .sources
            .iter()
            .map(|source| {
                (
                    source.source_id.clone(),
                    SkinDocumentTexture {
                        source_id: source.source_id.clone(),
                        texture: source.texture,
                        source_size: SkinImageSize {
                            width: source.size.width,
                            height: source.size.height,
                        },
                    },
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            key_mode: bmz_core::lane::KeyMode::K14,
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );

        assert!(
            items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                    if *texture == frame_texture
                        && (rect.x - 71.0 / 1920.0).abs() < 0.01
                        && (rect.width - 247.0 / 1920.0).abs() < 0.02
                        && rect.height > 0.1
                        && tint.a > 0.1
            )),
            "expected WMII DP judge detail panel body to render; got {items:?}"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_renders_fast_slow_during_replay_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([("Display FAST/SLOW".to_string(), "ON-A".to_string())]);
        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            &options,
            &BTreeMap::new(),
        )
        .unwrap();
        let sources = decoded.sources.iter().map(|source| SkinDocumentTexture {
            source_id: source.source_id.clone(),
            texture: source.texture,
            source_size: SkinImageSize { width: source.size.width, height: source.size.height },
        });
        let skin = SkinContext::from_manifest_and_document(
            SkinManifest::default(),
            decoded.document.clone(),
            sources,
        );
        let replay_snapshot = bmz_render::snapshot::RenderSnapshot {
            time: TimeUs(100_000),
            play_elapsed_time: TimeUs(100_000),
            replay_playback: true,
            key_mode: bmz_core::lane::KeyMode::K7,
            recent_judgements: vec![bmz_render::snapshot::DisplayJudgement {
                lane: bmz_core::lane::Lane::Key1,
                judge: bmz_core::judge::Judge::PGreat,
                side: Some(bmz_core::judge::TimingSide::Fast),
                text: "PGREAT FAST".to_string(),
                combo: 1,
                delta_us: -2_000,
                time: TimeUs(0),
                is_miss: false,
                timing_ms_suppressed: false,
            }],
            ..Default::default()
        };
        let has_wmii_fast_slow_image = |plan: &DrawPlan| {
            plan.commands.iter().any(|command| {
                matches!(
                    command,
                    DrawCommand::Image { rect, tint, .. }
                        if ((rect.x - 292.0 / 1920.0).abs() < 0.01
                            || (rect.x - 246.0 / 1920.0).abs() < 0.01)
                            && (rect.y - 502.0 / 1080.0).abs() < 0.01
                            && (rect.width - 82.0 / 1920.0).abs() < 0.01
                            && tint.a > 0.5
                )
            })
        };

        let mut snapshot = replay_snapshot.clone();
        crate::screens::play_snapshot::apply_fast_slow_display_filter(
            &mut snapshot,
            0,
            crate::config::profile_config::FastSlowDisplayScope::ThresholdMs,
        );

        let plan = DrawPlan::from_scene_with_skin(
            &AppSceneSnapshot::Play(snapshot),
            &skin,
            &mut DynamicTimerRuntime::default(),
        );

        assert!(
            has_wmii_fast_slow_image(&plan),
            "expected WMII replay PGREAT FAST/SLOW image to render; got {:?}",
            plan.commands
        );

        let mut auto_snapshot = replay_snapshot;
        crate::screens::play_snapshot::apply_fast_slow_display_filter(
            &mut auto_snapshot,
            0,
            crate::config::profile_config::FastSlowDisplayScope::Auto,
        );
        let auto_plan = DrawPlan::from_scene_with_skin(
            &AppSceneSnapshot::Play(auto_snapshot),
            &skin,
            &mut DynamicTimerRuntime::default(),
        );

        assert!(
            !has_wmii_fast_slow_image(&auto_plan),
            "expected WMII Auto scope to hide replay PGREAT FAST/SLOW; got {:?}",
            auto_plan.commands
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_applies_play_timing_headers_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

        assert_eq!(decoded.document.loadstart, 0);
        assert_eq!(decoded.document.loadend, 3000);
        assert_eq!(decoded.document.playstart, 1500);
        assert_eq!(decoded.document.fadeout, 500);
        assert_eq!(decoded.document.close, 2500);
    }

    #[test]
    fn wmii_fhd_lr2skin_uses_lr2_bitmap_fonts_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

        assert!(
            decoded.document.font.iter().any(|font| {
                font.id.starts_with("lr2font-")
                    && font.path.replace('\\', "/").ends_with("../font/songTitle/font.fnt")
            }),
            "expected LR2FONT font.lr2font to resolve to bundled font.fnt; got {:?}",
            decoded.document.font
        );
        assert!(
            decoded.document.text.iter().any(|text| {
                text.ref_id == 12 && text.font.starts_with("play:lr2font-") && text.size == 0
            }),
            "expected full-title text to keep its LR2 bitmap font id; got {:?}",
            decoded.document.text
        );
        assert!(
            decoded.document.text.iter().any(|text| {
                text.ref_id == 10 && text.font.starts_with("play:lr2font-") && text.size == 0
            }),
            "expected READY title text to use LR2 bitmap font index 0; got {:?}",
            decoded.document.text
        );
        assert!(
            decoded.document.text.iter().any(|text| {
                text.ref_id == 14 && text.font.starts_with("play:lr2font-") && text.size == 0
            }),
            "expected artist text to keep its LR2 bitmap font id; got {:?}",
            decoded.document.text
        );
        assert!(
            decoded.fonts.iter().any(|font| {
                font.stored_id.starts_with("play:lr2font-")
                    && matches!(font.data.as_ref(), Some(DecodedFontData::Bitmap(_)))
            }),
            "expected decoded LR2 bitmap font to be loaded"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_uses_dst_text_size_for_lr2_bitmap_fonts_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let title_id = decoded
            .document
            .text
            .iter()
            .find(|text| text.ref_id == 12)
            .map(|text| text.id.as_str())
            .expect("WMII full-title text should exist");
        let has_frame_height = |id: &str, height: i32| {
            decoded.document.destination.iter().any(|entry| match entry {
                bmz_render::skin::DestinationListEntry::Single(destination) => {
                    destination.id == id
                        && destination.dst.iter().any(|frame| match frame {
                            bmz_render::skin::SkinDstEntry::Frame(frame) => frame.h == Some(height),
                            bmz_render::skin::SkinDstEntry::Conditional { frames, .. } => {
                                frames.iter().any(|frame| frame.h == Some(height))
                            }
                        })
                }
                bmz_render::skin::DestinationListEntry::Conditional { destinations, .. } => {
                    destinations.iter().any(|destination| {
                        destination.id == id
                            && destination.dst.iter().any(|frame| match frame {
                                bmz_render::skin::SkinDstEntry::Frame(frame) => {
                                    frame.h == Some(height)
                                }
                                bmz_render::skin::SkinDstEntry::Conditional { frames, .. } => {
                                    frames.iter().any(|frame| frame.h == Some(height))
                                }
                            })
                    })
                }
            })
        };

        assert!(
            has_frame_height(title_id, 41),
            "expected WMII full-title bitmap font size to come from DST_TEXT h=41"
        );
        assert!(
            decoded.document.text.iter().any(|text| {
                text.ref_id == 14
                    && text.font.starts_with("play:lr2font-")
                    && has_frame_height(&text.id, 29)
            }),
            "expected WMII artist bitmap font size to come from DST_TEXT h=29"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_uses_lr2_bitmap_font_for_table_level_when_enabled() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([("Display Table Level".to_string(), "ON".to_string())]);
        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            &options,
            &BTreeMap::new(),
        )
        .unwrap();

        assert!(
            decoded.document.text.iter().any(|text| {
                text.ref_id == 1002 && text.font.starts_with("play:lr2font-") && text.size == 0
            }),
            "expected difficulty-table text to keep its LR2 bitmap font id; got {:?}",
            decoded.document.text
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_preserves_green_number_digit_width_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
        let green_numbers = decoded
            .document
            .value
            .iter()
            .filter(|value| matches!(value.ref_id, 313 | 1317 | 1321 | 1325))
            .collect::<Vec<_>>();

        assert!(!green_numbers.is_empty(), "expected WMII green-number value sprites");
        assert!(
            green_numbers.iter().all(|value| value.digit == 3),
            "LR2 keta field should remain 3 digits for WMII green numbers; got {green_numbers:?}"
        );

        assert!(
            decoded.document.value.iter().any(|value| value.ref_id == 310 && value.digit == 1),
            "expected WMII white high-speed integer digit to use LR2 keta=1"
        );
        assert!(
            decoded.document.value.iter().any(|value| value.ref_id == 311 && value.digit == 2),
            "expected WMII white high-speed decimal digits to use LR2 keta=2"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_keeps_runtime_difficulty_option_destinations_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

        for op in 150..=155 {
            assert!(
                decoded.document.destination.iter().any(|entry| match entry {
                    bmz_render::skin::DestinationListEntry::Single(destination) =>
                        destination.op.contains(&op),
                    bmz_render::skin::DestinationListEntry::Conditional {
                        destinations, ..
                    } => destinations.iter().any(|destination| destination.op.contains(&op)),
                }),
                "expected runtime difficulty op {op} to survive LR2 #IF conversion"
            );
        }
    }

    #[test]
    fn wmii_fhd_lr2skin_uses_relative_combo_destination_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([("Displayjudge".to_string(), "ON".to_string())]);
        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            &options,
            &BTreeMap::new(),
        )
        .unwrap();

        assert!(
            decoded.document.judge.iter().flat_map(|judge| &judge.numbers).any(|number| {
                number.dst.iter().any(|entry| match entry {
                    bmz_render::skin::SkinDstEntry::Frame(frame) => {
                        frame.x == Some(242) && frame.y == Some(0) && frame.h == Some(124)
                    }
                    bmz_render::skin::SkinDstEntry::Conditional { frames, .. } => {
                        frames.iter().any(|frame| {
                            frame.x == Some(242) && frame.y == Some(0) && frame.h == Some(124)
                        })
                    }
                })
            }),
            "expected WMII NOWCOMBO destination to stay relative to judge image"
        );
        assert!(
            decoded
                .document
                .judge
                .iter()
                .flat_map(|judge| &judge.images)
                .any(|image| { image.offsets.contains(&3) && image.offsets.contains(&32) }),
            "expected WMII NOWJUDGE destinations to include beatoraja LR2 judge and lift offsets"
        );
        assert!(
            decoded
                .document
                .judge
                .iter()
                .flat_map(|judge| &judge.numbers)
                .any(|number| { number.offsets.contains(&3) && number.offsets.contains(&32) }),
            "expected WMII NOWCOMBO destinations to include beatoraja LR2 judge and lift offsets"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_defaults_score_graph_to_off_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

        assert!(decoded.document.graph.iter().all(|graph| !matches!(graph.graph_type, 110..=115)));
        assert!(
            decoded
                .document
                .property
                .iter()
                .any(|property| property.name == "Score Graph" && property.def == "Off"),
            "expected beatoraja's built-in Score Graph option to default to Off"
        );
    }

    #[test]
    fn wmii_fhd_lr2skin_2p_side_maps_single_play_notes_to_active_lanes_when_available() {
        let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !skin_path.is_file() {
            return;
        }

        let options = BTreeMap::from([("PLAY SIDE".to_string(), "2P".to_string())]);
        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            &options,
            &BTreeMap::new(),
        )
        .unwrap();
        let note = decoded.document.note.as_ref().expect("WMII note definition should load");

        assert!(
            note.dst.len() <= 8,
            "single-play 2P side should remap LR2 2P lanes into active lanes; got {} dst lanes",
            note.dst.len()
        );
        assert!(
            note.dst.iter().take(8).any(|entry| match entry {
                bmz_render::skin::SkinDstEntry::Frame(frame) =>
                    frame.w.unwrap_or_default() > 0 && frame.h.unwrap_or_default() > 0,
                bmz_render::skin::SkinDstEntry::Conditional { frames, .. } =>
                    frames.iter().any(|frame| {
                        frame.w.unwrap_or_default() > 0 && frame.h.unwrap_or_default() > 0
                    }),
            }),
            "expected remapped 2P note lanes to have visible destinations"
        );
    }

    #[test]
    fn wildcard_skin_source_prefers_filepath_default() {
        let root = unique_test_dir("bmz-json-source");
        std::fs::create_dir_all(root.join("parts")).unwrap();
        std::fs::write(root.join("parts/default.png"), []).unwrap();
        std::fs::write(root.join("parts/blue.png"), []).unwrap();
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "filepath": [
                    { "name": "Parts", "path": "parts/*.png", "def": "blue" }
                ]
            }
            "#,
        )
        .unwrap();

        let resolved =
            resolve_json_skin_source_path(&root, "parts/*.png", &document, &BTreeMap::new())
                .unwrap();

        assert_eq!(resolved.file_name().and_then(|name| name.to_str()), Some("blue.png"));
    }

    #[test]
    fn wildcard_skin_source_prefers_user_file_selection() {
        let root = unique_test_dir("bmz-json-source");
        std::fs::create_dir_all(root.join("parts")).unwrap();
        std::fs::write(root.join("parts/default.png"), []).unwrap();
        std::fs::write(root.join("parts/blue.png"), []).unwrap();
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "filepath": [
                    { "name": "Parts", "path": "parts/*.png", "def": "blue" }
                ]
            }
            "#,
        )
        .unwrap();
        // ユーザ選択は `def` (blue) より優先される。
        let files = BTreeMap::from([("Parts".to_string(), "parts/default.png".to_string())]);

        let resolved =
            resolve_json_skin_source_path(&root, "parts/*.png", &document, &files).unwrap();

        assert_eq!(resolved.file_name().and_then(|name| name.to_str()), Some("default.png"));
    }

    #[test]
    fn wildcard_skin_source_falls_back_when_user_selection_missing() {
        let root = unique_test_dir("bmz-json-source");
        std::fs::create_dir_all(root.join("parts")).unwrap();
        std::fs::write(root.join("parts/default.png"), []).unwrap();
        std::fs::write(root.join("parts/blue.png"), []).unwrap();
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "filepath": [
                    { "name": "Parts", "path": "parts/*.png", "def": "blue" }
                ]
            }
            "#,
        )
        .unwrap();
        // 存在しないファイルを選択 → `def` (blue) へフォールバック。
        let files = BTreeMap::from([("Parts".to_string(), "parts/missing.png".to_string())]);

        let resolved =
            resolve_json_skin_source_path(&root, "parts/*.png", &document, &files).unwrap();

        assert_eq!(resolved.file_name().and_then(|name| name.to_str()), Some("blue.png"));
    }

    #[test]
    fn wildcard_skin_source_ignores_beatoraja_filter_suffix() {
        let root = unique_test_dir("bmz-json-source-filter");
        std::fs::create_dir_all(root.join("parts/lanecover_lift")).unwrap();
        std::fs::write(root.join("parts/lanecover_lift/default.png"), []).unwrap();
        std::fs::write(root.join("parts/lanecover_lift/TYPE-M.png"), []).unwrap();
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "filepath": [
                    {
                        "name": "レーンカバー",
                        "path": "parts/lanecover_lift/*.png|lanecover|",
                        "def": "default"
                    }
                ]
            }
            "#,
        )
        .unwrap();

        let resolved = resolve_json_skin_source_path(
            &root,
            "parts/lanecover_lift/*.png|lanecover|",
            &document,
            &BTreeMap::new(),
        )
        .unwrap();

        assert_eq!(resolved.file_name().and_then(|name| name.to_str()), Some("default.png"));
    }

    #[test]
    fn wildcard_skin_source_randomly_selects_match() {
        // beatoraja の SkinLoader.getPath 同様、ユーザ選択も def も無いワイルドカードは
        // ロードごとにランダムへ解決する。複数回呼んで両方の候補が選ばれることを確認。
        let root = unique_test_dir("bmz-json-source");
        std::fs::create_dir_all(root.join("parts")).unwrap();
        std::fs::write(root.join("parts/a.png"), []).unwrap();
        std::fs::write(root.join("parts/b.png"), []).unwrap();
        let document: SkinDocument = serde_json::from_str("{}").unwrap();

        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let resolved =
                resolve_json_skin_source_path(&root, "parts/*.png", &document, &BTreeMap::new())
                    .unwrap();
            let name =
                resolved.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string();
            assert!(name == "a.png" || name == "b.png", "unexpected match {name}");
            seen.insert(name);
        }
        assert_eq!(seen.len(), 2, "both candidates should be selected over many loads");
    }

    #[test]
    fn wildcard_skin_source_explicit_random_overrides_def() {
        // ユーザが明示的に "Random" を選んだら、具体 def があってもランダムにする。
        let root = unique_test_dir("bmz-json-source-explicit-random");
        std::fs::create_dir_all(root.join("parts")).unwrap();
        std::fs::write(root.join("parts/blue.png"), []).unwrap();
        std::fs::write(root.join("parts/red.png"), []).unwrap();
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "filepath": [
                    { "name": "Parts", "path": "parts/*.png", "def": "blue" }
                ]
            }
            "#,
        )
        .unwrap();
        let files = BTreeMap::from([("Parts".to_string(), RANDOM_FILE_SELECTION.to_string())]);

        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let resolved =
                resolve_json_skin_source_path(&root, "parts/*.png", &document, &files).unwrap();
            let name =
                resolved.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string();
            assert!(name == "blue.png" || name == "red.png", "unexpected match {name}");
            seen.insert(name);
        }
        assert_eq!(seen.len(), 2, "explicit Random should ignore def and pick randomly");
    }

    #[test]
    fn wildcard_skin_source_random_def_selects_match() {
        // filepath の def が "Random" の場合も具体ファイルとして解決せずランダムにする。
        let root = unique_test_dir("bmz-json-source-random-def");
        std::fs::create_dir_all(root.join("bg")).unwrap();
        std::fs::write(root.join("bg/one.mp4"), []).unwrap();
        std::fs::write(root.join("bg/two.mp4"), []).unwrap();
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "filepath": [
                    { "name": "BG", "path": "bg/*.mp4", "def": "Random" }
                ]
            }
            "#,
        )
        .unwrap();

        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let resolved =
                resolve_json_skin_source_path(&root, "bg/*.mp4", &document, &BTreeMap::new())
                    .unwrap();
            let name =
                resolved.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string();
            assert!(name == "one.mp4" || name == "two.mp4", "unexpected match {name}");
            seen.insert(name);
        }
        assert_eq!(seen.len(), 2, "def=Random should pick randomly among matches");
    }

    #[test]
    fn wildcard_skin_font_resolves_nested_file() {
        let root = unique_test_dir("bmz-json-font");
        std::fs::create_dir_all(root.join("frame/SP/Default")).unwrap();
        std::fs::write(root.join("frame/SP/Default/song.fnt"), []).unwrap();
        let document: SkinDocument = serde_json::from_str("{}").unwrap();

        let resolved =
            resolve_json_skin_asset_path(&root, "frame/SP/*/song.fnt", &document, &BTreeMap::new())
                .unwrap();

        assert_eq!(resolved.strip_prefix(&root).unwrap(), Path::new("frame/SP/Default/song.fnt"));
    }

    #[test]
    fn skin_asset_path_resolves_case_insensitive_file_names() {
        let root = unique_test_dir("bmz-json-font-case");
        std::fs::create_dir_all(root.join("_font")).unwrap();
        std::fs::write(root.join("_font/Artist.fnt"), []).unwrap();
        let document: SkinDocument = serde_json::from_str("{}").unwrap();

        let resolved =
            resolve_json_skin_asset_path(&root, "_font/artist.fnt", &document, &BTreeMap::new())
                .unwrap();

        assert_eq!(resolved.strip_prefix(&root).unwrap(), Path::new("_font/Artist.fnt"));
    }

    #[test]
    fn lr2_document_cache_reuses_when_unused_option_changes() {
        let root = unique_test_dir("bmz-lr2-document-cache-option");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("play.lr2skin");
        std::fs::write(
            &skin_path,
            r#"
#INFORMATION,0,Cache Test,Author
#CUSTOMOPTION,Unused,930,Off,On
#CUSTOMOPTION,Branch,910,Off,On
#IF,911
#IMAGE,on.png
#ELSE
#IMAGE,off.png
#ENDIF
"#,
        )
        .unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

        let first = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(first.document.source[0].path, "off.png");

        let unused_changed = BTreeMap::from([("Unused".to_string(), "On".to_string())]);
        let second = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &unused_changed,
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(second.cache_status, DocumentCacheStatus::Hit);
        assert_eq!(second.document.source[0].path, "off.png");
        assert!(second.document.enabled_options().contains(&931));

        let branch_changed = BTreeMap::from([("Branch".to_string(), "On".to_string())]);
        let third = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &branch_changed,
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(third.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(third.document.source[0].path, "on.png");
    }

    #[test]
    fn lr2_document_cache_misses_when_play_side_remap_changes() {
        let root = unique_test_dir("bmz-lr2-document-cache-play-side");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("play.lr2skin");
        std::fs::write(
            &skin_path,
            r#"
#INFORMATION,0,Cache Test,Author
#CUSTOMOPTION,PLAY SIDE,900,1P,2P
#IMAGE,base.png
"#,
        )
        .unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

        let first = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(first.document.source[0].path, "base.png");

        let play_side_2p = BTreeMap::from([("PLAY SIDE".to_string(), "2P".to_string())]);
        let second = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &play_side_2p,
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache),
        )
        .unwrap();
        assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(second.document.source[0].path, "base.png");
    }

    #[test]
    fn lr2_document_cache_misses_when_included_file_changes() {
        let root = unique_test_dir("bmz-lr2-document-cache-include");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("play.lr2skin");
        let include_path = root.join("parts.csv");
        std::fs::write(
            &skin_path,
            r#"
#INFORMATION,0,Cache Test,Author
#INCLUDE,parts.csv
"#,
        )
        .unwrap();
        std::fs::write(&include_path, "#IMAGE,off.png\n").unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

        let first = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(first.document.source[0].path, "off.png");

        std::fs::write(&include_path, "#IMAGE,on-longer-name.png\n").unwrap();
        let second = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache),
        )
        .unwrap();
        assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(second.document.source[0].path, "on-longer-name.png");
    }

    #[test]
    fn lr2_document_cache_misses_when_used_file_selection_changes() {
        let root = unique_test_dir("bmz-lr2-document-cache-file");
        std::fs::create_dir_all(root.join("parts")).unwrap();
        std::fs::write(root.join("parts/blue.png"), []).unwrap();
        std::fs::write(root.join("parts/red.png"), []).unwrap();
        let skin_path = root.join("play.lr2skin");
        std::fs::write(
            &skin_path,
            r#"
#INFORMATION,0,Cache Test,Author
#CUSTOMFILE,Parts,parts/*.png,blue
#IMAGE,parts/*.png
"#,
        )
        .unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

        let first = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(first.document.source[0].path, "parts/blue.png");

        let selected = BTreeMap::from([("Parts".to_string(), "red.png".to_string())]);
        let second = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &selected,
            &LuaLoadRuntimeState::default(),
            Some(cache),
        )
        .unwrap();
        assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(second.document.source[0].path, "parts/red.png");
    }

    #[test]
    fn lua_document_cache_reuses_when_unused_option_changes() {
        let root = unique_test_dir("bmz-lua-document-cache-option");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("play.luaskin");
        std::fs::write(
            &skin_path,
            r#"
local branch = 910
if skin_config and skin_config.option then
    branch = skin_config.option["Branch"] or 910
end
return {
    type = 0,
    property = {
        { name = "Unused", item = {{ name = "Off", op = 900 }, { name = "On", op = 901 }}, def = "Off" },
        { name = "Branch", item = {{ name = "Off", op = 910 }, { name = "On", op = 911 }}, def = "Off" },
    },
    source = {
        { id = "bg", path = branch == 911 and "on.png" or "off.png" },
    },
}
"#,
        )
        .unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

        let first = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(first.document.source[0].path, "off.png");

        let unused_changed = BTreeMap::from([("Unused".to_string(), "On".to_string())]);
        let second = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &unused_changed,
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(second.cache_status, DocumentCacheStatus::Hit);
        assert_eq!(second.document.source[0].path, "off.png");
        assert!(second.document.enabled_options().contains(&901));

        let branch_changed = BTreeMap::from([("Branch".to_string(), "On".to_string())]);
        let third = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &branch_changed,
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache),
        )
        .unwrap();
        assert_eq!(third.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(third.document.source[0].path, "on.png");
    }

    #[test]
    fn lua_document_cache_misses_when_required_module_option_changes() {
        let root = unique_test_dir("bmz-lua-document-cache-required-option");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("play.luaskin");
        let module_path = root.join("parts.lua");
        std::fs::write(
            &skin_path,
            r#"
local parts = require("parts")
return parts.build()
"#,
        )
        .unwrap();
        std::fs::write(
            &module_path,
            r#"
local M = {}
function M.build()
    local branch = 910
    if skin_config and skin_config.option then
        branch = skin_config.option["Branch"] or 910
    end
    return {
        type = 0,
        property = {
            { name = "Unused", item = {{ name = "Off", op = 900 }, { name = "On", op = 901 }}, def = "Off" },
            { name = "Branch", item = {{ name = "Off", op = 910 }, { name = "On", op = 911 }}, def = "Off" },
        },
        source = {
            { id = "bg", path = branch == 911 and "on.png" or "off.png" },
        },
    }
end
return M
"#,
        )
        .unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

        let first = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(first.document.source[0].path, "off.png");

        let unused_changed = BTreeMap::from([("Unused".to_string(), "On".to_string())]);
        let second = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &unused_changed,
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(second.cache_status, DocumentCacheStatus::Hit);
        assert_eq!(second.document.source[0].path, "off.png");

        let branch_changed = BTreeMap::from([("Branch".to_string(), "On".to_string())]);
        let third = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &branch_changed,
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache),
        )
        .unwrap();
        assert_eq!(third.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(third.document.source[0].path, "on.png");
    }

    #[test]
    fn lua_document_cache_misses_when_runtime_number_changes() {
        let root = unique_test_dir("bmz-lua-document-cache-number");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("result.luaskin");
        std::fs::write(
            &skin_path,
            r#"
local main_state = require("main_state")
local diff = main_state.number(178)
return {
    type = 7,
    source = {
        { id = "bg", path = diff == 0 and "zero.png" or "nonzero.png" },
    },
}
"#,
        )
        .unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

        let zero_state = LuaLoadRuntimeState {
            number_values: BTreeMap::from([(178, 0)]),
            text_values: BTreeMap::new(),
            option_values: BTreeMap::new(),
            ..LuaLoadRuntimeState::default()
        };
        let first = load_skin_document(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &zero_state,
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(first.document.source[0].path, "zero.png");

        let nonzero_state = LuaLoadRuntimeState {
            number_values: BTreeMap::from([(178, -1)]),
            text_values: BTreeMap::new(),
            option_values: BTreeMap::new(),
            ..LuaLoadRuntimeState::default()
        };
        let second = load_skin_document(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &nonzero_state,
            Some(cache),
        )
        .unwrap();
        assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(second.document.source[0].path, "nonzero.png");
    }

    #[test]
    fn lua_document_cache_misses_when_runtime_offset_changes() {
        let root = unique_test_dir("bmz-lua-document-cache-offset");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("play.luaskin");
        std::fs::write(
            &skin_path,
            r#"
local skin = {
    type = 1,
    offset = {
        { name = "Panel", id = 42, x = true },
    },
}
if skin_config == nil then
    return skin
end
local panel_x = skin_config.offset["Panel"].x
skin.source = {
    { id = "bg", path = panel_x == 0 and "zero.png" or "nonzero.png" },
}
return skin
"#,
        )
        .unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));
        let offset = |x| LuaLoadRuntimeState {
            offset_values: BTreeMap::from([(
                "Panel".to_string(),
                bmz_skin::LuaSkinOffsetValue { x, ..Default::default() },
            )]),
            offset_id_values: BTreeMap::from([(
                42,
                bmz_skin::LuaSkinOffsetValue { x, ..Default::default() },
            )]),
            ..Default::default()
        };

        let first = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &offset(0),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(first.document.source[0].path, "zero.png");

        let same = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &offset(0),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(same.cache_status, DocumentCacheStatus::Hit);

        let changed = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &offset(12),
            Some(cache),
        )
        .unwrap();
        assert_eq!(changed.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(changed.document.source[0].path, "nonzero.png");
    }

    #[test]
    fn lua_document_cache_misses_when_runtime_event_index_changes() {
        let root = unique_test_dir("bmz-lua-document-cache-event-index");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("result.luaskin");
        std::fs::write(
            &skin_path,
            r#"
local main_state = require("main_state")
local lnmode = main_state.event_index(308)
return {
    type = 7,
    source = {
        { id = "bg", path = lnmode == 0 and "ln.png" or "charge.png" },
    },
}
"#,
        )
        .unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

        let first = load_skin_document(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState {
                event_index_values: BTreeMap::from([(308, 0)]),
                ..LuaLoadRuntimeState::default()
            },
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(first.document.source[0].path, "ln.png");

        let second = load_skin_document(
            &skin_path,
            SkinKind::Result,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState {
                event_index_values: BTreeMap::from([(308, 2)]),
                ..LuaLoadRuntimeState::default()
            },
            Some(cache),
        )
        .unwrap();
        assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(second.document.source[0].path, "charge.png");
    }

    #[test]
    fn lua_document_cache_misses_when_runtime_text_changes() {
        let root = unique_test_dir("bmz-lua-document-cache-text");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("select.luaskin");
        std::fs::write(
            &skin_path,
            r#"
local main_state = require("main_state")
return {
    type = 0,
    text = {
        { id = "player", constantText = main_state.text(2) },
    },
}
"#,
        )
        .unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

        let first = load_skin_document(
            &skin_path,
            SkinKind::Select,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState {
                text_values: BTreeMap::from([(2, "Player One".to_string())]),
                ..LuaLoadRuntimeState::default()
            },
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(first.document.text[0].constant_text, "Player One");

        let second = load_skin_document(
            &skin_path,
            SkinKind::Select,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState {
                text_values: BTreeMap::from([(2, "Player Two".to_string())]),
                ..LuaLoadRuntimeState::default()
            },
            Some(cache),
        )
        .unwrap();
        assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(second.document.text[0].constant_text, "Player Two");
    }

    #[test]
    fn lua_document_cache_misses_when_used_file_selection_changes() {
        let root = unique_test_dir("bmz-lua-document-cache-file");
        std::fs::create_dir_all(root.join("parts")).unwrap();
        std::fs::write(root.join("parts/blue.png"), []).unwrap();
        std::fs::write(root.join("parts/red.png"), []).unwrap();
        let skin_path = root.join("play.luaskin");
        std::fs::write(
            &skin_path,
            r#"
local path = "parts/blue.png"
if skin_config and skin_config.get_path then
    path = skin_config.get_path("parts/*.png")
end
return {
    type = 0,
    filepath = {
        { name = "Parts", path = "parts/*.png", def = "blue" },
    },
    source = {
        { id = "bg", path = path },
    },
}
"#,
        )
        .unwrap();
        let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

        let first = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            Some(cache.clone()),
        )
        .unwrap();
        assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(
            Path::new(&first.document.source[0].path).canonicalize().unwrap(),
            std::fs::canonicalize(root.join("parts/blue.png")).unwrap()
        );

        let selected = BTreeMap::from([("Parts".to_string(), "red.png".to_string())]);
        let second = load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &selected,
            &LuaLoadRuntimeState::default(),
            Some(cache),
        )
        .unwrap();
        assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
        assert_eq!(
            Path::new(&second.document.source[0].path).canonicalize().unwrap(),
            std::fs::canonicalize(root.join("parts/red.png")).unwrap()
        );
    }

    #[test]
    fn required_skin_sources_excludes_unused_images() {
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "source": [
                    { "id": 1, "path": "used.png" },
                    { "id": 2, "path": "unused.png" },
                    { "id": 3, "path": "lift.png" }
                ],
                "image": [
                    { "id": "used", "src": 1, "x": 0, "y": 0, "w": 8, "h": 8 },
                    { "id": "unused", "src": 2, "x": 0, "y": 0, "w": 8, "h": 8 }
                ],
                "liftCover": [
                    { "id": "lift", "src": 3, "x": 0, "y": 0, "w": 8, "h": 8 }
                ],
                "destination": [
                    { "id": "used", "dst": [{ "x": 0, "y": 0, "w": 8, "h": 8 }] },
                    { "id": "lift", "dst": [{ "x": 0, "y": 0, "w": 8, "h": 8 }] }
                ]
            }
            "#,
        )
        .unwrap();

        let required = required_skin_source_ids(&document);

        assert!(required.contains("1"));
        assert!(!required.contains("2"));
        assert!(required.contains("3"));
    }

    #[test]
    fn supported_font_paths_include_vector_and_bitmap_fonts() {
        assert!(is_supported_font_path(Path::new("font.ttf")));
        assert!(is_supported_font_path(Path::new("font.OTF")));
        assert!(is_supported_font_path(Path::new("font.ttc")));
        assert!(is_supported_font_path(Path::new("font.fnt")));
        assert!(!is_supported_font_path(Path::new("font.png")));
        assert!(is_bitmap_font_path(Path::new("font.fnt")));
        assert!(!is_bitmap_font_path(Path::new("font.ttf")));
    }

    #[test]
    fn skin_font_cache_hit_skips_loader() {
        let root = unique_test_dir("bmz-font-cache-hit");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("font.ttf");
        std::fs::write(&path, b"not a real font").unwrap();
        let key = skin_font_cache_key(&path).unwrap();
        let expected = vec![1, 2, 3, 4];
        let cache = Arc::new(Mutex::new(SkinFontCache::default()));
        cache.lock().unwrap().insert(key.clone(), DecodedFontData::Vector(expected.clone()));

        let (actual, status, actual_key) = decode_font_with_cache(&path, Some(&cache)).unwrap();

        assert_eq!(status, FontCacheStatus::Hit);
        assert_eq!(actual_key, Some(key));
        match actual {
            DecodedFontData::Vector(bytes) => assert_eq!(bytes, expected),
            DecodedFontData::Bitmap(_) => panic!("expected cached vector font bytes"),
        }
    }

    #[test]
    fn skin_font_cache_evicts_least_recently_used_entry() {
        let mut cache = SkinFontCache::with_limit_bytes(8);
        let a = test_font_cache_key("a.ttf");
        let b = test_font_cache_key("b.ttf");
        let c = test_font_cache_key("c.ttf");

        cache.insert(a.clone(), DecodedFontData::Vector(vec![1, 1, 1, 1]));
        cache.insert(b.clone(), DecodedFontData::Vector(vec![2, 2, 2, 2]));
        assert!(cache.get(&a).is_some());
        cache.insert(c.clone(), DecodedFontData::Vector(vec![3, 3, 3, 3]));

        assert!(cache.get(&a).is_some());
        assert!(cache.get(&b).is_none());
        assert!(cache.get(&c).is_some());
    }

    #[test]
    fn skin_font_cache_skips_entries_larger_than_limit() {
        let mut cache = SkinFontCache::with_limit_bytes(4);
        let key = test_font_cache_key("too-large.ttf");

        cache.insert(key.clone(), DecodedFontData::Vector(vec![1, 2, 3, 4, 5]));

        assert!(cache.get(&key).is_none());
        assert_eq!(cache.total_bytes, 0);
    }

    #[test]
    fn installed_font_snapshot_skips_font_payload_decode() {
        let root = unique_test_dir("bmz-installed-font-skip");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("skin.json");
        let font_path = root.join("font.ttf");
        std::fs::write(&font_path, b"not a real font").unwrap();
        std::fs::write(
            &skin_path,
            r#"
            {
                "type": 0,
                "font": [
                    { "id": "font1", "path": "font.ttf" }
                ]
            }
            "#,
        )
        .unwrap();
        let key = skin_font_cache_key(&font_path).unwrap();
        let installed = HashMap::from([("play:font1".to_string(), key.clone())]);

        let decoded = decode_beatoraja_skin_with_options_and_runtime_state_and_caches(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            None,
            None,
            None,
            None,
            Some(installed),
        )
        .unwrap();

        assert_eq!(decoded.stats.font_count, 1);
        assert_eq!(decoded.stats.font_payload_skipped, 1);
        assert_eq!(decoded.stats.font_cache_hits, 0);
        assert_eq!(decoded.stats.font_cache_misses, 0);
        assert_eq!(decoded.fonts.len(), 1);
        assert_eq!(decoded.fonts[0].stored_id, "play:font1");
        assert_eq!(decoded.fonts[0].cache_key.as_ref(), Some(&key));
        assert!(decoded.fonts[0].data.is_none());
    }

    #[test]
    fn skin_source_asset_cache_hit_skips_loader() {
        let root = unique_test_dir("bmz-source-cache-hit");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("source.png");
        std::fs::write(&path, b"cached").unwrap();
        let key = skin_source_asset_cache_key(&path, false).unwrap();
        let expected = RgbaImageAsset { width: 1, height: 1, pixels: vec![1, 2, 3, 4] };
        let cache = Arc::new(Mutex::new(SkinSourceAssetCache::default()));
        cache.lock().unwrap().insert(key, expected.clone());

        let (actual, status) = load_source_asset_with_cache(&path, false, Some(&cache), || {
            panic!("cache hit must not call source loader")
        })
        .unwrap();

        assert_eq!(actual, expected);
        assert_eq!(status, SourceCacheStatus::Hit);
    }

    #[test]
    fn skin_source_asset_cache_misses_after_metadata_change() {
        let root = unique_test_dir("bmz-source-cache-metadata");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("source.png");
        std::fs::write(&path, b"old").unwrap();
        let key = skin_source_asset_cache_key(&path, false).unwrap();
        let stale = RgbaImageAsset { width: 1, height: 1, pixels: vec![1, 2, 3, 4] };
        let fresh = RgbaImageAsset { width: 1, height: 1, pixels: vec![5, 6, 7, 8] };
        let cache = Arc::new(Mutex::new(SkinSourceAssetCache::default()));
        cache.lock().unwrap().insert(key, stale);

        std::fs::write(&path, b"new and longer").unwrap();
        let (actual, status) =
            load_source_asset_with_cache(&path, false, Some(&cache), || Ok(fresh.clone())).unwrap();

        assert_eq!(actual, fresh);
        assert_eq!(status, SourceCacheStatus::Miss);
    }

    #[test]
    fn skin_gpu_texture_cache_reuses_inserted_source_textures() {
        let root = unique_test_dir("bmz-gpu-texture-cache");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("source.png");
        std::fs::write(&path, b"cached").unwrap();
        let key = skin_source_asset_cache_key(&path, false).unwrap();
        let size = SkinImageSize { width: 64.0, height: 32.0 };
        let mut cache = SkinGpuTextureCache::default();

        let allocated = cache.allocate_texture_id(SkinKind::Play);
        cache.insert(key.clone(), allocated, size);

        let cached = cache.get(&key).unwrap();
        assert_eq!(cached.texture, allocated);
        assert_eq!(cached.size, size);
        assert_ne!(cache.allocate_texture_id(SkinKind::Play), allocated);

        cache.clear();

        assert!(cache.get(&key).is_none());
        assert_eq!(cache.allocate_texture_id(SkinKind::Play), SkinTextureId(10_000));
    }

    #[test]
    fn decode_uses_gpu_texture_cache_to_skip_source_decode() {
        let root = unique_test_dir("bmz-source-texture-cache-hit");
        std::fs::create_dir_all(&root).unwrap();
        let skin_path = root.join("skin.json");
        let source_path = root.join("source.png");
        std::fs::write(&source_path, b"not a png").unwrap();
        std::fs::write(
            &skin_path,
            r#"
            {
                "type": 0,
                "source": [
                    { "id": 1, "path": "source.png" }
                ],
                "image": [
                    { "id": "img", "src": 1, "x": 0, "y": 0, "w": 64, "h": 32 }
                ],
                "destination": [
                    { "id": "img", "dst": [{ "x": 0, "y": 0, "w": 64, "h": 32 }] }
                ]
            }
            "#,
        )
        .unwrap();
        let key = skin_source_asset_cache_key(&source_path, false).unwrap();
        let texture = SkinTextureId(12_345);
        let size = SkinImageSize { width: 64.0, height: 32.0 };
        let texture_cache = Arc::new(Mutex::new(SkinGpuTextureCache::default()));
        texture_cache.lock().unwrap().insert(key.clone(), texture, size);

        let decoded = decode_beatoraja_skin_with_options_and_runtime_state_and_caches(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState::default(),
            None,
            None,
            Some(texture_cache),
            None,
            None,
        )
        .unwrap();

        assert_eq!(decoded.stats.source_texture_cache_hits, 1);
        assert_eq!(decoded.stats.source_texture_cache_hit_bytes, 64 * 32 * 4);
        assert_eq!(decoded.stats.source_cache_hits, 0);
        assert_eq!(decoded.stats.source_cache_misses, 0);
        assert_eq!(decoded.stats.decoded_source_bytes, 0);
        assert_eq!(decoded.sources.len(), 1);
        assert_eq!(decoded.sources[0].texture, texture);
        assert_eq!(decoded.sources[0].size, size);
        assert_eq!(decoded.sources[0].cache_key.as_ref(), Some(&key));
        assert!(decoded.sources[0].asset.is_none());
    }

    #[test]
    fn skin_gpu_texture_cache_reuses_inserted_video_textures_separately() {
        let root = unique_test_dir("bmz-gpu-video-texture-cache");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("source.mp4");
        std::fs::write(&path, b"cached-video").unwrap();
        let image_key = skin_source_asset_cache_key(&path, false).unwrap();
        let video_key = skin_source_asset_cache_key(&path, true).unwrap();
        assert_ne!(image_key, video_key);

        let size = SkinImageSize { width: 320.0, height: 180.0 };
        let mut cache = SkinGpuTextureCache::default();
        let allocated = cache.allocate_texture_id(SkinKind::Play);
        cache.insert(video_key.clone(), allocated, size);

        assert!(cache.get(&image_key).is_none());
        let cached = cache.get(&video_key).unwrap();
        assert_eq!(cached.texture, allocated);
        assert_eq!(cached.size, size);
    }

    fn test_font_cache_key(path: &str) -> SkinFontCacheKey {
        SkinFontCacheKey { path: PathBuf::from(path), modified: None, len: 0, is_bitmap: false }
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        path
    }
}
