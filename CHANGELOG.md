# Changelog

## 0.4.0

A new foundation. Almost everything changed since 0.3.0, and **nothing from 0.3.0
carries over**: rebuild your datasets with this version.

### What words-to-data is now

The U.S. Code and the laws that change it, as a dataset with evidence behind every
link. A dataset records which amendment made which change, who said so, by which
method at which version, and who has checked it. People and agents can query it,
check it and correct it with the command alone.

### Highlights

- **Matching makes no model call.** An amendment is placed from the bill's own
  markup, the OLRC's classification tables, the first window after the law's
  enactment in which its address changed, and the words it quotes. The model-based
  commands `extract-changes`, `score-amendments` and `match-amendments` are removed
  ([ADR 0013](docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md)).
  On Pub. L. 119-21 it links 478 of 603 amendments; the model pipeline linked 435, and
  it made errors this method does not.
- **Datasets grow.** `add-release-points` adds release points and `add-bills` adds
  laws to an existing dataset. A grown dataset gives the same links as one built from
  scratch.
- **Every link can be checked and corrected.** `settle --explain` shows the words, the
  OLRC's classification, and how the link was made. `settle` records a verdict as its
  own record ([ADR 0012](docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md)).
  `link-amendment` records a link or a no-link conclusion, and refuses a path that did
  not change.
- **The work list.** `residue` lists the amendments that no standing link names, with
  the reason each one stopped. It also lists the links the current version of a method
  no longer makes.
- **For agents.** [`docs/agents/working-a-dataset.md`](docs/agents/working-a-dataset.md)
  is a playbook for any agent. It was tested with fresh agents that had only the
  playbook, the binary and a dataset.

### Also new

- A SQLite store beside compact JSON. `build-dataset` writes SQLite when the
  output is named `.sqlite` or `.db`, and `convert-dataset` moves between the two.
- Progress bars, colour, and a time for each step in the command line. No long step
  runs in silence.
- Court opinions from CourtListener, with their U.S.C. citations as links
  (`add-opinions`, `cases-citing`).
- Readers: `search` (scoped, bounded, with snippets), `diff`, `path`, `annotations`,
  `votes`, `show-bill`, `amendment-addresses`, `info`, `validate`,
  `section-agreement`, `contradictions`, `redesignation-report`, `coverage`.
- Each method has a name and a version. A dataset records which method ran over
  which window.
- A bill's member records are fetched eight at a time. A rate-limit answer stops
  the download, so no member is left out with no warning.
- Network requests retry over IPv4 when an IPv6 route is unreachable. After one
  such answer, every later request goes over IPv4 at once: on some networks the
  error for parallel IPv6 connections comes only once a second.

### Breaking

- The stored format is new (`SCHEMA_VERSION` 10). Datasets from 0.3.0 cannot be read.
- The Python bindings and the annotator prototype are removed.
- `extract-changes`, `score-amendments` and `match-amendments` are removed. Datasets
  made with them still read, and their links still show.

### Known limits

- No command gives a single member's vote by name. `votes` counts by party.
- An amendment that is only partly linked does not show in `residue`. `coverage`
  shows its unlinked change.
