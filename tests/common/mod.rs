// Project: shoptet-scraper
// File: tests/common/mod.rs
// Description: Isolated test output directories inside the workspace.
// Author: Jan Alexandr Kopřiva
// Based on shoptet-scraper by Jan Alexandr Kopřiva
// License: MIT
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
pub struct Temp(pub PathBuf);
impl Default for Temp {
    fn default() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/.runtime")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
