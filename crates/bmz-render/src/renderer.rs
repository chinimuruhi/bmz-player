use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::mpsc;
use std::task::{Context as TaskContext, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread;
#[cfg(windows)]
use std::time::Duration;
use std::time::Instant;

use ab_glyph::{Font, FontArc, FontVec, Glyph, PxScale, ScaleFont, point};
use anyhow::{Context, Result, anyhow};
use image::ImageEncoder;

use crate::assets::{RgbaImageAsset, load_png_rgba};
use crate::bitmap_font::{BitmapFont, load_bitmap_font};
use crate::plan::{
    Color, DrawCommand, DrawPlan, Point, Rect, RectBatchCache, RectBatchCacheKey, RectCommand,
    TextAlign, TextCaret, TextOverflow, TextStyle, TextureId, UvRect,
};
use crate::scene::AppSceneSnapshot;
use crate::skin::{
    BlendMode, DynamicTimerRuntime, SkinClickHit, SkinContext, SkinDocument, SkinImageSize,
    SkinSliderHit,
};
use crate::ui::{EguiFrame, EguiPainter};

mod font;
mod geometry;
mod gpu;
mod pipeline;
mod screenshot;
mod text;
#[path = "renderer/text/cached_builder.rs"]
mod text_cached_builder;
#[path = "renderer/text/layout.rs"]
mod text_layout;
#[path = "renderer/text/raster_builder.rs"]
mod text_raster_builder;

#[cfg(test)]
use font::load_default_font;
pub use font::{
    SystemFontData, load_cjk_font_fallback_data, load_font_bytes_for_coverage,
    load_japanese_font_bytes, load_system_font_data_for_coverage,
};
use font::{block_on, load_default_font_fallbacks};
use geometry::*;
pub use pipeline::GpuUploader;
use pipeline::*;
use screenshot::*;
use text::*;
use text_cached_builder::*;
use text_layout::*;
use text_raster_builder::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WgpuBackend {
    #[default]
    Auto,
    Vulkan,
    Metal,
    Dx12,
    Gl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WgpuPresentMode {
    #[default]
    Fifo,
    FifoRelaxed,
    Immediate,
    Mailbox,
}

/// ゲーム / スキン描画に使う解像度。
///
/// `Skin` は現在のスキン document の `w` / `h` が表示領域より小さい場合だけ
/// 中間 render target を使い、最終 surface へ拡大する。egui は常に surface の
/// native 解像度で描画する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InternalResolutionMode {
    #[default]
    Native,
    Skin,
}

/// Surfaceへ実際に適用されたpresent設定。要求modeがGPU/OSで利用できない場合、
/// `effective_mode`はfallback後の値になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfacePresentationStatus {
    pub requested_mode: WgpuPresentMode,
    pub effective_mode: &'static str,
    pub maximum_frame_latency: u32,
}

/// 入力から表示までの待ちを最小化するため、通常modeでswapchainに許可する最大の
/// in-flight frame数。MailboxだけはDX12でこの値×monitor HzにFPSが制限されるため、
/// 既定値2を維持してFast VSyncがrefresh rateそのものへ落ちるのを避ける。
const LOW_LATENCY_MAXIMUM_FRAME_LATENCY: u32 = 1;
const MAILBOX_MAXIMUM_FRAME_LATENCY: u32 = 2;

impl WgpuBackend {
    pub fn to_wgpu(self) -> wgpu::Backends {
        match self {
            Self::Auto => auto_wgpu_backends(),
            Self::Vulkan => wgpu::Backends::VULKAN,
            Self::Metal => wgpu::Backends::METAL,
            Self::Dx12 => wgpu::Backends::DX12,
            Self::Gl => wgpu::Backends::GL,
        }
    }
}

/// 設定 UI に表示できるレンダリングバックエンドを、現在の OS / feature 構成から返す。
///
/// wgpu の `enabled_backend_features` は、対象プラットフォームとビルド時に有効な
/// backend feature を反映する。`Auto` は常に利用可能な論理選択肢として含める。
pub fn available_wgpu_backends() -> Vec<WgpuBackend> {
    [WgpuBackend::Auto, WgpuBackend::Vulkan, WgpuBackend::Metal, WgpuBackend::Dx12, WgpuBackend::Gl]
        .into_iter()
        .filter(|backend| {
            *backend == WgpuBackend::Auto
                || wgpu::Instance::enabled_backend_features().contains(backend.to_wgpu())
        })
        .collect()
}

fn auto_wgpu_backends() -> wgpu::Backends {
    #[cfg(target_os = "linux")]
    {
        // Prefer Vulkan on Linux. GL/GLES remains available only as an
        // explicit fallback when Vulkan surface/device creation fails.
        wgpu::Backends::VULKAN
    }

    #[cfg(target_os = "windows")]
    {
        // Prefer DirectX 12 on Windows. Vulkan and GL remain available only as
        // explicit fallbacks when DirectX 12 surface/device creation fails.
        wgpu::Backends::DX12
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        wgpu::Backends::all()
    }
}

fn fallback_wgpu_backends(backend: WgpuBackend) -> &'static [WgpuBackend] {
    match backend {
        #[cfg(target_os = "linux")]
        WgpuBackend::Auto => &[WgpuBackend::Vulkan, WgpuBackend::Gl],
        #[cfg(target_os = "windows")]
        WgpuBackend::Auto => &[WgpuBackend::Dx12, WgpuBackend::Vulkan, WgpuBackend::Gl],
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        WgpuBackend::Auto => &[WgpuBackend::Auto],
        WgpuBackend::Vulkan => &[WgpuBackend::Vulkan],
        WgpuBackend::Metal => &[WgpuBackend::Metal],
        WgpuBackend::Dx12 => &[WgpuBackend::Dx12],
        WgpuBackend::Gl => &[WgpuBackend::Gl],
    }
}

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

struct WgpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    present_modes: Vec<wgpu::PresentMode>,
    rect_pipeline: wgpu::RenderPipeline,
    rect_buffer: Option<wgpu::Buffer>,
    rect_buffer_capacity: usize,
    image_pipeline: wgpu::RenderPipeline,
    image_add_pipeline: wgpu::RenderPipeline,
    image_premultiplied_pipeline: wgpu::RenderPipeline,
    image_layer_pipeline: wgpu::RenderPipeline,
    image_bind_group_layout: wgpu::BindGroupLayout,
    image_sampler: wgpu::Sampler,
    image_sampler_linear: wgpu::Sampler,
    upscale_pipeline: wgpu::RenderPipeline,
    upscale_buffer: wgpu::Buffer,
    upscale_rect: Option<Rect>,
    internal_scene_target: Option<InternalSceneTarget>,
    image_textures: HashMap<TextureId, PreparedTexture>,
    image_bind_group_cache: HashMap<(TextureId, bool), wgpu::BindGroup>,
    image_bind_group_scratch: Vec<wgpu::BindGroup>,
    geometry_scratch: PlanGeometry,
    offscreen_rect_batches: HashMap<OffscreenRectBatchTextureKey, TextureId>,
    next_offscreen_rect_batch_texture_id: u32,
    image_buffer: Option<wgpu::Buffer>,
    image_buffer_capacity: usize,
    text_pipeline: wgpu::RenderPipeline,
    text_bind_group_layout: wgpu::BindGroupLayout,
    text_sampler: wgpu::Sampler,
    text_texture: Option<wgpu::Texture>,
    text_texture_view: Option<wgpu::TextureView>,
    text_bind_group: Option<wgpu::BindGroup>,
    text_texture_size: AtlasSize,
    text_atlas: TextAtlasCache,
    text_buffer: Option<wgpu::Buffer>,
    text_buffer_capacity: usize,
    default_fonts: FontFallbackChain,
    default_font_coverage: bmz_font::FontCoverage,
    default_font_search_paths: Vec<PathBuf>,
    egui: EguiPainter,
    pending_screenshot_readbacks: Vec<ScreenshotReadback>,
    screenshot_save_jobs: Vec<ScreenshotSaveJob>,
    // Drop the surface after GPU resources so Linux native contexts are
    // released before the window/display teardown.
    surface: wgpu::Surface<'static>,
}

struct InternalSceneTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    size: SurfaceSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct OffscreenRectBatchTextureKey {
    key: RectBatchCacheKey,
    width: u32,
    height: u32,
}

const OFFSCREEN_RECT_BATCH_TEXTURE_BASE: u32 = 0xF000_0000;
const OFFSCREEN_RECT_BATCH_TEXTURE_MAX_ENTRIES: usize = 64;

#[derive(Debug, Clone)]
struct PendingTexture {
    id: TextureId,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// GPU へアップロード済みのテクスチャ。別スレッド (skin upload worker) で
/// 生成してメインスレッドへ送るために公開する。`wgpu::Texture` /
/// `wgpu::TextureView` はどちらも `Send` なのでスレッド間で受け渡しできる。
pub struct PreparedTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl Renderer {
    pub fn attach_surface<T>(&mut self, window: T, size: SurfaceSize) -> Result<()>
    where
        T: Into<wgpu::SurfaceTarget<'static>> + Clone,
    {
        if !size.is_drawable() {
            self.gpu = None;
            return Ok(());
        }

        let mut gpu = WgpuRenderer::new_with_fallbacks(
            window,
            size,
            self.present_mode,
            self.backend,
            self.default_font_coverage,
            self.default_font_search_paths.clone(),
        )?;
        for texture in self.pending_textures.drain(..) {
            gpu.upsert_rgba_texture(texture.id, texture.width, texture.height, &texture.rgba);
        }
        self.gpu = Some(gpu);
        Ok(())
    }

    /// Drop GPU resources that depend on the window surface while the app still
    /// owns the native window.
    pub fn detach_surface(&mut self) {
        self.pending_egui = None;
        let Some(gpu) = self.gpu.take() else {
            return;
        };
        gpu.wait_idle_before_drop();
    }

