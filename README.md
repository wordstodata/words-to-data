# words-to-data

**The U.S. Code, and the laws that change it, as a dataset with evidence behind every link.**

words-to-data reads the Code as the Office of the Law Revision Counsel publishes it,
release point by release point, and the public laws that amend it. It records
**which amendment made which change**, and it keeps the reason for each record. Then
a person or an agent can ask what a law did, check the answer against the words,
and correct it.

No language model decides anything in the pipeline. An amendment is placed from
the publisher's own markup, the OLRC's classification tables, and the words the law
quotes. What the rules cannot place is listed as work with the reason it stopped,
and an agent or a person finishes it through commands that refuse a change that
did not happen.

```
$ words_to_data search dataset.sqlite "software development" --work uscode/title_26 --at 2025-07-30
uscode/title_26@2025-07-30  …/part_VI/section_174A/subsection_d/paragraph_3  [heading]
    Software development

$ words_to_data annotations dataset.sqlite --bill 119-hr-1 --path …/part_VI/section_174A
  [Pending] uscode/title_26 2025-07-18 -> 2025-07-30  insert 119-hr-1 amd 2841e731435d
      (a) In General.-Part VI of subchapter B of chapter 1 is amended by inserting after section 174 …
      link 7cdb05c5323a  …/part_VI/section_174A  [quoted_words]
```

## Install

```
cargo install words-to-data
```

This installs one binary, `words_to_data`. Every tool is a subcommand. The library
is the same crate; use `default-features = false` for the library alone.

Two keys are read from the environment when they are needed:

| variable | needed for |
| --- | --- |
| `CONGRESS_API_KEY` | loading bills, votes and members (`build-dataset --bills`, `add-bills`). Get one at <https://api.congress.gov/sign-up/> |
| `COURTLISTENER_API_KEY` | loading court opinions (`add-opinions`) |

Everything fetched is cached, by default under your user cache directory, so a
second build reads from disk. `--offline` reads only the cache.

## Build a dataset

```
# 1. Release points of the Code, and the laws you care about.
#    build-dataset writes compact JSON; convert it to SQLite to work on it.
words_to_data build-dataset dataset.json \
    --uslm-dates 2025-07-18,2025-07-30,2025-08-14 \
    --bills 119-hr-1
words_to_data convert-dataset dataset.json          # writes dataset.sqlite

# 2. The OLRC's classification of each law it holds.
words_to_data add-classifications dataset.sqlite

# 3. Link each amendment to the change it made.
words_to_data link-by-evidence dataset.sqlite

# 4. See what is left, and check the file.
words_to_data residue dataset.sqlite --bill 119-hr-1
words_to_data validate dataset.sqlite
```

A dataset **grows** in place. It never needs a rebuild to take in new material:

```
words_to_data add-release-points dataset.sqlite --uslm-dates 2026-01-23,2026-07-12
words_to_data add-bills dataset.sqlite --bills 119-s-1071,119-hr-998
```

Each command prints the steps to run next. A grown dataset gives the same links as
a dataset built from scratch with the same material.

## Ask it things

| to find | use |
| --- | --- |
| words anywhere in the Code, at a date, under a path | `search <ds> "<words>" --work … --at … --path …` (an empty query prints every field under a path) |
| what changed between two release points | `diff <ds> --from <work@date> --to <work@date> [--path …]` |
| one provision: its words, its changes, and the links on it | `path <ds> <path> --from … --to …` |
| which amendment made a change | `annotations <ds> --bill … --path …` |
| why a link says what it says | `settle <ds> --link <id> --explain` |
| a law's amendments, and where each one acts | `show-bill`, `amendment-addresses` |
| how the House voted on a bill | `votes <ds> <bill>` |
| which court opinions cite a provision, and whether it changed after each | `cases-citing` |
| the law's own text, such as effective dates | `search <ds> "shall apply" --work publiclawdocument_119-21` |

Every listing is bounded to 20 rows by default and says how many it did not show.
`--json` gives every row.

## How a change is linked

A **link** is a stored statement: *this change to the Code was made by this
amendment*. It records who made it, by which method at which version, and the
evidence. `link-by-evidence` makes links in five stages:

