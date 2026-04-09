// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::path::{Path, PathBuf};

pub use criterion::*;

#[global_allocator]
static GLOBAL: NeverGrowInPlaceAllocator = NeverGrowInPlaceAllocator;

pub struct PerfFixture {
    pub bucket: String,
    pub name: String,
    pub path: PathBuf,
    pub source_text: String,
}

pub fn load_perf_fixtures() -> Vec<PerfFixture> {
    let root = repo_root().join("fixtures/perf");
    let mut fixtures = Vec::new();

    for bucket in [
        "common",
        "description-heavy",
        "type-heavy",
        "special-tag",
        "malformed",
        "toolchain",
    ] {
        let bucket_dir = root.join(bucket);
        fixtures.extend(load_bucket_fixtures(&bucket_dir, bucket));
    }

    fixtures.sort_by(|left, right| {
        left.bucket
            .cmp(&right.bucket)
            .then_with(|| left.name.cmp(&right.name))
    });
    fixtures
}

fn load_bucket_fixtures(bucket_dir: &Path, bucket: &str) -> Vec<PerfFixture> {
    let Ok(entries) = fs::read_dir(bucket_dir) else {
        return Vec::new();
    };

    let mut fixtures = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsdoc") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Ok(source_text) = fs::read_to_string(&path) else {
            continue;
        };
        fixtures.push(PerfFixture {
            bucket: bucket.to_string(),
            name: name.to_string(),
            path,
            source_text,
        });
    }

    fixtures
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("benchmark crate should live under tasks/benchmark")
        .to_path_buf()
}

struct NeverGrowInPlaceAllocator;

// SAFETY: methods delegate to `System`.
unsafe impl GlobalAlloc for NeverGrowInPlaceAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }
}
