use aclfolio::*;
use proptest::prelude::*;
use std::{fs, path::PathBuf};
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data")
}
fn copy_fixture() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    for dir in ["xml", "json"] {
        fs::create_dir(t.path().join(dir)).unwrap();
        for e in fs::read_dir(fixture().join(dir)).unwrap() {
            let p = e.unwrap().path();
            fs::copy(&p, t.path().join(dir).join(p.file_name().unwrap())).unwrap();
        }
    }
    t
}
fn strict() -> LoadOptions {
    LoadOptions {
        policy: LoadPolicy::Strict,
        ..Default::default()
    }
}
fn edit(t: &tempfile::TempDir, from: &str, to: &str) {
    let p = t.path().join("xml/2022.test.xml");
    let s = fs::read_to_string(&p).unwrap();
    assert!(s.contains(from));
    fs::write(p, s.replace(from, to)).unwrap();
}
#[test]
fn schema_fields_and_inheritance() {
    let a = strict().open(fixture()).unwrap();
    assert!(a.diagnostics().is_empty());
    let p = a.paper("2022.test-long.1").unwrap().unwrap();
    assert_eq!(
        p.title().plain_text(),
        "A bold nested underlined small code NLP & link https://example.org/x x^2"
    );
    assert_eq!(
        p.abstract_text.as_ref().unwrap().plain_text(),
        "First <paragraph>.\n\nSecond nested paragraph."
    );
    assert_eq!(p.year(), "2022");
    assert_eq!(p.month(), Some("June"));
    assert_eq!(
        p.effective_ingest_date(),
        EffectiveDate {
            value: Some("2023-02-03"),
            origin: DateOrigin::Paper
        }
    );
    assert_eq!(p.authors[1].name.first.as_deref(), Some(""));
    assert_eq!(p.note.as_deref(), Some(""));
    assert_eq!(
        p.authors[0].variant.as_ref().unwrap().script.as_deref(),
        Some("Han")
    );
    assert_eq!(p.authors[0].openreview.as_deref(), Some("~Jane_Doe1"));
    assert!(a.resolve_person(&p.authors[0]).is_some());
    assert!(a.resolve_person(&p.authors[1]).is_none());
    assert_eq!(p.editors()[0].name.last, "Editor");
    assert_eq!(p.revisions.len(), 2);
    assert_eq!(p.errata.len(), 1);
    assert_eq!(p.retracted.as_ref().unwrap().text, "Retraction text");
    assert_eq!(p.removed.as_ref().unwrap().text, "Removal text");
    assert_eq!(p.videos[0].permission, Some(false));
    assert_eq!(p.videos[1].permission, None);
    assert_eq!(p.awards[0].name, "best");
    assert_eq!(p.mrf.as_ref().unwrap().src, "legacy");
    assert_eq!(p.attachments[0].filename, "code.zip");
    assert_eq!(p.urls.len(), 2);
    assert_eq!(p.pdf.as_ref().unwrap().hash, "abcdef03");
    assert_eq!(p.doi.as_deref(), Some("10.example/test"));
    let p = a.paper("2022.test-long.2").unwrap().unwrap();
    assert_eq!(p.effective_ingest_date().origin, DateOrigin::Volume);
    assert_eq!(p.month(), Some("May"));
    assert_eq!(p.editors()[0].name.last, "Doe");
    assert!(p.pdf.is_none());
    let front = a.paper("2022.test-long.0").unwrap().unwrap();
    assert!(front.frontmatter);
    assert!(front.stored().title.is_none());
    assert_eq!(front.title().plain_text(), "Proceedings of Test");
    assert_eq!(
        a.paper("C69-0125")
            .unwrap()
            .unwrap()
            .effective_ingest_date()
            .origin,
        DateOrigin::Absent
    );
    assert_eq!(a.works("jane-doe").count(), 2);
    assert_eq!(a.venue_volumes("test").count(), 7);
    assert_eq!(a.sig_volumes("sigtest").count(), 1);
    let e = a.event("test-2022").unwrap().definitions().next().unwrap();
    assert_eq!(e.talks.len(), 1);
    assert_eq!(e.links.as_ref().unwrap().len(), 1);
    assert_eq!(e.colocated.as_ref().unwrap()[0].as_str(), "2022.test-long");
    assert_eq!(a.person("jane-doe").unwrap().names.len(), 2);
    assert_eq!(a.sig("sigtest").unwrap().external_meetings.len(), 1);
    assert!(a.paper("2022.test-long.999").unwrap().is_none());
    assert!(a.paper("invalid").is_err());
}
#[test]
fn historical_ids_and_unicode() {
    for (p, v) in [
        ("P18-1001", "P18-1"),
        ("W18-6310", "W18-63"),
        ("C69-0101", "C69-01"),
        ("D19-5001", "D19-50"),
        ("D19-6601", "D19-66"),
        ("D19-6701", "D19-67"),
        ("2022.acl-long.01", "2022.acl-long"),
        ("2022.acl-long.a1", "2022.acl-long"),
        ("D19-1001", "D19-1"),
        ("C92-1025", "C92-1"),
        ("2022.acl-long.42", "2022.acl-long"),
    ] {
        let id: PaperId = p.parse().unwrap();
        assert_eq!(id.volume_id().as_str(), v);
        assert_eq!(id.to_string(), p);
    }
    for bad in [
        "",
        "日本",
        "P18-01",
        "2022.ACL-long.1",
        "P18-1xyz",
        "2022.acl-long.-1",
    ] {
        assert!(bad.parse::<PaperId>().is_err(), "{bad}");
    }
    assert!(serde_json::from_str::<PaperId>("\"bad\"").is_err());
}
#[test]
fn compatible_and_strict_diagnostics() {
    let t = copy_fixture();
    edit(
        &t,
        "<note></note>",
        "<note></note><future value=\"new\">data</future>",
    );
    let a = Anthology::open(t.path()).unwrap();
    assert!(
        a.diagnostics()
            .iter()
            .any(|d| d.kind == DiagnosticKind::UnknownField && d.line.is_some())
    );
    assert!(matches!(strict().open(t.path()), Err(Error::Validation(_))));
    let t = copy_fixture();
    edit(&t, "id=\"jane-doe\"", "id=\"unknown-person\"");
    let a = Anthology::open(t.path()).unwrap();
    assert!(
        a.diagnostics()
            .iter()
            .any(|d| d.kind == DiagnosticKind::Reference)
    );
    let t = copy_fixture();
    let p = t.path().join("json/people.json");
    let s = fs::read_to_string(&p)
        .unwrap()
        .replace("\"comment\":", "\"future\":42,\"comment\":");
    fs::write(&p, s).unwrap();
    let a = Anthology::open(t.path()).unwrap();
    assert_eq!(a.person("jane-doe").unwrap().extensions["future"], 42);
}
#[test]
fn update_add_modify_delete_rename_and_rollback() {
    let t = copy_fixture();
    let old = strict().open(t.path()).unwrap();
    let same = old.reload(t.path(), &strict()).unwrap();
    assert_eq!(same.reload_stats().parsed_collections, 0);
    assert_eq!(
        same.reload_stats().reused_collections,
        old.collections().len()
    );
    edit(&t, "<title>Inherited</title>", "<title>Changed</title>");
    edit(
        &t,
        "</volume>",
        "<paper id=\"3\"><title>New</title><bibkey>new</bibkey></paper></volume>",
    );
    fs::remove_file(t.path().join("xml/P18.xml")).unwrap();
    fs::rename(
        t.path().join("xml/C92.xml"),
        t.path().join("xml/renamed.xml"),
    )
    .unwrap();
    let new = old.reload(t.path(), &strict()).unwrap();
    let fresh = strict().open(t.path()).unwrap();
    assert_eq!(
        serde_json::to_value(new.collections().collect::<Vec<_>>()).unwrap(),
        serde_json::to_value(fresh.collections().collect::<Vec<_>>()).unwrap()
    );
    assert_eq!(new.diagnostics(), fresh.diagnostics());
    assert_eq!(new.provenance(), fresh.provenance());
    let d = old.diff(&new);
    assert_eq!(d.papers.added, vec!["2022.test-long.3"]);
    assert_eq!(d.papers.removed, vec!["P18-1025"]);
    assert_eq!(d.papers.modified, vec!["2022.test-long.2"]);
    edit(&t, "<year>2022</year>", "<year>2021</year>");
    let inherited = new.reload(t.path(), &strict()).unwrap();
    assert_eq!(new.diff(&inherited).papers.modified.len(), 4);
    edit(&t, "</collection>", "<!-- source-only -->\n</collection>");
    let commented = inherited.reload(t.path(), &strict()).unwrap();
    assert!(inherited.diff(&commented).source_only);
    edit(&t, "</collection>", "BROKEN");
    assert!(commented.reload(t.path(), &strict()).is_err());
    assert_eq!(
        old.paper("2022.test-long.2")
            .unwrap()
            .unwrap()
            .title()
            .plain_text(),
        "Inherited"
    );
}
#[test]
fn rejects_malformed_unsafe_and_ambiguous_sources() {
    for xml in [
        "<!DOCTYPE collection [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><collection id=\"P18\">&x;</collection>",
        "<collection id=\"P18\"><volume>",
        "<collection id=\"../escape\"/>",
    ] {
        let t = copy_fixture();
        fs::write(t.path().join("xml/bad.xml"), xml).unwrap();
        assert!(Anthology::open(t.path()).is_err());
    }
    let t = copy_fixture();
    edit(&t, "<note></note>", "<note/><note/>");
    assert!(Anthology::open(t.path()).is_err());
    let t = copy_fixture();
    fs::write(t.path().join("json/people.json"), "{\"x\":{},\"x\":{}}").unwrap();
    assert!(Anthology::open(t.path()).is_err());
    let t = copy_fixture();
    fs::write(t.path().join("xml/bad.xml"), [0xff, 0xfe]).unwrap();
    assert!(Anthology::open(t.path()).is_err());
    let t = copy_fixture();
    edit(&t, "permission=\"false\"", "permission=\"maybe\"");
    assert!(Anthology::open(t.path()).is_err());
    let mut o = strict();
    o.limits.file_bytes = 10;
    assert!(matches!(o.open(fixture()), Err(Error::Limit(_))));
    let mut o = strict();
    o.limits.xml_depth = 2;
    assert!(matches!(o.open(fixture()), Err(Error::Limit(_))));
    let t = copy_fixture();
    edit(&t, "2023-02-03", "2023-02-29");
    assert!(matches!(strict().open(t.path()), Err(Error::Validation(_))));
}
#[test]
fn concurrent_reads_and_deterministic_serialization() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Anthology>();
    let a = std::sync::Arc::new(strict().open(fixture()).unwrap());
    let expected = serde_json::to_string(&a.collections().collect::<Vec<_>>()).unwrap();
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let a = a.clone();
            std::thread::spawn(move || {
                serde_json::to_string(&a.collections().collect::<Vec<_>>()).unwrap()
            })
        })
        .collect();
    for t in threads {
        assert_eq!(t.join().unwrap(), expected);
    }
    let ids: Vec<_> = a.papers().map(|p| p.id().clone()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
}
proptest! {
    #[test] fn modern_roundtrip(year in 1000u16..9999,venue in "[a-z][a-z0-9]{0,15}",v in "[a-z0-9]{1,10}",n in 0u32..u32::MAX) {
        let s=format!("{year}.{venue}-{v}.{n}");let id:PaperId=s.parse().unwrap();prop_assert_eq!(id.to_string(),s);prop_assert_eq!(id.volume_id().paper_id(&n.to_string()).unwrap(),id);
    }
    #[test] fn arbitrary_identifiers_never_panic(s in ".{0,100}") {let _=s.parse::<AnthologyId>();let _=s.parse::<PaperId>();let _=s.parse::<VolumeId>();}
}

