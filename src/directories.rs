//! Snapshot-bound venue, SIG and event directory navigation.

use crate::{
    Anthology, Collection, EventGroup, ExternalMeeting, Sig, Talk, UrlReference, Venue, VolumeId,
    navigation::VolumeRef, text::RichText,
};
use std::collections::BTreeMap;

/// A venue and its snapshot-derived associations.
#[derive(Debug, Clone, Copy)]
pub struct VenueRef<'a> {
    pub(crate) root: &'a Anthology,
    pub(crate) id: &'a str,
    pub(crate) value: &'a Venue,
}

impl std::ops::Deref for VenueRef<'_> {
    type Target = Venue;

    fn deref(&self) -> &Venue {
        self.value
    }
}

impl<'a> VenueRef<'a> {
    /// Directory identifier.
    pub fn id(self) -> &'a str {
        self.id
    }

    /// Owning snapshot.
    pub fn root(self) -> &'a Anthology {
        self.root
    }

    /// ACL sponsorship flag, defaulting to false.
    pub fn is_acl(self) -> bool {
        self.value.is_acl.unwrap_or(false)
    }

    /// Front-page flag, defaulting to false.
    pub fn is_toplevel(self) -> bool {
        self.value.is_toplevel.unwrap_or(false)
    }

    /// Volumes explicitly tagged with this venue.
    pub fn volumes(self) -> impl Iterator<Item = VolumeRef<'a>> {
        self.root
            .venue_volumes(self.id)
            .map(move |volume| VolumeRef {
                anthology: self.root,
                volume,
            })
    }

    /// Symmetric related-venue associations; missing directory targets are errors.
    pub fn related(self) -> Result<Vec<Self>, String> {
        let mut ids = std::collections::BTreeSet::new();
        ids.extend(self.related_ids.iter().map(String::as_str));
        ids.extend(
            self.root
                .venues()
                .filter(|(_, v)| v.related_ids.iter().any(|x| x == self.id))
                .map(|(id, _)| id),
        );
        ids.into_iter()
            .map(|id| {
                self.root
                    .get_venue(id)
                    .ok_or_else(|| format!("unknown related venue {id}"))
            })
            .collect()
    }

    /// SIG associations used as ingestion hints.
    pub fn sigs(self) -> Vec<SigRef<'a>> {
        self.root
            .sig_refs()
            .filter(|s| s.venue_ids.iter().any(|v| v == self.id))
            .collect()
    }
}

/// A SIG and its snapshot-derived associations.
#[derive(Debug, Clone, Copy)]
pub struct SigRef<'a> {
    pub(crate) root: &'a Anthology,
    pub(crate) id: &'a str,
    pub(crate) value: &'a Sig,
}

impl std::ops::Deref for SigRef<'_> {
    type Target = Sig;

    fn deref(&self) -> &Sig {
        self.value
    }
}

/// A meeting represented either by a volume ID or by external metadata.
#[derive(Debug, Clone, Copy)]
pub enum SigMeeting<'a> {
    /// Anthology volume.
    Volume(&'a VolumeId),
    /// External meeting.
    External(&'a ExternalMeeting),
}

impl<'a> SigRef<'a> {
    /// Directory identifier.
    pub fn id(self) -> &'a str {
        self.id
    }

    /// Owning snapshot.
    pub fn root(self) -> &'a Anthology {
        self.root
    }

    /// Volumes explicitly tagged with this SIG.
    pub fn volumes(self) -> impl Iterator<Item = VolumeRef<'a>> {
        self.root.sig_volumes(self.id).map(move |volume| VolumeRef {
            anthology: self.root,
            volume,
        })
    }

    /// Symmetric related-SIG associations.
    pub fn related(self) -> Result<Vec<Self>, String> {
        let mut ids = std::collections::BTreeSet::new();
        ids.extend(self.related_ids.iter().map(String::as_str));
        ids.extend(
            self.root
                .sigs()
                .filter(|(_, s)| s.related_ids.iter().any(|x| x == self.id))
                .map(|(id, _)| id),
        );
        ids.into_iter()
            .map(|id| {
                self.root
                    .get_sig(id)
                    .ok_or_else(|| format!("unknown related SIG {id}"))
            })
            .collect()
    }

    /// Associated venues; these hints do not imply volume membership.
    pub fn venues(self) -> Result<Vec<VenueRef<'a>>, String> {
        self.value
            .venue_ids
            .iter()
            .map(|id| {
                self.root
                    .get_venue(id)
                    .ok_or_else(|| format!("unknown venue {id}"))
            })
            .collect()
    }

    /// Internal and external meetings grouped by publication year.
    pub fn meetings_by_year(self) -> BTreeMap<&'a str, Vec<SigMeeting<'a>>> {
        let mut result: BTreeMap<&str, Vec<_>> = BTreeMap::new();
        for v in self.volumes() {
            result
                .entry(&v.stored().metadata.year)
                .or_default()
                .push(SigMeeting::Volume(&v.stored().id));
        }
        for m in &self.value.external_meetings {
            result
                .entry(&m.year)
                .or_default()
                .push(SigMeeting::External(m));
        }
        result
    }
}

