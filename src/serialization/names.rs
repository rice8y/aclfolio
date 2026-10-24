//! Name and credit construction, normalization and XML conversion.

use super::{attr, element, optional};
use crate::{Name, NameSpecification};

fn parse_name(n: roxmltree::Node<'_, '_>) -> Result<Name, String> {
    let field = |tag| {
        n.children()
            .rfind(|c| c.has_tag_name(tag))
            .and_then(|c| c.text())
            .map(str::to_owned)
    };
    Name::new(
        field("first"),
        field("last").ok_or("name requires a last component")?,
        n.attribute("script").map(str::to_owned),
    )
}

impl Name {
    /// Construct a name from a `<variant>`, `<author>` or `<editor>` XML element.
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let d = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        parse_name(d.root_element())
    }

    /// Construct a name from its dictionary representation, validating the last
    /// component.
    pub fn from_dict(value: &serde_json::Value) -> Result<Self, String> {
        Self::new(
            value
                .get("first")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            value
                .get("last")
                .and_then(|v| v.as_str())
                .ok_or("name requires string last")?
                .into(),
            value
                .get("script")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
        )
    }
}

impl NameSpecification {
    /// Serialize an author/editor/speaker credit without changing the stored ID
    /// sentinel.
    pub fn to_xml(&self, tag: &str) -> String {
        let mut out = format!("<{tag}");
        attr(
            &mut out,
            "id",
            self.id
                .as_deref()
                .filter(|id| *id != crate::types::NO_PERSON_ID),
        );
        attr(&mut out, "orcid", self.orcid.as_deref());
        attr(&mut out, "openreview", self.openreview.as_deref());
        out.push('>');
        out.push_str(&element("first", self.name.first.as_deref().unwrap_or("")));
        out.push_str(&element("last", &self.name.last));
        optional(&mut out, "affiliation", self.affiliation.as_deref());
        for n in self.variants() {
            out.push_str(&n.to_xml("variant"));
        }
        out.push_str(&format!("</{tag}>"));
        out
    }

    /// Parse a detached credit, including every variant and ORCID normalization.
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let n = doc.root_element();
        let mut variants = n
            .children()
            .filter(|c| c.has_tag_name("variant"))
            .map(parse_name)
            .collect::<Result<Vec<_>, _>>()?;
        if n.attribute("id")
            .is_some_and(|id| !crate::ids::is_verified_person_id(id))
        {
            return Err("invalid explicit person ID".into());
        }
        let orcid = n.attribute("orcid").map(crate::ids::convert_orcid);
        if orcid
            .as_ref()
            .is_some_and(|s| !crate::ids::is_valid_orcid(s))
        {
            return Err("invalid ORCID".into());
        }
        Ok(Self {
            id: n.attribute("id").map(str::to_owned),
            orcid,
            openreview: n.attribute("openreview").map(str::to_owned),
            name: {
                let mut name = parse_name(n)?;
                name.script = None;
                name
            },
            variant: if variants.is_empty() {
                None
            } else {
                Some(variants.remove(0))
            },
            additional_variants: variants,
            affiliation: n
                .children()
                .rfind(|c| c.has_tag_name("affiliation"))
                .and_then(|c| c.text())
                .map(str::to_owned),
        })
    }

    /// Normalize a detached copy of the credit, preserving identity fields and
    /// variants.
    pub fn normalized(&self, casing: bool, latex: bool) -> Result<Self, String> {
        let mut result = self.clone();
        if latex {
            result.name = result.name.latex_normalize()?;
        }
        if casing {
            result.name = result.name.case_normalize(false);
        }
        Ok(result)
    }
}