    pub fn upsert_rgba_texture(
        &mut self,
        id: TextureId,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<()> {
        validate_rgba_texture(width, height, &rgba)?;
        if let Some(gpu) = &mut self.gpu {
            gpu.upsert_rgba_texture(id, width, height, &rgba);
        } else {
            self.pending_textures.push(PendingTexture { id, width, height, rgba });
        }
        Ok(())
    }

    pub fn upsert_rgba_texture_ref(
        &mut self,
        id: TextureId,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<()> {
        validate_rgba_texture(width, height, rgba)?;
        if let Some(gpu) = &mut self.gpu {
            gpu.upsert_rgba_texture(id, width, height, rgba);
        } else {
            self.pending_textures.push(PendingTexture { id, width, height, rgba: rgba.to_vec() });
        }
        Ok(())
    }

    pub fn upsert_image_asset(&mut self, id: TextureId, asset: &RgbaImageAsset) -> Result<()> {
        asset.validate()?;
        self.upsert_rgba_texture_ref(id, asset.width, asset.height, &asset.pixels)
    }

    /// skin upload worker 用に、GPU アップロード機能の clone を取り出す。
    /// surface 未接続 (`gpu` が None) の間は `None`。
    /// 返り値は `Send + Clone` なので別スレッドへ渡せる。
    pub fn gpu_uploader(&self) -> Option<GpuUploader> {
        self.gpu
            .as_ref()
            .map(|gpu| GpuUploader { device: gpu.device.clone(), queue: gpu.queue.clone() })
    }

    /// worker でアップロード済みの `PreparedTexture` をテクスチャ表へ差し込む。
    /// surface 未接続時は (worker が存在しないため通常起きないが) 無視する。
    pub fn insert_prepared_texture(&mut self, id: TextureId, prepared: PreparedTexture) {
        if let Some(gpu) = &mut self.gpu {
            gpu.image_textures.insert(id, prepared);
            gpu.image_bind_group_cache.retain(|(texture_id, _), _| *texture_id != id);
        } else {
            tracing::warn!(
                texture_id = id.0,
                "dropping prepared texture because gpu surface is not attached"
            );
        }
    }

    pub fn load_png_texture(&mut self, id: TextureId, path: &std::path::Path) -> Result<()> {
        let asset = load_png_rgba(path)?;
        self.upsert_image_asset(id, &asset)
    }

    pub fn load_font(&mut self, id: impl Into<String>, path: &std::path::Path) -> Result<()> {
        let id = id.into();
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read font: {}", path.display()))?;
        let font = FontArc::try_from_vec(bytes)
            .map_err(|error| anyhow!("failed to parse font {}: {error}", path.display()))?;
        self.insert_vector_font(id, font);
        if let Some(gpu) = &mut self.gpu {
            gpu.reset_text_atlas();
        }
        Ok(())
    }

    pub fn load_bitmap_font(
        &mut self,
        id: impl Into<String>,
        path: &std::path::Path,
    ) -> Result<()> {
        let font = load_bitmap_font(path)?;
        self.insert_bitmap_font_entry(id.into(), font);
        if let Some(gpu) = &mut self.gpu {
            gpu.reset_text_atlas();
        }
        Ok(())
    }

    /// 事前に読み込んだフォントバイト列を登録する。
    /// バックグラウンドスレッドで I/O を済ませた後に main スレッドから登録する用途。
    pub fn install_font_bytes(&mut self, id: impl Into<String>, bytes: Vec<u8>) -> Result<()> {
        let font = FontArc::try_from_vec(bytes)
            .map_err(|error| anyhow!("failed to parse font bytes: {error}"))?;
        self.insert_vector_font(id.into(), font);
        if let Some(gpu) = &mut self.gpu {
            gpu.reset_text_atlas();
        }
        Ok(())
    }

    /// 事前にパース済みの bitmap font を登録する。
    pub fn install_bitmap_font(&mut self, id: impl Into<String>, font: BitmapFont) {
        self.insert_bitmap_font_entry(id.into(), font);
        if let Some(gpu) = &mut self.gpu {
            gpu.reset_text_atlas();
        }
    }

    fn insert_vector_font(&mut self, id: String, font: FontArc) {
        self.bitmap_fonts.remove(&id);
        self.fonts.insert(id, font);
    }

    fn insert_bitmap_font_entry(&mut self, id: String, font: BitmapFont) {
        self.fonts.remove(&id);
        self.bitmap_fonts.insert(id, font);
    }

    pub fn set_skin_context(&mut self, skin_context: SkinContext) {
        self.set_play_skin_context(skin_context, false);
    }

    /// `preserve_dynamic_timers` が true のとき、プレイ中のスキン差し替え向けに
    /// `timer_observe_boolean` の経過時刻を維持する。
    pub fn set_play_skin_context(
        &mut self,
        skin_context: SkinContext,
        preserve_dynamic_timers: bool,
    ) {
        if !preserve_dynamic_timers {
            self.play_dynamic_timer_runtime.reset();
        }
        self.play_skin_context = skin_context;
    }

    pub fn set_select_skin_context(&mut self, skin_context: SkinContext) {
        self.select_dynamic_timer_runtime.reset();
        self.select_skin_context = skin_context;
    }

    pub fn set_decide_skin_context(&mut self, skin_context: SkinContext) {
        self.decide_dynamic_timer_runtime.reset();
        self.decide_skin_context = skin_context;
    }

    pub fn set_result_skin_context(&mut self, skin_context: SkinContext) {
        self.result_skin_context = skin_context;
        self.result_dynamic_timer_runtime.reset_for_document(self.result_skin_context.document());
    }

    /// リザルトスキンが定義する内部 runtime event を dispatch する。
    ///
    /// クリック入力などを app 層が解決した後に呼ぶ。event が未定義なら false。
    pub fn dispatch_result_skin_runtime_event(&mut self, event_id: i32) -> bool {
        let Some(document) = self.result_skin_context.document() else {
            return false;
        };
        self.result_dynamic_timer_runtime.dispatch_runtime_event(document, event_id)
    }

    /// 同じリザルトスキンで新しい scene に入る際、runtime state を初期化する。
    pub fn reset_result_skin_runtime(&mut self) {
        self.result_dynamic_timer_runtime.reset_for_document(self.result_skin_context.document());
    }

    /// リザルトスキンが宣言する終了フェードアウト時間 (ms)。
    /// ドキュメントスキンが無い場合や未指定の場合は 0 を返す。
    pub fn result_skin_fadeout_ms(&self) -> i32 {
        self.result_skin_context.document().map(|document| document.fadeout).unwrap_or(0).max(0)
    }

    pub fn result_skin_timer_animation_duration_ms(&self, timer: i32) -> i32 {
        self.result_skin_context.timer_animation_duration_ms(timer)
    }

    /// 選曲スキンの document (設定 UI が property/offset 定義を読むため公開)。
    pub fn select_skin_document(&self) -> Option<&SkinDocument> {
        self.select_skin_context.document()
    }

    pub fn select_skin_click_hit(
        &self,
        snapshot: &crate::scene::SelectSnapshot,
        x: f32,
        y: f32,
    ) -> Option<SkinClickHit> {
        let (x, y) = self.select_skin_canvas_point(x, y)?;
        self.select_skin_context.select_click_hit(snapshot, x, y)
    }

    pub fn result_skin_click_hit(
        &self,
        snapshot: &crate::scene::ResultSnapshot,
        x: f32,
        y: f32,
    ) -> Option<SkinClickHit> {
        let (x, y) = self.result_skin_canvas_point(x, y)?;
        let document = self.result_skin_context.document()?;
        let mut state = crate::plan::result_skin_draw_state(snapshot, document.ranktime);
        state.start_input_ms =
            crate::skin::skin_start_input_elapsed_ms(state.elapsed_ms, document.input);
        self.result_skin_context.result_click_hit(&state, x, y)
    }

    pub fn result_skin_slider_hit(
        &self,
        snapshot: &crate::scene::ResultSnapshot,
        x: f32,
        y: f32,
    ) -> Option<SkinSliderHit> {
        let (x, y) = self.result_skin_canvas_point(x, y)?;
        let document = self.result_skin_context.document()?;
        let mut state = crate::plan::result_skin_draw_state(snapshot, document.ranktime);
        state.start_input_ms =
            crate::skin::skin_start_input_elapsed_ms(state.elapsed_ms, document.input);
        self.result_skin_context.result_slider_hit(&state, x, y)
    }

    pub fn select_skin_slider_hit(
        &self,
        snapshot: &crate::scene::SelectSnapshot,
        x: f32,
        y: f32,
    ) -> Option<SkinSliderHit> {
        let (x, y) = self.select_skin_canvas_point(x, y)?;
        self.select_skin_context.select_slider_hit(snapshot, x, y)
    }

    /// プレイスキンの document。
    pub fn play_skin_document(&self) -> Option<&SkinDocument> {
        self.play_skin_context.document()
    }

    pub fn set_play_skin_user_selected_options(&mut self, enabled_options: Vec<i32>) -> bool {
        self.play_skin_context.set_user_selected_options(enabled_options)
    }

    pub fn play_skin_timer_animation_duration_ms(&self, timer: i32) -> i32 {
        self.play_skin_context.timer_animation_duration_ms(timer)
    }

    /// 決定スキンの document。
    pub fn decide_skin_document(&self) -> Option<&SkinDocument> {
        self.decide_skin_context.document()
    }

    /// リザルトスキンの document。
    pub fn result_skin_document(&self) -> Option<&SkinDocument> {
        self.result_skin_context.document()
    }

    pub fn resize_surface(&mut self, size: SurfaceSize) {
        let Some(gpu) = &mut self.gpu else {
            return;
        };
        if !size.is_drawable() {
            return;
        }

        gpu.resize(size);
    }

    pub fn render_scene(&mut self, scene: AppSceneSnapshot) -> Result<()> {
        self.render_scene_status(scene).map(|_| ())
    }

    pub fn render_scene_status(&mut self, scene: AppSceneSnapshot) -> Result<RenderSurfaceStatus> {
        let entering_scene = self.last_scene.as_ref().is_none_or(|previous| {
            std::mem::discriminant(previous) != std::mem::discriminant(&scene)
        });
        if entering_scene {
            match &scene {
                AppSceneSnapshot::Select(_) => self
                    .select_dynamic_timer_runtime
                    .reset_for_document(self.select_skin_context.document()),
                AppSceneSnapshot::Decide(_) => self
                    .decide_dynamic_timer_runtime
                    .reset_for_document(self.decide_skin_context.document()),
                AppSceneSnapshot::Play(_) => self
                    .play_dynamic_timer_runtime
                    .reset_for_document(self.play_skin_context.document()),
                AppSceneSnapshot::Result(_) => self
                    .result_dynamic_timer_runtime
                    .reset_for_document(self.result_skin_context.document()),
            }
        }
        let plan_start = Instant::now();
        let plan = match &scene {
            AppSceneSnapshot::Select(_) => DrawPlan::from_scene_with_skin(
                &scene,
                &self.select_skin_context,
                &mut self.select_dynamic_timer_runtime,
            ),
            AppSceneSnapshot::Decide(_) => DrawPlan::from_scene_with_skin(
                &scene,
                &self.decide_skin_context,
                &mut self.decide_dynamic_timer_runtime,
            ),
            AppSceneSnapshot::Play(_) => DrawPlan::from_scene_with_skin(
                &scene,
                &self.play_skin_context,
                &mut self.play_dynamic_timer_runtime,
            ),
            AppSceneSnapshot::Result(_) => DrawPlan::from_scene_with_skin(
                &scene,
                &self.result_skin_context,
                &mut self.result_dynamic_timer_runtime,
            ),
        };
        let plan_us = plan_start.elapsed().as_micros();
        let commands = plan.commands.len();
        self.last_plan_canvas_policy = self.canvas_policy_for_scene(&scene);
        self.last_scene = Some(scene);
        self.last_plan = Some(plan);

        let status = self.render_last_plan()?;
        self.last_frame_timings = Some(RenderFrameTimings {
            plan_us,
            commands,
            ..self.last_frame_timings.unwrap_or_default()
        });
        Ok(status)
    }

    /// 次の描画フレームで重ねる egui の描画データを差し込む。
    ///
    /// `render_scene_status` / `render_last_plan` の呼び出しで消費される。
    pub fn set_egui_frame(&mut self, frame: EguiFrame) {
        self.pending_egui = Some(frame);
    }

    pub fn set_present_mode(&mut self, present_mode: WgpuPresentMode) {
        if self.present_mode == present_mode {
            return;
        }
        self.present_mode = present_mode;
        if let Some(gpu) = &mut self.gpu {
            gpu.set_present_mode(present_mode);
            tracing::info!(requested = ?present_mode, "present mode updated");
        }
    }

    pub fn set_internal_resolution_mode(&mut self, mode: InternalResolutionMode) {
        if self.internal_resolution_mode == mode {
            return;
        }
        self.internal_resolution_mode = mode;
        if let Some(gpu) = &mut self.gpu {
            gpu.clear_internal_scene_target();
        }
        tracing::info!(?mode, "internal resolution mode updated");
    }

    pub fn surface_presentation_status(&self) -> Option<SurfacePresentationStatus> {
        let gpu = self.gpu.as_ref()?;
        Some(SurfacePresentationStatus {
            requested_mode: self.present_mode,
            effective_mode: wgpu_present_mode_label(gpu.config.present_mode),
            maximum_frame_latency: gpu.config.desired_maximum_frame_latency,
        })
    }

    pub fn set_backend(&mut self, backend: WgpuBackend) {
        self.backend = backend;
    }

    /// 未指定テキストの CJK 字形で最優先する地域 coverage を変更する。
    ///
    /// 優先 face に無い文字は、他の全 CJK coverage と一般 sans-serif へ
    /// 文字単位で fallback する。スキンが明示指定したフォントには影響しない。
    pub fn set_default_font_coverage(&mut self, coverage: bmz_font::FontCoverage) {
        if self.default_font_coverage == coverage {
            return;
        }
        self.default_font_coverage = coverage;
        if let Some(gpu) = &mut self.gpu {
            gpu.set_default_font_coverage(coverage);
        }
    }

    /// 未指定テキストの fallback として使う、アプリ同梱フォントの検索ディレクトリを設定する。
    ///
    /// ここで指定した resource font は OS フォントより先に解決される。明示指定された
    /// スキンフォントの選択には影響しない。
    pub fn set_default_font_search_paths(&mut self, paths: Vec<PathBuf>) {
        if self.default_font_search_paths == paths {
            return;
        }
        self.default_font_search_paths = paths;
        if let Some(gpu) = &mut self.gpu {
            gpu.set_default_font_search_paths(self.default_font_search_paths.clone());
        }
    }

    pub fn render_last_plan(&mut self) -> Result<RenderSurfaceStatus> {
        let egui = self.pending_egui.take();
        let screenshot = self.pending_screenshot.take();
        let Some(gpu) = &mut self.gpu else {
            return Ok(RenderSurfaceStatus::SkippedNoSurface);
        };
        let Some(plan) = &self.last_plan else {
            return Ok(RenderSurfaceStatus::SkippedNoSurface);
        };

        let (status, gpu_timings) = gpu.render_plan(
            plan,
            self.last_plan_canvas_policy,
            self.internal_resolution_mode,
            &self.fonts,
            &self.bitmap_fonts,
            egui.as_ref(),
            screenshot.as_ref(),
        )?;
        self.last_frame_timings = Some(RenderFrameTimings {
            draw_us: gpu_timings.draw_us,
            text_us: gpu_timings.text_us,
            geometry_us: gpu_timings.geometry_us,
            upload_us: gpu_timings.upload_us,
            submit_us: gpu_timings.submit_us,
            surface_us: gpu_timings.surface_us,
            bind_us: gpu_timings.bind_us,
            encode_us: gpu_timings.encode_us,
            queue_us: gpu_timings.queue_us,
            present_us: gpu_timings.present_us,
            steps: gpu_timings.steps,
            rect_steps: gpu_timings.rect_steps,
            image_steps: gpu_timings.image_steps,
            text_steps: gpu_timings.text_steps,
            rect_instances: gpu_timings.rect_instances,
            image_instances: gpu_timings.image_instances,
            text_instances: gpu_timings.text_instances,
            ..self.last_frame_timings.unwrap_or_default()
        });
        Ok(status)
    }

    pub fn request_screenshot(&mut self, path: impl Into<PathBuf>) {
        self.pending_screenshot =
            Some(ScreenshotRequest { path: path.into(), copy_to_clipboard: false });
    }

    pub fn request_screenshot_with_clipboard(&mut self, path: impl Into<PathBuf>) {
        self.pending_screenshot =
            Some(ScreenshotRequest { path: path.into(), copy_to_clipboard: true });
    }

    /// 次の描画フレームでスクリーンショットを撮る予定があるか。
    ///
    /// 撮影フレームではトースト等の一時 UI を隠す判定に使う。
    pub fn has_pending_screenshot(&self) -> bool {
        self.pending_screenshot.is_some()
    }

    pub fn flush_pending_screenshots(&mut self) -> Result<()> {
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        gpu.flush_pending_screenshots()
    }

    pub fn last_scene(&self) -> Option<&AppSceneSnapshot> {
        self.last_scene.as_ref()
    }

    pub fn last_plan(&self) -> Option<&DrawPlan> {
        self.last_plan.as_ref()
    }

    pub fn last_frame_timings(&self) -> Option<RenderFrameTimings> {
        self.last_frame_timings
    }

    fn select_skin_canvas_point(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let Some(surface) = self.gpu.as_ref().map(WgpuRenderer::surface_size) else {
            return Some((x, y));
        };
        let viewport =
            CanvasViewport::from_policy(surface, self.select_skin_canvas_render_policy());
        viewport.surface_to_canvas_point(x, y)
    }

    fn result_skin_canvas_point(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let Some(surface) = self.gpu.as_ref().map(WgpuRenderer::surface_size) else {
            return Some((x, y));
        };
        let viewport =
            CanvasViewport::from_policy(surface, self.result_skin_canvas_render_policy());
        viewport.surface_to_canvas_point(x, y)
    }

    fn canvas_policy_for_scene(&self, scene: &AppSceneSnapshot) -> CanvasRenderPolicy {
        match scene {
            AppSceneSnapshot::Select(_) => self.select_skin_canvas_render_policy(),
            AppSceneSnapshot::Decide(_) => self.decide_skin_canvas_render_policy(),
            AppSceneSnapshot::Play(_) => self.play_skin_canvas_render_policy(),
            AppSceneSnapshot::Result(_) => self.result_skin_canvas_render_policy(),
        }
    }

    fn select_skin_canvas_render_policy(&self) -> CanvasRenderPolicy {
        self.select_skin_context
            .document()
            .filter(|document| document.skin_type == 5)
            .map(CanvasRenderPolicy::skin_document)
            .unwrap_or_default()
    }

    fn decide_skin_canvas_render_policy(&self) -> CanvasRenderPolicy {
        self.decide_skin_context
            .document()
            .filter(|document| document.skin_type == 6)
            .map(CanvasRenderPolicy::skin_document)
            .unwrap_or_default()
    }

    fn play_skin_canvas_render_policy(&self) -> CanvasRenderPolicy {
        self.play_skin_context.document().map(CanvasRenderPolicy::skin_document).unwrap_or_default()
    }

    fn result_skin_canvas_render_policy(&self) -> CanvasRenderPolicy {
        self.result_skin_context
            .document()
            .filter(|document| matches!(document.skin_type, 7 | 15))
            .map(CanvasRenderPolicy::skin_document)
            .unwrap_or_default()
    }
}

impl fmt::Debug for Renderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Renderer")
            .field("last_scene", &self.last_scene)
            .field("last_plan", &self.last_plan)
            .field("font_count", &self.fonts.len())
            .field("bitmap_font_count", &self.bitmap_fonts.len())
            .field("gpu_attached", &self.gpu.is_some())
            .finish()
    }
}

fn resolve_wgpu_present_mode(
    requested: WgpuPresentMode,
    available: &[wgpu::PresentMode],
) -> wgpu::PresentMode {
    let preferred: &[wgpu::PresentMode] = match requested {
        WgpuPresentMode::Fifo => &[wgpu::PresentMode::Fifo],
        WgpuPresentMode::FifoRelaxed => &[wgpu::PresentMode::FifoRelaxed, wgpu::PresentMode::Fifo],
        WgpuPresentMode::Immediate => &[
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::FifoRelaxed,
            wgpu::PresentMode::Fifo,
        ],
        WgpuPresentMode::Mailbox => {
            &[wgpu::PresentMode::Mailbox, wgpu::PresentMode::FifoRelaxed, wgpu::PresentMode::Fifo]
        }
    };
    if let Some(mode) = preferred.iter().copied().find(|mode| available.contains(mode)) {
        return mode;
    }
    let fallback = available.first().copied().unwrap_or(wgpu::PresentMode::Fifo);
    tracing::warn!(
        requested = ?requested,
        available = ?available,
        fallback = ?fallback,
        "requested present mode is unavailable; using fallback"
    );
    fallback
}

fn configure_surface_settings(
    config: &mut wgpu::SurfaceConfiguration,
    requested_present_mode: WgpuPresentMode,
    available_present_modes: &[wgpu::PresentMode],
) {
    config.present_mode =
        resolve_wgpu_present_mode(requested_present_mode, available_present_modes);
    config.desired_maximum_frame_latency = match config.present_mode {
        wgpu::PresentMode::Mailbox => MAILBOX_MAXIMUM_FRAME_LATENCY,
        _ => LOW_LATENCY_MAXIMUM_FRAME_LATENCY,
    };
    config.usage |= wgpu::TextureUsages::COPY_SRC;
}

fn wgpu_present_mode_label(mode: wgpu::PresentMode) -> &'static str {
    match mode {
        wgpu::PresentMode::AutoVsync => "AutoVsync",
        wgpu::PresentMode::AutoNoVsync => "AutoNoVsync",
        wgpu::PresentMode::Fifo => "Fifo",
        wgpu::PresentMode::FifoRelaxed => "FifoRelaxed",
        wgpu::PresentMode::Immediate => "Immediate",
        wgpu::PresentMode::Mailbox => "Mailbox",
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use crate::scene::{AppSceneSnapshot, SelectSnapshot};

    use super::*;

    fn test_surface_size() -> SurfaceSize {
        SurfaceSize { width: 16, height: 9 }
    }

    #[test]
    fn screenshot_png_is_encoded_once_for_file_and_clipboard_use() {
        let png = encode_screenshot_png(1, 1, &[0x12, 0x34, 0x56, 0x78]).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let decoded = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .unwrap()
            .into_rgba8();
        assert_eq!(decoded.into_raw(), [0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn screenshot_dibv5_has_bottom_up_bgra_pixels() {
        let rgba = [
            1, 2, 3, 4, 5, 6, 7, 8, // top row
            9, 10, 11, 12, 13, 14, 15, 16, // bottom row
        ];
        let dib = screenshot_dibv5(2, 2, &rgba).unwrap();

        assert_eq!(dib.len(), 124 + rgba.len());
        assert_eq!(u32::from_le_bytes(dib[0..4].try_into().unwrap()), 124);
        assert_eq!(i32::from_le_bytes(dib[4..8].try_into().unwrap()), 2);
        assert_eq!(i32::from_le_bytes(dib[8..12].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(dib[14..16].try_into().unwrap()), 32);
        assert_eq!(u32::from_le_bytes(dib[16..20].try_into().unwrap()), 3);
        assert_eq!(&dib[124..], &[11, 10, 9, 12, 15, 14, 13, 16, 3, 2, 1, 4, 7, 6, 5, 8]);
    }

    #[test]
    fn screenshot_dibv5_rejects_mismatched_rgba_length() {
        let error = screenshot_dibv5(2, 2, &[0; 15]).unwrap_err();
        assert!(error.to_string().contains("expected 16, got 15"));
    }

    #[test]
    fn text_align_offset_anchors_zero_width_text_like_beatoraja() {
        assert_eq!(text_align_offset_px(TextAlign::Left, 0.0, 80.0), 0.0);
        assert_eq!(text_align_offset_px(TextAlign::Center, 0.0, 80.0), -40.0);
        assert_eq!(text_align_offset_px(TextAlign::Right, 0.0, 80.0), -80.0);
    }

    #[test]
    fn text_align_offset_uses_box_width_when_present() {
        assert_eq!(text_align_offset_px(TextAlign::Center, 120.0, 80.0), 20.0);
        assert_eq!(text_align_offset_px(TextAlign::Right, 120.0, 80.0), 40.0);
    }

    fn test_bitmap_font() -> BitmapFont {
        let mut pages = HashMap::new();
        pages.insert(
            0,
            crate::bitmap_font::BitmapFontPage {
                id: 0,
                path: std::path::PathBuf::from("page.png"),
                image: crate::assets::RgbaImageAsset {
                    width: 1,
                    height: 1,
                    pixels: vec![255, 255, 255, 255],
                },
            },
        );
        let mut glyphs = HashMap::new();
        glyphs.insert(
            'A',
            crate::bitmap_font::BitmapFontGlyph {
                id: 'A',
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                xoffset: 0,
                yoffset: 0,
                xadvance: 1,
                page: 0,
            },
        );
        BitmapFont {
            size: 10,
            line_height: 10,
            base: 8,
            ascent: 7.0,
            scale_width: 1,
            scale_height: 1,
            pages,
            glyphs,
        }
    }

    fn assert_approx(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.0001, "expected {expected}, got {actual}");
    }

    fn font_supports_japanese<F: Font>(font: &F) -> bool {
        font.glyph_id('あ').0 != 0 && font.glyph_id('日').0 != 0
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn auto_renderer_backend_prefers_vulkan_on_linux() {
        assert_eq!(auto_wgpu_backends(), wgpu::Backends::VULKAN);
        assert_eq!(
            fallback_wgpu_backends(WgpuBackend::Auto),
            &[WgpuBackend::Vulkan, WgpuBackend::Gl]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn auto_renderer_backend_prefers_dx12_on_windows() {
        assert_eq!(auto_wgpu_backends(), wgpu::Backends::DX12);
        assert_eq!(
            fallback_wgpu_backends(WgpuBackend::Auto),
            &[WgpuBackend::Dx12, WgpuBackend::Vulkan, WgpuBackend::Gl]
        );
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    #[test]
    fn auto_renderer_backend_keeps_default_candidates_on_other_platforms() {
        assert_eq!(auto_wgpu_backends(), wgpu::Backends::all());
        assert_eq!(fallback_wgpu_backends(WgpuBackend::Auto), &[WgpuBackend::Auto]);
    }

    #[test]
    fn present_mode_fallbacks_follow_vsync_mode_semantics() {
        use wgpu::PresentMode::{Fifo, FifoRelaxed, Mailbox};

        assert_eq!(resolve_wgpu_present_mode(WgpuPresentMode::Fifo, &[Fifo]), Fifo);
        assert_eq!(resolve_wgpu_present_mode(WgpuPresentMode::FifoRelaxed, &[Fifo]), Fifo);
        assert_eq!(
            resolve_wgpu_present_mode(WgpuPresentMode::Immediate, &[Mailbox, Fifo]),
            Mailbox
        );
        assert_eq!(
            resolve_wgpu_present_mode(WgpuPresentMode::Immediate, &[FifoRelaxed, Fifo]),
            FifoRelaxed
        );
        assert_eq!(
            resolve_wgpu_present_mode(WgpuPresentMode::Mailbox, &[FifoRelaxed, Fifo]),
            FifoRelaxed
        );
        assert_eq!(resolve_wgpu_present_mode(WgpuPresentMode::Mailbox, &[Fifo]), Fifo);
    }

    #[test]
    fn surface_settings_prioritize_low_latency_and_preserve_capture_usage() {
        let mut config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: 1,
            height: 1,
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };

        configure_surface_settings(
            &mut config,
            WgpuPresentMode::Mailbox,
            &[wgpu::PresentMode::Fifo],
        );

        assert_eq!(config.desired_maximum_frame_latency, 1);
        assert_eq!(config.present_mode, wgpu::PresentMode::Fifo);
        assert!(config.usage.contains(wgpu::TextureUsages::COPY_SRC));

        configure_surface_settings(
            &mut config,
            WgpuPresentMode::Mailbox,
            &[wgpu::PresentMode::Mailbox, wgpu::PresentMode::Fifo],
        );
        assert_eq!(config.present_mode, wgpu::PresentMode::Mailbox);
        assert_eq!(config.desired_maximum_frame_latency, 2);
    }

    #[test]
    fn screenshot_unpack_removes_row_padding() {
        let mapped =
            [1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 9, 10, 11, 12, 13, 14, 15, 16, 0, 0, 0, 0];

        let rgba = unpack_screenshot_rgba(&mapped, 2, 2, 12, wgpu::TextureFormat::Rgba8Unorm);

        assert_eq!(rgba, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
    }

    #[test]
    fn screenshot_unpack_converts_bgra_to_rgba() {
        let mapped = [10, 20, 30, 40];

        let rgba = unpack_screenshot_rgba(&mapped, 1, 1, 4, wgpu::TextureFormat::Bgra8Unorm);

        assert_eq!(rgba, vec![30, 20, 10, 40]);
    }

    #[test]
    fn renderer_records_last_scene() {
        let mut renderer = Renderer::default();
        let scene = AppSceneSnapshot::Select(SelectSnapshot {
            chart_count: 1,
            selected_index: 0,
            selected_chart_id: Some(7),
            selected_title: "test".to_string(),
            rows: Vec::new(),
            ..Default::default()
        });

        renderer.render_scene(scene.clone()).unwrap();

        assert_eq!(renderer.last_scene(), Some(&scene));
        assert!(renderer.last_plan().is_some());
    }

    #[test]
    fn select_skin_context_update_does_not_reset_play_dynamic_timers() {
        use crate::skin::{
            SKIN_DYNAMIC_TIMER_BASE, SkinDocument, SkinDrawState, SkinDynamicTimerDef, SkinManifest,
        };

        let mut renderer = Renderer::default();
        let mut document: SkinDocument =
            serde_json::from_str(r#"{ "type": 0, "w": 100, "h": 100 }"#).unwrap();
        document.dynamic_timers.push(SkinDynamicTimerDef {
            id: SKIN_DYNAMIC_TIMER_BASE,
            observe: "number(0) >= 0".to_string(),
        });
        let manifest: SkinManifest = SkinManifest::default();
        let context =
            SkinContext::from_manifest_and_document(manifest, document.clone(), Vec::new());
        renderer.set_play_skin_context(context, false);

        let mut state = SkinDrawState::default();
        renderer.play_dynamic_timer_runtime.advance(&document, &mut state, 5_000);
        assert_eq!(state.dynamic_timer_ms[0], Some(0));

        renderer.play_dynamic_timer_runtime.advance(&document, &mut state, 8_000);
        assert_eq!(state.dynamic_timer_ms[0], Some(3_000));

        renderer.set_select_skin_context(SkinContext::default());

        renderer.play_dynamic_timer_runtime.advance(&document, &mut state, 9_000);
        assert_eq!(state.dynamic_timer_ms[0], Some(4_000));
    }

    #[test]
    fn result_skin_fadeout_ms_reads_document_or_defaults_to_zero() {
        use crate::skin::{SkinContext, SkinDocument, SkinManifest};

        let mut renderer = Renderer::default();
        // ドキュメントスキン未設定なら 0 (フェードアウトなし)。
        assert_eq!(renderer.result_skin_fadeout_ms(), 0);

        let document: SkinDocument =
            serde_json::from_str(r#"{ "type": 7, "w": 100, "h": 100, "fadeout": 300 }"#).unwrap();
        let manifest: SkinManifest = SkinManifest::default();
        renderer.set_result_skin_context(SkinContext::from_manifest_and_document(
            manifest,
            document,
            [],
        ));

        assert_eq!(renderer.result_skin_fadeout_ms(), 300);
    }

    #[test]
    fn result_skin_timer_animation_duration_reads_document_or_defaults_to_zero() {
        use crate::skin::{SkinContext, SkinDocument, SkinManifest};

        let mut renderer = Renderer::default();
        assert_eq!(renderer.result_skin_timer_animation_duration_ms(2), 0);

        let document: SkinDocument = serde_json::from_str(
            r#"{
                "type": 7,
                "w": 100,
                "h": 100,
                "destination": [{
                    "id": "fadeout",
                    "timer": 2,
                    "dst": [{ "time": 0 }, { "time": 500 }]
                }]
            }"#,
        )
        .unwrap();
        let manifest: SkinManifest = SkinManifest::default();
        renderer.set_result_skin_context(SkinContext::from_manifest_and_document(
            manifest,
            document,
            [],
        ));

        assert_eq!(renderer.result_skin_timer_animation_duration_ms(2), 500);
    }

    #[test]
    fn surface_size_requires_non_zero_dimensions() {
        assert!(SurfaceSize { width: 1, height: 1 }.is_drawable());
        assert!(!SurfaceSize { width: 0, height: 1 }.is_drawable());
        assert!(!SurfaceSize { width: 1, height: 0 }.is_drawable());
    }

    #[test]
    fn canvas_viewport_expand_uses_full_surface() {
        let viewport = CanvasViewport::from_policy(
            SurfaceSize { width: 320, height: 240 },
            CanvasRenderPolicy::default(),
        );

        assert_eq!(viewport.rect, Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 });
        assert!(viewport.is_identity());
        assert_eq!(viewport.content_size(), SurfaceSize { width: 320, height: 240 });
    }

    #[test]
    fn skin_internal_resolution_only_downscales_larger_surface_content() {
        let policy = CanvasRenderPolicy {
            fit_mode: CanvasFitMode::Contain,
            canvas_size: Some(CanvasSize { width: 1920, height: 1080 }),
        };
        let four_k = SurfaceSize { width: 3840, height: 2160 };

        assert_eq!(
            policy.internal_render_size(four_k, InternalResolutionMode::Skin),
            Some(SurfaceSize { width: 1920, height: 1080 })
        );
        assert_eq!(policy.internal_render_size(four_k, InternalResolutionMode::Native), None);
        assert_eq!(
            policy.internal_render_size(
                SurfaceSize { width: 1280, height: 720 },
                InternalResolutionMode::Skin,
            ),
            None
        );
        assert_eq!(
            CanvasRenderPolicy::default()
                .internal_render_size(four_k, InternalResolutionMode::Skin),
            None
        );
    }

    #[test]
    fn skin_internal_resolution_keeps_surface_contain_rect_for_final_upscale() {
        let surface = SurfaceSize { width: 1000, height: 1000 };
        let policy = CanvasRenderPolicy {
            fit_mode: CanvasFitMode::Contain,
            canvas_size: Some(CanvasSize { width: 320, height: 180 }),
        };

        assert_eq!(
            policy.internal_render_size(surface, InternalResolutionMode::Skin),
            Some(SurfaceSize { width: 320, height: 180 })
        );
        let output = CanvasViewport::from_policy(surface, policy);
        assert_approx(output.rect.x, 0.0);
        assert_approx(output.rect.y, 0.21875);
        assert_approx(output.rect.width, 1.0);
        assert_approx(output.rect.height, 0.5625);
        assert!(
            CanvasViewport::from_policy(SurfaceSize { width: 320, height: 180 }, policy)
                .is_identity()
        );
    }

    #[test]
    fn canvas_viewport_contain_same_aspect_uses_identity_transform() {
        let viewport = CanvasViewport::from_policy(
            SurfaceSize { width: 2560, height: 1440 },
            CanvasRenderPolicy {
                fit_mode: CanvasFitMode::Contain,
                canvas_size: Some(CanvasSize { width: 1920, height: 1080 }),
            },
        );

        assert!(viewport.is_identity());
        assert_eq!(viewport.content_size(), SurfaceSize { width: 2560, height: 1440 });
    }

    #[test]
    fn canvas_viewport_contain_letterboxes_tall_surface() {
        let viewport = CanvasViewport::from_policy(
            SurfaceSize { width: 1000, height: 1000 },
            CanvasRenderPolicy {
                fit_mode: CanvasFitMode::Contain,
                canvas_size: Some(CanvasSize { width: 16, height: 9 }),
            },
        );

        assert_approx(viewport.rect.x, 0.0);
        assert_approx(viewport.rect.y, 0.21875);
        assert_approx(viewport.rect.width, 1.0);
        assert_approx(viewport.rect.height, 0.5625);
        assert!(!viewport.is_identity());
        assert_eq!(viewport.content_size(), SurfaceSize { width: 1000, height: 563 });
    }

    #[test]
    fn canvas_viewport_maps_surface_points_back_to_canvas() {
        let viewport = CanvasViewport::from_policy(
            SurfaceSize { width: 1000, height: 1000 },
            CanvasRenderPolicy {
                fit_mode: CanvasFitMode::Contain,
                canvas_size: Some(CanvasSize { width: 16, height: 9 }),
            },
        );

        let (x, y) = viewport.surface_to_canvas_point(0.5, 0.5).unwrap();
        assert_approx(x, 0.5);
        assert_approx(y, 0.5);
        assert!(viewport.surface_to_canvas_point(0.5, 0.1).is_none());
    }

    #[test]
    fn plan_geometry_applies_canvas_viewport_to_images() {
        let surface = SurfaceSize { width: 1000, height: 1000 };
        let viewport = CanvasViewport::from_policy(
            surface,
            CanvasRenderPolicy {
                fit_mode: CanvasFitMode::Contain,
                canvas_size: Some(CanvasSize { width: 16, height: 9 }),
            },
        );
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Image {
                rect: Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                uv: UvRect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                source_size: None,
                texture: TextureId(0),
                tint: Color::rgb(1.0, 1.0, 1.0),
                blend: BlendMode::Normal,
                linear_filter: false,
            }],
        };

        let geometry = encode_plan_geometry_with_rect_batch_resolver(
            &plan,
            &TextFrame::default(),
            surface,
            viewport,
            &mut |_, _| None,
        );
        let floats: Vec<f32> = geometry
            .images
            .chunks_exact(std::mem::size_of::<f32>())
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();

        assert_approx(floats[0], 0.0);
        assert_approx(floats[1], 0.21875);
        assert_approx(floats[2], 1.0);
        assert_approx(floats[3], 0.5625);
    }

    #[test]
    fn sampling_uv_insets_subregions_by_half_texel() {
        let uv = sampling_uv_with_half_texel_inset(
            UvRect { x: 0.25, y: 0.5, width: 0.125, height: 0.25 },
            Some(SkinImageSize { width: 256.0, height: 128.0 }),
        );

        assert_approx(uv.x, 0.25 + 0.5 / 256.0);
        assert_approx(uv.y, 0.5 + 0.5 / 128.0);
        assert_approx(uv.width, 0.125 - 1.0 / 256.0);
        assert_approx(uv.height, 0.25 - 1.0 / 128.0);
    }

    #[test]
    fn sampling_uv_keeps_full_texture_axes_unchanged() {
        let uv = sampling_uv_with_half_texel_inset(
            UvRect { x: 0.0, y: 0.25, width: 1.0, height: 0.5 },
            Some(SkinImageSize { width: 256.0, height: 128.0 }),
        );

        assert_approx(uv.x, 0.0);
        assert_approx(uv.width, 1.0);
        assert_approx(uv.y, 0.25 + 0.5 / 128.0);
        assert_approx(uv.height, 0.5 - 1.0 / 128.0);
    }

    #[test]
    fn sampling_uv_does_not_collapse_single_texel_regions() {
        let uv = sampling_uv_with_half_texel_inset(
            UvRect { x: 0.25, y: 0.5, width: 1.0 / 256.0, height: 1.0 / 128.0 },
            Some(SkinImageSize { width: 256.0, height: 128.0 }),
        );

        assert_approx(uv.x, 0.25);
        assert_approx(uv.y, 0.5);
        assert_approx(uv.width, 1.0 / 256.0);
        assert_approx(uv.height, 1.0 / 128.0);
    }

    #[test]
    fn text_instances_are_transformed_into_canvas_viewport() {
        let viewport = CanvasViewport::from_policy(
            SurfaceSize { width: 1000, height: 1000 },
            CanvasRenderPolicy {
                fit_mode: CanvasFitMode::Contain,
                canvas_size: Some(CanvasSize { width: 16, height: 9 }),
            },
        );
        let mut instances = Vec::new();
        for value in [0.1_f32, 0.2, 0.3, 0.4, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0] {
            instances.extend_from_slice(&value.to_le_bytes());
        }

        viewport.transform_text_instances(&mut instances);
        let floats: Vec<f32> = instances
            .chunks_exact(std::mem::size_of::<f32>())
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();

        assert_approx(floats[0], 0.1);
        assert_approx(floats[1], 0.33125);
        assert_approx(floats[2], 0.3);
        assert_approx(floats[3], 0.225);
        assert_approx(floats[4], 0.0);
        assert_approx(floats[8], 1.0);
    }

    #[test]
    fn text_wrapping_splits_lines_by_max_width() {
        let Some(font) = load_default_font() else { return };
        let scale = PxScale::from(16.0);
        let scaled_font = font.as_scaled(scale);
        let one_char_width = text_width_px("W", &font, &scaled_font);
        let lines = wrap_text_to_width("WWW", &font, &scaled_font, one_char_width * 1.5);

        assert_eq!(lines, vec!["W", "W", "W"]);
    }

    #[test]
    fn text_shadow_emits_extra_text_instances() {
        let Some(font) = load_default_font() else { return };
        let surface = SurfaceSize { width: 320, height: 240 };
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: "A".to_string(),
                caret: None,
                style: TextStyle {
                    font_id: None,
                    size: 0.1,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: Some(crate::plan::TextShadow {
                        color: Color::rgba(0.0, 0.0, 0.0, 0.5),
                        offset: Point { x: 0.01, y: 0.01 },
                    }),
                },
            }],
        };
        let frame = build_text_frame(&plan, &font, &HashMap::new(), &HashMap::new(), surface);

        assert_eq!(frame.instances.len(), TEXT_INSTANCE_BYTES * 2);
    }

    #[test]
    fn cached_text_frame_only_marks_new_glyphs_dirty() {
        let Some(font) = load_default_font() else { return };
        let surface = SurfaceSize { width: 320, height: 240 };
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: "FPS 20".to_string(),
                caret: None,
                style: TextStyle {
                    font_id: None,
                    size: 0.1,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            }],
        };
        let mut atlas = TextAtlasCache::new(TEXT_ATLAS_WIDTH);

        let first = build_text_frame_with_cache(
            &plan,
            &font,
            &HashMap::new(),
            &HashMap::new(),
            surface,
            &mut atlas,
        );
        let second = build_text_frame_with_cache(
            &plan,
            &font,
            &HashMap::new(),
            &HashMap::new(),
            surface,
            &mut atlas,
        );

        assert!(!first.instances.is_empty());
        assert!(!first.dirty_regions.is_empty());
        assert_eq!(second.instances.len(), first.instances.len());
        assert!(second.dirty_regions.is_empty());
    }

    #[test]
    fn cached_text_frame_reuses_static_text_layouts() {
        let Some(font) = load_default_font() else { return };
        let surface = SurfaceSize { width: 320, height: 240 };
        let style = TextStyle {
            font_id: None,
            size: 0.1,
            bitmap_size: None,
            color: Color::rgb(1.0, 1.0, 1.0),
            layer: crate::plan::TextLayer::Skin,
            align: TextAlign::Left,
            max_width: 0.0,
            overflow: TextOverflow::Overflow,
            wrapping: false,
            outline: None,
            shadow: None,
        };
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![
                DrawCommand::Text {
                    origin: Point { x: 0.1, y: 0.1 },
                    text: "STATIC".to_string(),
                    caret: None,
                    style: style.clone(),
                },
                DrawCommand::Text {
                    origin: Point { x: 0.1, y: 0.1 },
                    text: "STATIC".to_string(),
                    caret: None,
                    style,
                },
            ],
        };
        let mut atlas = TextAtlasCache::new(TEXT_ATLAS_WIDTH);

        let first = build_text_frame_with_cache(
            &plan,
            &font,
            &HashMap::new(),
            &HashMap::new(),
            surface,
            &mut atlas,
        );
        let layout_count = atlas.layouts.len();
        let second = build_text_frame_with_cache(
            &plan,
            &font,
            &HashMap::new(),
            &HashMap::new(),
            surface,
            &mut atlas,
        );

        assert!(!first.instances.is_empty());
        assert_eq!(layout_count, 1);
        assert_eq!(second.instances, first.instances);
        assert!(second.dirty_regions.is_empty());
    }