#[test]
fn event_groups_and_derived_association_origins() {
    let t = copy_fixture();
    let old = strict().open(t.path()).unwrap();
    let implicit = old.event("test-1969").unwrap();
    assert!(!implicit.is_explicit());
    assert_eq!(implicit.definitions().len(), 0);
    assert_eq!(implicit.volumes().count(), 6);
    assert_eq!(
        implicit.inferred_title().as_deref(),
        Some("Test Venue (1969)")
    );
    assert!(
        implicit
            .associations()
            .all(|(_, origin)| origin == EventAssociation::VenueYear)
    );
    let file = t.path().join("xml/C92.xml");
    let xml = fs::read_to_string(&file).unwrap().replace("</collection>","<event id=\"test-2022\"><meta><title>Another definition</title></meta></event></collection>");
    fs::write(&file, xml).unwrap();
    let new = old.reload(t.path(), &strict()).unwrap();
    let group = new.event("test-2022").unwrap();
    assert_eq!(group.definitions().len(), 2);
    assert_eq!(group.volumes().count(), 2);
    assert_eq!(new.volume_events(&"C92-1".parse().unwrap()).count(), 2);
    assert_eq!(old.diff(&new).events.modified, ["test-2022"]);
}
#[test]
fn directory_reloads_do_not_invent_people() {
    let t = copy_fixture();
    let old = strict().open(t.path()).unwrap();
    fs::write(t.path().join("json/people.json"), "{}").unwrap();
    let new = old.reload(t.path(), &LoadOptions::default()).unwrap();
    let fresh = Anthology::open(t.path()).unwrap();
    assert_eq!(new.reload_stats().parsed_collections, 0);
    assert_eq!(new.works("jane-doe").count(), 0);
    assert_eq!(old.works("jane-doe").count(), 2);
    assert_eq!(new.diagnostics(), fresh.diagnostics());
    assert_eq!(old.diff(&new).people.removed, ["jane-doe"]);
    assert!(
        new.paper("2022.test-long.1").unwrap().unwrap().authors[0]
            .id
            .is_some()
    );
    assert!(matches!(
        new.reload(t.path(), &strict()),
        Err(Error::Validation(_))
    ));
}
#[test]
fn reload_respects_stricter_limits_and_schema_version() {
    let t = copy_fixture();
    let a = strict().open(t.path()).unwrap();
    let mut smaller = strict();
    smaller.limits.xml_depth = 2;
    assert!(a.reload(t.path(), &smaller).is_err());
    fs::write(t.path().join("xml/schema.rnc"), "new schema").unwrap();
    assert!(matches!(
        a.reload(t.path(), &strict()),
        Err(Error::Validation(_))
    ));
    let a = a.reload(t.path(), &LoadOptions::default()).unwrap();
    assert!(
        a.diagnostics()
            .iter()
            .any(|d| d.file.ends_with("schema.rnc"))
    );
}
#[test]
fn duplicate_ids_and_nested_json_keys_are_rejected() {
    let t = copy_fixture();
    fs::copy(
        t.path().join("xml/C92.xml"),
        t.path().join("xml/duplicate.xml"),
    )
    .unwrap();
    assert!(Anthology::open(t.path()).is_err());
    let t = copy_fixture();
    edit(&t, "<paper id=\"2\">", "<paper id=\"1\">");
    assert!(Anthology::open(t.path()).is_err());
    let t = copy_fixture();
    fs::write(
        t.path().join("json/people.json"),
        r#"{"jane-doe":{"names":[{"last":"Doe","last":"Roe"}]}}"#,
    )
    .unwrap();
    assert!(Anthology::open(t.path()).is_err());
}
#[test]
fn source_only_rename_keeps_semantic_indices_identical() {
    let t = copy_fixture();
    let a = strict().open(t.path()).unwrap();
    fs::rename(t.path().join("xml/C92.xml"), t.path().join("xml/aaa.xml")).unwrap();
    let b = a.reload(t.path(), &strict()).unwrap();
    assert!(a.diff(&b).source_only);
}
#[test]
fn unicode_whitespace_and_newlines_do_not_join_words() {
    let t = copy_fixture();
    edit(
        &t,
        "<title>Inherited</title>",
        "<title>New\nline&#160;<b>and</b> Unicode</title>",
    );
    let a = strict().open(t.path()).unwrap();
    assert_eq!(
        a.paper("2022.test-long.2")
            .unwrap()
            .unwrap()
            .title()
            .plain_text(),
        "New line and Unicode"
    );
    for date in ["2023-01-01💥", "2023-01-01+あa:00", "2023-01-01+14:01"] {
        let t = copy_fixture();
        edit(&t, "2023-02-03", date);
        assert!(matches!(strict().open(t.path()), Err(Error::Validation(_))));
    }
}
#[cfg(unix)]
#[test]
fn symlink_sources_are_rejected() {
    let t = copy_fixture();
    std::os::unix::fs::symlink(t.path().join("xml/C92.xml"), t.path().join("xml/link.xml"))
        .unwrap();
    assert!(matches!(Anthology::open(t.path()), Err(Error::Limit(_))));
}

#[test]
fn snapshot_diff_preserves_name_metadata_beyond_name_equality() {
    let t = copy_fixture();
    let old = Anthology::open(t.path()).unwrap();
    edit(&t, "script=\"Han\"", "script=\"Hant\"");
    let new = old.reload(t.path(), &LoadOptions::default()).unwrap();
    let diff = old.diff(&new);
    assert!(diff.collections.modified.contains(&"2022.test".into()));
    assert!(diff.volumes.modified.contains(&"2022.test-long".into()));
    assert!(diff.papers.modified.contains(&"2022.test-long.1".into()));
    assert!(!diff.source_only);
    let before = &old.paper("2022.test-long.1").unwrap().unwrap().authors[0].variant;
    let after = &new.paper("2022.test-long.1").unwrap().unwrap().authors[0].variant;
    assert_eq!(before, after); // Python name equality ignores script; source diff must not.
}
