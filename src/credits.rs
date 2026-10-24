//! Snapshot-bound author, editor and speaker credits.

use crate::{
    Anthology, NameSpecification, PaperRef,
    directories::TalkRef,
    navigation::VolumeRef,
    people::{IdentityError, PersonRef},
};

/// Publication or talk containing a credit.
#[derive(Debug, Clone, Copy)]
pub enum CreditParent<'a> {
    /// Paper author or editor.
    Paper(PaperRef<'a>),
    /// Volume editor.
    Volume(VolumeRef<'a>),
    /// Talk speaker.
    Talk(TalkRef<'a>),
}

/// A name specification with explicit parent and identity navigation.
#[derive(Debug, Clone, Copy)]
pub struct CreditRef<'a> {
    parent: CreditParent<'a>,
    credit: &'a NameSpecification,
}

impl std::ops::Deref for CreditRef<'_> {
    type Target = NameSpecification;

    fn deref(&self) -> &NameSpecification {
        self.credit
    }
}

impl<'a> CreditRef<'a> {
    /// Containing publication/talk.
    pub fn parent(self) -> CreditParent<'a> {
        self.parent
    }

    /// Owning snapshot.
    pub fn root(self) -> &'a Anthology {
        match self.parent {
            CreditParent::Paper(p) => p.root(),
            CreditParent::Volume(v) => v.root(),
            CreditParent::Talk(t) => t.root(),
        }
    }

    /// Resolve this publication credit through the complete person index.
    pub fn resolve(self) -> Result<PersonRef<'a>, IdentityError> {
        self.root().resolve(self.credit)
    }

    /// Original name specification.
    pub fn stored(self) -> &'a NameSpecification {
        self.credit
    }

    /// Given name component.
    pub fn first(self) -> Option<&'a str> {
        self.credit.name.first.as_deref().filter(|s| !s.is_empty())
    }

    /// Effective name after the reference XML constructor's empty-component
    /// normalization.
    pub fn name(self) -> std::borrow::Cow<'a, crate::Name> {
        effective_name(&self.credit.name)
    }

    /// Effective variants, retaining source order and script metadata.
    pub fn variants(self) -> impl Iterator<Item = std::borrow::Cow<'a, crate::Name>> {
        self.credit.variants().map(effective_name)
    }

    /// Family name component.
    pub fn last(self) -> &'a str {
        &self.credit.name.last
    }
}

fn effective_name(name: &crate::Name) -> std::borrow::Cow<'_, crate::Name> {
    if name.first.as_deref() == Some("") {
        let mut normalized = name.clone();
        normalized.first = None;
        std::borrow::Cow::Owned(normalized)
    } else {
        std::borrow::Cow::Borrowed(name)
    }
}

impl<'a> PaperRef<'a> {
    /// Explicit author and editor credits, excluding inherited editor credits.
    pub fn namespec_credits(self) -> impl Iterator<Item = CreditRef<'a>> {
        self.namespecs().map(move |credit| CreditRef {
            parent: CreditParent::Paper(self),
            credit,
        })
    }

    /// Authors with parent and person navigation.
    pub fn author_credits(self) -> impl ExactSizeIterator<Item = CreditRef<'a>> {
        self.stored().authors.iter().map(move |credit| CreditRef {
            parent: CreditParent::Paper(self),
            credit,
        })
    }

    /// Effective editors; inherited credits retain their volume parent.
    pub fn editor_credits(self) -> impl ExactSizeIterator<Item = CreditRef<'a>> {
        let parent = if self.stored().editors.is_empty() {
            CreditParent::Volume(self.volume_ref())
        } else {
            CreditParent::Paper(self)
        };
        self.editors()
            .iter()
            .map(move |credit| CreditRef { parent, credit })
    }
}

impl<'a> VolumeRef<'a> {
    /// Editors with parent and person navigation.
    pub fn editor_credits(self) -> impl ExactSizeIterator<Item = CreditRef<'a>> {
        self.namespecs().iter().map(move |credit| CreditRef {
            parent: CreditParent::Volume(self),
            credit,
        })
    }
}

impl<'a> TalkRef<'a> {
    /// Speakers with parent and person navigation.
    pub fn speaker_credits(self) -> impl ExactSizeIterator<Item = CreditRef<'a>> {
        self.stored().speakers.iter().map(move |credit| CreditRef {
            parent: CreditParent::Talk(self),
            credit,
        })
    }
}

impl CreditRef<'_> {
    /// Resolve even an unindexed talk speaker, preserving inferred variants in an
    /// owned result.
    pub fn resolved_person(self) -> Result<crate::people::ResolvedPerson, IdentityError> {
        self.root().resolve_credit(self.credit)
    }
}
