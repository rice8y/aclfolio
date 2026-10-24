# Read-only metadata model

`Collection → Volume → Paper` is an ownership hierarchy. `PaperRef` provides a borrowed paper/volume pair. IDs and indices hold links without copying the full corpus. Cloning an `Anthology` shares collection allocations and clones its indices and directory records; sharing `Arc<Anthology>` is cheaper for concurrent readers.

| Source | Rust representation |
|---|---|
| Collection ID, volumes, optional event | `Collection` |
| Volume ID, type, explicit ingest date | `Volume` |
| Booktitle, shortbooktitle, ordered editors | `VolumeMetadata`, `RichText`, `NameSpecification` |
| Publisher, address, month, year, ISBN, DOI | Optional stored strings, required year |
| Volume PDF, URLs, SIGs, venues, journal volume/issue/title | Typed references, ordered strings |
| Paper ID, publication type, ingest date | `Paper` with `PaperId` |
| Title, abstract | Optional `RichText`; title absent only for front matter in validated snapshots |
| Bibkey, DOI, pages, issue, journal, language, month, note | Stored strings with presence preserved |
| Ordered authors/editors, affiliation, IDs, variants | `NameSpecification`, `Name` |
| PDF, URL, video, attachment, MRF | `PdfReference`, `UrlReference`, `Video`, `Attachment`, `Mrf` |
| Revision, erratum, award, removed, retracted | `Revision`, `Erratum`, `Award`, independent `Notice` options |
| Front matter | `Paper` flagged `frontmatter`; ID zero; no fabricated stored title |
| Event meta, links, talks, speakers, colocation | `Event`, `EventMetadata`, `Talk` |
| Event aggregation and derived venue/year links | `EventGroup`, `EventAssociation` |
| People names, ORCID, comment, degree, similar, matching flag | `Person` |
| Venue acronym/name/URL/description/type/flags/legacy letter/related IDs | `Venue` |
| SIG acronym/name/URL/description/related IDs/venues/external meetings | `Sig`, `ExternalMeeting` |

Raw optional scalar fields retain a present empty string as `Some("")`; absent values are `None`. The inferred identity index normalizes empty given names to absent, matching the reference name model. Repeating fields use vectors; zero occurrences have no distinct empty-container spelling, except explicit event `links` and `colocated` wrappers, which use `Option<Vec<_>>`. JSON optional list omission and empty lists share the same vector value, consistent with the directory contract. Unknown JSON keys are retained in `extensions` and diagnosed.

Mixed text is an ordered recursive enum, not stripped XML. Formatting element names, link targets, TeX, paragraph markers and their nested text remain accessible. Whitespace-only structural indentation is ignored by the entity parser; whitespace inside rich text is retained until projecting to plain text.

All dates remain source strings. Ingestion dates have calendar checks at runtime; full XML datatype conformance is a separate RNC validation step. Year and month are not coerced to a single calendar instant: months often contain ranges or names. Checksum fields are upstream eight-character CRC32 values. SHA-256 provenance hashes are a separate concept and never replace attachment/PDF checksums.

The raw `Person` directory remains available through `people()` and `person()`. `PersonIndex` builds the official identity graph, including variants and inferred unverified people. `PersonRef` navigates that graph; `CreditRef` binds a name specification to its paper, volume or talk. A cloned index supports hypothetical resolution without changing the snapshot. `Name` equality ignores script metadata and treats absent and empty given names equivalently, matching the reference.

`MarkupText` is the reference-compatible text wrapper. `RichText` is the stored mixed-content tree and supplies the same rendering conversions; `plain_text()` is an additional raw projection. XML conversion writes strings or detached elements, never authoritative files. `FileReference` represents effective PDF, thumbnail, attachment, video and URL references using `FileKind` and explicit `Config`.

The public snapshot exposes no mutable access. Owning record types are ordinary Rust structs for convenient export; editing a detached copy cannot change a snapshot.
