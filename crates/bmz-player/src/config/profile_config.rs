use std::collections::BTreeMap;

use bmz_core::lane::KeyMode;
use bmz_gameplay::rule::RuleMode;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::i18n::AppLocale;
use crate::ln_policy::LnPolicySetting;
use crate::select_options::SessionMode;

#[path = "profile_config/bindings.rs"]
mod bindings;
#[path = "profile_config/input.rs"]
mod input;
#[path = "profile_config/play_mode.rs"]
mod play_mode;
#[path = "profile_config/preferences.rs"]
mod preferences;
#[path = "profile_config/schema.rs"]
mod schema;
#[path = "profile_config/skin_ir.rs"]
mod skin_ir;

pub use bindings::*;
pub use input::*;
pub use play_mode::*;
pub use preferences::*;
pub use schema::*;
pub use skin_ir::*;

impl ProfileConfig {
    pub fn migrate_legacy_key_mode_conversion(&mut self) {
        if self.play.seven_to_six && self.play.key_mode_conversion == KeyModeConversionConfig::Off {
            self.play.key_mode_conversion = KeyModeConversionConfig::SevenToSix;
        }
        self.play.seven_to_six = false;
    }
}

#[cfg(test)]
#[path = "profile_config/tests.rs"]
mod tests;
