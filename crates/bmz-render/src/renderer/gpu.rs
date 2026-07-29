use super::*;

impl WgpuRenderer {
    pub(super) fn new_with_fallbacks<T>(
        window: T,
        size: SurfaceSize,
        present_mode: WgpuPresentMode,
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
        configure_surface_settings(&mut config, present_mode, &capabilities.present_modes);
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

    pub(super) fn resize(&mut self, size: SurfaceSize) {
        if !size.is_drawable() {
            return;
        }

        self.config.width = size.width;
        self.config.height = size.height;
        self.clear_internal_scene_target();
        self.clear_offscreen_rect_batches();
        self.configure_surface();
    }

    pub(super) fn surface_size(&self) -> SurfaceSize {
        SurfaceSize { width: self.config.width, height: self.config.height }
    }

    pub(super) fn clear_internal_scene_target(&mut self) {
        self.internal_scene_target = None;
        self.upscale_rect = None;
    }

    pub(super) fn ensure_internal_scene_target(&mut self, size: SurfaceSize) {
        if self.internal_scene_target.as_ref().is_some_and(|target| target.size == size) {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bmz-render internal scene target"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bmz-render internal scene bind group"),
            layout: &self.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.image_sampler_linear),
                },
            ],
        });
        self.internal_scene_target =
            Some(InternalSceneTarget { _texture: texture, view, bind_group, size });
        self.upscale_rect = None;
        tracing::info!(
            width = size.width,
            height = size.height,
            "created internal scene render target"
        );
    }

    pub(super) fn update_upscale_instance(&mut self, rect: Rect, surface_size: SurfaceSize) {
        if self.upscale_rect == Some(rect) {
            return;
        }
        let mut bytes = Vec::with_capacity(IMAGE_INSTANCE_BYTES);
        encode_image_instance(
            &mut bytes,
            &rect,
            &UvRect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
            &Color::rgb(1.0, 1.0, 1.0),
            0.0,
            Point { x: 0.5, y: 0.5 },
            surface_size.width as f32 / surface_size.height.max(1) as f32,
        );
        self.queue.write_buffer(&self.upscale_buffer, 0, &bytes);
        self.upscale_rect = Some(rect);
    }

    pub(super) fn poll_screenshot_work(&mut self) {
        if !self.pending_screenshot_readbacks.is_empty()
            && let Err(error) = self.device.poll(wgpu::PollType::Poll)
        {
            tracing::warn!(%error, "failed to poll screenshot readback work");
        }
        self.drain_ready_screenshot_readbacks();
        self.join_finished_screenshot_save_jobs();
    }

    pub(super) fn enqueue_screenshot_readback(
        &mut self,
        request: ScreenshotRequest,
        capture: ScreenshotCapture,
    ) {
        let rx = capture.start_readback();
        tracing::debug!(
            path = %request.path.display(),
            width = capture.width,
            height = capture.height,
            "screenshot readback queued"
        );
        self.pending_screenshot_readbacks.push(ScreenshotReadback { request, capture, rx });
    }

    pub(super) fn drain_ready_screenshot_readbacks(&mut self) {
        let mut index = 0;
        while index < self.pending_screenshot_readbacks.len() {
            match self.pending_screenshot_readbacks[index].rx.try_recv() {
                Ok(result) => {
                    let readback = self.pending_screenshot_readbacks.swap_remove(index);
                    self.finish_screenshot_readback(readback, result);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    index += 1;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let readback = self.pending_screenshot_readbacks.swap_remove(index);
                    tracing::warn!(
                        path = %readback.request.path.display(),
                        "screenshot readback callback dropped"
                    );
                }
            }
        }
    }

    pub(super) fn finish_screenshot_readback(
        &mut self,
        readback: ScreenshotReadback,
        result: Result<(), wgpu::BufferAsyncError>,
    ) {
        if let Err(error) = result {
            tracing::warn!(
                %error,
                path = %readback.request.path.display(),
                "failed to map screenshot buffer"
            );
            return;
        }

        let width = readback.capture.width;
        let height = readback.capture.height;
        let rgba = readback.capture.mapped_rgba();
        tracing::debug!(
            path = %readback.request.path.display(),
            width,
            height,
            "screenshot readback completed"
        );
        match spawn_screenshot_save_job(readback.request, width, height, rgba) {
            Ok(job) => self.screenshot_save_jobs.push(job),
            Err(error) => {
                tracing::warn!(%error, "failed to start screenshot save job");
            }
        }
    }

    pub(super) fn join_finished_screenshot_save_jobs(&mut self) {
        let mut index = 0;
        while index < self.screenshot_save_jobs.len() {
            if self.screenshot_save_jobs[index].handle.is_finished() {
                let job = self.screenshot_save_jobs.swap_remove(index);
                finish_screenshot_save_job(job);
            } else {
                index += 1;
            }
        }
    }

    pub(super) fn flush_pending_screenshots(&mut self) -> Result<()> {
        while !self.pending_screenshot_readbacks.is_empty() {
            self.device.poll(wgpu::PollType::wait_indefinitely())?;
            self.drain_ready_screenshot_readbacks();
        }
        while let Some(job) = self.screenshot_save_jobs.pop() {
            finish_screenshot_save_job(job);
        }
        Ok(())
    }

    pub(super) fn wait_idle_before_drop(&self) {
        if let Err(error) = self.device.poll(wgpu::PollType::wait_indefinitely()) {
            tracing::warn!(%error, "failed to wait for renderer device before surface drop");
        }
    }

    pub(super) fn render_plan(
        &mut self,
        plan: &DrawPlan,
        canvas_policy: CanvasRenderPolicy,
        internal_resolution_mode: InternalResolutionMode,
        fonts: &HashMap<String, FontArc>,
        bitmap_fonts: &HashMap<String, BitmapFont>,
        egui: Option<&EguiFrame>,
        screenshot_request: Option<&ScreenshotRequest>,
    ) -> Result<(RenderSurfaceStatus, GpuRenderTimings)> {
        let draw_start = Instant::now();
        let mut timings = GpuRenderTimings::default();
        self.poll_screenshot_work();
        // egui のテクスチャ更新は、描画をスキップするフレームでも必ず適用する。
        // TexturesDelta は累積ストリームのため、取りこぼすと後続フレームの
        // 部分更新が未確保テクスチャを参照して panic する。
        if let Some(frame) = egui {
            self.egui.update_textures(&self.device, &self.queue, frame);
        }

        let surface_size = SurfaceSize { width: self.config.width, height: self.config.height };
        if !surface_size.is_drawable() {
            timings.draw_us = draw_start.elapsed().as_micros();
            return Ok((RenderSurfaceStatus::SkippedZeroSize, timings));
        }
        let output_viewport = CanvasViewport::from_policy(surface_size, canvas_policy);
        let internal_render_size =
            canvas_policy.internal_render_size(surface_size, internal_resolution_mode);
        let render_size = internal_render_size.unwrap_or(surface_size);
        let canvas_viewport = CanvasViewport::from_policy(render_size, canvas_policy);

        let text_start = Instant::now();
        let mut text_frame =
            self.build_text_frame(plan, fonts, bitmap_fonts, canvas_viewport.content_size());
        if !canvas_viewport.is_identity() {
            canvas_viewport.transform_text_instances(&mut text_frame.instances);
            canvas_viewport.transform_text_caret_rects(&mut text_frame.command_caret_rects);
        }
        timings.text_us = text_start.elapsed().as_micros();
        let geometry_start = Instant::now();
        let mut geometry = std::mem::take(&mut self.geometry_scratch);
        encode_plan_geometry_into(
            plan,
            &text_frame,
            render_size,
            canvas_viewport,
            &mut |rects, cache| self.offscreen_rect_batch_texture(rects, cache, render_size),
            &mut geometry,
        );
        let geometry_stats = geometry.stats();
        timings.steps = geometry_stats.steps;
        timings.rect_steps = geometry_stats.rect_steps;
        timings.image_steps = geometry_stats.image_steps;
        timings.text_steps = geometry_stats.text_steps;
        timings.rect_instances = geometry_stats.rect_instances;
        timings.image_instances = geometry_stats.image_instances;
        timings.text_instances = geometry_stats.text_instances;
        timings.geometry_us = geometry_start.elapsed().as_micros();
        let upload_start = Instant::now();
        self.ensure_rect_buffer(geometry.rects.len());
        if let Some(buffer) = &self.rect_buffer
            && !geometry.rects.is_empty()
        {
            self.queue.write_buffer(buffer, 0, &geometry.rects);
        }
        self.ensure_image_buffer(geometry.images.len());
        if let Some(buffer) = &self.image_buffer
            && !geometry.images.is_empty()
        {
            self.queue.write_buffer(buffer, 0, &geometry.images);
        }
        self.upload_text_frame(&text_frame);
        timings.upload_us = upload_start.elapsed().as_micros();

        let submit_start = Instant::now();
        let surface_start = Instant::now();
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(output)
            | wgpu::CurrentSurfaceTexture::Suboptimal(output) => output,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.configure_surface();
                timings.surface_us = surface_start.elapsed().as_micros();
                timings.submit_us = submit_start.elapsed().as_micros();
                timings.draw_us = draw_start.elapsed().as_micros();
                self.geometry_scratch = geometry;
                return Ok((RenderSurfaceStatus::Reconfigured, timings));
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                timings.surface_us = surface_start.elapsed().as_micros();
                timings.submit_us = submit_start.elapsed().as_micros();
                timings.draw_us = draw_start.elapsed().as_micros();
                self.geometry_scratch = geometry;
                return Ok((RenderSurfaceStatus::TimedOut, timings));
            }
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Validation => {
                timings.surface_us = surface_start.elapsed().as_micros();
                timings.submit_us = submit_start.elapsed().as_micros();
                timings.draw_us = draw_start.elapsed().as_micros();
                self.geometry_scratch = geometry;
                return Ok((RenderSurfaceStatus::TimedOut, timings));
            }
        };
        timings.surface_us = surface_start.elapsed().as_micros();
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        // image ステップごとの bind group を、レンダーパスが encoder を借りる前に作る。
        // steps 内の image ステップと同じ順序で並ぶ。
        let bind_start = Instant::now();
        let mut image_bind_groups = std::mem::take(&mut self.image_bind_group_scratch);
        image_bind_groups.clear();
        image_bind_groups.reserve(geometry_stats.image_steps);
        for step in &geometry.steps {
            if let DrawStep::Image { texture, linear, .. } = step {
                image_bind_groups.push(self.image_bind_group(*texture, *linear));
            }
        }
        let text_bind_group = self.text_bind_group();
        timings.bind_us = bind_start.elapsed().as_micros();
        let internal_target = internal_render_size.map(|size| {
            self.ensure_internal_scene_target(size);
            self.update_upscale_instance(output_viewport.rect, surface_size);
            let target =
                self.internal_scene_target.as_ref().expect("internal scene target was created");
            (target.view.clone(), target.bind_group.clone())
        });
        let encode_start = Instant::now();
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("bmz-render clear encoder"),
        });
        if let Some((internal_view, upscale_bind_group)) = &internal_target {
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bmz-render internal scene pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: internal_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(plan.clear.to_wgpu()),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                draw_plan_geometry(
                    &mut pass,
                    &geometry,
                    &self.rect_pipeline,
                    self.rect_buffer.as_ref(),
                    &self.image_pipeline,
                    &self.image_add_pipeline,
                    &self.image_premultiplied_pipeline,
                    &self.image_layer_pipeline,
                    &image_bind_groups,
                    self.image_buffer.as_ref(),
                    &self.text_pipeline,
                    text_bind_group.as_ref(),
                    self.text_buffer.as_ref(),
                );
            }
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bmz-render internal upscale pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(plan.clear.to_wgpu()),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.upscale_pipeline);
                pass.set_bind_group(0, upscale_bind_group, &[]);
                pass.set_vertex_buffer(0, self.upscale_buffer.slice(..));
                pass.draw(0..6, 0..1);
            }
        } else {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bmz-render scene surface pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(plan.clear.to_wgpu()),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            draw_plan_geometry(
                &mut pass,
                &geometry,
                &self.rect_pipeline,
                self.rect_buffer.as_ref(),
                &self.image_pipeline,
                &self.image_add_pipeline,
                &self.image_premultiplied_pipeline,
                &self.image_layer_pipeline,
                &image_bind_groups,
                self.image_buffer.as_ref(),
                &self.text_pipeline,
                text_bind_group.as_ref(),
                self.text_buffer.as_ref(),
            );
        }

        // ゲーム / スキン描画の上に egui を重ねる。staging 用 CommandBuffer は
        // egui パスを含む encoder より前に submit する必要がある。
        let egui_staging = match egui {
            Some(frame) => self.egui.paint(
                &self.device,
                &self.queue,
                &mut encoder,
                &view,
                frame,
                [self.config.width, self.config.height],
            ),
            None => Vec::new(),
        };

        let screenshot = screenshot_request.map(|request| {
            let capture = ScreenshotCapture::new(
                &self.device,
                self.config.width,
                self.config.height,
                self.config.format,
            );
            capture.copy_from_surface(&mut encoder, &output.texture);
            (request.clone(), capture)
        });
        let command_buffer = encoder.finish();
        timings.encode_us = encode_start.elapsed().as_micros();
        let queue_start = Instant::now();
        self.queue.submit(egui_staging.into_iter().chain(std::iter::once(command_buffer)));
        timings.queue_us = queue_start.elapsed().as_micros();
        if let Some((request, capture)) = screenshot {
            self.enqueue_screenshot_readback(request, capture);
        }
        if let Some(frame) = egui {
            self.egui.free_textures(frame);
        }
        self.image_bind_group_scratch = image_bind_groups;
        self.geometry_scratch = geometry;
        let present_start = Instant::now();
        output.present();
        timings.present_us = present_start.elapsed().as_micros();
        timings.submit_us = submit_start.elapsed().as_micros();
        timings.draw_us = draw_start.elapsed().as_micros();
        Ok((RenderSurfaceStatus::Rendered, timings))
    }

    pub(super) fn ensure_rect_buffer(&mut self, used_bytes: usize) {
        if used_bytes == 0 || used_bytes <= self.rect_buffer_capacity {
            return;
        }

        let capacity = used_bytes.next_power_of_two();
        self.rect_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bmz-render rect instance buffer"),
            size: capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.rect_buffer_capacity = capacity;
    }

    pub(super) fn ensure_image_buffer(&mut self, used_bytes: usize) {
        if used_bytes == 0 || used_bytes <= self.image_buffer_capacity {
            return;
        }

        let capacity = used_bytes.next_power_of_two();
        self.image_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bmz-render image instance buffer"),
            size: capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.image_buffer_capacity = capacity;
    }

    pub(super) fn offscreen_rect_batch_texture(
        &mut self,
        rects: &[RectCommand],
        cache: RectBatchCache,
        surface_size: SurfaceSize,
    ) -> Option<TextureId> {
        if rects.is_empty() || !surface_size.is_drawable() {
            return None;
        }
        let width = normalized_extent_to_pixels(cache.bounds.width, surface_size.width);
        let height = normalized_extent_to_pixels(cache.bounds.height, surface_size.height);
        if width == 0 || height == 0 {
            return None;
        }
        let key = OffscreenRectBatchTextureKey { key: cache.key, width, height };
        if let Some(texture) = self.offscreen_rect_batches.get(&key) {
            return Some(*texture);
        }
        if self.offscreen_rect_batches.len() >= OFFSCREEN_RECT_BATCH_TEXTURE_MAX_ENTRIES {
            self.clear_offscreen_rect_batches();
        }
        let texture_id = self.allocate_offscreen_rect_batch_texture_id();
        self.render_rect_batch_to_offscreen_texture(texture_id, rects, cache.bounds, width, height);
        self.offscreen_rect_batches.insert(key, texture_id);
        Some(texture_id)
    }

    pub(super) fn allocate_offscreen_rect_batch_texture_id(&mut self) -> TextureId {
        let texture_id = TextureId(self.next_offscreen_rect_batch_texture_id);
        self.next_offscreen_rect_batch_texture_id = self
            .next_offscreen_rect_batch_texture_id
            .checked_add(1)
            .unwrap_or(OFFSCREEN_RECT_BATCH_TEXTURE_BASE);
        texture_id
    }

    pub(super) fn render_rect_batch_to_offscreen_texture(
        &mut self,
        texture_id: TextureId,
        rects: &[RectCommand],
        bounds: Rect,
        width: u32,
        height: u32,
    ) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bmz-render offscreen rect batch"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let rect_bytes = encode_local_rect_batch(rects, bounds);
        if !rect_bytes.is_empty() {
            let rect_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("bmz-render offscreen rect batch buffer"),
                size: rect_bytes.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue.write_buffer(&rect_buffer, 0, &rect_bytes);
            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bmz-render offscreen rect batch encoder"),
            });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("bmz-render offscreen rect batch pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_vertex_buffer(0, rect_buffer.slice(..));
                pass.draw(0..6, 0..(rect_bytes.len() / RECT_INSTANCE_BYTES) as u32);
            }
            self.queue.submit(std::iter::once(encoder.finish()));
        }
        self.image_textures.insert(texture_id, PreparedTexture { texture, view, width, height });
        self.image_bind_group_cache
            .retain(|(cached_texture_id, _), _| *cached_texture_id != texture_id);
    }

    pub(super) fn clear_offscreen_rect_batches(&mut self) {
        for texture_id in self.offscreen_rect_batches.drain().map(|(_, texture_id)| texture_id) {
            self.image_textures.remove(&texture_id);
            self.image_bind_group_cache
                .retain(|(cached_texture_id, _), _| *cached_texture_id != texture_id);
        }
        self.next_offscreen_rect_batch_texture_id = OFFSCREEN_RECT_BATCH_TEXTURE_BASE;
    }

    pub(super) fn configure_surface(&self) {
        self.surface.configure(&self.device, &self.config);
    }

    pub(super) fn set_present_mode(&mut self, present_mode: WgpuPresentMode) {
        configure_surface_settings(&mut self.config, present_mode, &self.present_modes);
        self.configure_surface();
        tracing::info!(
            requested = ?present_mode,
            effective = ?self.config.present_mode,
            available = ?self.present_modes,
            maximum_frame_latency = self.config.desired_maximum_frame_latency,
            "configured renderer present mode"
        );
    }

    pub(super) fn set_default_font_coverage(&mut self, coverage: bmz_font::FontCoverage) {
        self.default_font_coverage = coverage;
        self.default_fonts = load_default_font_fallbacks(coverage, &self.default_font_search_paths);
        self.reset_text_atlas();
    }

    pub(super) fn set_default_font_search_paths(&mut self, paths: Vec<PathBuf>) {
        if self.default_font_search_paths == paths {
            return;
        }
        self.default_font_search_paths = paths;
        self.default_fonts = load_default_font_fallbacks(
            self.default_font_coverage,
            &self.default_font_search_paths,
        );
        self.reset_text_atlas();
    }

    pub(super) fn build_text_frame(
        &mut self,
        plan: &DrawPlan,
        fonts: &HashMap<String, FontArc>,
        bitmap_fonts: &HashMap<String, BitmapFont>,
        surface: SurfaceSize,
    ) -> TextFrame {
        if !surface.is_drawable() {
            return TextFrame::default();
        }
        build_text_frame_with_fallback_cache(
            plan,
            &self.default_fonts,
            fonts,
            bitmap_fonts,
            surface,
            &mut self.text_atlas,
        )
    }

    pub(super) fn upload_text_frame(&mut self, frame: &TextFrame) {
        self.ensure_text_buffer(frame.instances.len());
        if let Some(buffer) = &self.text_buffer
            && !frame.instances.is_empty()
        {
            self.queue.write_buffer(buffer, 0, &frame.instances);
        }

        if frame.size.width == 0 || frame.size.height == 0 {
            return;
        }

        let recreate_texture = self.text_texture_size != frame.size || self.text_texture.is_none();
        self.ensure_text_texture(frame.size);
        let texture = self.text_texture.as_ref().expect("text texture exists after ensure");
        if recreate_texture {
            let pixels = self.text_atlas.pixels_for_size(frame.size);
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(frame.size.width * 4),
                    rows_per_image: Some(frame.size.height),
                },
                wgpu::Extent3d {
                    width: frame.size.width,
                    height: frame.size.height,
                    depth_or_array_layers: 1,
                },
            );
            return;
        }

        for region in &frame.dirty_regions {
            if region.pixels.is_empty() || region.size.width == 0 || region.size.height == 0 {
                continue;
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: region.origin.0, y: region.origin.1, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &region.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(region.size.width * 4),
                    rows_per_image: Some(region.size.height),
                },
                wgpu::Extent3d {
                    width: region.size.width,
                    height: region.size.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    pub(super) fn reset_text_atlas(&mut self) {
        self.text_atlas = TextAtlasCache::new(TEXT_ATLAS_WIDTH);
        self.text_texture = None;
        self.text_texture_view = None;
        self.text_bind_group = None;
        self.text_texture_size = AtlasSize::default();
    }

    pub(super) fn ensure_text_buffer(&mut self, used_bytes: usize) {
        if used_bytes == 0 || used_bytes <= self.text_buffer_capacity {
            return;
        }

        let capacity = used_bytes.next_power_of_two();
        self.text_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bmz-render text instance buffer"),
            size: capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.text_buffer_capacity = capacity;
    }

    pub(super) fn ensure_text_texture(&mut self, size: AtlasSize) {
        if self.text_texture_size == size && self.text_texture.is_some() {
            return;
        }

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bmz-render text atlas"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.text_texture = Some(texture);
        self.text_texture_view = Some(view);
        self.text_bind_group = None;
        self.text_texture_size = size;
    }

    pub(super) fn text_bind_group(&mut self) -> Option<wgpu::BindGroup> {
        if let Some(bind_group) = &self.text_bind_group {
            return Some(bind_group.clone());
        }
        let view = self.text_texture_view.as_ref()?;
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bmz-render text bind group"),
            layout: &self.text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.text_sampler),
                },
            ],
        });
        self.text_bind_group = Some(bind_group.clone());
        Some(bind_group)
    }

    pub(super) fn upsert_rgba_texture(
        &mut self,
        id: TextureId,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) {
        if let Some(texture) = self.image_textures.get(&id)
            && texture.width == width
            && texture.height == height
        {
            write_rgba_texture(&self.queue, &texture.texture, width, height, rgba);
            return;
        }
        let texture = create_rgba_texture(&self.device, &self.queue, id, width, height, rgba);
        self.image_textures.insert(id, texture);
        self.image_bind_group_cache.retain(|(texture_id, _), _| *texture_id != id);
    }

    pub(super) fn image_bind_group(
        &mut self,
        texture_id: TextureId,
        linear: bool,
    ) -> wgpu::BindGroup {
        let resolved_texture_id =
            if self.image_textures.contains_key(&texture_id) { texture_id } else { TextureId(0) };
        if let Some(bind_group) =
            self.image_bind_group_cache.get(&(resolved_texture_id, linear)).cloned()
        {
            return bind_group;
        }
        let texture =
            self.image_textures.get(&resolved_texture_id).expect("fallback texture is registered");
        let _keep_texture_alive = &texture.texture;
        let sampler = if linear { &self.image_sampler_linear } else { &self.image_sampler };
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bmz-render image bind group"),
            layout: &self.image_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        self.image_bind_group_cache.insert((resolved_texture_id, linear), bind_group.clone());
        bind_group
    }
}