1. **Classify.** The OLRC's classification tables say which Code section each part
   of a law was classified to. `add-classifications` stores each row as a link.
2. **Address.** The bill's own markup resolves the section each instruction acts
   on, through the publisher's references and the enclosing clauses. The OLRC row is
   the fallback.
3. **Window.** The first pair of release points after the law's enactment in which
   something under that address changed.
4. **Resolve.** One change under the address is linked. Several are told apart by the
   words the law quotes: struck, inserted, or enacted. A change goes to an amendment
   only if that amendment's own words can have made it.
5. **Residue.** Every amendment that is not linked is listed with the stage and the
   reason it stopped. The list is derived, never stored.

The reasons are in [`docs/adr/0013`](docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md).

## Resolving facts

A link from the pipeline is a **suggestion**. The dataset keeps the record of who
checked it:

- `residue <ds> --bill <bill>` is the work list:
  - amendments that no link names;
  - amendments whose every link was refuted;
  - links that the current version of a method no longer makes.
- `settle <ds> --link <id> --verdict confirmed|refuted|disputed --reviewer … --reason …`
  records a review. A review is its own record. The link is never rewritten, and the
  newest review stands.
- `link-amendment` records a link that a person or an agent found. It refuses any path
  that did not change in the window. With `--no-link` it records that an amendment
  has no correct link, and why.
- `info` counts every kind of link as unreviewed, confirmed, refuted or disputed.

Nothing is deleted. Evidence is kept once and for ever
([`docs/adr/0005`](docs/adr/0005-evidence-is-stored-once-and-never-deleted.md)).

## For agents

[`docs/agents/working-a-dataset.md`](docs/agents/working-a-dataset.md) is a playbook
that any agent can follow with the command alone. It covers how to answer a question,
how to work the residue, and how to review links. Agents that were given only this
playbook, the binary and a dataset answered questions about recent law correctly,
with evidence, in about twenty tool calls. The same agents worked a law's residue
down from 55 open items to 3.

## Commands

| command | does |
| --- | --- |
| `build-dataset` | build a dataset from release points and, optionally, bills |
| `add-release-points` | add release points to an existing dataset |
| `add-bills` | add laws to an existing dataset |
| `add-classifications` | store the OLRC's classification of each law as links |
| `link-by-evidence` | link each amendment to the change it made |
| `redesignations` | record the renumberings a law states |
| `add-opinions` | add court opinions and the U.S.C. citations they make |
| `residue` | the work list: what is not linked, what was refuted, what is outdated |
| `settle` | review a link, or explain it with `--explain` |
| `link-amendment` | record a link, or a no-link conclusion, that a reviewer found |
| `search`, `diff`, `path` | read the Code's text and its changes |
| `annotations`, `show-bill`, `amendment-addresses`, `bills`, `votes` | read laws and their links |
| `section-agreement`, `contradictions`, `redesignation-report`, `coverage` | find links worth a second look |
| `cases-citing` | court opinions that cite a provision |
| `info`, `expressions`, `validate` | what a dataset holds, and whether it is consistent |
| `convert-dataset` | compact JSON to SQLite, or the reverse |

`words_to_data <command> --help` describes each command in full.

## Concepts

[`CONTEXT.md`](CONTEXT.md) is the glossary: work, expression, path, link, review,
residue, outdated link, and the others. [`docs/adr/`](docs/adr/) records each design
decision and why it was made. Start with
[`0001`](docs/adr/0001-structural-paths-locate-not-identify.md) (a path locates, it
does not identify),
[`0004`](docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md) (a link is
identified by what it says),
[`0012`](docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md)
(a review is its own record) and
[`0013`](docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md).

## Development

```
cd tests/test_data && tar xf test_files.tar.xz && cd ../..   # the test corpus is an archive
cargo test --no-fail-fast
```

Without the archive extracted, about 190 tests fail on missing files. Tests use real
committed data only. [`CLAUDE.md`](CLAUDE.md) holds the working rules, including the
merge queue that lands every pull request.

## License

MIT or Apache-2.0, at your option.
