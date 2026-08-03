use std::path::Path;

use anyhow::Result;

use super::app_config::{
    AppConfig, ensure_default_difficulty_table_sources, normalize_song_root_paths,
};
use super::play_input::{normalize_profile_input, validate_play_inherit_config};
use super::profile_config::ProfileConfig;

pub fn load_app_config(path: &Path) -> Result<AppConfig> {
    let text = std::fs::read_to_string(path)?;
    parse_app_config(&text)
}

fn parse_app_config(text: &str) -> Result<AppConfig> {
    let mut config: AppConfig = toml::from_str(text)?;
    normalize_song_root_paths(&mut config.songs.roots);
    ensure_default_difficulty_table_sources(&mut config);
    Ok(config)
}

pub fn load_profile_config(path: &Path) -> Result<ProfileConfig> {
    let text = std::fs::read_to_string(path)?;
    parse_profile_config(&text)
}

fn parse_profile_config(text: &str) -> Result<ProfileConfig> {
    let mut config: ProfileConfig = toml::from_str(text)?;
    config.skin.migrate_legacy_offsets();
    config.ir.normalize_builtin_providers();
    normalize_profile_input(&mut config.input);
    validate_play_inherit_config(&config.input).map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::app_config::PathEntry;

    #[test]
    fn parse_app_config_normalizes_and_deduplicates_song_roots() {
        let mut config = AppConfig::default();
        config.songs.roots = vec![
            PathEntry { path: r"\\?\G:\BMS".to_string(), enabled: false, recursive: false },
            PathEntry { path: "G:/BMS".to_string(), enabled: true, recursive: true },
        ];
        let text = toml::to_string(&config).unwrap();

        let loaded = parse_app_config(&text).unwrap();

        assert_eq!(loaded.songs.roots.len(), 1);
        assert_eq!(loaded.songs.roots[0].path, "G:/BMS");
        assert!(!loaded.songs.roots[0].enabled);
        assert!(!loaded.songs.roots[0].recursive);
    }

    #[test]
    fn parse_profile_config_restores_builtin_ir_provider_slots() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.ir.providers.clear();
        let text = toml::to_string(&profile).unwrap();

        let loaded = parse_profile_config(&text).unwrap();

        assert_eq!(loaded.ir.providers.len(), 2);
        assert_eq!(
            loaded.ir.providers[0],
            crate::config::profile_config::IrProviderConfig::bmz_ir()
        );
        assert_eq!(
            loaded.ir.providers[1],
            crate::config::profile_config::IrProviderConfig::rian_ir()
        );
    }
}
