use super::*;

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
    mode: &WindowMode,
    monitor: Option<MonitorHandle>,
) -> Option<Fullscreen> {
    match mode {
        WindowMode::Windowed => None,
        WindowMode::BorderlessFullscreen => Some(Fullscreen::Borderless(monitor)),
        WindowMode::ExclusiveFullscreen => {
            let monitor = monitor?;
            match pick_exclusive_video_mode(&monitor) {
                Some(video_mode) => Some(Fullscreen::Exclusive(video_mode)),
                None => {
                    tracing::warn!("no exclusive video mode available; using borderless");
                    Some(Fullscreen::Borderless(Some(monitor)))
                }
            }
        }
    }
}

/// 排他フルスクリーン用に、解像度とリフレッシュレートが最大の video mode を選ぶ。
pub(super) fn pick_exclusive_video_mode(monitor: &MonitorHandle) -> Option<VideoModeHandle> {
    monitor.video_modes().max_by_key(|mode| {
        let size = mode.size();
        (u64::from(size.width) * u64::from(size.height), mode.refresh_rate_millihertz())
    })
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
