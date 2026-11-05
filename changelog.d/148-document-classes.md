### A dataset reports its document classes ([#148](https://github.com/wordstodata/words-to-data/issues/148))

A dataset now derives the document class of each work from the node type at the
root of the work: `uscode`, `bill` or `judicial`. The class is a projection
(`docs/adr/0007`). It is not stored and it is not declared, so the SQLite schema
and `SCHEMA_VERSION` do not change. No dataset needs a rebuild, and a rebuilt
dataset gives the same answers.

- `Scope::classes` gives each class held, with its works. `WorkCoverage` has a
  new `class` field, so each entry of `scope.held` in `info --json` has a new
  `class` key. All other keys are as before.
- `DocumentReader::root_type` reads the root node type of one expression and
  does not read its tree.
- A `--between` span now reads only works of a class that is published at
  release points (`NodeType::RELEASE_POINT_CLASSES`), and that is only `uscode`.
  The "Skipping N work(s)" notice does not name a bill or a case. It still names
  a work of the Code that is absent at one of the two dates.
- `works_between` does not pair a work of another class, and it does not put
  that work in `skipped`.
- The bill document lookup (`bill_document`, `bill_public_laws`) selects works by
  class and not by the prefix of the work id, so it finds a bill document stored
  under a work id of another convention. `bill_public_laws` still reads the law
  number from the work id, so such a document has no number there.
- `carry-reviews` and the `contents` of a path read the class from the node type
  too, and not from the prefix `judicial/`.
