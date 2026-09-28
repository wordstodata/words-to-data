# Words To Data - Convert Legal Documents Into Diffable Data Structures

[![CI](https://github.com/Scronkfinkle/words-to-data/actions/workflows/ci.yml/badge.svg)](https://github.com/Scronkfinkle/words-to-data/actions/workflows/ci.yml)

## Overview

`words_to_data` parses US Code titles and Public Laws (bills) from USLM XML format, providing structured access to legislative text, the ability to track what changed between two readings of one document, and tools for annotating how bills amend existing law.

Written in Rust.

## Features

- **Dataset-centric workflow** - Manage legal documents, bills, and annotations in a single structure
- **Work-scoped storage** - A document is keyed by what it is and when it was published, so documents that share no release cycle can sit in one dataset
- **Parse USC and Public Law documents** - Extract hierarchical structure from USLM XML files
- **Rich text content** - Capture heading, chapeau, proviso, content, and continuation fields
- **Bill amendment extraction** - Identify USC references and amending actions from bills
- **Hierarchical diffing** - Compute word-level differences between two expressions of one work
- **Congress data integration** - Fetch bill metadata and text from Congress.gov API
- **Court opinions** - Store an opinion from CourtListener beside the statutes it construes, as one node, with the field its text came from recorded
- **U.S. Code citations as links** - Read the citations out of an opinion's text and record each as a `judicial.cites` link, with the matched text as evidence

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
words-to-data = "0.3.0"
```

## Getting Data
- Title data: https://uscode.house.gov/download/download.shtml
- Bill data: https://congress.gov

## Build a Dataset: The Five Steps

Five commands turn an empty directory into a finished dataset. Run them in this
order:

```
build-dataset → add-classifications → link-by-evidence → [redesignations] → convert-dataset
```

**No step calls a model.** The old pipeline had three more commands:
`extract-changes`, `score-amendments` and `match-amendments`. Two of them sent
requests to a model. They were removed in #252, and `link-by-evidence` does
their work from the bill's own markup
(`docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`). A
dataset built before #252 keeps the model links and the model replies it holds,
and every reader still reads them (`docs/adr/0005`).

A dataset that missed a step used to look complete. One rebuild wrote 889
`legislature.amended_by` links and no redesignation links at all, and nothing
reported it (#150).

The file now records **which method, at which version, ran over which window**,
for the two steps that write statements into a window — step 3 and step 4
(#182). It records the method and not the command: "`redesignations` has run
here" stays true for ever while the reading behind it changes underneath. `info`
prints it under **Methods run**, one line for each method and window, with the
number of works that line covers. A run is recorded once per work, so the record
holds one entry per work: `info --json` carries every one of them as
`method_runs`, and a reader that wants the works by name reads them there.

Build the CLI first:

```bash
cargo build --release
# The binary is target/release/words_to_data
```

### What each step costs

| Step | Calls a model | Cache | A second run costs |
| --- | --- | --- | --- |
| 1. `build-dataset` | no | `<user cache dir>/words_to_data` | bandwidth, on a cache miss |
| 2. `add-classifications` | no | `<user cache dir>/words_to_data` | nothing, while the cache holds the pages |
| 3. `link-by-evidence` | no | not applicable | nothing |
| 4. `redesignations` (for a re-run only) | no | not applicable | nothing |
| 5. `convert-dataset` | no | not applicable | nothing |

### Step 1 — `build-dataset`

```bash
words_to_data build-dataset \
  --uslm-dates 2025-07-18,2025-07-30 \
  --bills 119-hr-1 \
  dataset.json
```

The output path is positional, and it is the last argument here. `--bills` needs
the `CONGRESS_API_KEY` environment variable. Get a key from
https://api.congress.gov/sign-up/.

This step downloads each release point and keeps it in a cache. The default cache
directory is `<user cache dir>/words_to_data`, which is `~/.cache/words_to_data`
on Linux. Use `--cache-dir` for a different directory. The extracted files of one
release point use approximately 660 MB of disk. A later build reads the cache and
downloads nothing.

**This step records redesignations, after it has loaded everything.** Loading a
bill loads a bill; the renumberings it states are recorded by an explicit step
over a named window, and this command runs that step itself once every release
point and every bill is in (#181). At that point it knows every window it made,
which is the knowledge it did not have while it was loading. Over the two
committed release points, `build-dataset` writes **80**
`legislature.redesignated_as` links. Check the number with
`words_to_data info dataset.json`.

A dataset that **grew** instead of being built has to be given that step by
hand, with step 4. `words_to_data validate` names each bill and window that is
waiting for it.

### Step 2 — `add-classifications`

```bash
words_to_data add-classifications dataset.json --output dataset-classified.json
```

This step stores the OLRC's classification of each public law the dataset holds,
as `olrc.classified_from` links. Step 3 reads them as evidence, and never as the
reason for a link. See [What the OLRC classified](#what-the-olrc-classified--add-classifications)
below.

### Step 3 — `link-by-evidence`

```bash
# A SQLite dataset is changed in place.
words_to_data link-by-evidence dataset.sqlite

# A compact JSON dataset must be told where to write.
words_to_data link-by-evidence dataset.json --output dataset-linked.json
```

This step links each amendment of every public law to the change it made, from
the address the bill's markup names, the window after the law's enactment, and
the words the bill quotes. It writes each link as `legislature.amended_by`. It
takes no span: it reads every window after each law's enactment. Over the
committed corpus it links **458** of the **603** amendments of `119-hr-1`, as
**1185** links. See [Matching with no model](#matching-with-no-model--link-by-evidence)
below, and `residue` for the amendments it did not link.

### Step 4 — `redesignations` (for a grown dataset, or a re-run)

Step 1 runs this same step over every window it made, so the ordinary build path
does not include this command. Run it when the dataset **grew**: a release point
added after a bill makes a window nothing has been resolved against, and
`words_to_data validate` names each bill and window that is waiting. Run it also
to record the redesignations again without a rebuild — after a change to the
reader, for example — or to see the report for one named bill.

```bash
# A SQLite dataset is changed in place.
words_to_data redesignations dataset.sqlite \
  --bill-id 119-hr-1 \
  --between 2025-07-18 2025-07-30

# A compact JSON dataset must be told where to write.
words_to_data redesignations dataset.json \
  --bill-id 119-hr-1 \
  --between 2025-07-18 2025-07-30 \
  --output dataset-redesignated.json
```

This step takes either form as well, under the same rule as step 3.

`--bill-id` names which bill in the dataset to read. The command reads that
bill's own document, which step 1 stored, so nothing opens the Congress cache a
second time. A clause inside "in subsection (a)--" is about a different
provision from the same clause outside it, and the stored bill holds that
nesting. `docs/adr/0009-a-source-is-parsed-once-a-bill-is-a-document.md` records
the decision.

The command prints each statement that it cannot place. A statement that no
reader can turn into two paths is recorded, and never dropped.

**The link total it prints is the total the dataset holds.** One bill can state
one move in two clauses — `119-hr-1` does at 26 U.S.C. 163(j), in § 70341(a) and
§ 70341(c) — and a link is identified by what it says, so the two statements are
one link. Where that happens the command says how many renumberings merged, and
that no write was lost. A count that falls with no word about why reads as a lost
write, and a merge and a lost write need different work
([#220](https://github.com/wordstodata/words-to-data/issues/220)).

**Which two release points a redesignation is checked against is an open
question.** This command is told, with `--between` or `--from`/`--to`. Step 1
names every window the dataset holds. Nothing compares the bill's date with
those dates. Each work in the corpus holds three release points, so each work
offers two windows, and running the step over both places many of the same moves
twice. `contradictions` finds those, and it does not choose between them: the
choice is [#172](https://github.com/wordstodata/words-to-data/issues/172). Do not
read this document as an answer to it.

### Step 5 — `convert-dataset`

```bash
words_to_data convert-dataset dataset.json dataset.sqlite
```

The output argument is positional and optional. Without it, the command swaps the
extension of the input. The direction comes from the two extensions.

**Every step takes either form now** (#180, #195, #199). Step 1 always writes
compact JSON, whatever the output name. Steps 2, 3 and 4 read or write either
form, and a dataset they change is changed in place. So convert as soon as step
1 is done, and let the rest of the pipeline work on the database.

### What the finished dataset holds

`words_to_data info dataset.json` reports the links of each kind. A complete run
gives three kinds:

- `legislature.redesignated_as`, from step 1. The committed corpus gives 80.
- `olrc.classified_from`, from step 2. The three committed release points give 673.
- `legislature.amended_by`, from step 3. The three committed release points give 1185.

A count of zero for `legislature.redesignated_as` says that step 1 did not record
them. A count of zero for `olrc.classified_from` says that step 2 did not run. A
count of zero for `legislature.amended_by` says that step 3 did not run.

`info` also carries one line of renumbering counts, in this shape:

```text
Renumbering: 57 statement(s), 80 link(s), 17 not placed
```

Three numbers that measure three different things, and a reader adds none of
them: one clause can state fourteen renumberings. The last one is what the
corpus said and this build could not turn into two paths. The figures above are
`119-hr-1` measured over the seven titles it renumbers provisions in.

### What this build could not place — `redesignation-report`

```bash
words_to_data redesignation-report dataset.json
words_to_data redesignation-report dataset.json --bill-id 119-hr-1 --json
```

Read-only, and it takes either form. One row for each link, and one row for each
statement no reader placed. The weakest come first: the statements nothing
placed at all, then the placed ones from the least corroborated upwards, so a
reviewer reads the doubtful handful first.

Each row says which bill, where in the bill the words sit, which amendment, the
clause, which reader read it, whether it was placed, the reason when it was not,
the two paths when it was, the window the link came from, and the corroboration
figure. `--json` is what an agent reads, and the rows are stable, so two runs
over one dataset give one answer.

The window matters as soon as a dataset holds more than one. One method run over
two windows can place one move twice, and without the window the two rows are the
same row with two scores.

**It reads the dataset and nothing else** — no XML, and no model call. Nothing
about an unplaced statement is stored beside the links, because the words and
the path are already in the bill's own document and a stored row would go stale:
the same bill leaves 31 statements unplaced against title 26 alone and 17
against the whole Code.

### More than one link about one thing — `contradictions`

```bash
words_to_data contradictions dataset.sqlite
words_to_data contradictions dataset.sqlite --json
```

Read-only. It lists every subject the dataset holds more than one link about, in
two categories, because they are two different facts:

* **Duplication** — same subject, same object, links in **more than one window**.
  One method, run over two windows, placed one move twice.
* **Disagreement** — same subject, a **different** object. Two links that cannot
  both be true.

```text
Links read:   111
Duplication:  47 subject(s)
Disagreement: 0 subject(s)

Duplication — one subject, one object, links in more than one window:
  uscode/title_26/…/section_45X/subsection_c/paragraph_6/subparagraph_R [legislature.redesignated_as]
    2025-07-18 -> 2025-07-30  rule:bill_redesignation [amendingAction type=redesignate@1]  corroboration 1.00
      -> uscode/title_26/…/section_45X/subsection_c/paragraph_6/subparagraph_S
    2025-07-30 -> 2025-08-14  rule:bill_redesignation [amendingAction type=redesignate@1]  corroboration 0.48
      -> uscode/title_26/…/section_45X/subsection_c/paragraph_6/subparagraph_S
```

Every kind is grouped by the same rule, including a kind this build has never
seen. Grouping happens within one kind: an opinion citing a section and a bill
renumbering it say different things about one path.

**It writes nothing.** The contradiction is computed from what the dataset
already says, the links coexist, and none is stamped, preferred or deleted.
Which link to keep is a separate and open question, so nothing is ordered by the
corroboration figure — on a measured corpus the false link scored higher in five
pairs out of 64, worst case 0.22 against 0.71.

### Growing a dataset — `add-release-points`

A dataset does not have to be built again when one more release point comes out.

```bash
# A SQLite dataset grows in place.
words_to_data add-release-points dataset.sqlite --uslm-dates 2025-08-13

# A compact JSON dataset must be told where to write.
words_to_data add-release-points dataset.json --uslm-dates 2025-08-13 \
  --output dataset-2025-08-13.json
```

**A compact JSON dataset is never written back over its input.** The file is
written whole, so a write that stopped part way would destroy the dataset it was
growing, together with every model call in it. A run without `--output` therefore
refuses and says so. A SQLite dataset grows in place, under a transaction, which
is the store giving the guarantee the JSON form cannot. `add-opinions` follows
the same rule.

The command reads the same mirror and the same cache as step 1, and `--offline`
reads only the cache. It adds release points and runs no step over them, so it
ends by naming each window it made and what that window holds. A window holding
no link has had no step run over it, **or** had one that found nothing. The two
are now told apart by the record of what ran: a window that a method covered
holds a `method_runs` entry naming that method and its version, whether or not
the method found anything to write (#182).

**Run the window steps again after the dataset grows.** Step 4 takes a
`--between` span, and the run names the span to give it. Step 3 takes no span,
and the run names it too. A redesignation is recorded by a step over a named
window, so a bill in a dataset that grew holds no link into the new window until
step 4 runs over it (#181).

**`validate` says which of those steps is outstanding, for redesignations.** It
names each bill and window where the bill states renumberings, the window could
hold them, and the dataset holds no link — and it prints the command that closes
the gap. It says nothing about a statement no reader could place, because that is
finished work with a reason rather than a step nobody has run (#183). The list is
read out of the links the dataset holds, and nothing is stored.

### What the OLRC classified — `add-classifications`

The Office of the Law Revision Counsel publishes, for each session of Congress,
which Code section each section of a public law was classified to, and the kind
of change. This command stores that table for every public law a dataset holds,
as `olrc.classified_from` links. It calls no model.

```bash
words_to_data add-classifications dataset.sqlite
```

Each link says: this Code section (the subject, as its structural path) was
classified from this section of the law (`olrc.classification:119-21:71301(a)`).
It is `Asserted`, its source names the table page, and its payload holds the
table's description (`new`, `nt new`, blank for amended, and the rest). The table
stops at the section, and a law it does not list is not a law that changed
nothing. A row the dataset cannot address is printed with its reason.

The tables are fetched from the OLRC and cached. A page the cache holds is never
fetched again, and `--offline` reads only the cache. What the table says, and how
each row is read, is in the `words_to_data::olrc` module documentation.

### Matching with no model — `link-by-evidence`

This command links each amendment of every public law the dataset holds to the
change it made, and calls no model
(`docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`).

```bash
words_to_data link-by-evidence dataset.sqlite
words_to_data link-by-evidence dataset.json --output dataset-linked.json
```

For each amendment it reads three things the dataset already holds:

1. **The address.** The section, and the provision below it, that the bill's
   markup names (`amendment-addresses` prints them).
2. **The window.** The first window after the law's enactment date in which
   something under the address changed. A later window that changed too is
   named in the evidence, and is never a second link.
3. **The words the bill quotes.** A struck string is in a change's before text,
   an inserted string is in its after text, and an enacted block is the text of
   an added provision. The changes under one section are given to the
   amendments addressed there all together, so one amendment does not take
   another's change. A provision edited in place can carry the edits of
   several amendments, and each that shows words of its own there is linked
   to it. A tie that the words cannot break is left alone.

   A law's amendments act in order, and a later one can insert words inside
   an earlier one's words. So a quoted string, or an enacted block, is also
   read with the words that later amendments of the same law inserted taken
   out.

   Common words alone — `"and"`, `", or"`, `"the"` — decide nothing, unless
   they are all the change struck or inserted. A struck "and" shows in every
   list whose end moved.

Each change becomes one `legislature.amended_by` link, in the shape
`match-amendments` wrote before it was removed (#252), with the method `address, window and quoted
words@2`. Its evidence says the address, the window, the words that placed it,
and what the OLRC classification (if `add-classifications` has run) says of the
section. A note in the classification never counts.

An amendment it cannot link is not stored. The stage it stopped at, and why, is
worked out again whenever it is asked for, because a stored reason goes false
the moment someone links the amendment. The quoted strings are read from the
stored bill, so a dataset built before this command existed must be rebuilt to
carry them. When a law's own words strike or insert quoted strings and none is
stored, the command prints a warning that names the law and says to rebuild
the dataset with `build-dataset`. It still links, and it links fewer of that
law's amendments.

### What is left — `residue`

This command lists every amendment of a public law that no
`legislature.amended_by` link names, from any source: `link-by-evidence`, an
agent through `link-amendment`, or `match-amendments` in a dataset built before
that command was removed (#252). It stores nothing, so
an amendment leaves the list as soon as a link names it.

```bash
words_to_data residue dataset.sqlite --bill 119-hr-1
words_to_data residue dataset.sqlite --json
```

Each row gives the stage the evidence method stopped at (`address`, `window` or
`resolve`), the reason, the address, the window and the changes under the
address. It also gives the OLRC's classification of the amendment's section of
the law, when `add-classifications` has run. Each row is in one category:

- **work** — something is left to resolve. An agent starts here.
- **unwritten** — the method links the amendment, and `link-by-evidence` has
  not written the link.
- **not held** — the amendment changes a table of sections, or the OLRC
  classifies its section of the law only as a note or as the heading before a
  section. The dataset holds none of these, so this is not a miss.
- **quiet** — the Code holds the address, and nothing under it changed after
  the law's enactment. Usually the dataset does not yet reach the date the
  amendment takes effect. This is not work.

The human output gives the counts for every row, then a screenful of rows, work
first, and says how many it did not show. `--json` gives every row.

## Quick Start

### Dataset Workflow

The `Dataset` is the primary abstraction for working with legal documents over time. It holds expressions, bills, and annotations together.

A **work** is a document as a concept, with no date: `uscode/title_26`. An **expression** is that work as it read on one date, written `uscode/title_26@2025-07-18`. A dataset is keyed by expression, so documents that share no release cycle — court opinions, state codes — sit side by side without pretending to.

```rust
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::uslm::bill_parser::parse_bill_amendments;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metadata = DatasetMetadata {
        name: "Tax Code Changes".to_string(),
        description: "Tracking Title 26 changes".to_string(),
        author: "Author".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    };
    let mut dataset = Dataset::new(metadata);

    // Add documents. Each file becomes one expression per work it holds.
    dataset.add_uslm_xml("path/to/old.xml", "2025-07-18", Some("Before".into()))?;
    dataset.add_uslm_xml("path/to/new.xml", "2025-07-30", Some("After".into()))?;

    // Add bill
    let bill = parse_bill_amendments("119-21", "path/to/bill.xml")?;
    dataset.add_bill(bill)?;

    // Diff two expressions of one work
    let title_26 = WorkId::new("uscode/title_26");
    let before = ExpressionId::new(title_26.clone(), "2025-07-18");
    let after = ExpressionId::new(title_26, "2025-07-30");
    let diff = dataset.compute_diff(&before, &after)?;

    // Navigate to specific section
    if let Some(s174a) = diff.find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a") {
        for change in &s174a.changes {
            println!("{:?}: {} → {}", change.field_name, change.old_value, change.new_value);
        }
    }

    // Save dataset
    dataset.save("my_dataset.json", Format::Compact)?;
    Ok(())
}
```

An `ExpressionId` parses from the same form it prints, so it can come straight off a command line or out of a citation:

```rust
use words_to_data::dataset::ExpressionId;

