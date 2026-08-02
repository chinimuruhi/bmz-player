use super::*;
use std::cmp::Reverse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WindowFocusUpdate {
    pub(super) effective_focused: bool,
    pub(super) focus_lost: bool,
}

/// winit の focus event と処理時点の native focus state から、アプリが使う状態を決める。
///
/// macOS では NSWindowDelegate の通知が event queue に積まれた後、処理されるまでに
/// key window が戻ることがある。`Window::has_focus()` は処理時点の
/// `NSWindow.isKeyWindow` を読むため、macOS だけはこちらを正とする。
/// 他 platform は従来どおり event 値を採用する。
pub(super) fn resolve_window_focus_update(
    previous_effective_focused: bool,
    event_focused: bool,
    native_focused: bool,
    is_macos: bool,
) -> WindowFocusUpdate {
    let effective_focused = if is_macos { native_focused } else { event_focused };
    WindowFocusUpdate {
        effective_focused,
        focus_lost: previous_effective_focused && !effective_focused,
    }
}

pub(super) fn window_attributes_from_config(
    video: &crate::config::app_config::VideoConfig,
) -> WindowAttributes {
    WindowAttributes::default()
        .with_title("bmz-player")
        .with_window_icon(app_window_icon())
        .with_inner_size(PhysicalSize::new(video.width.max(1), video.height.max(1)))
}

pub(super) fn app_window_icon() -> Option<Icon> {
    let image = image::load_from_memory(app_window_icon_png()).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).ok()
}

#[cfg(target_os = "windows")]
pub(super) fn app_window_icon_png() -> &'static [u8] {
    include_bytes!("../../../../assets/app-icon/bmz-player-window-windows.png")
}

#[cfg(not(target_os = "windows"))]
pub(super) fn app_window_icon_png() -> &'static [u8] {
    include_bytes!("../../../../assets/app-icon/bmz-player-window.png")
}

