use super::*;

pub(super) struct PendingPlayPreload {
    pub(super) generation: u64,
    pub(super) chart_id: i64,
    pub(super) input: SharedInputBackend,
    pub(super) audio_progress: Arc<AtomicU32>,
    pub(super) prepared_chart: Arc<OnceLock<PreparedPlayChart>>,
    pub(super) rx: Receiver<PlayPreloadResult>,
}

pub(super) struct PlayPreloadResult {
    pub(super) generation: u64,
    pub(super) chart_id: i64,
    pub(super) result: std::result::Result<PreloadedInputPlaySession, String>,
}

/// 中間リザルト中に先読みしている次のコース譜面。
///
/// preload に渡した開始条件をそのまま Play 入場へ引き継ぎ、曲間で
/// gauge/combo/arrange 条件を作り直して食い違わせないために保持する。
pub(super) struct PendingCourseStageLaunch {
    pub(super) course_id: i64,
    pub(super) entry_index: usize,
    pub(super) chart_id: i64,
    pub(super) options: PlayStartOptions,
    pub(super) preload_generation: u64,
    pub(super) preload_error: Option<String>,
}

impl PendingCourseStageLaunch {
    pub(super) fn matches(&self, course_id: i64, entry_index: usize, chart_id: i64) -> bool {
        self.course_id == course_id && self.entry_index == entry_index && self.chart_id == chart_id
    }
}

/// Media kept across same-song retry (beatoraja `BMSResource` style).
/// Cleared when leaving result back to select, or when starting an unrelated chart.
pub(super) struct PlayMediaCache {
    pub(super) chart_id: i64,
    /// Present for SameArrange reuse of the exact chart Arc.
    pub(super) chart: Option<std::sync::Arc<PlayableChart>>,
    pub(super) opponent_chart: Option<std::sync::Arc<PlayableChart>>,
    pub(super) source_ln_profile: Option<crate::ln_policy::ChartLnProfile>,
    pub(super) skin_attempt: Option<bmz_render::snapshot::SkinAttemptState>,
    pub(super) chart_length_ms: u64,
    pub(super) render_snapshot_cache:
        Option<crate::screens::play_snapshot::PlayRenderSnapshotCache>,
    pub(super) chart_normalization_gain: f32,
    pub(super) applied_arrange: Option<crate::screens::play_session::AppliedArrange>,
    pub(super) score_key: Option<crate::storage::score_db::ScoreKey>,
    pub(super) assist_runtime: bmz_gameplay::session::AssistRuntime,
    pub(super) score_save_disabled: bool,
    pub(super) bga_frames: BgaFrameCatalog,
    pub(super) bga_assets: Vec<BgaAssetRef>,
    pub(super) video_bga_decoders: crate::video_bga::VideoBgaDecoderMap,
}