impl Anthology {
    /// Look up a venue with snapshot navigation.
    pub fn get_venue(&self, id: &str) -> Option<VenueRef<'_>> {
        self.venues()
            .find(|(key, _)| *key == id)
            .map(|(id, value)| VenueRef {
                root: self,
                id,
                value,
            })
    }

    /// Iterate over all venues with snapshot navigation.
    pub fn venue_refs(&self) -> impl ExactSizeIterator<Item = VenueRef<'_>> {
        self.venues().map(|(id, value)| VenueRef {
            root: self,
            id,
            value,
        })
    }

    /// Look up a SIG with snapshot navigation.
    pub fn get_sig(&self, id: &str) -> Option<SigRef<'_>> {
        self.sigs()
            .find(|(key, _)| *key == id)
            .map(|(id, value)| SigRef {
                root: self,
                id,
                value,
            })
    }

    /// Iterate over all SIGs with snapshot navigation.
    pub fn sig_refs(&self) -> impl ExactSizeIterator<Item = SigRef<'_>> {
        self.sigs().map(|(id, value)| SigRef {
            root: self,
            id,
            value,
        })
    }
}

impl<'a> VolumeRef<'a> {
    /// Venues explicitly attached to this volume.
    pub fn venues(self) -> Result<Vec<VenueRef<'a>>, String> {
        self.volume
            .metadata
            .venues
            .iter()
            .map(|id| {
                self.anthology
                    .get_venue(id)
                    .ok_or_else(|| format!("unknown venue {id}"))
            })
            .collect()
    }

    /// SIGs explicitly attached to this volume.
    pub fn sigs(self) -> Result<Vec<SigRef<'a>>, String> {
        self.volume
            .metadata
            .sigs
            .iter()
            .map(|id| {
                self.anthology
                    .get_sig(id)
                    .ok_or_else(|| format!("unknown SIG {id}"))
            })
            .collect()
    }
}

/// Origin of an event's indexed colocation, matching the official EventLink
/// enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EventLink {
    /// Explicit colocation in XML.
    Explicit,
    /// Venue/year association.
    Inferred,
}

impl<'a> EventGroup<'a> {
    /// Snapshot containing this event.
    pub fn root(self) -> &'a Anthology {
        self.anthology
    }

    /// Effective definition, using the last explicit definition in collection
    /// order.
    pub fn definition(self) -> Option<&'a crate::Event> {
        self.definitions().last()
    }

    /// Owning collection selected by the official event-index algorithm.
    pub fn collection(self) -> Option<&'a Collection> {
        self.collections().last().or_else(|| {
            self.anthology
                .volume_refs(None)
                .find(|v| {
                    v.metadata
                        .venues
                        .iter()
                        .any(|id| format!("{id}-{}", v.metadata.year) == self.id())
                })
                .map(VolumeRef::collection)
        })
    }

    /// Effective title, generated from the venue for implicit events.
    pub fn title(self) -> Result<Option<RichText>, String> {
        if let Some(e) = self.definition() {
            return Ok(e.metadata.as_ref().and_then(|m| m.title.clone()));
        }
        Ok(self.id().rsplit_once('-').and_then(|(venue, year)| {
            self.anthology
                .venue(venue)
                .map(|v| RichText::from_string(&format!("{} ({year})", v.name), true))
        }))
    }

    /// Effective event location.
    pub fn location(self) -> Option<&'a str> {
        self.definition()?
            .metadata
            .as_ref()?
            .location
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// Effective event dates.
    pub fn dates(self) -> Option<&'a str> {
        self.definition()?
            .metadata
            .as_ref()?
            .dates
            .as_deref()
            .filter(|s| !s.is_empty())
    }

    /// Configured event landing page.
    pub fn web_url(self) -> String {
        let c = self.anthology.config();
        c.format(&c.event_page_template, self.id())
    }

    /// Effective event links, with last value winning for each role.
    pub fn links(self) -> BTreeMap<&'a str, &'a UrlReference> {
        self.definition()
            .into_iter()
            .flat_map(|e| e.links.iter().flatten())
            .map(|u| (u.kind.as_deref().unwrap_or("attachment"), u))
            .collect()
    }

    /// Talks on the effective explicit definition.
    pub fn talks(self) -> impl Iterator<Item = TalkRef<'a>> {
        self.definition()
            .into_iter()
            .flat_map(|e| &e.talks)
            .map(move |talk| TalkRef { event: self, talk })
    }

    /// Indexed colocations. Collection membership and venue/year links are inferred
    /// colocations.
    pub fn colocated_ids(self) -> BTreeMap<VolumeId, EventLink> {
        self.associations()
            .map(|(id, kind)| {
                (
                    id.clone(),
                    if kind == crate::EventAssociation::Explicit {
                        EventLink::Explicit
                    } else {
                        EventLink::Inferred
                    },
                )
            })
            .collect()
    }

    /// Volumes in the official event index, reporting dangling references.
    pub fn indexed_volumes(self) -> Result<Vec<VolumeRef<'a>>, String> {
        self.colocated_ids()
            .keys()
            .map(|id| {
                self.anthology
                    .get_volume(id.as_str())
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("unknown colocated volume {id}"))
            })
            .collect()
    }
}

