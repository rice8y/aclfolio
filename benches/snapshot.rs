use aclfolio::{Anthology, LoadOptions};
use std::{hint::black_box, time::Instant};
fn main() {
    let path =
        std::env::var("ANTHOLOGY_DATA").expect("set ANTHOLOGY_DATA to an immutable data directory");
    let t = Instant::now();
    let a = Anthology::open(&path).unwrap();
    let full = t.elapsed();
    let ids: Vec<_> = a.papers().take(1000).map(|p| p.id().clone()).collect();
    let t = Instant::now();
    for _ in 0..100 {
        for id in &ids {
            black_box(a.paper_by_id(id));
        }
    }
    let lookup = t.elapsed();
    let t = Instant::now();
    for p in a.papers() {
        black_box(p.id());
    }
    let iteration = t.elapsed();
    let t = Instant::now();
    let b = a.reload(&path, &LoadOptions::default()).unwrap();
    let incremental = t.elapsed();
    let mut changed_measurement = serde_json::Value::Null;
    if let Ok(changed) = std::env::var("ANTHOLOGY_CHANGED_DATA") {
        let t = Instant::now();
        let updated = a.reload(&changed, &LoadOptions::default()).unwrap();
        let changed_reload = t.elapsed();
        let t = Instant::now();
        let fresh = Anthology::open(&changed).unwrap();
        let changed_full = t.elapsed();
        let diff = updated.diff(&fresh);
        assert!(diff.collections.modified.is_empty());
        changed_measurement = serde_json::json!({"changed_reload_ms":changed_reload.as_secs_f64()*1000., "changed_full_ms":changed_full.as_secs_f64()*1000.,"reuse":updated.reload_stats()});
    }
    println!(
        "{}",
        serde_json::json!({"papers":a.papers().len(),"full_load_ms":full.as_secs_f64()*1000.,"lookups":ids.len()*100,"lookup_ms":lookup.as_secs_f64()*1000.,"iteration_ms":iteration.as_secs_f64()*1000.,"unchanged_reload_ms":incremental.as_secs_f64()*1000.,"reuse":b.reload_stats(),"changed":changed_measurement})
    );
}
