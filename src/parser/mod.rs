//! Source XML parsing with explicit snapshot versus detached-constructor
//! validation.

use crate::{
    error::{Diagnostic, DiagnosticKind, Error},
    ids::{CollectionId, PaperId, VolumeId, build_id, convert_orcid},
    model::{
        Attachment, Award, Collection, Erratum, Event, EventMetadata, Mrf, Name, NameSpecification,
        Notice, Paper, PdfReference, Revision, Talk, UrlReference, Video, Volume, VolumeMetadata,
    },
    text::RichText,
    types::DeletionKind,
};
use roxmltree::{Document, Node};
use std::{collections::BTreeMap, path::Path};
use validation::check;

mod validation;

type XmlNode<'a> = Node<'a, 'a>;

fn children_named<'a>(node: XmlNode<'a>, tag: &'a str) -> impl Iterator<Item = XmlNode<'a>> {
    node.children().filter(move |c| c.has_tag_name(tag))
}

fn last_child<'a>(node: XmlNode<'a>, tag: &str) -> Option<XmlNode<'a>> {
    node.children().rfind(|c| c.has_tag_name(tag))
}

fn descendant_text(node: XmlNode<'_>) -> String {
    node.descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect()
}

fn optional_text(node: XmlNode<'_>, tag: &str) -> Option<String> {
    last_child(node, tag).map(|node| {
        node.first_child()
            .filter(|c| c.is_text())
            .and_then(|c| c.text())
            .unwrap_or("")
            .to_owned()
    })
}

fn optional_attribute(node: XmlNode<'_>, key: &str) -> Option<String> {
    node.attribute(key).map(str::to_owned)
}

fn text_or_default(node: XmlNode<'_>, key: &str) -> String {
    optional_text(node, key).unwrap_or_default()
}

fn attribute_or_default(node: XmlNode<'_>, key: &str) -> String {
    optional_attribute(node, key).unwrap_or_default()
}

fn parse_name(node: XmlNode<'_>) -> Name {
    Name {
        first: optional_text(node, "first"),
        last: text_or_default(node, "last"),
        script: optional_attribute(node, "script"),
        extensions: BTreeMap::new(),
    }
}

fn parse_credit(node: XmlNode<'_>) -> NameSpecification {
    NameSpecification {
        id: optional_attribute(node, "id"),
        orcid: optional_attribute(node, "orcid").map(|value| convert_orcid(&value)),
        openreview: optional_attribute(node, "openreview"),
        name: parse_name(node),
        variant: children_named(node, "variant").next().map(parse_name),
        additional_variants: node
            .children()
            .filter(|child| child.has_tag_name("variant"))
            .skip(1)
            .map(parse_name)
            .collect(),
        affiliation: optional_text(node, "affiliation"),
    }
}

fn parse_url(node: XmlNode<'_>) -> UrlReference {
    UrlReference {
        kind: optional_attribute(node, "type"),
        url: descendant_text(node),
        checksum: optional_attribute(node, "hash"),
    }
}

fn parse_pdf(node: XmlNode<'_>) -> Option<PdfReference> {
    last_child(node, "pdf").map(|child| PdfReference {
        hash: attribute_or_default(child, "hash"),
    })
}

fn parse_urls(node: XmlNode<'_>) -> Vec<UrlReference> {
    children_named(node, "url").map(parse_url).collect()
}

fn parse_credits(node: XmlNode<'_>, tag: &str) -> Vec<NameSpecification> {
    children_named(node, tag).map(parse_credit).collect()
}

fn parse_notice(node: XmlNode<'_>) -> Notice {
    Notice {
        date: attribute_or_default(node, "date"),
        text: descendant_text(node),
    }
}

fn parse_metadata(node: XmlNode<'_>) -> VolumeMetadata {
    VolumeMetadata {
        booktitle: last_child(node, "booktitle")
            .map(RichText::from_node)
            .unwrap_or_default(),
        shortbooktitle: last_child(node, "shortbooktitle").map(RichText::from_node),
        editors: parse_credits(node, "editor"),
        publisher: optional_text(node, "publisher"),
        address: optional_text(node, "address"),
        month: optional_text(node, "month"),
        year: text_or_default(node, "year"),
        isbn: optional_text(node, "isbn"),
        pdf: parse_pdf(node),
        urls: parse_urls(node),
        doi: optional_text(node, "doi"),
        sigs: children_named(node, "sig").map(descendant_text).collect(),
        venues: children_named(node, "venue").map(descendant_text).collect(),
        journal_volume: optional_text(node, "journal-volume"),
        journal_issue: optional_text(node, "journal-issue"),
        journal_title: optional_text(node, "journal-title"),
    }
}

