# words-to-data

**The U.S. Code, and the laws that change it, as a dataset with evidence behind every link.**

words-to-data reads the Code release point by release point, and the public laws
that amend it. It records **which amendment made which change**, with the evidence
for each record. No language model decides anything: an amendment is placed from
the publisher's markup, the OLRC's classification tables, and the words the law
quotes. What the rules cannot place is listed as work, with the reason it stopped.

## Try it

You need Rust, about 3.5 GB of disk, and a free
[Congress API key](https://api.congress.gov/sign-up/).

```
cargo install words-to-data
export CONGRESS_API_KEY=<your key>

words_to_data build-dataset law.sqlite --uslm-dates 2025-07-18,2025-07-30 --bills 119-hr-1
words_to_data add-classifications law.sqlite
words_to_data link-by-evidence law.sqlite
```

That is the whole Code at two release points, and Public Law 119-21 (the
reconciliation act of July 2025) linked to the changes it made. It takes about a
minute and a half. Now ask it something:

```
$ words_to_data search law.sqlite "domestic research or experimental" --work uscode/title_26 --at 2025-07-30
...
uscode/title_26@2025-07-30  …/subchapter_B/part_VI/section_174A  [heading]
    Domestic research or experimental expenditures

$ words_to_data annotations law.sqlite --bill 119-hr-1 --path uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174A
  [Pending] uscode/title_26 2025-07-18 -> 2025-07-30  insert 119-hr-1 amd 2841e731435d …
      (a) In General.-Part VI of subchapter B of chapter 1 is amended by inserting after section 174 the f…
      link 7cdb05c5323a  …/section_174A  [quoted_words]

$ words_to_data settle law.sqlite --link 7cdb05c5323a --explain
How it was made:
  Source: rule:evidence_matching
  Method: address, window and quoted words@4
  Address: /us/usc/t26/s174A, from the bill's markup
  Window: uscode/title_26@2025-07-18 to 2025-07-30
    the first window after the law's enactment on 2025-07-04 in which something under the
    address changed.
  Changes under the address: 1
  Chosen: by quoted words
    the change shows the words the bill quotes: enacted text 1 of 1.
...
```

`words_to_data residue law.sqlite --bill 119-hr-1` lists what the rules could not
link, and why. `words_to_data --help` lists every command.

## Give it to an agent

Give an agent three things: the `words_to_data` binary, a dataset, and
[`docs/agents/working-a-dataset.md`](docs/agents/working-a-dataset.md). The
playbook tells it how to answer a question with evidence, how to work the residue,
and how to review links. It needs no other tools.

In the test of the playbook ([#267](https://github.com/wordstodata/words-to-data/pull/267)),
six new agents answered four questions about Public Law 119-21 correctly, in 17–22
tool calls each, and worked its residue from 55 items to 3. Every answer was
checked by hand.

The agent records what it finds with `link-amendment` and `settle`. Both refuse a
change that did not happen, and neither deletes anything: a review is its own
record, and the newest review stands.

## Use it as a library

```toml
[dependencies]
words-to-data = { version = "0.4", default-features = false }
```

```rust
use words_to_data::dataset::Dataset;

let dataset = Dataset::open_sqlite("law.sqlite")?;
let path = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174A";
for annotation in dataset.annotations_for_path(path)? {
    println!("{:?} by {}", annotation.operation, annotation.source_bill.bill_id);
}
// Insert by 119-hr-1
```

`default-features = false` leaves out the command line and its dependencies.

## More

- **Grow a dataset** in place, with no rebuild: `add-release-points` and
  `add-bills`. A grown dataset holds the same links as one built from scratch.
- **Court opinions** that cite a provision: `add-opinions` (needs
  `COURTLISTENER_API_KEY`), then `cases-citing`.
- **Everything is cached**, under your user cache directory. `--offline` reads only
  the cache.
- **Concepts:** [`CONTEXT.md`](CONTEXT.md) is the glossary. [`docs/adr/`](docs/adr/)
  records each design decision and its reason. Start with
  [`0013`](docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md),
  how a change is linked.

## Development

```
cd tests/test_data && tar xf test_files.tar.xz && cd ../..   # the test corpus is an archive
cargo test
```

Without the archive, about 190 tests fail on missing files. Tests use real committed
data only. [`CLAUDE.md`](CLAUDE.md) holds the working rules, including the merge
queue.

## License

MIT or Apache-2.0, at your option.