let id: ExpressionId = "uscode/title_26@2025-07-18".parse()?;
assert_eq!(id.to_string(), "uscode/title_26@2025-07-18");
```

### Download from Congress API

Bills can be automatically fetched with additional metadata from the congress.gov API

```rust
use words_to_data::congress::CongressClient;
use words_to_data::dataset::{Dataset, DatasetMetadata};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with API key (get from https://api.congress.gov/sign-up/)
    let client = CongressClient::new(std::env::var("CONGRESS_API_KEY")?);

    // Download bill data (XML + sponsors + members)
    let download = client.download_bill("119-hr-1")?;

    // Create dataset and load bill
    let metadata = DatasetMetadata {
        name: "HR 1 Analysis".to_string(),
        description: "Tracking HR 1 amendments".to_string(),
        author: "Author".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    };
    let mut dataset = Dataset::new(metadata);

    // Load bill into dataset (parses XML, stores sponsors/members)
    let bill_id = dataset.load_bill_download(&download)?;
    println!("Loaded bill: {}", bill_id);

    // Access bill data
    let bill = dataset.get_bill(&bill_id).unwrap();
    println!("Amendments: {}", bill.amendments.len());

    Ok(())
}
```

## Core Concepts

### Dataset

The `Dataset` is the primary abstraction for working with versioned legal documents:

- **DatasetMetadata**: Name, description, author, license, version
- **WorkId**: A document as a concept, with no date — `uscode/title_26`
- **ExpressionId**: That work as it read on one date — `uscode/title_26@2025-07-18`
- **Expression**: One expression's tree, plus an optional label
- **Bills**: Parsed bill data with extracted amendments
- **Annotations**: Links diff paths to bill amendments with verification status

Use `Dataset` to load documents, compute diffs, and track which amendment caused each change. Ask `works()` what documents it holds and `expressions(&work)` when each was published; `scope()` reports both together, so an empty result can be answered with "out of scope" rather than "not found".

### Document nodes

Documents are represented as trees of `DocumentNode` structures. Each node contains:

- **NodeData**: Its path, its type, its date, its text, and where the text came from
- **Children**: Nested child nodes forming the document hierarchy

A node says nothing about which class of document it belongs to beyond its type,
which is an open namespaced string — `uscode.section`, `judicial.opinion`. The
facts only one class understands travel beside it in a payload the core stores and
never reads: `words_to_data::uslm::UslmFacts` reads the US Code's,
`words_to_data::judicial::OpinionFacts` reads a court opinion's. A whole document
is one node where nothing has taken it apart, which is how a court opinion is
stored. See `docs/adr/0006-a-document-node-is-class-neutral.md`.

The library uses two types of paths:

1. **Structural Path**: Full hierarchy including all nodes
   Example: `uscode/title_26/subtitle_A/chapter_1/section_174`

2. **USLM ID**: Official USLM identifier (excludes structural-only elements), in
   the `uscode` payload rather than in a core field
   Example: `/us/usc/t26/s174/a/1`

### Court opinions, and the question they answer

A court opinion goes into the same dataset as the statutes it construes. It is one
work with one expression, dated the day the court filed it, holding one node. Two
commands do the work:

```bash
# Fetch opinions from CourtListener and record their U.S. Code citations as links.
# Needs COURTLISTENER_API_KEY; --offline reads only what is already cached.
words_to_data add-opinions dataset.sqlite --opinions 109019,122262,406879

