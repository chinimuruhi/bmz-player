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
