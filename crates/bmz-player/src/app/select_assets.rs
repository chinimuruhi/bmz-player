use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

use bmz_audio::sample::DecodedSample;
use bmz_render::assets::RgbaImageAsset;
use bmz_render::skin::SkinImageSize;

use crate::chart_preview::SelectChartPreview;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectMetaImageSlot {
    Stage,
    Backbmp,
    Banner,
}

pub(super) enum SelectMetaImageCacheEntry {
    Loading,
    Ready(RgbaImageAsset),
    Missing,
}

pub(super) struct SelectMetaImageResult {
    pub(super) slot: SelectMetaImageSlot,
    pub(super) key: String,
    pub(super) path: Option<PathBuf>,
    pub(super) result: std::result::Result<RgbaImageAsset, String>,
}

pub(super) enum SelectPreviewCacheEntry {
    Loading,
    Ready(PreparedSelectPreview),
    Missing,
}

#[derive(Clone)]
pub(super) struct PreparedSelectPreview {
    pub(super) sample: DecodedSample,
    pub(super) normalization_gain: f32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectPreviewFade {
    #[default]
    Silent,
    FadingIn {
        started_at: Instant,
    },
    Playing,
    FadingOut {
        started_at: Instant,
    },
}

pub(super) struct SelectPreviewResult {
    pub(super) key: String,
    pub(super) path: Option<PathBuf>,
    pub(super) result: std::result::Result<PreparedSelectPreview, String>,
}

/// 選曲プレビューの重いロード処理は一度に1件だけ実行する。
/// 選択が変わった間の要求は最新の1件だけ残し、古い要求を捨てる。
#[derive(Debug, Default)]
pub(super) struct SelectPreviewLoadQueue {
    active: bool,
    pending: Option<String>,
}

impl SelectPreviewLoadQueue {
    pub(super) fn request(&mut self, key: String) -> Option<String> {
        if self.active {
            self.pending = Some(key);
            None
        } else {
            self.active = true;
            Some(key)
        }
    }

    pub(super) fn finish(&mut self) -> Option<String> {
        if let Some(next) = self.pending.take() {
            Some(next)
        } else {
            self.active = false;
            None
        }
    }
}

#[derive(Debug, Default)]
struct SelectMetaImageState {
    source: Option<String>,
    loaded: bool,
    size: Option<SkinImageSize>,
}

/// 選曲画面固有の画像・試聴音源について、選択変更をまたいで維持するロード状態。
///
/// GPU texture upload と profile 設定の解釈は `WinitApp` に残し、この型は
/// worker channel、CPU asset cache、現在選択中の再生状態だけを所有する。
pub(super) struct SelectAssetRuntime {
    stage: SelectMetaImageState,
    backbmp: SelectMetaImageState,
    banner: SelectMetaImageState,
    pub(super) preview_source: Option<String>,
    pub(super) preview_playing: bool,
    pub(super) preview_fade: SelectPreviewFade,
    pub(super) preview_normalization_gain: f32,
    pub(super) preview: Option<SelectChartPreview>,
    pub(super) meta_image_cache: HashMap<String, SelectMetaImageCacheEntry>,
    pub(super) meta_image_tx: mpsc::Sender<SelectMetaImageResult>,
    pub(super) meta_image_rx: Receiver<SelectMetaImageResult>,
    pub(super) preview_cache: HashMap<String, SelectPreviewCacheEntry>,
    pub(super) preview_tx: mpsc::Sender<SelectPreviewResult>,
    pub(super) preview_rx: Receiver<SelectPreviewResult>,
    pub(super) preview_load_queue: SelectPreviewLoadQueue,
    pub(super) generated_preview_loading: bool,
}

impl SelectAssetRuntime {
    pub(super) fn new(preview: Option<SelectChartPreview>) -> Self {
        let (meta_image_tx, meta_image_rx) = mpsc::channel();
        let (preview_tx, preview_rx) = mpsc::channel();
        Self {
            stage: SelectMetaImageState::default(),
            backbmp: SelectMetaImageState::default(),
            banner: SelectMetaImageState::default(),
            preview_source: None,
            preview_playing: false,
            preview_fade: SelectPreviewFade::Silent,
            preview_normalization_gain: 1.0,
            preview,
            meta_image_cache: HashMap::new(),
            meta_image_tx,
            meta_image_rx,
            preview_cache: HashMap::new(),
            preview_tx,
            preview_rx,
            preview_load_queue: SelectPreviewLoadQueue::default(),
            generated_preview_loading: false,
        }
    }

