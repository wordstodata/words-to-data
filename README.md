# Words To Data

**Your agent's favorite legal database.**

words-to-data holds the U.S. Code, release point by release point, and the public
laws that amend it. It records **which amendment made which change**, and the
evidence for each record. An agent asks it questions with one command,
`words_to_data`, and every answer comes with something it can quote.

## Try it with SLEUTH

[SLEUTH](https://github.com/wordstodata/sleuth-pi) is the official agent. It runs
in the [pi](https://github.com/earendil-works/pi) coding agent. These steps give
you SLEUTH and an example dataset. You need `curl`, `zstd` and 6 GB of free disk
space.

1. Install words-to-data. Each [release](https://github.com/wordstodata/words-to-data/releases/latest)
   has the command for Linux (x86_64, aarch64), macOS (Apple silicon, Intel) and
   Windows (x86_64). On Linux x86_64:

   ```
   curl -fL https://github.com/wordstodata/words-to-data/releases/latest/download/words_to_data-x86_64-unknown-linux-musl.tar.gz | tar xz
   sudo mv words_to_data /usr/local/bin/
   ```

   On macOS, use `aarch64-apple-darwin` (Apple silicon) or `x86_64-apple-darwin`
   (Intel) in the file name. With [Rust](https://rustup.rs), `cargo install
   words-to-data` also works.

2. Install pi. The installer also installs Node.js when it is necessary.

   ```
   curl -fsSL https://pi.dev/install.sh | sh
   ```

3. Install SLEUTH into pi.

   ```
   pi install git:github.com/wordstodata/sleuth-pi
   ```

4. Download the example dataset (280 MB) and decompress it (5.3 GB).

   ```
   curl -fL https://wordstodata.com/assets/wordstodata-example.sqlite.zst | zstd -d -o example.sqlite
   ```

5. Start pi in the same directory. Type `/login` to connect a model with a
   subscription or an API key. Then open the dataset and ask a question:

   ```
   pi
   /login
   /dataset example.sqlite
   Where in the Internal Revenue Code is the new Trump account section, and in which printing of the Code does it first appear?
   ```

The session is read-only. SLEUTH finds the section from the words alone, and it
shows the evidence. A real answer (2026-10-23), cut:

> **Where:** the new Trump account section is **26 U.S.C. § 530A ("Trump
> accounts")**, in a brand-new **part IX ("TRUMP ACCOUNTS")** added at the end of
> subchapter F of chapter 1:
> `uscode/title_26/subtitle_A/chapter_1/subchapter_F/part_IX/section_530A`
>
> **First printing:** the **2025-07-30** printing of Title 26. `w2d_path` reports
> the section present at 2025-07-30, 2025-08-14, 2026-01-23 and 2026-07-12, … a
> search for "Trump account" at 2025-07-18 returns 0 fields.
>
> **Source:** OLRC classifies `part_IX/section_530A` as new from **Pub. L. 119-21
> § 70204(a)(1)** (bill 119-hr-1) …

Then ask more questions, for example:

- What does 26 U.S.C. § 280C(c)(1) say now, and what did it say before the One Big Beautiful Bill Act changed it?
- In Boeing Co. v. United States, what did Justice Thomas say at 537 U.S. 459, and which Code sections does his dissent cite?
- BMW (US) Holding Corp. v. United States (Court of Federal Claims, August 2026) cites the bonus depreciation rule, 26 U.S.C. § 168(k). Has § 168(k) changed since the decision?

The example dataset holds:

- **The U.S. Code**, every title, at 5 printings: 2025-07-18, 2025-07-30,
  2025-08-14, 2026-01-23 and 2026-07-12.
- **7 bills of the 119th Congress**, with their amendments linked to the changes
  in the Code, and the House roll calls. Two of them are H.R. 1 (Pub. L. 119-21,
  the One Big Beautiful Bill Act) and S. 1071 (the NDAA for FY2026, Pub. L. 119-60).
- **45 court writings in 28 cases**, with pages, paragraphs, footnotes and their
  citations of the Code.

## Why agents like it

- **Evidence, not recall.** A link says what made it: the address in the bill's
  markup, the window it changed in, and the words the law quotes.
  `settle --explain` prints all of it. The agent quotes the record and does not
  rely on what it remembers about a law.
- **No model inside.** The dataset is built by rules from the publisher's markup
  and the OLRC's classification tables. What the rules cannot place goes on a work
  list (`residue`) with the reason it stopped. An answer is never a guess that
  looks like data.
- **It says what it does not know.** `cases-citing` names each period the
  dataset holds no printing for, and says that this is not a statement that
  nothing changed. `residue` gives the reason each unlinked amendment stopped.
- **Output that fits in context.** Listing commands stop at 20 rows and say how
  many they did not show, and how to get the rest. `--path`, `--work`, `--at` and
  `--bill` narrow a listing, and `--json` gives machine-readable rows.
- **Writes that are hard to get wrong.** `link-amendment` refuses a path that did
  not change. `settle` records a review as its own record, and nothing is deleted.
  A wrong link stays visible, and the newest review stands.

## Give it to your agent

### SLEUTH, the official agent

[SLEUTH-pi](https://github.com/wordstodata/sleuth-pi) is a package for the
[pi](https://github.com/earendil-works/pi) coding agent. It has a tool for each
`words_to_data` command, the playbook as four skills (answer, residue, review,
build), and a write gate. A session is read-only unless you give it `--write`.
[Try it with SLEUTH](#try-it-with-sleuth) gives the steps to install it.

```
pi install git:github.com/wordstodata/sleuth-pi
pi
/dataset law.sqlite
```

### Any other agent

Give the agent three things: the `words_to_data` binary, a dataset, and
[`docs/agents/working-a-dataset.md`](docs/agents/working-a-dataset.md). The
playbook tells it how to answer a question with evidence, how to work the residue,
and how to review links. It needs no other tools.

In the test of the playbook ([#267](https://github.com/wordstodata/words-to-data/pull/267)),
six new agents answered four questions about Public Law 119-21 correctly, in 17–22
tool calls each, and worked its residue from 55 items to 3. Every answer was
checked by hand.

## Build a dataset

You need Rust, about 3.5 GB of disk, and a free
[Congress API key](https://api.congress.gov/sign-up/).

```
cargo install --git https://github.com/wordstodata/words-to-data
export CONGRESS_API_KEY=<your key>

words_to_data build-dataset law.sqlite --uslm-dates 2025-07-18,2025-07-30 --bills 119-hr-1
words_to_data add-classifications law.sqlite
words_to_data link-by-evidence law.sqlite
```

That is the whole Code at two release points, and Public Law 119-21 (the
reconciliation act of July 2025) linked to the changes it made. It takes about a
minute and a half.

## What your agent sees

Three questions, and the real output. Where is section 174A, which amendment made
it, and why should anyone believe that?

```
$ words_to_data search law.sqlite "domestic research or experimental" --work uscode/title_26 --at 2025-07-30
...
uscode/title_26@2025-07-30  …/subchapter_B/part_VI/section_174A  [heading]  1 occurrence
    Domestic research or experimental expenditures

$ words_to_data annotations law.sqlite --bill 119-hr-1 --path uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174A
  [Pending] uscode/title_26 2025-07-18 -> 2025-07-30  insert 119-hr-1 amd 2841e731435d …
      (a) In General.-Part VI of subchapter B of chapter 1 is amended by inserting after section 174 the f…
      link 7cdb05c5323a  …/section_174A  [quoted_words]

$ words_to_data settle law.sqlite --link 7cdb05c5323a --explain
How it was made:
  Source: rule:evidence_matching
  Method: address, window and quoted words@5
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
  `release-points` lists the release-point dates the mirror offers and the cache
  holds.
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
