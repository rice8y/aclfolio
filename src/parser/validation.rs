//! Structural checks and calendar validation for collection XML.

use super::{Context, XmlNode, last_child};
use crate::{DiagnosticKind, Error};
use std::collections::BTreeMap;

#[derive(Default)]
struct ElementShape {
    allowed_attributes: &'static str,
    allowed_children: &'static str,
    repeatable_children: &'static str,
    required_children: &'static str,
    required_attributes: &'static str,
    allow_text: bool,
}

// Runtime structural coverage of the pinned RNC. The compatibility runner also
// performs full lexical validation against that grammar.
fn shape(node: XmlNode<'_>) -> ElementShape {
    let tag = node.tag_name().name();
    match tag {
        "collection" => ElementShape {
            allowed_attributes: "id",
            allowed_children: "volume event",
            repeatable_children: "volume",
            required_attributes: "id",
            ..ElementShape::default()
        },
        "volume" => ElementShape {
            allowed_attributes: "id type ingest-date",
            allowed_children: "meta frontmatter paper",
            repeatable_children: "paper",
            required_children: "meta",
            required_attributes: "id type",
            ..ElementShape::default()
        },
        "paper" => ElementShape {
            allowed_attributes: "id ingest-date type",
            allowed_children: "abstract attachment bibkey author doi editor revision erratum retracted \
                removed issue journal mrf note pages title pdf url video language month award",
            repeatable_children: "attachment author editor revision erratum url video award",
            required_children: "bibkey title",
            required_attributes: "id",
            ..ElementShape::default()
        },
        "frontmatter" => ElementShape {
            allowed_children: "pdf url pages bibkey revision attachment doi",
            repeatable_children: "url revision attachment",
            required_children: "bibkey",
            ..ElementShape::default()
        },
        "meta" if node.parent().is_some_and(|p| p.has_tag_name("volume")) => ElementShape {
            allowed_children: "booktitle shortbooktitle editor publisher address month year isbn pdf url \
                doi sig venue journal-volume journal-issue journal-title",
            repeatable_children: "editor url sig venue",
            required_children: "booktitle year venue",
            ..ElementShape::default()
        },
        "meta" => ElementShape {
            allowed_children: "title location dates",
            ..ElementShape::default()
        },
        "event" => ElementShape {
            allowed_attributes: "id",
            allowed_children: "meta links talk colocated",
            repeatable_children: "talk",
            required_attributes: "id",
            ..ElementShape::default()
        },
        "links" => ElementShape {
            allowed_children: "url",
            repeatable_children: "url",
            ..ElementShape::default()
        },
        "colocated" => ElementShape {
            allowed_children: "volume-id",
            repeatable_children: "volume-id",
            ..ElementShape::default()
        },
        "talk" => ElementShape {
            allowed_attributes: "type",
            allowed_children: "title speaker url",
            repeatable_children: "speaker url",
            required_children: "title",
            ..ElementShape::default()
        },
        "author" | "editor" | "speaker" => ElementShape {
            allowed_attributes: "id orcid openreview",
            allowed_children: "first last variant affiliation",
            repeatable_children: "variant",
            required_children: "last",
            ..ElementShape::default()
        },
        "variant" => ElementShape {
            allowed_attributes: "script",
            allowed_children: "first last",
            required_children: "last",
            required_attributes: "script",
            ..ElementShape::default()
        },
        "title" | "booktitle" | "shortbooktitle" | "b" | "i" | "u" | "sc" | "tt" | "fixed-case" => {
            ElementShape {
                allowed_children: "b i u sc tt a url fixed-case tex-math",
                repeatable_children: "b i u sc tt a url fixed-case tex-math",
                allow_text: true,
                ..ElementShape::default()
            }
        }
        "abstract" => ElementShape {
            allowed_children: "b i u sc tt a url fixed-case tex-math par",
            repeatable_children: "b i u sc tt a url fixed-case tex-math par",
            allow_text: true,
            ..ElementShape::default()
        },
        "a" => ElementShape {
            allowed_attributes: "href",
            allowed_children: "b i u sc tt a url fixed-case tex-math",
            repeatable_children: "b i u sc tt a url fixed-case tex-math",
            required_attributes: "href",
            allow_text: true,
            ..ElementShape::default()
        },
        "url" => ElementShape {
            allowed_attributes: "type",
            allow_text: true,
            ..ElementShape::default()
        },
        "pdf" => ElementShape {
            allowed_attributes: "hash",
            required_attributes: "hash",
            ..ElementShape::default()
        },
        "attachment" => ElementShape {
            allowed_attributes: "type hash",
            required_attributes: "hash",
            allow_text: true,
            ..ElementShape::default()
        },
        "revision" => ElementShape {
            allowed_attributes: "id date hash",
            required_attributes: "id hash",
            allow_text: true,
            ..ElementShape::default()
        },
        "erratum" => ElementShape {
            allowed_attributes: "id date hash",
            required_attributes: "id hash",
            ..ElementShape::default()
        },
        "retracted" | "removed" => ElementShape {
            allowed_attributes: "date",
            required_attributes: "date",
            allow_text: true,
            ..ElementShape::default()
        },
        "mrf" => ElementShape {
            allowed_attributes: "src",
            required_attributes: "src",
            allow_text: true,
            ..ElementShape::default()
        },
        "video" => ElementShape {
            allowed_attributes: "href permission",
            required_attributes: "href",
            ..ElementShape::default()
        },
        "award" => ElementShape {
            allowed_attributes: "name",
            required_attributes: "name",
            allow_text: true,
            ..ElementShape::default()
        },
        "par" => ElementShape::default(),
        _ => ElementShape {
            allow_text: true,
            ..ElementShape::default()
        },
    }
}

