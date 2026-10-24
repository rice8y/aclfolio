use super::render;
use serde_json::json;

fn bibliography(layout: &str) -> String {
    format!(
        r#"<style xmlns="http://purl.org/net/xbiblio/csl" version="1.0"><bibliography><layout>{layout}</layout></bibliography></style>"#
    )
}

#[test]
fn groups_suppress_literal_labels_when_all_variables_are_missing() {
    let data = json!({"id": "test", "type": "book", "title": "Title"});
    let style = bibliography(
        r#"<group delimiter=" "><text value="pp."/><text variable="page"/></group><text variable="title"/>"#,
    );
    assert_eq!(render(&data, &style, false).unwrap(), "Title");
}

#[test]
fn name_substitution_consumes_the_variable_it_uses() {
    let data = json!({"id": "test", "type": "book", "title": "Title"});
    let style = bibliography(
        r#"<names variable="author"><name/><substitute><text variable="title"/></substitute></names><text variable="title" prefix=" / "/>"#,
    );
    assert_eq!(render(&data, &style, false).unwrap(), "Title");
}

#[test]
fn citation_links_respect_segment_boundaries_and_font_formatting() {
    let data =
        json!({"id": "test", "type": "book", "title": "Title", "URL": "https://example.org"});
    let plain = bibliography(
        r#"<group delimiter=" "><text value="Read"/><text variable="title"/></group>"#,
    );
    let styled = bibliography(
        r#"<group delimiter=" " font-style="italic"><text value="Read"/><text variable="title"/></group>"#,
    );
    assert_eq!(
        render(&data, &plain, true).unwrap(),
        r#"Read <a href="https://example.org">Title</a>"#,
    );
    assert_eq!(render(&data, &styled, true).unwrap(), "<i>Read Title</i>");
}
