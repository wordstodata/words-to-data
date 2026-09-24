# A Dataset lives in a database; JSON is serialization

Status: accepted. Described as the system is today, with each gap named and pointed at its issue.

A Dataset is kept in a **database** and changed there. A **W2D file** is that Dataset
serialized to one file, so it can be carried to another party. The two are not peers,
and the difference is not which is better: one is a place, the other is a writing-down.

## Why this needed deciding

The two forms had been argued about as competitors — which is the working store, which
is the shipped artifact — and the argument kept restarting because the vocabulary could
not hold the distinction. `CONTEXT.md` named the W2D file and gave it a purpose. It had
no word at all for the other form, so every sentence about it had to say "the SQLite
backend", and a thing with no name cannot have stated guarantees.

The code had already drawn the line and nobody had read it back:

```rust
/// On-disk serialization format for in-memory datasets.
///
/// SQLite is a backend, not a serialization format.
pub enum Format { Compact, Json }
```

## What a reader may rely on from each

Stated as it is today. Where a row is a gap rather than a property, it names the work.

| | W2D file | Database |
| --- | --- | --- |
| schema version checked, mismatch refused | yes | yes (`docs/adr/0003`) |
| survives an interrupted write | **no** — written whole (#186) | yes, under a transaction (#187) |
| grows in place | no — an append must name an `--output` (#180) | yes |
| the whole pipeline can run against it | yes | yes (#180, #195, #199) |
| one file you can hand to somebody | yes | yes |

**"Portable" does not separate them.** A SQLite file is one file and can be handed over
as easily as a JSON one. What separates them is that a W2D file is a writing-down that
does not depend on an engine to read.

**There is no specification for the W2D format.** `docs/adr/0002` obliges a reader to
"preserve it when it writes the file again", and calls silent loss the one failure a
portable format cannot have — an obligation on readers we have not yet given anyone
enough to implement. This ADR does not claim the W2D file is an open format. It is the
form we hand over. Writing the specification, or softening the claim, is open work.

## The pipeline reaches the database now, and that was plumbing

**This is done.** Every command takes either form, and `load::refuse_sqlite` is gone with
its last caller. What follows is what the work was, because the reasoning is what makes
the next such gap recognisable.

Three commands refused a SQLite file: `extract-changes`, `match-amendments` and
`redesignations` — the three that write back into a Dataset. `load::refuse_sqlite` said
they "cannot **yet** take a SQLite file", and the "yet" was accurate.

Inspection found no deeper reason. Almost everything those commands call was already
generic over the storage: `add_reply`, `add_link`, `record_redesignations`,
`compute_diff`, `annotated_paths`, `bill_document`, `list_bill_ids`. Exactly two methods
were stranded on `impl Dataset<InMemoryStorage>` — `add_changes_to_amendment` and
`set_amendment_provenance` — and all three commands hard-coded `Dataset::load(…,
Format::Compact)` followed by a whole-file `save`.

That was the same fault #180 met: `add_uslm_xml`, `add_uslm_folder` and `add_works_of`
were stranded the same way, and widening them to `impl<S: Storage>` is what let a Dataset
grow at all.

#195 widened `match-amendments` and `redesignations`. #199 did `extract-changes`, and
that one was not a widening: the two stranded methods changed one amendment at a time
through a map only the in-memory store has, and the trait's only rung between `get_bill`
and `add_bill` would have rewritten a whole bill for each change. `119-hr-1` holds 603
amendments in one bill. `LegislatureWriter::update_amendments` takes the whole reading in
one call instead, so a backend writes each bill once and a database writes under one
transaction. Measured on that bill: **45 ms for 603 amendments, against 27 s** for the
whole-bill rewrite per change.

**With one consequence that must be said plainly: accepting both forms does not close
the durability gap.** A W2D input still loads into memory and is written whole. The gap
closes only when the work is done against a database. That is the reason a user would
load a file in rather than work on it where it sits.

## Consequences

**Reading a W2D file stays fully supported.** Most inspect commands already run against
either form, and a recipient who wants to ask one question should not need a conversion
step first. The line is at **writing**: anything that changes a Dataset wants the
database.

**`convert-dataset` is two unlike things under one name.** `.sqlite → .json` writes a
Dataset out; `.json → .sqlite` puts one where it can be worked on. It defaults by
swapping the extension, which is the shape of a pair of peers and is part of why the two
forms read as competitors. Renaming it is not decided here — there is no evidence yet
about what that would cost a reader.

**`Format::Json` is not a domain concept.** It is raw JSON "for debugging/interop", read
by nothing but `save` and `load`. It gets no glossary entry and no guarantees. It is the
dump; the W2D file is not.

## Considered and rejected

**Two peer forms, neither primary.** The shape the code suggested and the framing this
decision started from. It fails because a store and a serialization are not the same kind
of thing, and treating them as alternatives is what made the question feel unanswerable.

**The W2D file as the working form, and the database as an export.** Closer to what the
code does today, since three pipeline steps run only against JSON. Rejected because that
state is an accident of unported plumbing rather than a decision, and because it puts the
commands that most need a transaction on the form that has none.

**Calling the database a "store" or a "backend" in the glossary.** Rejected on the
maintainer's call: SQLite is a database, and inventing a word for a thing that already
has one is precious. "Backend" is a word about architecture, which `CONTEXT.md` should be
free of.

Note this removed `database` from the `_Avoid_` list on **Dataset**. That entry was
insisting a Dataset is not a generic pile of material; "corpus" and "collection" carry
that on their own. It was never about where a Dataset is kept.