    #[test]
    fn cached_vector_glyph_uses_supersampled_atlas_pixels() {
        let Some(font) = load_default_font() else { return };
        let mut atlas = TextAtlasCache::new(TEXT_ATLAS_WIDTH);
        let Some(glyph) =
            atlas.cached_vector_glyph(DEFAULT_TEXT_FONT_ID, 'A', PxScale::from(24.0), &font)
        else {
            return;
        };

        assert!(glyph.width as f32 > glyph.display_width);
        assert!(glyph.height as f32 > glyph.display_height);
    }

    #[test]
    fn text_atlas_resets_when_height_reaches_limit() {
        let mut atlas = TextAtlasCache::new(TEXT_ATLAS_WIDTH);
        // 上限を超える行を積み、アトラス高さを限界まで成長させる。
        let glyph_height = 64;
        while atlas.atlas_height() < TEXT_ATLAS_MAX_HEIGHT {
            for _ in 0..(TEXT_ATLAS_WIDTH / 32) {
                atlas.reserve(16, glyph_height);
            }
        }
        assert!(atlas.atlas_height() >= TEXT_ATLAS_MAX_HEIGHT);

        // フレーム境界でリセットされ、GPU テクスチャ上限を超えない高さに戻る。
        atlas.begin_frame();
        assert_eq!(atlas.pen_y, 0);
        assert_eq!(atlas.pen_x, 0);
        assert!(atlas.atlas_height() < TEXT_ATLAS_MAX_HEIGHT);
        assert!(atlas.glyphs.is_empty());
        assert_eq!(atlas.layouts.len(), 0);
    }

