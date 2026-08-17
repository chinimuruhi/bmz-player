use anyhow::{Context, Result};
use bmz_core::input::InputDeviceKind;

use crate::cli::ReplayCommand;
use crate::config::app_config::AppConfig;
use crate::config::load::load_app_config;
use crate::paths::{resolve_app_paths, resolve_profile_paths};
use crate::storage::library_db::LibraryDatabase;
use crate::storage::migration::{migrate_library_db, migrate_score_db};
use crate::storage::replay_import::write_replay_import_details;
use crate::storage::replay_import::{ImportBeatorajaReplaysRequest, import_beatoraja_replays};
use crate::storage::score_db::ScoreDatabase;

pub fn run_replay_command(command: ReplayCommand) -> Result<()> {
    match command {
        ReplayCommand::Import { path, overwrite, controller } => {
            import_replays(&path, overwrite, controller)
        }
    }
}

fn import_replays(path: &str, overwrite: bool, controller: bool) -> Result<()> {
    let app_paths = resolve_app_paths()?;
    app_paths.ensure_dirs()?;
    let app_config = if app_paths.config_toml.exists() {
        load_app_config(&app_paths.config_toml)
            .with_context(|| format!("failed to load {}", app_paths.config_toml.display()))?
    } else {
        AppConfig::default()
    };
    let profile_paths = resolve_profile_paths(&app_paths, &app_config.active_profile)?;
    profile_paths.ensure_dirs()?;
    migrate_library_db(&app_paths.library_db)?;
    migrate_score_db(&profile_paths.score_db)?;
    let library_db = LibraryDatabase::open(&app_paths.library_db)?;
    let mut score_db = ScoreDatabase::open(&profile_paths.score_db)?;
    let mut request = ImportBeatorajaReplaysRequest::new(path);
    request.overwrite_protected_slots = overwrite;
    request.device_kind =
        if controller { InputDeviceKind::Controller } else { InputDeviceKind::Keyboard };
    let mut report =
        import_beatoraja_replays(&library_db, &mut score_db, &profile_paths, &request)?;
    if !report.issues.is_empty() {
        let details = write_replay_import_details(&app_paths.logs_dir, &request.source, &report)?;
        report.details_path = Some(details);
    }

    println!("{}", report.summary());
    if let Some(warning) = &report.threshold_warning {
        println!("warning: {warning}");
    }
    for issue in &report.issues {
        println!("{}: {}", issue.path.display(), issue.message);
    }
    if let Some(path) = &report.details_path {
        println!("details: {}", path.display());
    }
    Ok(())
}
