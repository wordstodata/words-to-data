# A source is parsed once: a bill is a document, not a bag of amendments

Status: accepted. Built in part — see **Order of work** at the end.

A bill's XML is read twice by two different parsers, for one reason: the first read throws away the structure the second read needs.

`build-dataset` parses a public law into `Bill { bill_id, amendments }`, where a `BillAmendment` carries a content hash, its action types, one flat `amending_text`, the word-level changes a model found, and that model's provenance. Nothing records **where in the bill the amendment sat**. `redesignations` needs exactly that, because a bill nests its instructions:

```text
Section 163(h)(3)(F) is amended--
  (1) in clause (i)--
        (B) by redesignating subclauses (III) and (IV) as subclauses (IV) and (V)
  (2) by striking clause (ii) and redesignating clauses (iii) and (iv) as ...
```

The first redesignation is inside clause (i). The second is not, because item (2) closed that scope. Flattened to one string the two read alike. So the command takes `--bill-xml` and reads the file again.

The second read recovers nothing the bill did not already give us. It agrees with the stored bill on identity — `amendment_id_around` calls the same `compute_amendment_id` — and measurement over `119-hr-1` confirms it: all 88 amendment ids behind the 89 redesignation links are already keys of the stored bill. The second parse exists to recover what the first one dropped.

**The cause was deeper than a missing field.** `uslm::parser::parse` on a public law returned a root and nothing else: a `pLaw` root holds `<main>`, the parser stepped into `<main>` only for a `uscDoc`, so the section and the ten titles below it were dropped (#114). The dataset had never held a bill's structure at all. The parser holds it now; storing it is the rest of step 4 below.

## The decision

A bill is a document. One read of the XML produces one stored document, and every later reader — the rule reader, the model reader, a review tool nobody has written yet — reads the dataset.

Four parts follow from it:

**A bill is a Work with an Expression**, in the same collection as the Code. Court opinions already enter this way, keyed by work and `date_filed`, and ADR 0006 made a node class-neutral so that a US Code document is one class among others rather than the shape of the core. A bill needs no new collection to live in.

**The enacted quoted text lives in the bill node's class payload, not in the hierarchy.** `#86` settled that quoted amendment text must not enter the tree as provisions — it is not law in force, and as a provision it becomes a searchable, diffable location an annotation can name. But `extract-changes` needs those words, and `BillAmendment.amending_text` carries them today. If the tree drops them, the record is split between a tree and a flat string, and "parse once" becomes "parse once and keep a copy".

**An amendment keeps its content hash as its identity, and gains a path.** ADR 0001 is exactly this distinction: a structural path locates and does not identify. A bill's path also moves when a publisher renumbers a title, while `sha256(bill_id:amending_text)` survives a rebuild — which is what `Link::id` and the 928 existing `legislature.amended_by` links rely on.

**A statement this build cannot place is recorded at the bill's own path.** Most of the unplaced statements in `119-hr-1` carry no US Code path, precisely because no Code path could be made. Once the bill is a document, every one has a path into the bill, which points at the words that defeated the reader.

## Consequences

**`redesignations` drops `--bill-xml` and `--bill-id`.** Its flags become a span, and `build-dataset` records the links itself as part of loading a bill (#150).

**`works_between` will list every bill as skipped.** It walks every work in the reader, so `--between 2025-07-18 2025-07-30` will report "Skipping 5 work(s) not held at both dates: bill/119-hr-1, …". That is a true statement made in a misleading place, and it belongs to #148 — a dataset cannot declare which document classes it holds, so a command cannot ask for the Code alone. We accept the noise rather than grow a second collection to hide it.

**A reader that wants the bill's words no longer needs the Congress cache.** Today a dataset is not self-contained for this question: the answer lives in a file beside it.

## Step 5 needed no schema break, and this ADR is why

Triage concluded that storing unplaced statements meant a new SQLite table or an eleventh compact field, and `SCHEMA_VERSION` going up. That rested on a premise this ADR removed: at the time, the dataset did not hold the bill, so the words of an unplaced statement lived only in the XML and had to be put somewhere new.

Once step 4 landed, both halves of the record were already stored. The words and the path are in the bill's own document. What makes a statement **placed** is a `legislature.redesignated_as` link. So an unplaced statement is a statement with no link, and the reason is reproduced by resolving the same statements against the same windows — which `docs/adr/0010-two-readers-one-resolver-a-model-never-writes-a-path.md` already required of a rule reading.

Measured over `119-hr-1`, against the same bill each time:

| Dataset | Works | Statements | Not placed |
| --- | --- | --- | --- |
| title 26 | 2 | 57 | 31 |
| + title 7 | 3 | 57 | 26 |
| + titles 42, 20, 10, 12 | 7 | 57 | 19 |

Seven titles give 17. The count falls while the bill does not change, so an unplaced statement is a fact about the bill **measured against the windows the dataset holds**, and not a fact about the bill. A row written when the bill was loaded becomes false as soon as `add-release-points` runs (#180), and nothing re-sweeps.

The derivation costs about 0.12 s for each window, so about 7 s over the 57 works of the full Code — against a compact file of that corpus that is 982 MB and costs far more than that to open.

## Considered and rejected

**Add the position to `BillAmendment`.** A section field and a list of container steps, filled in at parse time. It fixes the redesignation case and nothing else: the bill's structure stays unstored, so the next reader with a new question opens the XML again. It also stores a reading of the bill while the bill itself remains unread, which inverts ADR 0007 — the record is what was said, and what the bill said is its words in their structure.

**Let `build-dataset` call `record_redesignations` with the XML it fetched.** This is what #150 proposed, and it is the first step below, because it stops the dataset from silently lacking 90 links. It is not this ADR's answer, because it does not parse once — it keeps two parsers of one file and moves the duplication inside one command, where it is harder to see.

**Store an unplaced statement in the bill node's class payload.** Non-breaking, and wrong: a payload holds what the source said, and this is a fact about our reading of the source measured against a dataset that grows. It would make the bill's own record change because a different work arrived.

**Store unplaced statements in their own table or compact field.** Honest about the shape, and it stores a derivation that goes stale — 31 rows against title 26 alone, 17 against the whole Code, for one unchanging bill. It also costs `SCHEMA_VERSION` for something the file already holds.

**A separate collection for bills.** Keeps the Code's works pure and avoids the `works_between` noise above. Rejected: it would restate the class-neutrality that ADR 0006 already bought, and the noise is a symptom of #148 rather than a reason to duplicate a concept.

## Order of work

Recorded here because the sequence was a decision, and because the middle of it is where a reader will find the code disagreeing with this ADR.

| | Step | State |
| --- | --- | --- |
| 1 | `build-dataset` records redesignations, through the XML it holds (#150) | built |
| 2 | Both ends checked, and the words at them recorded as `Corroboration` | to do |
| 3 | The diff walks into a moved pair, so a rewrite inside it is reported | to do |
| 4 | A bill becomes a document (#114) — this ADR | built (#196): the bill is a work with an expression, an amendment has a path, and `--bill-xml` is gone |
| 5 | Unplaced statements reported, where an agent can read them | built (#153): a derivation, and no schema break — see below |
| 6 | The model reader (ADR 0010) | to do |

Step 1 wires `build-dataset` to the XML, and step 4 rewrites that wiring to read the stored bill. The rework is a few lines, and it buys a correct dataset four steps earlier.
