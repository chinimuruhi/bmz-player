use super::*;

fn test_ir_provider(provider: &str, base_url: &str) -> IrProviderConfig {
    IrProviderConfig {
        provider: provider.to_string(),
        provider_key: String::new(),
        base_url: base_url.to_string(),
        enabled: false,
        account_display_name: String::new(),
        account_id: String::new(),
        send_policy: IrSendPolicyConfig::default(),
        role: IrProviderRoleConfig::default(),
        last_login_at: None,
        last_success_at: None,
    }
}

use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_test_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{name}-{nanos}-{counter}"))
}

fn test_offset_def(name: &str, id: i32) -> SkinOffsetDef {
    SkinOffsetDef {
        category: "test".to_string(),
        name: name.to_string(),
        id,
        x: true,
        y: true,
        w: true,
        h: true,
        r: true,
        a: true,
    }
}

#[path = "tests/cases_01.rs"]
mod cases_01;
#[path = "tests/cases_02.rs"]
mod cases_02;
#[path = "tests/cases_03.rs"]
mod cases_03;
#[path = "tests/cases_04.rs"]
mod cases_04;
