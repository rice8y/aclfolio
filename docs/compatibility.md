# Compatibility

The Rust API follows the read-only behavior of the official Python `acl_anthology` **1.3.4** library. Administrative record editing, identity reassignment, ingestion and writes to authoritative metadata files are outside its scope. Detached construction, XML conversion, normalization, downloads and Git acquisition are supported. Rust applications do not need Python.

## Stored records and effective metadata

Stored records preserve source values. Snapshot-bound views apply inheritance, resolve relationships and supply defaults. Choose the API according to the representation you need:

- `RichText::plain_text()` retains raw TeX while normalizing whitespace; `as_text()` interprets supported TeX math and markup.
- `person()` and `people()` expose the stored people directory. `get_person()` and `person_index()` also include inferred identities. `resolve_person()` and `works()` follow explicit identity links; `CreditRef::resolve()` and `PersonRef` use the complete identity graph.
- Raw optional fields distinguish absent values from present empty strings. Effective accessors apply defaults, including the `1900-01-01` ingestion-date fallback.
- `EventGroup::definitions()` retains all explicit event definitions. `definition()`, `title()` and other effective methods apply the reference library's selection rules.

See the [data model](data-model.md) for record types and source-preservation semantics.

## Snapshot lifecycle

`Anthology::open` eagerly loads collections and their publication, venue, SIG and event relationships. Identity and citation-key indices initialize on demand. `reset_indices` clears those derived caches; `reload` creates a replacement snapshot. A failed load or reload does not alter an existing snapshot.

Rust APIs use `Result`, `Option`, enums and iterators to represent failures, absent values and query results. Diagnostics are returned to the caller rather than written through a global logger. Detached hypothetical inference does not change stored records.

## Validation

The default compatible policy retains unknown fields and reference inconsistencies and returns diagnostics. Strict policy rejects such diagnostics. An identity may be absent from the stored people directory while still being present in the inferred graph, so strict source validation can reject data that remains usable through the effective APIs.

Snapshot loading checks record structure and cross-record references. Detached constructors validate the fields required by that constructor. `schema::validate_xml` separately validates a complete XML document against the bundled Relax NG grammar.

## Citation rendering

The CSL renderer follows **citeproc-py 0.11.1**, including its supported CSL subset and error behavior. It does not provide full CSL specification coverage. `to_citation(Some(style_path))` accepts a custom CSL file; unsupported elements or invalid input can return an error.

Bundled locale, transliteration, LaTeX and language data support offline rendering. Component licenses and attribution are listed in [NOTICE](../NOTICE).