    #[test]
    fn text_outline_emits_surrounding_text_instances() {
        let Some(font) = load_default_font() else { return };
        let surface = SurfaceSize { width: 320, height: 240 };
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: "A".to_string(),
                caret: None,
                style: TextStyle {
                    font_id: None,
                    size: 0.1,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: Some(crate::plan::TextOutline {
                        color: Color::rgba(0.0, 0.0, 0.0, 0.5),
                        width: 0.01,
                    }),
                    shadow: None,
                },
            }],
        };
        let frame = build_text_frame(&plan, &font, &HashMap::new(), &HashMap::new(), surface);

        assert_eq!(frame.instances.len(), TEXT_INSTANCE_BYTES * 9);
    }

    #[test]
    fn load_default_font_prefers_japanese_capable_font() {
        let Some(font) = load_default_font() else { return };
        // CJK 対応フォントが環境にあれば、必ずそれが採用されていなければならない。
        let cjk_available = bmz_font::resolve_system_font(true).is_some();
        if cjk_available {
            assert!(font_supports_japanese(&font));
        }
    }

    #[test]
    fn default_font_fallback_chain_prefers_requested_coverage() {
        for coverage in bmz_font::ALL_FONT_COVERAGES {
            if bmz_font::resolve_system_font_for_coverage(coverage).is_none() {
                continue;
            }
            let fonts = load_default_font_fallbacks(coverage, &[]);
            let Some(primary) = fonts.primary() else {
                panic!("resolved {coverage:?} font should be loadable");
            };
            assert!(
                coverage.glyph_probes().iter().all(|ch| primary.font.glyph_id(*ch).0 != 0),
                "primary face should match requested {coverage:?} coverage"
            );
        }
    }

    #[test]
    fn bundled_noto_cjk_supplies_ui_fallbacks() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/fonts/noto-cjk");
        let font_roots = vec![root];
        let fallbacks = load_cjk_font_fallback_data(bmz_font::FontCoverage::Japanese, &font_roots);

        assert!(fallbacks.iter().any(|(coverage, data)| {
            *coverage == bmz_font::FontCoverage::Japanese
                && bmz_font::font_supports_coverage(
                    &data.bytes,
                    data.font_index,
                    bmz_font::FontCoverage::Japanese,
                )
        }));
    }

    #[test]
    fn default_font_fallback_uses_selected_face_in_glyph_and_layout_cache_keys() {
        let fonts = load_default_font_fallbacks(bmz_font::FontCoverage::Japanese, &[]);
        let Some(primary) = fonts.primary() else { return };
        let fallback_char = bmz_font::ALL_FONT_COVERAGES
            .iter()
            .flat_map(|coverage| coverage.glyph_probes())
            .copied()
            .find(|ch| {
                fonts.select(*ch).is_some_and(|selected| {
                    selected.cache_id != primary.cache_id && selected.font.glyph_id(*ch).0 != 0
                })
            });
        let Some(fallback_char) = fallback_char else {
            return;
        };
        let selected_id = fonts.select(fallback_char).unwrap().cache_id.clone();
        let surface = SurfaceSize { width: 320, height: 240 };
        let text = format!("A{fallback_char}");
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: text.clone(),
                caret: None,
                style: TextStyle {
                    font_id: None,
                    size: 0.1,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            }],
        };
        let mut atlas = TextAtlasCache::new(TEXT_ATLAS_WIDTH);

        let frame = build_text_frame_with_fallback_cache(
            &plan,
            &fonts,
            &HashMap::new(),
            &HashMap::new(),
            surface,
            &mut atlas,
        );

        assert!(!frame.instances.is_empty());
        assert!(
            atlas
                .glyphs
                .keys()
                .any(|key| { key.ch == fallback_char && key.font_id == selected_id })
        );
        assert!(
            !atlas
                .glyphs
                .keys()
                .any(|key| { key.ch == fallback_char && key.font_id != selected_id })
        );
        assert!(
            atlas
                .layouts
                .entries
                .keys()
                .any(|key| { key.text == text && key.font_id.contains(&selected_id) })
        );
    }

    #[test]
    fn explicit_vector_font_does_not_use_default_fallback_faces() {
        let default_fonts = load_default_font_fallbacks(bmz_font::FontCoverage::Japanese, &[]);
        let Some(primary) = default_fonts.primary() else { return };
        let fallback_char = bmz_font::ALL_FONT_COVERAGES
            .iter()
            .flat_map(|coverage| coverage.glyph_probes())
            .copied()
            .find(|ch| {
                primary.font.glyph_id(*ch).0 == 0
                    && default_fonts
                        .select(*ch)
                        .is_some_and(|selected| selected.font.glyph_id(*ch).0 != 0)
            });
        let Some(fallback_char) = fallback_char else {
            return;
        };
        let custom_id = "skin:custom";
        let mut custom_fonts = HashMap::new();
        custom_fonts.insert(custom_id.to_string(), primary.font.clone());
        let surface = SurfaceSize { width: 320, height: 240 };
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: fallback_char.to_string(),
                caret: None,
                style: TextStyle {
                    font_id: Some(custom_id.to_string()),
                    size: 0.1,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            }],
        };
        let mut atlas = TextAtlasCache::new(TEXT_ATLAS_WIDTH);

        build_text_frame_with_fallback_cache(
            &plan,
            &default_fonts,
            &custom_fonts,
            &HashMap::new(),
            surface,
            &mut atlas,
        );

        assert!(atlas.glyphs.keys().all(|key| key.font_id == custom_id));
        assert!(atlas.layouts.entries.keys().all(|key| key.font_id == custom_id));
    }

    #[test]
    fn explicit_vector_font_renders_without_default_fallback() {
        let Some(font) = load_default_font() else { return };
        let surface = SurfaceSize { width: 320, height: 240 };
        let mut fonts = HashMap::new();
        fonts.insert("skin:custom".to_string(), font);
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: "A".to_string(),
                caret: None,
                style: TextStyle {
                    font_id: Some("skin:custom".to_string()),
                    size: 0.1,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            }],
        };
        let mut atlas = TextAtlasCache::new(TEXT_ATLAS_WIDTH);

        let frame = build_text_frame_with_fallback_cache(
            &plan,
            &FontFallbackChain::default(),
            &fonts,
            &HashMap::new(),
            surface,
            &mut atlas,
        );

        assert!(!frame.instances.is_empty());
    }

    #[test]
    fn text_without_any_font_is_skipped_without_default_fallback() {
        let surface = SurfaceSize { width: 320, height: 240 };
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: "A".to_string(),
                caret: None,
                style: TextStyle {
                    font_id: None,
                    size: 0.1,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            }],
        };
        let mut atlas = TextAtlasCache::new(TEXT_ATLAS_WIDTH);

        let frame = build_text_frame_with_fallback_cache(
            &plan,
            &FontFallbackChain::default(),
            &HashMap::new(),
            &HashMap::new(),
            surface,
            &mut atlas,
        );

        assert!(frame.instances.is_empty());
        assert_eq!(frame.command_quad_counts, vec![0]);
    }

    #[test]
    fn japanese_text_emits_glyph_quads_with_default_font() {
        let Some(font) = load_default_font() else { return };
        if !font_supports_japanese(&font) {
            return;
        }
        let surface = SurfaceSize { width: 320, height: 240 };
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: "日本語と記号★♪".to_string(),
                caret: None,
                style: TextStyle {
                    font_id: None,
                    size: 0.1,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            }],
        };
        let frame = build_text_frame(&plan, &font, &HashMap::new(), &HashMap::new(), surface);

        assert!(!frame.instances.is_empty());
        assert!(frame.pixels.contains(&255));
    }

    #[test]
    fn bitmap_font_text_uses_registered_font() {
        let surface = SurfaceSize { width: 320, height: 240 };
        let mut pages = HashMap::new();
        pages.insert(
            0,
            crate::bitmap_font::BitmapFontPage {
                id: 0,
                path: std::path::PathBuf::from("page.png"),
                image: crate::assets::RgbaImageAsset {
                    width: 1,
                    height: 1,
                    pixels: vec![255, 255, 255, 255],
                },
            },
        );
        let mut glyphs = HashMap::new();
        glyphs.insert(
            'A',
            crate::bitmap_font::BitmapFontGlyph {
                id: 'A',
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                xoffset: 0,
                yoffset: 0,
                xadvance: 1,
                page: 0,
            },
        );
        let mut bitmap_fonts = HashMap::new();
        bitmap_fonts.insert(
            "bitmap".to_string(),
            BitmapFont {
                size: 10,
                line_height: 10,
                base: 8,
                ascent: 7.0,
                scale_width: 1,
                scale_height: 1,
                pages,
                glyphs,
            },
        );
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: "A".to_string(),
                caret: None,
                style: TextStyle {
                    font_id: Some("bitmap".to_string()),
                    size: 0.1,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            }],
        };

        let mut atlas = TextAtlasCache::new(TEXT_ATLAS_WIDTH);
        let frame = build_text_frame_with_fallback_cache(
            &plan,
            &FontFallbackChain::default(),
            &HashMap::new(),
            &bitmap_fonts,
            surface,
            &mut atlas,
        );

        assert_eq!(frame.instances.len(), TEXT_INSTANCE_BYTES);
        assert!(!frame.dirty_regions.is_empty());
    }

    #[test]
    fn bitmap_glyph_non_integer_scale_uses_interpolated_alpha() {
        let page = crate::bitmap_font::BitmapFontPage {
            id: 0,
            path: std::path::PathBuf::from("page.png"),
            image: crate::assets::RgbaImageAsset {
                width: 2,
                height: 1,
                pixels: vec![255, 255, 255, 0, 255, 255, 255, 255],
            },
        };
        let glyph = crate::bitmap_font::BitmapFontGlyph {
            id: 'A',
            x: 0,
            y: 0,
            width: 2,
            height: 1,
            xoffset: 0,
            yoffset: 0,
            xadvance: 2,
            page: 0,
        };

        let pixels = rasterized_bitmap_glyph_pixels(glyph, &page, 1.5, 3, 1);
        let middle_alpha = pixels[7];

        assert!(middle_alpha > 0 && middle_alpha < 255);
    }

    #[test]
    fn bitmap_font_text_positions_glyphs_from_destination_baseline() {
        let Some(default_font) = load_default_font() else { return };
        let surface = SurfaceSize { width: 100, height: 100 };
        let mut pages = HashMap::new();
        pages.insert(
            0,
            crate::bitmap_font::BitmapFontPage {
                id: 0,
                path: std::path::PathBuf::from("page.png"),
                image: crate::assets::RgbaImageAsset {
                    width: 1,
                    height: 1,
                    pixels: vec![255, 255, 255, 255],
                },
            },
        );
        let mut glyphs = HashMap::new();
        glyphs.insert(
            'A',
            crate::bitmap_font::BitmapFontGlyph {
                id: 'A',
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                xoffset: 0,
                yoffset: 7,
                xadvance: 1,
                page: 0,
            },
        );
        let mut bitmap_fonts = HashMap::new();
        bitmap_fonts.insert(
            "bitmap".to_string(),
            BitmapFont {
                size: 30,
                line_height: 45,
                base: 34,
                ascent: 12.0,
                scale_width: 1,
                scale_height: 1,
                pages,
                glyphs,
            },
        );
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: "A".to_string(),
                caret: None,
                style: TextStyle {
                    font_id: Some("bitmap".to_string()),
                    size: 0.3,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            }],
        };

        let frame = build_text_frame(&plan, &default_font, &HashMap::new(), &bitmap_fonts, surface);
        let y = f32::from_le_bytes(frame.instances[4..8].try_into().unwrap());

        assert!((y - 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn bitmap_font_shrink_keeps_text_vertically_centered_in_destination() {
        let Some(default_font) = load_default_font() else { return };
        let surface = SurfaceSize { width: 100, height: 100 };
        let mut pages = HashMap::new();
        pages.insert(
            0,
            crate::bitmap_font::BitmapFontPage {
                id: 0,
                path: std::path::PathBuf::from("page.png"),
                image: crate::assets::RgbaImageAsset {
                    width: 10,
                    height: 10,
                    pixels: vec![255; 10 * 10 * 4],
                },
            },
        );
        let mut glyphs = HashMap::new();
        glyphs.insert(
            'A',
            crate::bitmap_font::BitmapFontGlyph {
                id: 'A',
                x: 0,
                y: 0,
                width: 10,
                height: 10,
                xoffset: 0,
                yoffset: 7,
                xadvance: 10,
                page: 0,
            },
        );
        let mut bitmap_fonts = HashMap::new();
        bitmap_fonts.insert(
            "bitmap".to_string(),
            BitmapFont {
                size: 10,
                line_height: 10,
                base: 7,
                ascent: 7.0,
                scale_width: 10,
                scale_height: 10,
                pages,
                glyphs,
            },
        );
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: "AAAA".to_string(),
                caret: None,
                style: TextStyle {
                    font_id: Some("bitmap".to_string()),
                    size: 0.2,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.4,
                    overflow: TextOverflow::Shrink,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            }],
        };

        let frame = build_text_frame(&plan, &default_font, &HashMap::new(), &bitmap_fonts, surface);
        let y = f32::from_le_bytes(frame.instances[4..8].try_into().unwrap());

        assert!((y - 0.15).abs() < f32::EPSILON);
    }

    #[test]
    fn bitmap_font_text_uses_bitmap_size_for_scale() {
        let Some(default_font) = load_default_font() else { return };
        let surface = SurfaceSize { width: 100, height: 100 };
        let mut pages = HashMap::new();
        pages.insert(
            0,
            crate::bitmap_font::BitmapFontPage {
                id: 0,
                path: std::path::PathBuf::from("page.png"),
                image: crate::assets::RgbaImageAsset {
                    width: 1,
                    height: 1,
                    pixels: vec![255, 255, 255, 255],
                },
            },
        );
        let mut glyphs = HashMap::new();
        glyphs.insert(
            'A',
            crate::bitmap_font::BitmapFontGlyph {
                id: 'A',
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                xoffset: 0,
                yoffset: 0,
                xadvance: 1,
                page: 0,
            },
        );
        let mut bitmap_fonts = HashMap::new();
        bitmap_fonts.insert(
            "bitmap".to_string(),
            BitmapFont {
                size: 10,
                line_height: 10,
                base: 8,
                ascent: 7.0,
                scale_width: 1,
                scale_height: 1,
                pages,
                glyphs,
            },
        );
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::Text {
                origin: Point { x: 0.1, y: 0.1 },
                text: "A".to_string(),
                caret: None,
                style: TextStyle {
                    font_id: Some("bitmap".to_string()),
                    size: 0.3,
                    bitmap_size: Some(0.1),
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: crate::plan::TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            }],
        };

        let frame = build_text_frame(&plan, &default_font, &HashMap::new(), &bitmap_fonts, surface);
        let width = f32::from_le_bytes(frame.instances[8..12].try_into().unwrap());

        assert!((width - 0.01).abs() < f32::EPSILON);
    }

    #[test]
    fn rendering_without_surface_is_skipped() {
        let mut renderer = Renderer::default();
        let scene = AppSceneSnapshot::Select(SelectSnapshot::default());

        assert_eq!(
            renderer.render_scene_status(scene).unwrap(),
            RenderSurfaceStatus::SkippedNoSurface
        );
    }

    #[test]
    fn scene_clear_colors_are_distinct() {
        let select = DrawPlan::from_scene(&AppSceneSnapshot::Select(SelectSnapshot::default()));
        let play = DrawPlan::from_scene(&AppSceneSnapshot::Play(Default::default()));

        assert_ne!(select.clear, play.clear);
    }

    fn sample_image(texture: u32, blend: BlendMode) -> DrawCommand {
        DrawCommand::Image {
            rect: crate::plan::Rect { x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
            uv: crate::plan::UvRect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
            source_size: None,
            texture: crate::plan::TextureId(texture),
            tint: Color::rgb(1.0, 1.0, 1.0),
            blend,
            linear_filter: false,
        }
    }

    fn sample_rect() -> DrawCommand {
        DrawCommand::Rect {
            rect: crate::plan::Rect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 },
            color: Color::rgb(1.0, 1.0, 1.0),
        }
    }

    fn sample_text() -> DrawCommand {
        DrawCommand::Text {
            origin: Point { x: 0.1, y: 0.1 },
            text: "x".to_string(),
            caret: None,
            style: TextStyle {
                font_id: None,
                size: 0.1,
                bitmap_size: None,
                color: Color::rgb(1.0, 1.0, 1.0),
                layer: crate::plan::TextLayer::Skin,
                align: TextAlign::Left,
                max_width: 0.0,
                overflow: TextOverflow::Overflow,
                wrapping: false,
                outline: None,
                shadow: None,
            },
        }
    }

    #[test]
    fn vector_text_caret_rect_uses_font_advance_without_changing_text() {
        let Some(font) = load_default_font() else { return };
        let surface = SurfaceSize { width: 1000, height: 100 };
        let style = TextStyle {
            font_id: None,
            size: 0.1,
            bitmap_size: None,
            color: Color::rgb(1.0, 1.0, 1.0),
            layer: crate::plan::TextLayer::Skin,
            align: TextAlign::Left,
            max_width: 0.0,
            overflow: TextOverflow::Overflow,
            wrapping: false,
            outline: None,
            shadow: None,
        };
        let caret = TextCaret { byte_index: "A".len(), color: Color::rgb(0.8, 0.9, 1.0) };

        let rect =
            vector_text_caret_rect(&Point { x: 0.1, y: 0.2 }, "AB", &style, &font, surface, caret)
                .expect("caret rect");

        let scaled = font.as_scaled(PxScale::from(10.0));
        let expected_x = (100.0 + text_width_px("A", &font, &scaled)) / 1000.0;
        assert_approx(rect.rect.x, expected_x);
        assert_approx(rect.rect.y, 0.2);
        assert_approx(rect.rect.height, 0.1);
        assert_eq!(rect.color, caret.color);
    }

    #[test]
    fn plan_geometry_draws_text_caret_after_text() {
        let mut plan = DrawPlan { clear: Color::rgb(0.0, 0.0, 0.0), commands: vec![sample_text()] };
        let DrawCommand::Text { caret, .. } = &mut plan.commands[0] else {
            unreachable!();
        };
        *caret = Some(TextCaret { byte_index: 1, color: Color::rgb(1.0, 1.0, 1.0) });
        let text_frame = TextFrame {
            command_quad_counts: vec![1],
            command_caret_rects: vec![Some(RectCommand {
                rect: Rect { x: 0.2, y: 0.3, width: 0.01, height: 0.1 },
                color: Color::rgb(1.0, 1.0, 1.0),
            })],
            ..TextFrame::default()
        };

        let geometry = encode_plan_geometry(&plan, &text_frame, test_surface_size());

        assert_eq!(
            geometry.steps,
            vec![
                DrawStep::Text { range: 0..TEXT_INSTANCE_BYTES },
                DrawStep::Rects { range: 0..RECT_INSTANCE_BYTES },
            ]
        );
        assert_eq!(geometry.rects.len(), RECT_INSTANCE_BYTES);
    }

    #[test]
    fn plan_geometry_encodes_one_rect_instance_per_rect_command() {
        let plan = DrawPlan::from_scene(&AppSceneSnapshot::Select(SelectSnapshot {
            chart_count: 1,
            ..Default::default()
        }));

        let geometry = encode_plan_geometry(&plan, &TextFrame::default(), test_surface_size());
        let rect_count = plan
            .commands
            .iter()
            .filter(|command| matches!(command, DrawCommand::Rect { .. }))
            .count();

        assert_eq!(geometry.rects.len(), rect_count * RECT_INSTANCE_BYTES);
    }

    #[test]
    fn plan_geometry_encodes_rect_batch_instances() {
        let rect = crate::plan::Rect { x: 0.1, y: 0.2, width: 0.3, height: 0.4 };
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::RectBatch {
                rects: std::sync::Arc::from([
                    crate::plan::RectCommand { rect, color: Color::rgb(1.0, 0.0, 0.0) },
                    crate::plan::RectCommand { rect, color: Color::rgb(0.0, 1.0, 0.0) },
                ]),
                cache: None,
            }],
        };

        let geometry = encode_plan_geometry(&plan, &TextFrame::default(), test_surface_size());

        assert_eq!(geometry.rects.len(), RECT_INSTANCE_BYTES * 2);
        assert_eq!(
            geometry.stats(),
            DrawStepStats { steps: 1, rect_steps: 1, rect_instances: 2, ..Default::default() }
        );
    }

    #[test]
    fn plan_geometry_skips_invisible_rects_and_images() {
        let visible_rect = crate::plan::Rect { x: 0.1, y: 0.2, width: 0.3, height: 0.4 };
        let zero_width_rect = crate::plan::Rect { width: 0.0, ..visible_rect };
        let mut transparent_image = sample_image(0, BlendMode::Normal);
        let DrawCommand::Image { tint, .. } = &mut transparent_image else { panic!() };
        *tint = Color::rgba(1.0, 1.0, 1.0, 0.0);
        let mut zero_size_image = sample_image(1, BlendMode::Normal);
        let DrawCommand::Image { rect, .. } = &mut zero_size_image else { panic!() };
        rect.height = 0.0;

        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![
                DrawCommand::Rect { rect: visible_rect, color: Color::rgba(1.0, 0.0, 0.0, 0.0) },
                DrawCommand::Rect { rect: zero_width_rect, color: Color::rgb(0.0, 1.0, 0.0) },
                DrawCommand::RectBatch {
                    rects: std::sync::Arc::from([
                        crate::plan::RectCommand {
                            rect: visible_rect,
                            color: Color::rgba(1.0, 1.0, 1.0, 0.0),
                        },
                        crate::plan::RectCommand {
                            rect: zero_width_rect,
                            color: Color::rgb(1.0, 1.0, 1.0),
                        },
                    ]),
                    cache: None,
                },
                transparent_image,
                zero_size_image,
            ],
        };

        let geometry = encode_plan_geometry(&plan, &TextFrame::default(), test_surface_size());

        assert!(geometry.rects.is_empty());
        assert!(geometry.images.is_empty());
        assert_eq!(geometry.steps, Vec::new());
        assert_eq!(geometry.stats(), DrawStepStats::default());
    }

    #[test]
    fn plan_geometry_can_replace_cached_rect_batch_with_image_instance() {
        let rect = crate::plan::Rect { x: 0.1, y: 0.2, width: 0.3, height: 0.4 };
        let cache =
            crate::plan::RectBatchCache { key: crate::plan::RectBatchCacheKey(42), bounds: rect };
        let texture = TextureId(123);
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::RectBatch {
                rects: std::sync::Arc::from([crate::plan::RectCommand {
                    rect,
                    color: Color::rgb(1.0, 0.0, 0.0),
                }]),
                cache: Some(cache),
            }],
        };

        let geometry = encode_plan_geometry_with_rect_batch_resolver(
            &plan,
            &TextFrame::default(),
            test_surface_size(),
            CanvasViewport::from_policy(test_surface_size(), CanvasRenderPolicy::default()),
            &mut |_, _| Some(texture),
        );

        assert!(geometry.rects.is_empty());
        assert_eq!(
            geometry.stats(),
            DrawStepStats { steps: 1, image_steps: 1, image_instances: 1, ..Default::default() }
        );
        assert_eq!(
            geometry.steps,
            vec![DrawStep::Image {
                texture,
                blend: BlendMode::Premultiplied,
                linear: false,
                range: 0..IMAGE_INSTANCE_BYTES,
            }]
        );
    }

    #[test]
    fn plan_geometry_groups_consecutive_images_by_texture() {
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![
                sample_image(0, BlendMode::Normal),
                sample_image(0, BlendMode::Normal),
                sample_image(7, BlendMode::Normal),
            ],
        };

        let geometry = encode_plan_geometry(&plan, &TextFrame::default(), test_surface_size());
        let image_step_sizes: Vec<_> = geometry
            .steps
            .iter()
            .filter_map(|step| match step {
                DrawStep::Image { range, .. } => Some(range.len()),
                _ => None,
            })
            .collect();

        assert_eq!(image_step_sizes, vec![IMAGE_INSTANCE_BYTES * 2, IMAGE_INSTANCE_BYTES]);
        assert_eq!(geometry.images.len(), IMAGE_INSTANCE_BYTES * 3);
        assert_eq!(
            geometry.stats(),
            DrawStepStats { steps: 2, image_steps: 2, image_instances: 3, ..Default::default() }
        );
    }

    #[test]
    fn plan_geometry_separates_image_blend_modes() {
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![sample_image(0, BlendMode::Normal), sample_image(0, BlendMode::Add)],
        };

        let geometry = encode_plan_geometry(&plan, &TextFrame::default(), test_surface_size());
        let blends: Vec<_> = geometry
            .steps
            .iter()
            .filter_map(|step| match step {
                DrawStep::Image { blend, .. } => Some(*blend),
                _ => None,
            })
            .collect();

        assert_eq!(blends, vec![BlendMode::Normal, BlendMode::Add]);
    }

    #[test]
    fn additive_image_blend_uses_source_alpha() {
        let blend = image_blend_state(BlendMode::Add);

        assert_eq!(blend.color.src_factor, wgpu::BlendFactor::SrcAlpha);
        assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::One);
        assert_eq!(blend.color.operation, wgpu::BlendOperation::Add);
    }

    #[test]
    fn premultiplied_image_blend_does_not_apply_source_alpha_twice() {
        let blend = image_blend_state(BlendMode::Premultiplied);

        assert_eq!(blend.color.src_factor, wgpu::BlendFactor::One);
        assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::OneMinusSrcAlpha);
        assert_eq!(blend.color.operation, wgpu::BlendOperation::Add);
        assert_eq!(blend.alpha.src_factor, wgpu::BlendFactor::One);
        assert_eq!(blend.alpha.dst_factor, wgpu::BlendFactor::OneMinusSrcAlpha);
        assert_eq!(blend.alpha.operation, wgpu::BlendOperation::Add);
    }

    #[test]
    fn plan_geometry_splits_image_steps_around_other_commands() {
        // 同じテクスチャの画像でも、間に rect を挟めば別ステップになる。
        // rect が2枚の画像の「間」に描かれ、commands の順序が保たれることの回帰テスト。
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![
                sample_image(1, BlendMode::Normal),
                sample_rect(),
                sample_image(1, BlendMode::Normal),
            ],
        };

        let geometry = encode_plan_geometry(&plan, &TextFrame::default(), test_surface_size());

        assert_eq!(geometry.steps.len(), 3);
        assert!(matches!(geometry.steps[0], DrawStep::Image { .. }));
        assert!(matches!(geometry.steps[1], DrawStep::Rects { .. }));
        assert!(matches!(geometry.steps[2], DrawStep::Image { .. }));
    }

    #[test]
    fn plan_geometry_orders_text_steps_by_command_position() {
        // Text コマンドが Image より前にあれば、描画ステップも Image より前 (背面) になる。
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![sample_text(), sample_image(1, BlendMode::Normal)],
        };
        // sample_text が 2 quad を生成したと仮定したテキストフレーム。
        let text_frame = TextFrame { command_quad_counts: vec![2], ..TextFrame::default() };

        let geometry = encode_plan_geometry(&plan, &text_frame, test_surface_size());

        assert_eq!(geometry.steps.len(), 2);
        assert_eq!(geometry.steps[0], DrawStep::Text { range: 0..TEXT_INSTANCE_BYTES * 2 });
        assert!(matches!(geometry.steps[1], DrawStep::Image { .. }));
        assert_eq!(
            geometry.stats(),
            DrawStepStats {
                steps: 2,
                image_steps: 1,
                text_steps: 1,
                image_instances: 1,
                text_instances: 2,
                ..Default::default()
            }
        );
    }

    #[test]
    fn plan_geometry_writes_rotation_instance_data() {
        let plan = DrawPlan {
            clear: Color::rgb(0.0, 0.0, 0.0),
            commands: vec![DrawCommand::RotatedImage {
                rect: crate::plan::Rect { x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
                uv: crate::plan::UvRect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                source_size: None,
                texture: crate::plan::TextureId(0),
                tint: Color::rgb(1.0, 1.0, 1.0),
                blend: BlendMode::Normal,
                linear_filter: false,
                angle_rad: 1.25,
                center: Point { x: 0.0, y: 1.0 },
            }],
        };

        let geometry = encode_plan_geometry(&plan, &TextFrame::default(), test_surface_size());
        let floats: Vec<f32> = geometry
            .images
            .chunks_exact(std::mem::size_of::<f32>())
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();

        assert_eq!(floats.len(), IMAGE_INSTANCE_FLOATS);
        assert_eq!(floats[12], 1.25);
        assert_eq!(floats[13], 0.0);
        assert_eq!(floats[14], 1.0);
        assert!((floats[15] - 16.0 / 9.0).abs() < f32::EPSILON);
    }

    #[test]
    fn validate_rgba_texture_rejects_invalid_payloads() {
        assert!(validate_rgba_texture(1, 1, &[255, 255, 255, 255]).is_ok());
        assert!(validate_rgba_texture(0, 1, &[255, 255, 255, 255]).is_err());
        assert!(validate_rgba_texture(1, 1, &[255]).is_err());
    }

    #[test]
    fn renderer_queues_texture_assets_before_surface_attach() {
        let mut renderer = Renderer::default();
        let asset =
            crate::assets::RgbaImageAsset { width: 1, height: 1, pixels: vec![255, 0, 0, 255] };

        renderer.upsert_image_asset(crate::plan::TextureId(9), &asset).unwrap();

        assert_eq!(renderer.pending_textures.len(), 1);
        assert_eq!(renderer.pending_textures[0].id, crate::plan::TextureId(9));
    }

    #[test]
    fn installing_vector_font_replaces_stale_bitmap_font_with_same_id() {
        let Some(font) = load_default_font() else { return };
        let mut renderer = Renderer::default();

        renderer.insert_bitmap_font_entry("play:0".to_string(), test_bitmap_font());
        renderer.insert_vector_font("play:0".to_string(), font);

        assert!(renderer.fonts.contains_key("play:0"));
        assert!(!renderer.bitmap_fonts.contains_key("play:0"));
    }

    #[test]
    fn installing_bitmap_font_replaces_stale_vector_font_with_same_id() {
        let Some(font) = load_default_font() else { return };
        let mut renderer = Renderer::default();

        renderer.insert_vector_font("play:0".to_string(), font);
        renderer.insert_bitmap_font_entry("play:0".to_string(), test_bitmap_font());

        assert!(renderer.bitmap_fonts.contains_key("play:0"));
        assert!(!renderer.fonts.contains_key("play:0"));
    }
}
