use super::*;

pub(super) struct SkinDecodeRequest {
    generation: u64,
    path: PathBuf,
    kind: SkinKind,
    options: BTreeMap<String, String>,
    files: BTreeMap<String, String>,
    runtime_state: bmz_skin::LuaLoadRuntimeState,
    installed_font_cache: HashMap<String, SkinFontCacheKey>,
}

impl SkinDecodeRequest {
    pub(super) fn new(
        generation: u64,
        path: PathBuf,
        kind: SkinKind,
        options: BTreeMap<String, String>,
        files: BTreeMap<String, String>,
        runtime_state: bmz_skin::LuaLoadRuntimeState,
    ) -> Self {
        Self {
            generation,
            path,
            kind,
            options,
            files,
            runtime_state,
            installed_font_cache: HashMap::new(),
        }
    }

    pub(super) fn reuse_installed_fonts(mut self, pipeline: &SkinPipelineRuntime) -> Self {
        self.installed_font_cache.clone_from(&pipeline.installed_font_cache);
        self
    }
}

pub(super) fn spawn_skin_decode(pipeline: &SkinPipelineRuntime, request: SkinDecodeRequest) {
    let SkinDecodeRequest {
        generation,
        path,
        kind,
        options,
        files,
        runtime_state,
        installed_font_cache,
    } = request;
    let tx = pipeline.decode_tx.clone();
    let source_cache = pipeline.source_asset_cache.clone();
    let document_cache = pipeline.document_cache.clone();
    let texture_cache = pipeline.gpu_texture_cache.clone();
    let font_cache = pipeline.font_cache.clone();
    let send_path = path.clone();
    let queued_at = Instant::now();
    thread::Builder::new()
        .name(format!("skin-decode-{:?}", kind))
        .spawn(move || {
            let decode_started_at = Instant::now();
            let result = decode_beatoraja_skin_with_options_and_runtime_state_and_caches(
                &path,
                kind,
                &options,
                &files,
                &runtime_state,
                Some(document_cache),
                Some(source_cache),
                Some(texture_cache),
                Some(font_cache),
                Some(installed_font_cache),
            );
            let decode_finished_at = Instant::now();
            let _ = tx.send(PendingSkinResult {
                generation,
                path: send_path,
                kind,
                queued_at,
                decode_started_at,
                decode_finished_at,
                result,
            });
        })
        .expect("failed to spawn skin decode thread");
}

/// upload worker のループ。decode 結果を受け取り、GPU アップロードして main へ返す。
/// decode 側 (`decode_rx`) が全て drop されるとループを抜ける (アプリ終了時)。
pub(super) fn skin_upload_worker(
    decode_rx: Receiver<PendingSkinResult>,
    upload_tx: mpsc::SyncSender<PendingUploadResult>,
    uploader: bmz_render::renderer::GpuUploader,
    texture_cache: SharedSkinGpuTextureCache,
    event_proxy: EventLoopProxy<AppUserEvent>,
) {
    while let Ok(PendingSkinResult {
        generation,
        path,
        kind,
        queued_at,
        decode_started_at,
        decode_finished_at,
        result,
    }) = decode_rx.recv()
    {
        let upload_started_at = Instant::now();
        let uploaded = result.map(|decoded| {
            upload_decoded_skin_with_texture_cache(&uploader, decoded, Some(&texture_cache))
        });
        let upload_finished_at = Instant::now();
        if upload_tx
            .send(PendingUploadResult {
                generation,
                path,
                kind,
                queued_at,
                decode_started_at,
                decode_finished_at,
                upload_started_at,
                upload_finished_at,
                uploaded,
            })
            .is_err()
        {
            // main 側受信端が drop された (アプリ終了)。
            break;
        }
        let _ = event_proxy.send_event(AppUserEvent::SkinUploadReady { sent_at: Instant::now() });
    }
}