fn has(list: &str, value: &str) -> bool {
    list.split_whitespace().any(|x| x == value)
}

pub(super) fn check(context: &mut Context<'_>, node: XmlNode<'_>) -> Result<(), Error> {
    let ElementShape {
        allowed_attributes,
        allowed_children,
        repeatable_children,
        required_children,
        required_attributes,
        allow_text,
    } = shape(node);
    for attribute in node.attributes() {
        if !has(allowed_attributes, attribute.name()) || attribute.namespace().is_some() {
            context.diagnostic(
                node,
                DiagnosticKind::UnknownField,
                format!(
                    "unknown attribute {} on {}",
                    attribute.name(),
                    node.tag_name().name()
                ),
            );
        }
        if attribute.name() == "hash"
            && (attribute.value().len() != 8
                || !attribute
                    .value()
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        {
            context.diagnostic(
                node,
                DiagnosticKind::Structure,
                "checksum must contain eight lowercase hexadecimal digits",
            );
        }
        if matches!(attribute.name(), "date" | "ingest-date") && !valid_date(attribute.value()) {
            context.diagnostic(
                node,
                DiagnosticKind::Structure,
                format!("invalid date {}", attribute.value()),
            );
        }
        if attribute.name() == "permission"
            && !matches!(attribute.value(), "true" | "false" | "1" | "0")
        {
            return Err(context.error(node, "invalid video permission boolean"));
        }
    }
    for attribute in required_attributes.split_whitespace() {
        if node.attribute(attribute).is_none() {
            return Err(context.error(node, format!("missing required attribute {attribute}")));
        }
    }
    for tag in required_children.split_whitespace() {
        if last_child(node, tag).is_none() {
            return Err(context.error(node, format!("missing required element {tag}")));
        }
    }
    let mut counts = BTreeMap::new();
    for child in node.children().filter(|node| node.is_element()) {
        let tag = child.tag_name().name();
        if !has(allowed_children, tag) || child.tag_name().namespace().is_some() {
            context.diagnostic(
                child,
                DiagnosticKind::UnknownField,
                format!("unknown element {tag} in {}", node.tag_name().name()),
            );
        } else {
            let count = counts.entry(tag).or_insert(0);
            *count += 1;
            if *count > 1 && !has(repeatable_children, tag) {
                return Err(context.error(child, format!("duplicate singleton element {tag}")));
            }
        }
        check(context, child)?;
    }
    if !allow_text
        && node
            .children()
            .any(|child| child.is_text() && !child.text().unwrap_or_default().trim().is_empty())
    {
        context.diagnostic(node, DiagnosticKind::Structure, "unexpected character data");
    }
    if node.has_tag_name("volume")
        && !matches!(node.attribute("type"), Some("journal" | "proceedings"))
    {
        context.diagnostic(node, DiagnosticKind::Structure, "unknown volume type");
    }
    Ok(())
}

pub(crate) fn valid_date(value: &str) -> bool {
    // XML Schema dates allow signed, extended years and timezone suffixes.
    // Compare the year modulo 400, avoiding overflow on arbitrarily long years.
    let value = value.trim();
    if !value.is_ascii() {
        return false;
    }
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let Some((year, rest)) = unsigned.split_once('-') else {
        return false;
    };
    if year.len() < 4
        || !year.bytes().all(|c| c.is_ascii_digit())
        || year.bytes().all(|c| c == b'0')
        || (year.len() > 4 && year.starts_with('0'))
        || rest.len() < 5
        || rest.as_bytes()[2] != b'-'
    {
        return false;
    }
    let year_modulo_400 = year
        .bytes()
        .fold(0u32, |acc, b| (acc * 10 + u32::from(b - b'0')) % 400);
    let Ok(month) = rest[..2].parse::<usize>() else {
        return false;
    };
    let Ok(day) = rest[3..5].parse::<u32>() else {
        return false;
    };
    if !rest[..2]
        .bytes()
        .chain(rest[3..5].bytes())
        .all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let is_leap_year =
        year_modulo_400 % 4 == 0 && (year_modulo_400 % 100 != 0 || year_modulo_400 % 400 == 0);
    let days_in_month = [
        0,
        31,
        if is_leap_year { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let zone = &rest[5..];
    let valid_zone = zone.is_empty()
        || zone == "Z"
        || (zone.len() == 6
            && matches!(zone.as_bytes()[0], b'+' | b'-')
            && zone.as_bytes()[3] == b':'
            && zone[1..3]
                .bytes()
                .chain(zone[4..].bytes())
                .all(|b| b.is_ascii_digit())
            && zone[1..3].parse::<u8>().is_ok_and(|h| h <= 14)
            && zone[4..].parse::<u8>().is_ok_and(|m| m < 60)
            && (&zone[1..3] != "14" || &zone[4..] == "00"));
    month < 13 && day > 0 && day <= days_in_month[month] && valid_zone
}
#[cfg(test)]
mod tests {
    use super::super::parse;
    use super::*;
    use proptest::prelude::*;
    use std::path::Path;

    #[test]
    fn xml_schema_date_ranges() {
        for date in [
            "2000-02-29",
            "2024-02-29Z",
            "-0001-01-01",
            "12000-02-29+14:00",
            "2001-03-01-09:30",
        ] {
            assert!(valid_date(date), "{date}");
        }
        for date in [
            "1900-02-29",
            "0000-01-01",
            "02024-01-01",
            "2024-00-01",
            "2024-01-00",
            "2024-01-01+14:01",
            "2024-01-01+aa:00",
        ] {
            assert!(!valid_date(date), "{date}");
        }
    }
    proptest! {
        #[test]
        fn arbitrary_xml_and_dates_never_panic(s in ".{0,1024}") {
            let _ = valid_date(&s);
            let _ = parse(Path::new("untrusted.xml"), &s, 1000, 20);
        }
    }
}
