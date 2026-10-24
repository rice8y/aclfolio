use aclfolio::{
    Anthology, MarkupText,
    config::Config,
    files::{FileKind, FileReference},
    ids,
    text::RichText,
    xml::XmlElement,
};
fn corpus() -> Anthology {
    Anthology::open(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/data")).unwrap()
}
#[test]
fn official_navigation_and_identity_rules() {
    let a = corpus();
    a.load_all().unwrap();
    let p = a.get_paper("2022.test-long.1").unwrap().unwrap();
    let empty = p.author_credits().nth(1).unwrap();
    assert_eq!(empty.stored().name.first.as_deref(), Some(""));
    assert_eq!(empty.first(), None);
    assert_eq!(empty.name().first, None);
    assert_eq!(
        p.full_id_tuple(),
        ids::IdParts("2022.test".into(), Some("long".into()), Some("1".into()))
    );
    assert_eq!(
        p.author_credits().next().unwrap().resolve().unwrap().id,
        "jane-doe"
    );
    assert_eq!(
        p.author_credits().nth(1).unwrap().resolve().unwrap().id,
        "unresolved/unverified"
    );
    let person = a.get_person("jane-doe").unwrap().unwrap();
    assert!(person.papers().any(|p| p.is_frontmatter()));
    assert_eq!(person.namespecs().unwrap().len(), person.item_ids.len());
    let mut hypothetical = a.person_index().unwrap().clone();
    let mut ns = p.authors[0].clone();
    ns.id = None;
    ns.orcid = None;
    // Explicitly disabled name matching prevents the slug heuristic from selecting Jane's record.
    assert_eq!(
        hypothetical.get_by_namespec(&ns).unwrap().id,
        "jane-doe/unverified"
    );
    assert!(a.get_person("jane-doe/unverified").unwrap().is_none());
    assert_eq!(
        p.deletion().unwrap().0,
        aclfolio::serialization::DeletionKind::Removed
    );
    assert_eq!(p.ingest_date_value().unwrap().to_string(), "2023-02-03");
    assert_eq!(p.language_name().unwrap().as_deref(), Some("English"));
    assert_eq!(a.get_venue("test").unwrap().sigs()[0].id(), "sigtest");
    assert!(
        a.get_sig("sigtest")
            .unwrap()
            .meetings_by_year()
            .contains_key("2022")
    );
    assert_eq!(
        a.get_event("test-2022")
            .unwrap()
            .talks()
            .next()
            .unwrap()
            .speaker_credits()
            .next()
            .unwrap()
            .resolve()
            .unwrap()
            .id,
        "jane-doe"
    );
}
#[test]
fn conversions_preserve_markup_and_reference_semantics() {
    let text = RichText::from_latex(r"An \emph{example} of {NLP} with $x_1^2$", true).unwrap();
    assert_eq!(
        text.as_xml(),
        "An <i>example</i> of <fixed-case>NLP</fixed-case> with <tex-math>x_1^2</tex-math>"
    );
    assert_eq!(text.as_text().unwrap(), "An example of NLP with x12");
    assert_eq!(aclfolio::files::compute_checksum(b"123456789"), "cbf43926");
    let local = FileReference::from_xml(
        FileKind::Attachment,
        "<attachment hash=\"cbf43926\">data.zip</attachment>",
    )
    .unwrap();
    assert_eq!(
        local.url(&Config::default()),
        "https://aclanthology.org/attachments/data.zip"
    );
    assert_eq!(
        local.to_xml("attachment"),
        "<attachment hash=\"cbf43926\">data.zip</attachment>"
    );
    assert!(local.validate_external().is_err());
    let v = FileReference::from_xml(
        FileKind::Video,
        "<video href=\"https://example.org/v\" permission=\"0\"/>",
    )
    .unwrap();
    assert!(!v.permission);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("paper.pdf");
    std::fs::write(&path, b"123456789").unwrap();
    let f = FileReference::from_file(FileKind::Pdf, &path, &Config::default()).unwrap();
    assert_eq!(f.name, "paper");
    assert_eq!(f.checksum.as_deref(), Some("cbf43926"));
}
#[test]
fn pure_xml_comparison_and_schema_validation() {
    let a = "<paper id=\"1\"><title>A <b>Title</b></title><bibkey>key</bibkey><pages/></paper>";
    let b = "<paper id=\"1\"><bibkey>key</bibkey><title>A <b>Title</b></title></paper>";
    aclfolio::xml::assert_equals(a, b).unwrap();
    assert!(aclfolio::xml::assert_equals(a, &b.replace("Title", "TITLE")).is_err());
    let mut e = XmlElement::parse("<title>A <b>word</b></title>").unwrap();
    e.append_text("!");
    assert_eq!(e.stringify_children(), "A <b>word</b>!");
    let valid = "<collection id=\"2024.test\"><volume id=\"1\" type=\"journal\"><meta><booktitle>T</booktitle><year>2024</year><venue>cl</venue></meta><paper id=\"1\" ingest-date=\"2024-02-29\"><title>T</title><bibkey>test-key</bibkey></paper></volume></collection>";
    aclfolio::schema::validate_xml(valid).unwrap();
    assert!(aclfolio::schema::validate_xml(&valid.replace("2024-02-29", "2023-02-29")).is_err());
    assert!(
        aclfolio::schema::validate_xml(&valid.replace("<title>T</title>", "<title><par/></title>"))
            .is_err()
    );
}
#[test]
fn reference_ids_cover_non_corpus_valid_inputs() {
    assert_eq!(
        ids::parse_id("D19-9901").unwrap(),
        ids::IdParts("D19".into(), Some("99".into()), Some("1".into()))
    );
    assert_eq!(
        ids::build_id("D19", Some("99"), Some("1")).unwrap(),
        "D19-9901"
    );
    let xml = "<collection id=\"2024.test\"><volume id=\"main\" type=\"proceedings\"><meta><booktitle>T</booktitle><year>2024</year><venue>acl</venue></meta><paper id=\"a1\"><title>T</title><bibkey>test-key</bibkey></paper><paper id=\"01\"><title>U</title><bibkey>test-other</bibkey></paper></volume></collection>";
    let (c, _) = aclfolio::Collection::from_xml(xml).unwrap();
    assert_eq!(c.volumes[0].papers[0].id.as_str(), "2024.test-main.a1");
    assert_eq!(c.volumes[0].papers[1].local_id, "01");
    assert_eq!(
        aclfolio::languages::display_name("zh-Hant-TW").unwrap(),
        "Chinese (Traditional, Taiwan)"
    );
    assert!(aclfolio::languages::display_name("en-US-Latn").is_err());
}

#[test]
fn cache_lifecycle_and_detached_inference() {
    let mut a = corpus();
    assert!(!a.people_index_loaded());
    assert!(!a.bibkey_index_loaded());
    a.load_all().unwrap();
    a.bibkey_index().unwrap();
    assert!(a.people_index_loaded());
    assert!(a.bibkey_index_loaded());
    a.reset_indices();
    assert!(!a.people_index_loaded());
    assert!(!a.bibkey_index_loaded());
    let v = a.get_volume("2022.test-long").unwrap().unwrap();
    assert!(matches!(
        a.infer_event(v, "test", false).unwrap(),
        Some(aclfolio::directories::InferredEvent::Existing(_))
    ));
    assert!(a.infer_event(v, "missing", false).unwrap().is_none());
    assert!(a.infer_event(v, "missing", true).is_err());
    let original = v.metadata.editors[0].clone();
    let normalized = original.normalized(true, true).unwrap();
    assert_eq!(original.id, normalized.id);
    let text = MarkupText::from_xml("<title>A <b>B</b></title>").unwrap();
    assert_eq!(text.to_string(), "A <b>B</b>");
    let config = Config {
        url_prefix: "https://example.invalid".into(),
        ..Config::default()
    };
    let pdf = FileReference::new(FileKind::Pdf, "2022.test-long.1", None);
    assert_eq!(pdf.template_field(), "pdf_location_template");
    assert_eq!(
        pdf.url(&config),
        "https://example.invalid/2022.test-long.1.pdf"
    );
}

#[test]
fn bibkey_lookup_reports_index_failures_and_numeric_ids_do_not_overflow() {
    let a = corpus();
    assert_eq!(
        a.get_paper_by_bibkey("doe-2022-test")
            .unwrap()
            .unwrap()
            .id()
            .as_str(),
        "2022.test-long.1"
    );
    assert!(a.get_paper_by_bibkey("not-present").unwrap().is_none());
    let dir = tempfile::tempdir().unwrap();
    for folder in ["xml", "json"] {
        std::fs::create_dir(dir.path().join(folder)).unwrap();
        for entry in std::fs::read_dir(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/data/").to_owned() + folder,
        )
        .unwrap()
        {
            let entry = entry.unwrap();
            std::fs::copy(
                entry.path(),
                dir.path().join(folder).join(entry.file_name()),
            )
            .unwrap();
        }
    }
    let path = dir.path().join("xml/2022.test.xml");
    let original = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        original.replace("test-2022-inherited", "doe-2022-test"),
    )
    .unwrap();
    let a = Anthology::open(dir.path()).unwrap();
    assert!(a.get_paper_by_bibkey("doe-2022-test").is_err());
    let large = "99999999999999999999999999999999999999999999999999999999";
    std::fs::write(
        &path,
        original.replace("<paper id=\"2\">", &format!("<paper id=\"{large}\">")),
    )
    .unwrap();
    let a = Anthology::open(dir.path()).unwrap();
    assert_eq!(
        a.get_volume("2022.test-long")
            .unwrap()
            .unwrap()
            .generate_paper_id(),
        format!("1{}", "0".repeat(large.len()))
    );
}