# Which cases cite a provision, and has it moved under them since?
words_to_data cases-citing dataset.sqlite --cites "26 U.S.C. § 174" --chain
```

The second is statutory research run backwards: not "what controls this point" but
"Congress amended this provision — which cases construing the old text can no
longer be relied on?" It reports the verification state of each citation link, the
printings it can compare, and — the part that makes it honest — the period between
the opinion and the earliest printing held, which it says nothing about:

```
Snow v. Commissioner (judicial/opinion_109019@1974-05-13)
  opinion text: courtlistener:opinion/109019:html_lawbox / markup / Asserted
  cites …/section_174 — link is MachineSuggested, matched "26 U. S. C. § 174"
    2025-07-18 → 2025-07-30: CHANGED, at 4 path(s): …
    1974-05-13 → 2025-07-18: OUT OF SCOPE. This dataset holds no printing of the
    cited work in that period, so it cannot say whether the provision changed in
    it. It is not a statement that nothing changed.
```

With `--chain` it carries on through `legislature.amended_by` to the amendment,
the bill, its sponsor and the roll call. See
`docs/research/a-court-opinion-in-the-core.md` and
`docs/adr/0008-an-opinions-text-is-one-named-field-chosen-for-fidelity.md`.

The opinion records come from [CourtListener](https://www.courtlistener.com/),
by Free Law Project, read through its API under its terms. The analysis above is
ours; Free Law Project has not produced, endorsed or verified it.

### Text Content Fields

Each node can contain up to five distinct text fields:

- **Heading**: Section or subsection title
- **Chapeau**: Opening text before enumerated items
- **Proviso**: Conditional or qualifying clauses
- **Content**: Main body text
- **Continuation**: Text appearing after child elements

### Diffs

The `TreeDiff` structure mirrors the node hierarchy and tracks:

- **Field changes**: Word-level differences in text content fields
- **Added nodes**: New child nodes in the newer version
- **Removed nodes**: Nodes that existed in the older version
- **Child diffs**: Recursive diffs for matching child nodes

Diffs are computed using word-level granularity via the `similar` crate.

### Amending Actions

The publisher's schema defines twelve amending actions (`uslm-2.0.17.xsd`, `AmendingActionTypeEnum`):

`enact`, `add`, `amend`, `substitute`, `redesignate`, `repeal`, `repealAndReserve`, `insert`, `delete`, `conform`, `noChange`, `unknown`

The five public laws in the cache use six of them: `insert`, `delete`, `amend`, `add`, `redesignate`, `repeal`.

`AmendingAction` is that list: one variant for each of the twelve values, and no value the publisher cannot emit. An action type this build does not know is reported, never dropped (#156).

## API Documentation

Generate and view the full API documentation:

```bash
cargo doc --open
```

### Development

```bash
# Run tests
cargo test
```