/// A talk bound to its containing event.
#[derive(Debug, Clone, Copy)]
pub struct TalkRef<'a> {
    event: EventGroup<'a>,
    talk: &'a Talk,
}

impl std::ops::Deref for TalkRef<'_> {
    type Target = Talk;

    fn deref(&self) -> &Talk {
        self.talk
    }
}

impl<'a> TalkRef<'a> {
    /// Original talk metadata.
    pub fn stored(self) -> &'a Talk {
        self.talk
    }

    /// Parent event.
    pub fn event(self) -> EventGroup<'a> {
        self.event
    }

    /// Owning snapshot.
    pub fn root(self) -> &'a Anthology {
        self.event.root()
    }

    /// Owning collection.
    pub fn collection(self) -> Option<&'a Collection> {
        self.event.collection()
    }

    /// Attachment roles with the last value for repeated roles.
    pub fn attachments(self) -> BTreeMap<&'a str, &'a UrlReference> {
        self.talk
            .url
            .iter()
            .chain(&self.talk.additional_urls)
            .map(|u| (u.kind.as_deref().unwrap_or("attachment"), u))
            .collect()
    }
}

/// An existing indexed event or a detached event inferred for a hypothetical
/// venue/year.
#[derive(Debug)]
pub enum InferredEvent<'a> {
    /// Event already present in the snapshot, with all accumulated associations.
    Existing(EventGroup<'a>),
    /// A pure inferred value and its owning collection; it has no colocations yet.
    Detached {
        /// Event metadata generated from the venue and year.
        event: crate::Event,
        /// Parent collection.
        collection: &'a Collection,
    },
}

impl Anthology {
    /// Resolve an implicit event, optionally constructing a detached value if
    /// absent.
    /// Construction does not edit the snapshot or authoritative records.
    pub fn infer_event<'a>(
        &'a self,
        volume: VolumeRef<'a>,
        venue_id: &str,
        create: bool,
    ) -> Result<Option<InferredEvent<'a>>, String> {
        let id = format!("{venue_id}-{}", volume.year());
        if let Some(event) = self.get_event(&id) {
            return Ok(Some(InferredEvent::Existing(event)));
        }
        if !create {
            return Ok(None);
        }
        if !crate::ids::is_valid_event_id(&id) {
            return Err(format!("invalid event ID {id}"));
        }
        let venue = self
            .get_venue(venue_id)
            .ok_or_else(|| format!("unknown venue {venue_id}"))?;
        Ok(Some(InferredEvent::Detached {
            event: crate::Event {
                id,
                metadata: Some(crate::EventMetadata {
                    title: Some(RichText::from_string(
                        &format!("{} ({})", venue.name, volume.year()),
                        true,
                    )),
                    location: None,
                    dates: None,
                }),
                links: None,
                talks: vec![],
                colocated: None,
            },
            collection: volume.collection(),
        }))
    }
}
