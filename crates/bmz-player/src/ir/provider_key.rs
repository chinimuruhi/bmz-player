use crate::config::profile_config::{IrConfig, IrProviderConfig};

pub fn configured_provider_key(entry: &IrProviderConfig) -> Option<&str> {
    let key = entry.provider_key.trim();
    if key.is_empty() { None } else { Some(key) }
}

pub fn provider_config_for_key<'a>(
    ir_config: &'a IrConfig,
    key: &str,
) -> Option<&'a IrProviderConfig> {
    ir_config.providers.iter().find(|entry| {
        entry.enabled
            && !entry.base_url.is_empty()
            && configured_provider_key(entry).is_some_and(|provider_key| provider_key == key)
    })
}

pub fn primary_provider_config(ir_config: &IrConfig) -> Option<&IrProviderConfig> {
    let usable = |entry: &IrProviderConfig| {
        entry.enabled && !entry.base_url.is_empty() && configured_provider_key(entry).is_some()
    };
    ir_config
        .providers
        .iter()
        .filter(|entry| usable(entry))
        .find(|entry| {
            configured_provider_key(entry).is_some_and(|key| key == ir_config.primary_provider)
        })
        .or_else(|| ir_config.providers.iter().find(|entry| usable(entry)))
}
