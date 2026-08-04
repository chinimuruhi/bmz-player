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

/// Media kept across same-song retry (beatoraja `BMSResource` style).
/// Cleared when leaving result back to select, or when starting an unrelated chart.
pub(super) struct PlayMediaCache {
    pub(super) chart_id: i64,
    /// Present for SameArrange reuse of the exact chart Arc.
    pub(super) chart: Option<std::sync::Arc<PlayableChart>>,
    pub(super) source_ln_profile: Option<crate::ln_policy::ChartLnProfile>,
    pub(super) render_snapshot_cache:
        Option<crate::screens::play_snapshot::PlayRenderSnapshotCache>,
    pub(super) chart_normalization_gain: f32,
    pub(super) applied_arrange: Option<crate::screens::play_session::AppliedArrange>,
    pub(super) score_key: Option<crate::storage::score_db::ScoreKey>,
    pub(super) bga_frames: BgaFrameCatalog,
    pub(super) bga_assets: Vec<BgaAssetRef>,
    pub(super) video_bga_decoders: crate::video_bga::VideoBgaDecoderMap,
}