fn parse_paper(node: XmlNode<'_>, id: PaperId) -> Paper {
    Paper {
        local_id: if node.has_tag_name("frontmatter") {
            "0".into()
        } else {
            attribute_or_default(node, "id")
        },
        id,
        frontmatter: node.has_tag_name("frontmatter")
            || node.attribute("type") == Some("frontmatter"),
        last_deletion: node
            .children()
            .rfind(|child| child.has_tag_name("removed") || child.has_tag_name("retracted"))
            .map(|child| {
                if child.has_tag_name("removed") {
                    DeletionKind::Removed
                } else {
                    DeletionKind::Retracted
                }
            }),
        kind: optional_attribute(node, "type"),
        ingest_date: optional_attribute(node, "ingest-date"),
        title: last_child(node, "title").map(RichText::from_node),
        abstract_text: last_child(node, "abstract").map(RichText::from_node),
        bibkey: text_or_default(node, "bibkey"),
        authors: parse_credits(node, "author"),
        editors: parse_credits(node, "editor"),
        doi: optional_text(node, "doi"),
        pages: optional_text(node, "pages"),
        issue: optional_text(node, "issue"),
        journal: optional_text(node, "journal"),
        language: optional_text(node, "language"),
        month: optional_text(node, "month"),
        note: optional_text(node, "note"),
        mrf: last_child(node, "mrf").map(|child| Mrf {
            src: attribute_or_default(child, "src"),
            value: descendant_text(child),
        }),
        pdf: parse_pdf(node),
        urls: parse_urls(node),
        attachments: children_named(node, "attachment")
            .map(|child| Attachment {
                kind: optional_attribute(child, "type"),
                hash: attribute_or_default(child, "hash"),
                filename: descendant_text(child),
            })
            .collect(),
        revisions: children_named(node, "revision")
            .map(|child| Revision {
                id: attribute_or_default(child, "id"),
                date: optional_attribute(child, "date"),
                hash: child.attribute("hash").unwrap_or("None").to_owned(),
                text: descendant_text(child),
            })
            .collect(),
        errata: children_named(node, "erratum")
            .map(|child| Erratum {
                id: attribute_or_default(child, "id"),
                date: optional_attribute(child, "date"),
                hash: optional_attribute(child, "hash").filter(|value| !value.is_empty()),
            })
            .collect(),
        retracted: last_child(node, "retracted").map(parse_notice),
        removed: last_child(node, "removed").map(parse_notice),
        videos: children_named(node, "video")
            .map(|child| Video {
                href: attribute_or_default(child, "href"),
                permission: child
                    .attribute("permission")
                    .map(|value| value == "true" || value == "1"),
            })
            .collect(),
        awards: children_named(node, "award")
            .map(|child| Award {
                name: attribute_or_default(child, "name"),
                text: descendant_text(child),
            })
            .collect(),
    }
}

struct Context<'a> {
    file: &'a Path,
    diagnostics: Vec<Diagnostic>,
}

impl Context<'_> {
    fn error(&self, node: XmlNode<'_>, message: impl ToString) -> Error {
        let position = node.document().text_pos_at(node.range().start);
        Error::Parse {
            file: self.file.into(),
            entity: entity(node),
            line: position.row,
            column: position.col,
            message: message.to_string(),
        }
    }

    fn diagnostic(&mut self, node: XmlNode<'_>, kind: DiagnosticKind, message: impl ToString) {
        let position = node.document().text_pos_at(node.range().start);
        self.diagnostics.push(Diagnostic {
            file: self.file.into(),
            entity: entity(node),
            line: Some(position.row),
            column: Some(position.col),
            kind,
            message: message.to_string(),
        });
    }
}

fn entity(node: XmlNode<'_>) -> Option<String> {
    node.ancestors()
        .find_map(|node| node.attribute("id").map(str::to_owned))
}

/// Detached constructors validate only the fields they require; snapshot
/// loading
/// additionally reports unknown fields and structural inconsistencies.
enum ValidationMode {
    Snapshot,
    Detached,
}

pub(crate) fn parse(
    file: &Path,
    input: &str,
    max_nodes: u32,
    max_depth: usize,
) -> Result<(Collection, Vec<Diagnostic>), Error> {
    parse_inner(file, input, max_nodes, max_depth, ValidationMode::Snapshot)
}

