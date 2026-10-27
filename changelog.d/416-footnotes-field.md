### The OLRC's footnotes are out of the words of the law ([#416](https://github.com/wordstodata/words-to-data/issues/416))

**Breaking: the stored format changes** (`SCHEMA_VERSION` 11, was 10). This
change is for 0.7.0.

The OLRC prints editorial footnotes in the Code: *"So in original. The comma
probably should not appear."*, *"See References in Text note below."*. The
parser kept the mark and the footnote text in the words of the provision, so
26 U.S.C. 164(b)(6) read *"…December 31, 2017,22 So in original. The comma
probably should not appear.—"*. The OLRC numbers its footnotes again in each
printing, so `diff` showed a footnote that only got a new number as a change to
the law, and `coverage` put it in the work queue.

Now the marks and the footnote text go out of the five fields of words, and out
of the number of an element (*"§ 9815."*). The footnote text goes in a sixth
text field, `footnotes`, one footnote on each line
(`docs/adr/0015-a-publishers-footnotes-are-a-sixth-text-field-the-diff-does-not-compare.md`):

- **`search` reads it.** A hit in a footnote shows the field `[footnotes]`. A
  search for "So in original" on the benchmark dataset finds every provision
  that it found before (28,106 provisions), and 3,003 more: a second mark for a
  footnote (a bare `<sup>2</sup>`) gives its provision the footnote text that
  the OLRC printed at the first mark.
- **`diff` and `coverage` do not compare it.** In titles 26 and 42, 51 changes
  that were only a footnote go, and no change comes. Each cell is changed paths
  / added and removed nodes / the work queue of `coverage`:

  | Window | Before | After |
  |---|---|---|
  | title 26, 2025-07-18 → 2025-07-30 | 536 / 518 / 1,052 | 503 / 518 / 1,020 |
  | title 26, 2025-07-30 → 2026-01-23 | 14 / 3 / 16 | 9 / 3 / 12 |
  | title 42, 2025-07-18 → 2025-07-30 | 62 / 53 / 115 | 59 / 53 / 112 |
  | title 42, 2025-07-30 → 2026-01-23 | 292 / 202 / 494 | 282 / 202 / 484 |

- **The evidence matcher reads the words without the footnotes.** Method
  version: `address, window and quoted words@9` (was 8). The rules do not
  change. On a fresh build of `eval.sqlite`, `legislature.amended_by` goes from
  3,452 to 3,460 links: 14 go and 22 come, and each one was examined.
  - 22 come, and all are right. An enacted block now matches words that a
    footnote broke (10 U.S.C. 222c(f), 26 U.S.C. 63(b)(6)), and a change that a
    footnote hid now shows its quoted words (26 U.S.C. 6050W(a)(2), inserted
    "and"; 31 U.S.C. 9705(f)(4)(B), which before went to the wrong amendment).
  - 10 go because their change was only a footnote, for example a renumbered
    footnote on a provision that 6 U.S.C. 124n moved.
  - 1 goes to the right amendment (31 U.S.C. 9705(f)(4)(B), above).
  - 2 go where two amendments of Pub. L. 119-60 make the same correction
    (10 U.S.C. 4872(e)(1), 10 U.S.C. 5531(b)(2)). The footnote's own words
    ("Probably should be "paragraph (1)"") gave the second amendment a
    match. The change keeps its link to the first.
  - 1 right link goes: 10 U.S.C. 132a, whose heading changes only in case,
    "improvement officer" to "Improvement Officer". The matcher reads words in
    lower case, so a change of case only shows nothing. Before, the footnote
    repeated the words and gave the match by chance. The change is now in the
    work queue of `coverage`.

  `legislature.redesignated_as`, `olrc.classified_from` and `judicial.cites`
  do not change, and `section-agreement` finds 0 disagreements.

A raised number that is part of the words stays: the top of a fraction
(`1⁄12`), a power (`kg/m2`), an isotope (`235U`). The narrow space that the
OLRC puts before a mark goes with the mark.

#### Upgrade

- **Build each dataset again** with this build: `build-dataset`, then each
  command that added to it (`add-classifications`, `link-by-evidence`,
  `add-opinions`, …). A dataset at schema 10 does not open. The error names the
  versions and the step. There is no migration.
- **`search` output** can show the field `footnotes`. `search --json` gives
  `"field": "footnotes"` for such a hit.
- **The words of a provision change** where it holds a footnote: in `diff`,
  `path` and `--json` output. The number of an element (in the
  USLM payload, `number_display`) has no footnote.
- **Rust library:** `TextContentField` has a new variant, `Footnotes`, and
  `NodeData` a new field, `footnotes`. A `match` on `TextContentField` and a
  `NodeData { … }` literal must name them. The compact JSON form writes
  `footnotes` only on a node that has footnotes.

#### Known limits

- The place of a mark in the words is not kept. To know which comma a
  footnote is about, read the OLRC's text.
- A footnote that the OLRC prints in the foot of an XHTML table (`<tfoot>`) is
  not USLM footnote markup, and it stays in the words: 5 U.S.C. 10203(a).
