//! Cross-record validation. Parsing and schema validation live in their own
//! modules.

use super::Anthology;
use crate::error::diagnostic;
use crate::{Diagnostic, DiagnosticKind, NameSpecification, model::Extensions};
use std::path::Path;

impl Anthology {
    pub(super) fn validate(&self) -> Vec<Diagnostic> {
        let mut validator = Validator {
            anthology: self,
            diagnostics: Vec::new(),
        };
        validator.collections();
        validator.directories();
        validator.directory_extensions();
        validator.diagnostics
    }
}

struct Validator<'a> {
    anthology: &'a Anthology,
    diagnostics: Vec<Diagnostic>,
}

impl Validator<'_> {
    fn reference(&mut self, file: &Path, entity: &str, message: String) {
        self.diagnostics.push(diagnostic(
            file,
            Some(entity.into()),
            DiagnosticKind::Reference,
            message,
        ));
    }

    fn credit(&mut self, file: &Path, entity: &str, credit: &NameSpecification) {
        if let Some(id) = &credit.id {
            if id != crate::types::NO_PERSON_ID && !self.anthology.people.contains_key(id) {
                self.reference(file, entity, format!("unresolved explicit person ID {id}"));
            }
        }
    }

    fn extensions(&mut self, file: &str, entity: &str, extensions: &Extensions) {
        for field in extensions.keys() {
            self.diagnostics.push(diagnostic(
                file,
                Some(entity.into()),
                DiagnosticKind::UnknownField,
                format!("unknown JSON field {field}"),
            ));
        }
    }

    fn collections(&mut self) {
        for (file, source) in &self.anthology.sources {
            let c = &self.anthology.collections[&source.id];
            for v in &c.volumes {
                for id in &v.metadata.venues {
                    if !self.anthology.venues.contains_key(id) {
                        self.reference(file, v.id.as_str(), format!("unknown venue {id}"));
                    }
                }
                for id in &v.metadata.sigs {
                    if !self.anthology.sigs.contains_key(id) {
                        self.reference(file, v.id.as_str(), format!("unknown SIG {id}"));
                    }
                }
                for credit in &v.metadata.editors {
                    self.credit(file, v.id.as_str(), credit);
                }
                for p in &v.papers {
                    for credit in p.authors.iter().chain(&p.editors) {
                        self.credit(file, p.id.as_str(), credit);
                    }
                }
            }
            if let Some(e) = &c.event {
                for id in e.colocated.iter().flatten() {
                    if !self.anthology.indices.volumes.contains_key(id) {
                        self.reference(file, &e.id, format!("unknown colocated volume {id}"));
                    }
                }
                for t in &e.talks {
                    for credit in &t.speakers {
                        self.credit(file, &e.id, credit);
                    }
                }
            }
        }
    }

    fn directories(&mut self) {
        for (id, p) in &self.anthology.people {
            for related in &p.similar {
                if !self.anthology.people.contains_key(related) {
                    self.reference(
                        Path::new("json/people.json"),
                        id,
                        format!("unknown similar person {related}"),
                    );
                }
            }
        }
        for (id, v) in &self.anthology.venues {
            for related in &v.related_ids {
                if !self.anthology.venues.contains_key(related) {
                    self.reference(
                        Path::new("json/venues.json"),
                        id,
                        format!("unknown related venue {related}"),
                    );
                }
            }
        }
        for (id, s) in &self.anthology.sigs {
            for related in &s.related_ids {
                if !self.anthology.sigs.contains_key(related) {
                    self.reference(
                        Path::new("json/sigs.json"),
                        id,
                        format!("unknown related SIG {related}"),
                    );
                }
            }
            for venue in &s.venue_ids {
                if !self.anthology.venues.contains_key(venue) {
                    self.reference(
                        Path::new("json/sigs.json"),
                        id,
                        format!("unknown associated venue {venue}"),
                    );
                }
            }
        }
    }

    fn directory_extensions(&mut self) {
        for (id, p) in &self.anthology.people {
            self.extensions("json/people.json", id, &p.extensions);
            for n in &p.names {
                self.extensions("json/people.json", id, &n.extensions);
            }
        }
        for (id, v) in &self.anthology.venues {
            self.extensions("json/venues.json", id, &v.extensions);
        }
        for (id, s) in &self.anthology.sigs {
            self.extensions("json/sigs.json", id, &s.extensions);
            for m in &s.external_meetings {
                self.extensions("json/sigs.json", id, &m.extensions);
            }
        }
    }
}