    fn meta_image(&self, slot: SelectMetaImageSlot) -> &SelectMetaImageState {
        match slot {
            SelectMetaImageSlot::Stage => &self.stage,
            SelectMetaImageSlot::Backbmp => &self.backbmp,
            SelectMetaImageSlot::Banner => &self.banner,
        }
    }

    fn meta_image_mut(&mut self, slot: SelectMetaImageSlot) -> &mut SelectMetaImageState {
        match slot {
            SelectMetaImageSlot::Stage => &mut self.stage,
            SelectMetaImageSlot::Backbmp => &mut self.backbmp,
            SelectMetaImageSlot::Banner => &mut self.banner,
        }
    }

    pub(super) fn meta_image_source(&self, slot: SelectMetaImageSlot) -> &Option<String> {
        &self.meta_image(slot).source
    }

    pub(super) fn set_meta_image_source(
        &mut self,
        slot: SelectMetaImageSlot,
        source: Option<String>,
    ) {
        self.meta_image_mut(slot).source = source;
    }

    pub(super) fn meta_image_loaded(&self, slot: SelectMetaImageSlot) -> bool {
        self.meta_image(slot).loaded
    }

    pub(super) fn set_meta_image_loaded(&mut self, slot: SelectMetaImageSlot, loaded: bool) {
        self.meta_image_mut(slot).loaded = loaded;
    }

    pub(super) fn meta_image_size(&self, slot: SelectMetaImageSlot) -> Option<SkinImageSize> {
        self.meta_image(slot).size
    }

    pub(super) fn set_meta_image_size(
        &mut self,
        slot: SelectMetaImageSlot,
        size: Option<SkinImageSize>,
    ) {
        self.meta_image_mut(slot).size = size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_initializes_preview_and_worker_state() {
        let runtime = SelectAssetRuntime::new(None);

        assert!(runtime.preview.is_none());
        assert_eq!(runtime.preview_source, None);
        assert!(!runtime.preview_playing);
        assert_eq!(runtime.preview_fade, SelectPreviewFade::Silent);
        assert_eq!(runtime.preview_normalization_gain, 1.0);
        assert!(runtime.meta_image_cache.is_empty());
        assert!(runtime.preview_cache.is_empty());
        assert!(!runtime.generated_preview_loading);
    }

    #[test]
    fn meta_image_slots_keep_independent_state() {
        let mut runtime = SelectAssetRuntime::new(None);
        let stage_size = SkinImageSize { width: 640.0, height: 480.0 };

        runtime.set_meta_image_source(SelectMetaImageSlot::Stage, Some("stage".to_string()));
        runtime.set_meta_image_loaded(SelectMetaImageSlot::Stage, true);
        runtime.set_meta_image_size(SelectMetaImageSlot::Stage, Some(stage_size));

        assert_eq!(runtime.meta_image_source(SelectMetaImageSlot::Stage).as_deref(), Some("stage"));
        assert!(runtime.meta_image_loaded(SelectMetaImageSlot::Stage));
        assert_eq!(runtime.meta_image_size(SelectMetaImageSlot::Stage), Some(stage_size));
        assert_eq!(runtime.meta_image_source(SelectMetaImageSlot::Banner), &None);
        assert!(!runtime.meta_image_loaded(SelectMetaImageSlot::Banner));
        assert_eq!(runtime.meta_image_size(SelectMetaImageSlot::Banner), None);
    }
}
