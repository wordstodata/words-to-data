### A citation range is one item in `cases-citing` and `annotations` ([#383](https://github.com/wordstodata/words-to-data/issues/383))

Since #309 and #366, a range the court wrote is more than one link:
"26 U. S. C. §§ 991-997" is a link to each of its two ends, and
"§§994(a)(1)-(3)" is a link to each of its three paragraphs. The readers
listed those links as citations that had no relation. Now a reader groups
them. The links of one range have the same passage, and each holds a place
with the same `start` and the same `written` form, which reads as a range.
The group is calculated each time a command reads the links. Nothing new is
stored, `SCHEMA_VERSION` does not change, no method version changes, and no
dataset needs a rebuild.

- `cases-citing` gives each range of a writing as one item before the
  provisions it cites:
  `cites one range in <passage>, written "<words>": N links, one to each provision below`,
  then one line for each provision, `<path> — link <id>`. The windows of each
  provision follow, as before.
- `annotations` and `path` give a range as one item with the same first line,
  and each link of the range as a line below it. A link of the range that the
  listing does not hold, because it is outside the path asked about, is named
  on its line: `<path> [link <id>], which this listing does not hold`.
- `--json`: each case of `cases-citing` and each row of `citations` in
  `annotations` gets `ranges`, a list of the ranges the link is one of. Each
  range gives `passage`, `start`, `written`, `words` and `members`, and each
  member gives its `link` and its `path`. A case or a row that is part of no
  range has no `ranges` key, so its JSON is as before.

A single citation does not change. Two citations of one provision at two
places are not a range, and a form with no dash, such as the OCR form
"§§994(a)(1)(3)", is not a range.