/// 設定のウィンドウモードに対応する winit の `Fullscreen` を返す。
///
/// 排他フルスクリーンはモニタの video mode が必要で、取得できない場合は
/// ボーダレスへフォールバックする。
pub(super) fn fullscreen_from_config(
    video: &crate::config::app_config::VideoConfig,
    monitor: Option<MonitorHandle>,
) -> Option<Fullscreen> {
    match &video.mode {
        WindowMode::Windowed => None,
        WindowMode::BorderlessFullscreen => Some(Fullscreen::Borderless(monitor)),
        WindowMode::ExclusiveFullscreen => {
            let monitor = monitor?;
            match pick_exclusive_video_mode(
                &monitor,
                PhysicalSize::new(video.width.max(1), video.height.max(1)),
                video.target_fps,
            ) {
                Some(video_mode) => Some(Fullscreen::Exclusive(video_mode)),
                None => {
                    tracing::warn!("no exclusive video mode available; using borderless");
                    Some(Fullscreen::Borderless(Some(monitor)))
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VideoModeSpec {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) refresh_millihertz: u32,
    pub(super) bit_depth: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VideoModeResolutionReason {
    Configured,
    ClosestSupported,
    LegacyLargest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VideoModeRefreshReason {
    ClosestAtOrAbove,
    HighestBelow,
    HighestUnlimited,
    LegacyHighestAtLargestResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VideoModeSelection {
    pub(super) index: usize,
    pub(super) resolution_reason: VideoModeResolutionReason,
    pub(super) refresh_reason: VideoModeRefreshReason,
}

/// 設定解像度を優先し、その解像度内で target FPS に適した video mode を選ぶ。
///
/// 完全一致が無い場合だけ、width/height 差、面積差の順で最も近い対応解像度へ
/// フォールバックする。同一解像度では target 以上の最も近い refresh rate、
/// target 以上が無ければ最高 refresh rate、target=0 なら最高 refresh rate を使う。
pub(super) fn select_exclusive_video_mode(
    candidates: &[VideoModeSpec],
    requested_size: PhysicalSize<u32>,
    target_fps: u32,
) -> Option<VideoModeSelection> {
    let resolution_reason = if candidates
        .iter()
        .any(|mode| mode.width == requested_size.width && mode.height == requested_size.height)
    {
        VideoModeResolutionReason::Configured
    } else {
        VideoModeResolutionReason::ClosestSupported
    };
    let selected_resolution = candidates
        .iter()
        .filter(|mode| {
            resolution_reason == VideoModeResolutionReason::ClosestSupported
                || (mode.width == requested_size.width && mode.height == requested_size.height)
        })
        .min_by_key(|mode| resolution_fallback_key(**mode, requested_size))?;

    let at_resolution = candidates.iter().enumerate().filter(|(_, mode)| {
        mode.width == selected_resolution.width && mode.height == selected_resolution.height
    });
    let target_millihertz = u64::from(target_fps) * 1_000;
    let (index, refresh_reason) = if target_fps == 0 {
        let (index, _) =
            at_resolution.max_by_key(|(_, mode)| (mode.refresh_millihertz, mode.bit_depth))?;
        (index, VideoModeRefreshReason::HighestUnlimited)
    } else if let Some((index, _)) = at_resolution
        .clone()
        .filter(|(_, mode)| u64::from(mode.refresh_millihertz) >= target_millihertz)
        .min_by_key(|(_, mode)| {
            (u64::from(mode.refresh_millihertz) - target_millihertz, Reverse(mode.bit_depth))
        })
    {
        (index, VideoModeRefreshReason::ClosestAtOrAbove)
    } else {
        let (index, _) =
            at_resolution.max_by_key(|(_, mode)| (mode.refresh_millihertz, mode.bit_depth))?;
        (index, VideoModeRefreshReason::HighestBelow)
    };
    Some(VideoModeSelection { index, resolution_reason, refresh_reason })
}

/// macOS では設定解像度と target FPS を考慮する。他 platform は、このmacOS向け
/// 修正で既存挙動を変えないよう、従来の「最大面積、その中で最高refresh rate」を保つ。
pub(super) fn select_platform_exclusive_video_mode(
    candidates: &[VideoModeSpec],
    requested_size: PhysicalSize<u32>,
    target_fps: u32,
    is_macos: bool,
) -> Option<VideoModeSelection> {
    if is_macos {
        return select_exclusive_video_mode(candidates, requested_size, target_fps);
    }

    let (index, _) = candidates.iter().enumerate().max_by_key(|(_, mode)| {
        (u64::from(mode.width) * u64::from(mode.height), mode.refresh_millihertz)
    })?;
    Some(VideoModeSelection {
        index,
        resolution_reason: VideoModeResolutionReason::LegacyLargest,
        refresh_reason: VideoModeRefreshReason::LegacyHighestAtLargestResolution,
    })
}

fn resolution_fallback_key(
    mode: VideoModeSpec,
    requested_size: PhysicalSize<u32>,
) -> (u64, u64, Reverse<u64>) {
    let dimension_distance = u64::from(mode.width.abs_diff(requested_size.width))
        + u64::from(mode.height.abs_diff(requested_size.height));
    let mode_area = u64::from(mode.width) * u64::from(mode.height);
    let requested_area = u64::from(requested_size.width) * u64::from(requested_size.height);
    (dimension_distance, mode_area.abs_diff(requested_area), Reverse(mode_area))
}

pub(super) fn pick_exclusive_video_mode(
    monitor: &MonitorHandle,
    requested_size: PhysicalSize<u32>,
    target_fps: u32,
) -> Option<VideoModeHandle> {
    let modes = monitor.video_modes().collect::<Vec<_>>();
    let specs = modes
        .iter()
        .map(|mode| {
            let size = mode.size();
            VideoModeSpec {
                width: size.width,
                height: size.height,
                refresh_millihertz: mode.refresh_rate_millihertz(),
                bit_depth: mode.bit_depth(),
            }
        })
        .collect::<Vec<_>>();
    let selection = select_platform_exclusive_video_mode(
        &specs,
        requested_size,
        target_fps,
        cfg!(target_os = "macos"),
    )?;
    let selected = specs[selection.index];
    let monitor_size = monitor.size();
    let candidate_modes = specs
        .iter()
        .map(|mode| {
            format!(
                "{}x{}@{:.3}Hz/{}bpp",
                mode.width,
                mode.height,
                f64::from(mode.refresh_millihertz) / 1_000.0,
                mode.bit_depth
            )
        })
        .collect::<Vec<_>>();
    let monitor_name = monitor.name().unwrap_or_else(|| "unknown".to_string());
    tracing::info!(
        monitor = %monitor_name,
        monitor_width = monitor_size.width,
        monitor_height = monitor_size.height,
        requested_width = requested_size.width,
        requested_height = requested_size.height,
        target_fps,
        selected_width = selected.width,
        selected_height = selected.height,
        selected_refresh_hz = f64::from(selected.refresh_millihertz) / 1_000.0,
        selected_bit_depth = selected.bit_depth,
        resolution_reason = ?selection.resolution_reason,
        refresh_reason = ?selection.refresh_reason,
        candidate_modes = ?candidate_modes,
        "selected exclusive fullscreen video mode"
    );
    modes.into_iter().nth(selection.index)
}

pub(super) fn format_error_chain(error: &anyhow::Error) -> String {
    error.chain().map(ToString::to_string).collect::<Vec<_>>().join(": ")
}

pub(super) fn open_external_url(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .context("failed to open URL with cmd /C start")?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn().context("failed to open URL with open")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(url).spawn().context("failed to open URL with xdg-open")?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        anyhow::bail!("opening URLs is not supported on this platform: {url}");
    }
    Ok(())
}

pub(super) fn open_file_browser_path(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let target = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        };
        Command::new("explorer")
            .arg(target)
            .spawn()
            .context("failed to open path with explorer")?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn().context("failed to open path with open")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        };
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .context("failed to open path with xdg-open")?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        anyhow::bail!("opening paths is not supported on this platform: {}", path.display());
    }
    Ok(())
}

pub(super) fn open_file_with_default_app(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn()
            .context("failed to open file with cmd /C start")?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn().context("failed to open file with open")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn().context("failed to open file with xdg-open")?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        anyhow::bail!("opening files is not supported on this platform: {}", path.display());
    }
    Ok(())
}

pub(super) fn primary_ir_provider_for_profile(
    profile: &ProfileConfig,
) -> Option<&crate::config::profile_config::IrProviderConfig> {
    let key = if profile.ir.primary_provider.trim().is_empty() {
        profile
            .ir
            .providers
            .iter()
            .find(|provider| {
                provider.enabled
                    && !provider.base_url.trim().is_empty()
                    && crate::ir::provider_key::configured_provider_key(provider).is_some()
            })
            .and_then(crate::ir::provider_key::configured_provider_key)
    } else {
        Some(profile.ir.primary_provider.trim())
    }?;
    crate::ir::provider_key::provider_config_for_key(&profile.ir, key)
}

pub(super) fn launch_update_installer(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new(path)
            .arg("/SP-")
            .spawn()
            .with_context(|| format!("failed to launch update installer: {}", path.display()))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!(
            "automatic installer launch is only supported on Windows: {}",
            path.display()
        );
    }
}
