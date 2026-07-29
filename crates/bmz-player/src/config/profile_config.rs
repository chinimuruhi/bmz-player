use std::collections::BTreeMap;

use bmz_gameplay::rule::RuleMode;
use bmz_render::scene::ResultGradeDiffDisplay;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::i18n::AppLocale;
use crate::ln_policy::LnPolicySetting;
use crate::select_options::SessionMode;

#[path = "profile_config/bindings.rs"]
mod bindings;
#[path = "profile_config/input.rs"]
mod input;
#[path = "profile_config/preferences.rs"]
mod preferences;
#[path = "profile_config/schema.rs"]
mod schema;
#[path = "profile_config/skin_ir.rs"]
mod skin_ir;

pub use bindings::*;
pub use input::*;
pub use preferences::*;
pub use schema::*;
pub use skin_ir::*;

#[cfg(test)]
#[path = "profile_config/tests.rs"]
mod tests;
