use crate::scene::{AppSceneSnapshot, SelectSnapshot};

use super::*;

fn test_surface_size() -> SurfaceSize {
    SurfaceSize { width: 16, height: 9 }
}

#[test]
fn screenshot_png_is_encoded_once_for_file_and_clipboard_use() {
    let png = encode_screenshot_png(1, 1, &[0x12, 0x34, 0x56, 0x78]).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let decoded =
        image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap().into_rgba8();
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
    assert_eq!(fallback_wgpu_backends(WgpuBackend::Auto), &[WgpuBackend::Vulkan, WgpuBackend::Gl]);
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
    assert_eq!(resolve_wgpu_present_mode(WgpuPresentMode::Immediate, &[Mailbox, Fifo]), Mailbox);
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

    configure_surface_settings(&mut config, WgpuPresentMode::Mailbox, &[wgpu::PresentMode::Fifo]);

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
    let mapped = [1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 9, 10, 11, 12, 13, 14, 15, 16, 0, 0, 0, 0];

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
    let context = SkinContext::from_manifest_and_document(manifest, document.clone(), Vec::new());
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
        CanvasRenderPolicy::default().internal_render_size(four_k, InternalResolutionMode::Skin),
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
        CanvasViewport::from_policy(SurfaceSize { width: 320, height: 180 }, policy).is_identity()
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/fonts/noto-cjk");
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
        atlas.glyphs.keys().any(|key| { key.ch == fallback_char && key.font_id == selected_id })
    );
    assert!(
        !atlas.glyphs.keys().any(|key| { key.ch == fallback_char && key.font_id != selected_id })
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

    assert_eq!(renderer.render_scene_status(scene).unwrap(), RenderSurfaceStatus::SkippedNoSurface);
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
    let rect_count =
        plan.commands.iter().filter(|command| matches!(command, DrawCommand::Rect { .. })).count();

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
    let asset = crate::assets::RgbaImageAsset { width: 1, height: 1, pixels: vec![255, 0, 0, 255] };

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