pub(crate) fn parse_detached(
    file: &Path,
    input: &str,
    max_nodes: u32,
    max_depth: usize,
) -> Result<(Collection, Vec<Diagnostic>), Error> {
    parse_inner(file, input, max_nodes, max_depth, ValidationMode::Detached)
}

fn parse_inner(
    file: &Path,
    input: &str,
    max_nodes: u32,
    max_depth: usize,
    validation: ValidationMode,
) -> Result<(Collection, Vec<Diagnostic>), Error> {
    let document = Document::parse_with_options(
        input,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: max_nodes,
            ..Default::default()
        },
    )
    .map_err(|error| Error::Parse {
        file: file.into(),
        entity: None,
        line: error.pos().row,
        column: error.pos().col,
        message: error.to_string(),
    })?;
    let mut context = Context {
        file,
        diagnostics: vec![],
    };
    for node in document.descendants().filter(|node| node.is_element()) {
        if node.ancestors().take(max_depth.saturating_add(2)).count() > max_depth.saturating_add(1)
        {
            return Err(Error::Limit(format!(
                "{}: XML nesting exceeds {max_depth}",
                file.display()
            )));
        }
    }
    let root = document.root_element();
    if !root.has_tag_name("collection") || root.tag_name().namespace().is_some() {
        return Err(context.error(root, "expected unnamespaced collection root"));
    }
    if matches!(validation, ValidationMode::Snapshot) {
        check(&mut context, root)?;
    }
    let id: CollectionId = attribute_or_default(root, "id")
        .parse()
        .map_err(|error| context.error(root, error))?;
    let mut volumes = Vec::new();
    for node in children_named(root, "volume") {
        let local = attribute_or_default(node, "id");
        let volume_id: VolumeId = build_id(id.as_str(), Some(&local), None)
            .map_err(|error| context.error(node, error))?
            .parse()
            .map_err(|error| context.error(node, error))?;
        let metadata = parse_metadata(
            last_child(node, "meta").ok_or_else(|| context.error(node, "missing meta"))?,
        );
        let mut papers = Vec::new();
        for paper_node in node.children().filter(|paper_node| {
            paper_node.has_tag_name("paper") || paper_node.has_tag_name("frontmatter")
        }) {
            let local = if paper_node.has_tag_name("frontmatter") {
                "0".to_owned()
            } else {
                attribute_or_default(paper_node, "id")
            };
            if paper_node.has_tag_name("paper") && local == "0" {
                context.diagnostic(
                    paper_node,
                    DiagnosticKind::Structure,
                    "paper id must be positive; use frontmatter for zero",
                );
            }
            let paper_id = build_id(
                id.as_str(),
                Some(&attribute_or_default(node, "id")),
                Some(&local),
            )
            .map_err(|error| context.error(paper_node, error))?
            .parse()
            .map_err(|error| context.error(paper_node, error))?;
            papers.push(parse_paper(paper_node, paper_id));
        }
        volumes.push(Volume {
            id: volume_id,
            local_id: local,
            kind: attribute_or_default(node, "type"),
            ingest_date: optional_attribute(node, "ingest-date"),
            metadata,
            papers,
        });
    }
    let event = last_child(root, "event")
        .map(|node| -> Result<Event, Error> {
            Ok(Event {
                id: attribute_or_default(node, "id"),
                metadata: last_child(node, "meta").map(|metadata| EventMetadata {
                    title: last_child(metadata, "title").map(RichText::from_node),
                    location: optional_text(metadata, "location"),
                    dates: optional_text(metadata, "dates"),
                }),
                links: last_child(node, "links").map(parse_urls),
                talks: children_named(node, "talk")
                    .map(|talk| Talk {
                        kind: optional_attribute(talk, "type"),
                        title: last_child(talk, "title")
                            .map(RichText::from_node)
                            .unwrap_or_default(),
                        speakers: parse_credits(talk, "speaker"),
                        url: children_named(talk, "url").next().map(parse_url),
                        additional_urls: talk
                            .children()
                            .filter(|child| child.has_tag_name("url"))
                            .skip(1)
                            .map(parse_url)
                            .collect(),
                    })
                    .collect(),
                colocated: last_child(node, "colocated")
                    .map(|colocated| {
                        children_named(colocated, "volume-id")
                            .map(|volume| {
                                descendant_text(volume)
                                    .parse()
                                    .map_err(|error| context.error(volume, error))
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?,
            })
        })
        .transpose()?;
    Ok((Collection { id, volumes, event }, context.diagnostics))
}
