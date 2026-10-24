//! Opt-in integration test against the exact pinned upstream dataset.
use aclfolio::{Anthology, LoadOptions};
#[test]
#[ignore = "requires ANTHOLOGY_DATA pointing to the pinned upstream data directory"]
fn pinned_inventory_and_full_reload_equivalence() {
    let path = std::env::var("ANTHOLOGY_DATA").expect("set ANTHOLOGY_DATA");
    let a = Anthology::open(&path).unwrap();
    assert_eq!(a.collections().len(), 1735);
    assert_eq!(a.volumes().len(), 3521);
    assert_eq!(a.papers().len(), 137663);
    assert_eq!(a.people().len(), 49843);
    assert_eq!(a.venues().len(), 534);
    assert_eq!(a.sigs().len(), 35);
    assert_eq!(a.diagnostics().len(), 1);
    assert_eq!(a.diagnostics()[0].entity.as_deref(), Some("andrei-c-coman"));
    assert_eq!(
        a.provenance()
            .files
            .get(std::path::Path::new("xml/schema.rnc"))
            .map(String::as_str),
        Some(aclfolio::source::SUPPORTED_SCHEMA_SHA256)
    );
    let b = a.reload(path, &LoadOptions::default()).unwrap();
    assert_eq!(b.reload_stats().parsed_collections, 0);
    assert_eq!(b.reload_stats().reused_collections, 1735);
    assert_eq!(a.diff(&b), aclfolio::SnapshotDiff::default());
}
