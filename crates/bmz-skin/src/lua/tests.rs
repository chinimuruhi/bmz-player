use super::*;

fn unique_skin_test_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bmz-lua-{tag}-{nanos}-{n}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn test_skin_path_context(root: &Path) -> SkinPathContext {
    let entry = root.join("test.luaskin");
    fs::write(&entry, "return {}").unwrap();
    SkinPathContext::new(&entry, [root.to_path_buf()]).unwrap()
}

#[path = "tests/cases_01.rs"]
mod cases_01;
#[path = "tests/cases_02.rs"]
mod cases_02;
#[path = "tests/cases_03.rs"]
mod cases_03;
