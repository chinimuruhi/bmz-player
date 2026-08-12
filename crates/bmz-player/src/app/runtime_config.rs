use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyboardInputBackend {
    Window,
    RawInput,
}

pub(super) fn play_input_backend_for_context(
    active: Option<&SharedInputBackend>,
    pending_start: bool,
    preloaded: Option<&SharedInputBackend>,
    pending_preload: Option<&SharedInputBackend>,
) -> Option<SharedInputBackend> {
    if let Some(active) = active {
        return Some(active.clone());
    }
    if !pending_start {
        return None;
    }
    preloaded.or(pending_preload).cloned()
}

pub(super) fn keyboard_input_backend_for_config(
    config: &AppConfig,
) -> Option<KeyboardInputBackend> {
    if !config.input.keyboard_enabled {
        return None;
    }
    match config.input.backend {
        InputBackendKind::Auto if cfg!(target_os = "windows") => {
            Some(KeyboardInputBackend::RawInput)
        }
        InputBackendKind::RawInput if cfg!(target_os = "windows") => {
            Some(KeyboardInputBackend::RawInput)
        }
        _ => Some(KeyboardInputBackend::Window),
    }
}

pub(super) fn config_renderer_backend(
    backend: crate::config::app_config::RendererBackend,
) -> bmz_render::WgpuBackend {
    match backend {
        crate::config::app_config::RendererBackend::Auto => bmz_render::WgpuBackend::Auto,
        crate::config::app_config::RendererBackend::Vulkan => bmz_render::WgpuBackend::Vulkan,
        crate::config::app_config::RendererBackend::Metal => bmz_render::WgpuBackend::Metal,
        crate::config::app_config::RendererBackend::Dx12 => bmz_render::WgpuBackend::Dx12,
        crate::config::app_config::RendererBackend::Gl => bmz_render::WgpuBackend::Gl,
    }
}

pub(super) fn config_present_mode(
    video: &crate::config::app_config::VideoConfig,
) -> bmz_render::WgpuPresentMode {
    match video.vsync_mode {
        crate::config::app_config::VsyncModeConfig::Vsync => bmz_render::WgpuPresentMode::Fifo,
        crate::config::app_config::VsyncModeConfig::AdaptiveVsync => {
            bmz_render::WgpuPresentMode::FifoRelaxed
        }
        crate::config::app_config::VsyncModeConfig::VsyncOff => {
            bmz_render::WgpuPresentMode::Immediate
        }
        crate::config::app_config::VsyncModeConfig::FastVsync => {
            bmz_render::WgpuPresentMode::Mailbox
        }
    }
}

pub(super) fn config_frame_latency_mode(
    video: &crate::config::app_config::VideoConfig,
) -> bmz_render::WgpuFrameLatencyMode {
    match video.frame_latency_mode {
        crate::config::app_config::FrameLatencyModeConfig::Auto => {
            bmz_render::WgpuFrameLatencyMode::Auto
        }
        crate::config::app_config::FrameLatencyModeConfig::LowLatency => {
            bmz_render::WgpuFrameLatencyMode::LowLatency
        }
        crate::config::app_config::FrameLatencyModeConfig::Stable => {
            bmz_render::WgpuFrameLatencyMode::Stable
        }
    }
}

pub(super) fn config_internal_resolution_mode(
    video: &crate::config::app_config::VideoConfig,
) -> bmz_render::InternalResolutionMode {
    match video.internal_resolution {
        InternalResolutionModeConfig::Native => bmz_render::InternalResolutionMode::Native,
        InternalResolutionModeConfig::Skin => bmz_render::InternalResolutionMode::Skin,
    }
}
