#[derive(Default)]
pub struct Renderer {
    last_scene: Option<AppSceneSnapshot>,
    last_plan: Option<DrawPlan>,
    play_skin_context: SkinContext,
    select_skin_context: SkinContext,
    decide_skin_context: SkinContext,
    result_skin_context: SkinContext,
    last_plan_canvas_policy: CanvasRenderPolicy,
    pending_textures: Vec<PendingTexture>,
    fonts: HashMap<String, FontArc>,
    bitmap_fonts: HashMap<String, BitmapFont>,
    gpu: Option<WgpuRenderer>,
    pending_egui: Option<EguiFrame>,
    pending_screenshot: Option<ScreenshotRequest>,
    play_dynamic_timer_runtime: DynamicTimerRuntime,
    select_dynamic_timer_runtime: DynamicTimerRuntime,
    decide_dynamic_timer_runtime: DynamicTimerRuntime,
    result_dynamic_timer_runtime: DynamicTimerRuntime,
    last_frame_timings: Option<RenderFrameTimings>,
    /// サーフェス生成時および `set_present_mode` で参照する希望 present mode。
    present_mode: WgpuPresentMode,
    internal_resolution_mode: InternalResolutionMode,
    backend: WgpuBackend,
    default_font_coverage: bmz_font::FontCoverage,
    default_font_search_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderFrameTimings {
    pub plan_us: u128,
    pub draw_us: u128,
    pub text_us: u128,
    pub geometry_us: u128,
    pub upload_us: u128,
    pub submit_us: u128,
    pub surface_us: u128,
    pub bind_us: u128,
    pub encode_us: u128,
    pub queue_us: u128,
    pub present_us: u128,
    pub commands: usize,
    pub steps: usize,
    pub rect_steps: usize,
    pub image_steps: usize,
    pub text_steps: usize,
    pub rect_instances: usize,
    pub image_instances: usize,
    pub text_instances: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct GpuRenderTimings {
    draw_us: u128,
    text_us: u128,
    geometry_us: u128,
    upload_us: u128,
    submit_us: u128,
    surface_us: u128,
    bind_us: u128,
    encode_us: u128,
    queue_us: u128,
    present_us: u128,
    steps: usize,
    rect_steps: usize,
    image_steps: usize,
    text_steps: usize,
    rect_instances: usize,
    image_instances: usize,
    text_instances: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CanvasFitMode {
    #[default]
    Expand,
    Contain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CanvasSize {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CanvasRenderPolicy {
    fit_mode: CanvasFitMode,
    canvas_size: Option<CanvasSize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CanvasViewport {
    rect: Rect,
    content_size: SurfaceSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSurfaceStatus {
    Rendered,
    SkippedNoSurface,
    SkippedZeroSize,
    Reconfigured,
    TimedOut,
}

impl SurfaceSize {
    pub fn is_drawable(self) -> bool {
        self.width > 0 && self.height > 0
    }
}

impl CanvasRenderPolicy {
    fn skin_document(document: &SkinDocument) -> Self {
        Self {
            fit_mode: CanvasFitMode::Contain,
            canvas_size: Some(CanvasSize { width: document.w.max(1), height: document.h.max(1) }),
        }
    }

    fn internal_render_size(
        self,
        surface: SurfaceSize,
        mode: InternalResolutionMode,
    ) -> Option<SurfaceSize> {
        if mode == InternalResolutionMode::Native || !surface.is_drawable() {
            return None;
        }
        let canvas = self.canvas_size?;
        let content = CanvasViewport::from_policy(surface, self).content_size();
        // 小さいwindowでスキン解像度へ一度拡大してから縮小すると、負荷だけが増える。
        // 両辺が表示領域より小さい場合に限り内部targetを使う。
        (canvas.width < content.width && canvas.height < content.height)
            .then_some(SurfaceSize { width: canvas.width, height: canvas.height })
    }
}

impl CanvasViewport {
    fn from_policy(surface: SurfaceSize, policy: CanvasRenderPolicy) -> Self {
        let full =
            Self { rect: Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 }, content_size: surface };
        if !surface.is_drawable() || policy.fit_mode == CanvasFitMode::Expand {
            return full;
        }

        let Some(canvas) = policy.canvas_size else {
            return full;
        };
        if canvas.width == 0 || canvas.height == 0 {
            return full;
        }

        let surface_aspect = surface.width as f32 / surface.height as f32;
        let canvas_aspect = canvas.width as f32 / canvas.height as f32;
        if !surface_aspect.is_finite() || !canvas_aspect.is_finite() {
            return full;
        }
        if (surface_aspect - canvas_aspect).abs() <= f32::EPSILON {
            return full;
        }

        let rect = if surface_aspect > canvas_aspect {
            let width = canvas_aspect / surface_aspect;
            Rect { x: (1.0 - width) * 0.5, y: 0.0, width, height: 1.0 }
        } else {
            let height = surface_aspect / canvas_aspect;
            Rect { x: 0.0, y: (1.0 - height) * 0.5, width: 1.0, height }
        };
        Self { rect, content_size: Self::content_size_for_rect(surface, rect) }
    }

    fn content_size(self) -> SurfaceSize {
        self.content_size
    }

    fn is_identity(self) -> bool {
        self.rect == Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 }
    }

    fn transform_rect(self, rect: Rect) -> Rect {
        Rect {
            x: self.rect.x + rect.x * self.rect.width,
            y: self.rect.y + rect.y * self.rect.height,
            width: rect.width * self.rect.width,
            height: rect.height * self.rect.height,
        }
    }

    fn surface_to_canvas_point(self, x: f32, y: f32) -> Option<(f32, f32)> {
        let right = self.rect.x + self.rect.width;
        let bottom = self.rect.y + self.rect.height;
        if x < self.rect.x || x > right || y < self.rect.y || y > bottom {
            return None;
        }
        Some(((x - self.rect.x) / self.rect.width, (y - self.rect.y) / self.rect.height))
    }

    fn transform_rect_command(self, command: RectCommand) -> RectCommand {
        RectCommand { rect: self.transform_rect(command.rect), color: command.color }
    }

    fn transform_rect_batch_cache(self, cache: RectBatchCache) -> RectBatchCache {
        RectBatchCache { bounds: self.transform_rect(cache.bounds), ..cache }
    }

    fn transform_text_instances(self, instances: &mut [u8]) {
        for instance in instances.chunks_exact_mut(TEXT_INSTANCE_BYTES) {
            let x = f32::from_le_bytes(instance[0..4].try_into().unwrap());
            let y = f32::from_le_bytes(instance[4..8].try_into().unwrap());
            let width = f32::from_le_bytes(instance[8..12].try_into().unwrap());
            let height = f32::from_le_bytes(instance[12..16].try_into().unwrap());
            let rect = self.transform_rect(Rect { x, y, width, height });
            instance[0..4].copy_from_slice(&rect.x.to_le_bytes());
            instance[4..8].copy_from_slice(&rect.y.to_le_bytes());
            instance[8..12].copy_from_slice(&rect.width.to_le_bytes());
            instance[12..16].copy_from_slice(&rect.height.to_le_bytes());
        }
    }

    fn transform_text_caret_rects(self, rects: &mut [Option<RectCommand>]) {
        for rect in rects.iter_mut().flatten() {
            *rect = self.transform_rect_command(*rect);
        }
    }

    fn content_size_for_rect(surface: SurfaceSize, rect: Rect) -> SurfaceSize {
        SurfaceSize {
            width: normalized_extent_to_pixels(rect.width, surface.width),
            height: normalized_extent_to_pixels(rect.height, surface.height),
        }
    }
}

fn validate_rgba_texture(width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(anyhow!("texture dimensions must be non-zero"));
    }
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(anyhow!(
            "rgba texture length mismatch: expected {expected} bytes, got {}",
            rgba.len()
        ));
    }
    Ok(())
}
