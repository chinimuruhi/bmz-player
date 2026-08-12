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

pub(super) fn resolve_wgpu_present_mode(
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

pub(super) fn configure_surface_settings(
    config: &mut wgpu::SurfaceConfiguration,
    requested_present_mode: WgpuPresentMode,
    frame_latency_mode: WgpuFrameLatencyMode,
    available_present_modes: &[wgpu::PresentMode],
) {
    config.present_mode =
        resolve_wgpu_present_mode(requested_present_mode, available_present_modes);
    config.desired_maximum_frame_latency = resolve_maximum_frame_latency(
        frame_latency_mode,
        config.present_mode,
        cfg!(target_os = "macos"),
    );
    config.usage |= wgpu::TextureUsages::COPY_SRC;
}

pub(super) fn resolve_maximum_frame_latency(
    mode: WgpuFrameLatencyMode,
    effective_present_mode: wgpu::PresentMode,
    is_macos: bool,
) -> u32 {
    match mode {
        WgpuFrameLatencyMode::LowLatency => LOW_LATENCY_MAXIMUM_FRAME_LATENCY,
        WgpuFrameLatencyMode::Stable => STABLE_MAXIMUM_FRAME_LATENCY,
        WgpuFrameLatencyMode::Auto
            if is_macos && effective_present_mode == wgpu::PresentMode::Immediate =>
        {
            STABLE_MAXIMUM_FRAME_LATENCY
        }
        WgpuFrameLatencyMode::Auto => LOW_LATENCY_MAXIMUM_FRAME_LATENCY,
    }
}

pub(super) fn wgpu_present_mode_label(mode: wgpu::PresentMode) -> &'static str {
    match mode {
        wgpu::PresentMode::AutoVsync => "AutoVsync",
        wgpu::PresentMode::AutoNoVsync => "AutoNoVsync",
        wgpu::PresentMode::Fifo => "Fifo",
        wgpu::PresentMode::FifoRelaxed => "FifoRelaxed",
        wgpu::PresentMode::Immediate => "Immediate",
        wgpu::PresentMode::Mailbox => "Mailbox",
    }
}
use super::*;
