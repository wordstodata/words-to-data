# Links are stored, and identified by what they say

Status: accepted

`docs/adr/0002-links-live-in-the-core.md` put one `Link` type in the core: subject, namespaced kind, object, provenance. It is a projection. `Link::from_annotation` reads the `annotations` and `annotation_paths` tables and builds links as they are asked for, and nothing stores one. A schema whose only link table carries columns for `bill_id` and `amendment_id` holds exactly one kind, so the promise of ADR 0002 — that another party defines `westlaw.headnote` and our reader carries it — cannot be kept by the storage beneath it.

We therefore store links directly. `annotations` and `annotation_paths` go, and `ChangeAnnotation` becomes a projection *out of* links rather than the thing they are projected from. This ADR records how a stored link is shaped and named, which are the parts that are expensive to change later.

## A target is a tag and a JSON value, with the queried parts promoted

`Target` variants no longer hold one value each: a provision holds a path, an expression holds a work and a date, an external reference holds a reference and a display string, and a change holds a work, a path, and two dates. A column per part does not fit a set that grows, so a target is stored as its variant tag beside the whole value as JSON.

JSON cannot be indexed into, and every query `LinkReader` answers reads into a target: by path, by expression pair, the distinct pair list, by kind, by namespace. Those parts are therefore promoted to their own indexed columns beside the JSON. The promoted columns are a denormalised index of the value, which is two places holding one fact, and ADR 0002 warns against exactly that. It is deliberate here and it is one-directional: the JSON is the record, the columns are derived from it on write, and nothing reads a promoted column as the truth.

A namespace gets no column of its own. It is a prefix of the kind, so one index on `kind` serves both queries.

## The subject of an amendment link is a change, not a provision

`annotations` is keyed by an expression pair, and `get_annotations(from, to)` is what coverage and validation are built on. A link whose subject is a bare provision path cannot answer that question, and the projection we ship today silently drops the pair.

So the core gains a target that names a change: a work, a path, and the two dates it changed between. This widens the closed set ADR 0002 lists, which is the point of the set being ours. It carries one work rather than two expression ids, because two copies of one work can disagree and after an edit one of them will; the public API still takes two expression ids and refuses a mismatch, as `compute_diff` already does.

Several amendments can cause one change. That is not a conflict to resolve: it is several links sharing a subject, and a reader asking what caused a change gets them all. The table therefore carries no uniqueness constraint on subject and kind.

## A link is identified by what it says, not by where it is stored

A link's identity is a content hash of its subject, kind, and object. A row id is meaningless outside one file, and datasets are rebuilt rather than migrated, so an identity that does not survive a rebuild cannot be pointed at, confirmed, or deduplicated. We already mint `amendment_id` as a content hash, so the mechanism is not new.

Provenance is not part of the hash. Re-running a model against a fact it has already stated updates one link instead of accumulating a second, which is what makes a rebuild idempotent. The cost is that two parties independently asserting the same fact collapse into one record, and only the surviving provenance says who. We accept that until a second asserter actually exists.

**A human-touched provenance survives a machine re-write.** When a write collides with a stored link whose verification is `HumanConfirmed`, `Disputed`, or `Refuted`, the stored provenance stays. Otherwise the new one replaces it. Without this rule, re-running the pipeline destroys human review, quietly, and the loss is invisible until someone looks for a confirmation that is no longer there.

## Facts that only one kind understands live in a payload the core never reads

An amending action, a free-text note, and a bill id have no honest home in the core. They are legislature concepts, and today the action is crammed into `Provenance.method` as a `Debug` string, which round-trips only because the enum happens to carry no data.

A link therefore carries a namespaced payload that the core stores, hands back unchanged, and never interprets. This is what ADR 0002 asks for — a reader preserves what it does not understand — made storable.

Two rules keep it from becoming a dumping ground. The core never reads it. And nothing needed to *report* a link may live there, because a reader that cannot read the payload must still be able to say what the link is, who said it, and how far it can be trusted.

A timestamp is the exception that proves the rule: every statement has a when, so it belongs in `Provenance` beside the source, not in a payload that only one extension can open.

## Consequences

`ChangeAnnotation` is reconstructed by grouping links on the amendment, the expression pair, and the source. Grouping without the source would merge two annotators' accounts of one amendment into a single record with one provenance, which loses who said what.

`LinkWriter::add_annotation` is replaced by `add_link` rather than kept beside it. Two ways to write one fact means the convenient one is used, and the convenient one can only express the single kind we own.

An external reference is fully qualified — the bill and the amendment, not the amendment alone — so a reader that does not know the legislature extension can still resolve what it points at. The `bill_id` column that exists today is a leftover from before the core and the extensions were separated.

`SCHEMA_VERSION` goes up. Datasets are rebuilt, never migrated, and `#68` makes the break loud.

## Considered and rejected

**Keep `annotations` and add a second table for other kinds.** The kind we own stays cheap and every other kind is a second-class citizen, which is the arrangement ADR 0002 exists to end.

**A column per target part.** Fits today and breaks on the next variant, and it makes the meaning of a column depend on the tag anyway.

**A UUID minted at write time.** Unique, and useless: it changes on every rebuild, so nothing outside the file can point at a link and no rebuild can tell an old fact from a new one.

**Provenance in the identity hash.** Nothing collapses and nothing is lost, but every re-run of the pipeline grows the table by the number of facts it restated, and a reader asking what caused a change gets the same answer many times over.

**A list of provenances on one link.** The most honest model, since several parties really can assert one fact. Rejected for now because every reader would handle a list to ask a single question, and no second asserter exists yet. This is the decision to revisit first when one does.
