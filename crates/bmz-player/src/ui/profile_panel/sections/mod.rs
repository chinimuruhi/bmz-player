use super::*;

mod core;
mod display;
mod ir;
mod play;
mod ui;

pub(super) use core::*;
pub(super) use display::*;
pub(super) use ir::*;
pub(super) use play::*;
pub(super) use ui::*;

pub(super) struct ProfileSectionContext<'a> {
    pub(super) profile: &'a mut ProfileConfig,
    pub(super) app_config: &'a mut AppConfig,
    pub(super) show_fps: &'a mut bool,
    pub(super) ir_login: &'a mut IrLoginUiState,
    pub(super) ir_device_key: &'a mut IrDeviceKeyUiState,
    pub(super) profile_manager: &'a mut ProfileManagerUiState,
    pub(super) profile_root: &'a std::path::Path,
    pub(super) unrestricted: bool,
    pub(super) text: Localizer,
    pub(super) save_clicked: bool,
    pub(super) save_app_config: bool,
}
