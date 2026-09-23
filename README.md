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

## Build a Dataset: The Six Steps

Six commands turn an empty directory into a finished dataset. Run them in this
order:

```
build-dataset → extract-changes → score-amendments → match-amendments → [redesignations] → convert-dataset
```

A dataset that missed a step looks complete. The file keeps no list of the steps
that ran, so a missing step shows only as an absence. One rebuild wrote 889
`legislature.amended_by` links and no redesignation links at all, and nothing
reported it (#150). Read the counts that each command prints, and compare them
with the counts in this section.

Build the CLI first:

```bash
cargo build --release
# The binary is target/release/words_to_data
```

### What each step costs

| Step | Calls a model | Cache | A second run costs |
| --- | --- | --- | --- |
| 1. `build-dataset` | no | `<user cache dir>/words_to_data` | bandwidth, on a cache miss |
| 2. `extract-changes` | **yes** | `changes_cache.json`, beside the dataset | nothing, while that file stays there |
| 3. `score-amendments` | no | not applicable | nothing |
| 4. `match-amendments` | **yes** | `matches_cache.json`, beside the dataset | nothing, while that file stays there |
| 5. `redesignations` (for a re-run only) | no | not applicable | nothing |
| 6. `convert-dataset` | no | not applicable | nothing |

**Two steps call a model, and only two: `extract-changes` and `match-amendments`.
These two steps spend money. No other step sends a request to a model.**

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
hand, with step 5. `words_to_data validate` names each bill and window that is
waiting for it.

### Step 2 — `extract-changes` (calls a model)

```bash
words_to_data extract-changes dataset.json --threads 8
```

This step reads the word-level changes out of each amendment, and writes them
into the dataset. It sends one request for each amendment that carries no changes
yet. The five committed bills hold **606** amendments, so a cold run sends
approximately 606 requests.

The command speaks to an OpenAI-compatible chat-completions server. The default
is a local server at `http://localhost:8080`. For a hosted endpoint, give
`--base-url` and `--model`. Put the key in the `W2D_API_KEY` environment
variable: a key in `--api-key` goes into the shell history and into `ps`.

**Keep `changes_cache.json` beside the dataset.** The command writes this file
into the same directory as the dataset, and reads it at the start of each run. A
run that finds the cache sends no request for an amendment the cache holds. A run
that does not find the cache buys all of those replies again. `--no-cache` forces
a new request for each amendment, which is correct only when you know that the
replies must change.

The command writes the cache after each successful request, so you can stop a run
and start it again without a loss. A request that fails is not cached, and a
later run tries it again.

### Step 3 — `score-amendments` (no model)

```bash
words_to_data score-amendments dataset.json --between 2025-07-18 2025-07-30
```

This step compares each amendment against the US Code diff, and gives each pair a
similarity score. The calculation is deterministic, and the same dataset always
gives the same scores.

Use `--between FROM TO` for every work that both dates hold, or `--from` and
`--to` together for one named pair, such as
`--from uscode/title_26@2025-07-18 --to uscode/title_26@2025-07-30`.

The scores go to `similarity_scores.json` beside the dataset, or to the path in
`--output`. **That file is a report, and no command reads it.** Step 4 calculates
the same scores again from the dataset. So step 3 writes nothing into the
dataset, and a reader who skips it gets the same dataset. Run it to see which
candidates step 4 will offer the model, and at which cutoff. The two steps have
the same default cutoff of 0.4.

### Step 4 — `match-amendments` (calls a model)

```bash
# A SQLite dataset is changed in place.
words_to_data match-amendments dataset.sqlite --between 2025-07-18 2025-07-30

# A compact JSON dataset must be told where to write.
words_to_data match-amendments dataset.json --between 2025-07-18 2025-07-30 \
  --output dataset-matched.json
```

This step asks the model which change each amendment caused, and writes each
answer into the dataset as a `legislature.amended_by` link. It takes the same
span flags as step 3, and the same model flags as step 2.

It takes either form the dataset comes in. A SQLite dataset is changed in place,
under a transaction. A compact JSON dataset is written whole, so it is never
written back over its input, and a run without `--output` refuses before it
sends its first request (#186).

It sends a request for each amendment that has candidates and that no cached
reply answers. A cold run over the committed corpus sends approximately **656**
requests, one for each amendment with candidates across the 58 works.

**Keep `matches_cache.json` beside the dataset.** The command writes this file
into the same directory as the dataset, and reads it at the start of each run. A
run that finds the cache sends no request for an amendment that the cache
answers, and it prints the replies that it reused and the calls that it made. A
run that does not find the cache buys all of those replies again. `--no-cache`
forces a new request for each amendment, which is correct only when you know that
the replies must change.

The command writes the cache after each successful request, so you can stop a run
and start it again without a loss. A request that fails is not cached, and a
later run tries it again.

The cache holds each reply under two hashes: the candidates that the model saw,
and the prompt that the command sent them in. The command reuses a reply only
when both agree with the run that it makes now. A new prompt, or a new
`--similarity-cutoff`, is a different question, and it buys new replies.

**The cache also makes a rebuild reproducible.** Before the cache, the same commit
over the same sources gave 893 links on one run and 899 on the next (#123). A run
that reuses the cached replies writes the same links each time.

The command writes `candidates.json` beside the dataset. That file records the
question for a reader, and no command reads it.

**"Beside the dataset" means the directory, not the name.** Both files take a
fixed name in the directory the dataset sits in, so a `dataset.json` and the
`dataset.sqlite` it converts to share one cache and one `candidates.json`. For
the cache that is what you want, because a reply is keyed on the candidates and
the prompt rather than on the file they came from: convert the dataset and the
replies you already bought still answer. For `candidates.json` it means the
later run overwrites the earlier one's report.

### Step 5 — `redesignations` (for a grown dataset, or a re-run)

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

This step takes either form as well, under the same rule as step 4.

`--bill-id` names which bill in the dataset to read. The command reads that
bill's own document, which step 1 stored, so nothing opens the Congress cache a
second time. A clause inside "in subsection (a)--" is about a different
provision from the same clause outside it, and the stored bill holds that
nesting. `docs/adr/0009-a-source-is-parsed-once-a-bill-is-a-document.md` records
the decision.

The command prints each statement that it cannot place. A statement that no
reader can turn into two paths is recorded, and never dropped.

**Which two release points a redesignation is checked against is an open
question.** This command is told, with `--between` or `--from`/`--to`. Step 1
names every window the dataset holds. Nothing compares the bill's date with
those dates. Each work in the corpus holds two release points today, so each
work offers one pair, and the question does not yet bite. See
[#172](https://github.com/wordstodata/words-to-data/issues/172). Do not read this
document as an answer to it.

### Step 6 — `convert-dataset`

```bash
words_to_data convert-dataset dataset.json dataset.sqlite
```

The output argument is positional and optional. Without it, the command swaps the
extension of the input. The direction comes from the two extensions.

**Step 2 is the one that still needs compact JSON.** `extract-changes` refuses a
SQLite file and tells you to convert it first (#199). Step 1 always writes
compact JSON, whatever the output name. Steps 3, 4 and 5 read or write either
form, and a dataset they change is changed in place. So convert as soon as step
2 is done, and let the rest of the pipeline work on the database.

### What the finished dataset holds

`words_to_data info dataset.json` reports the links of each kind. A complete run
gives two kinds:

- `legislature.redesignated_as`, from step 1. The committed corpus gives 80.
- `legislature.amended_by`, from step 4. The last measured run gave 899.

A count of zero for `legislature.redesignated_as` says that step 1 did not record
them. A count of zero for `legislature.amended_by` says that step 4 did not run.

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
the two paths when it was, and the corroboration figure. `--json` is what an
agent reads, and the rows are stable, so two runs over one dataset give one
answer.

**It reads the dataset and nothing else** — no XML, and no model call. Nothing
about an unplaced statement is stored beside the links, because the words and
the path are already in the bill's own document and a stored row would go stale:
the same bill leaves 31 statements unplaced against title 26 alone and 17
against the whole Code.

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
no link has had no step run over it, **or** had one that found nothing — a
dataset records no list of the steps that ran, so nobody can tell the two apart
from the file.

**Run the window steps again after the dataset grows.** Steps 4 and 5 take a
`--between` span, and the run names the span to give them. A redesignation is
recorded by a step over a named window, so a bill in a dataset that grew holds no
link into the new window until step 5 runs over it (#181).

**`validate` says which of those steps is outstanding, for redesignations.** It
names each bill and window where the bill states renumberings, the window could
hold them, and the dataset holds no link — and it prints the command that closes
the gap. It says nothing about a statement no reader could place, because that is
finished work with a reason rather than a step nobody has run (#183). The list is
read out of the links the dataset holds, and nothing is stored.

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
