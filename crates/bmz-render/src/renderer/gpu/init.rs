use super::*;

impl WgpuRenderer {
    pub(in crate::renderer) fn new_with_fallbacks<T>(
        window: T,
        size: SurfaceSize,
        present_mode: WgpuPresentMode,
        frame_latency_mode: WgpuFrameLatencyMode,
        backend: WgpuBackend,
        default_font_coverage: bmz_font::FontCoverage,
        default_font_search_paths: Vec<PathBuf>,
    ) -> Result<Self>
    where
        T: Into<wgpu::SurfaceTarget<'static>> + Clone,
    {
        let candidates = fallback_wgpu_backends(backend);
        let mut last_error = None;
        for candidate in candidates {
            match Self::new(
                window.clone(),
                size,
                present_mode,
                frame_latency_mode,
                *candidate,
                default_font_coverage,
                default_font_search_paths.clone(),
            ) {
                Ok(renderer) => {
                    if backend == WgpuBackend::Auto && *candidate != WgpuBackend::Auto {
                        tracing::info!(backend = ?candidate, "selected auto renderer backend");
                    }
                    return Ok(renderer);
                }
                Err(error) => {
                    tracing::warn!(
                        requested = ?backend,
                        candidate = ?candidate,
                        %error,
                        "failed to initialize renderer backend candidate"
                    );
                    last_error = Some(error);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("no renderer backend candidates available")))
    }

    fn new<T>(
        window: T,
        size: SurfaceSize,
        present_mode: WgpuPresentMode,
        frame_latency_mode: WgpuFrameLatencyMode,
        backend: WgpuBackend,
        default_font_coverage: bmz_font::FontCoverage,
        default_font_search_paths: Vec<PathBuf>,
    ) -> Result<Self>
    where
        T: Into<wgpu::SurfaceTarget<'static>>,
    {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = backend.to_wgpu();
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance.create_surface(window).context("failed to create wgpu surface")?;
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .context("no compatible GPU adapter found")?;
        // beatoraja スキンには 8192px を超える縦長/横長 PNG (背景アニメシート等) が
        // 含まれることがある。Apple Silicon / モダンGPU 環境では 16384px までは許容
        // されるので、アダプタが報告する上限まで広げて取得する。
        let adapter_limits = adapter.limits();
        let required_limits = wgpu::Limits {
            max_texture_dimension_2d: adapter_limits.max_texture_dimension_2d,
            max_texture_dimension_1d: adapter_limits.max_texture_dimension_1d,
            ..Default::default()
        };
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("bmz-render device"),
            required_features: wgpu::Features::empty(),
            required_limits,
            experimental_features: wgpu::ExperimentalFeatures::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .context("failed to request wgpu device")?;
        let capabilities = surface.get_capabilities(&adapter);
        let mut config = surface
            .get_default_config(&adapter, size.width, size.height)
            .ok_or_else(|| anyhow!("surface is not supported by the selected adapter"))?;
        // sRGB フレームバッファだと PNG の sRGB 値が二重 gamma エンコードされて白っぽくなる。
        // beatoraja (libGDX) は GL_FRAMEBUFFER_SRGB を使わないため値をそのまま表示する。
        // それと合わせるため sRGB サフィックスを除去して non-sRGB サーフェスとして使う。
        config.format = config.format.remove_srgb_suffix();
        configure_surface_settings(
            &mut config,
            present_mode,
            frame_latency_mode,
            &capabilities.present_modes,
        );
        surface.configure(&device, &config);
        tracing::info!(
            requested = ?present_mode,
            effective = ?config.present_mode,
            available = ?capabilities.present_modes,
            maximum_frame_latency = config.desired_maximum_frame_latency,
            backend = ?backend,
            "configured renderer present mode"
        );
        let rect_pipeline = create_rect_pipeline(&device, config.format);
        let image_bind_group_layout = create_image_bind_group_layout(&device);
        let image_sampler = create_image_sampler(&device);
        let image_sampler_linear = create_image_sampler_linear(&device);
        let image_pipeline = create_image_pipeline(
            &device,
            config.format,
            &image_bind_group_layout,
            BlendMode::Normal,
        );
        let image_add_pipeline =
            create_image_pipeline(&device, config.format, &image_bind_group_layout, BlendMode::Add);
        let image_premultiplied_pipeline = create_image_pipeline(
            &device,
            config.format,
            &image_bind_group_layout,
            BlendMode::Premultiplied,
        );
        let image_layer_pipeline = create_image_pipeline(
            &device,
            config.format,
            &image_bind_group_layout,
            BlendMode::LayerMask,
        );
        let upscale_pipeline =
            create_upscale_pipeline(&device, config.format, &image_bind_group_layout);
        let upscale_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bmz-render internal upscale buffer"),
            size: IMAGE_INSTANCE_BYTES as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let image_textures = create_default_image_textures(&device, &queue);
        let text_bind_group_layout = create_text_bind_group_layout(&device);
        let text_sampler = create_text_sampler(&device);
        let text_pipeline = create_text_pipeline(&device, config.format, &text_bind_group_layout);
        let egui = EguiPainter::new(&device, config.format);

        Ok(Self {
            device,
            queue,
            config,
            present_modes: capabilities.present_modes,
            rect_pipeline,
            rect_buffer: None,
            rect_buffer_capacity: 0,
            image_pipeline,
            image_add_pipeline,
            image_premultiplied_pipeline,
            image_layer_pipeline,
            image_bind_group_layout,
            image_sampler,
            image_sampler_linear,
            upscale_pipeline,
            upscale_buffer,
            upscale_rect: None,
            internal_scene_target: None,
            image_textures,
            image_bind_group_cache: HashMap::new(),
            image_bind_group_scratch: Vec::new(),
            geometry_scratch: PlanGeometry::default(),
            offscreen_rect_batches: HashMap::new(),
            next_offscreen_rect_batch_texture_id: OFFSCREEN_RECT_BATCH_TEXTURE_BASE,
            image_buffer: None,
            image_buffer_capacity: 0,
            text_pipeline,
            text_bind_group_layout,
            text_sampler,
            text_texture: None,
            text_texture_view: None,
            text_bind_group: None,
            text_texture_size: AtlasSize::default(),
            text_atlas: TextAtlasCache::new(TEXT_ATLAS_WIDTH),
            text_buffer: None,
            text_buffer_capacity: 0,
            default_fonts: load_default_font_fallbacks(
                default_font_coverage,
                &default_font_search_paths,
            ),
            default_font_coverage,
            default_font_search_paths,
            egui,
            pending_screenshot_readbacks: Vec::new(),
            screenshot_save_jobs: Vec::new(),
            surface,
        })
    }
}
