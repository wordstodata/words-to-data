### `residue`, `validate` and `redesignation-report` are faster ([#426](https://github.com/wordstodata/words-to-data/issues/426))

On the eval dataset (0.7.0, 93 works), `residue --bill 119-hr-1` takes 3.8 s
(was 31.2 s), `residue` takes 4.6 s (was 34.9 s), `validate` takes 16.9 s (was
49.9 s) and `redesignation-report` takes 13.0 s (was 42.5 s). The output of
each command does not change. The dataset does not change: no table, column,
index or `SCHEMA_VERSION` changes, and no dataset needs a rebuild.

- The diff compares the words of a field only when the two texts are not the
  same but for format characters. Before, it computed a word diff of every
  field of every provision, also of the fields that did not change, and then
  discarded it. `diff`, `coverage` and `cases-citing` use the same diff, so
  this change applies to them too.
- The evidence matcher and the redesignation step read each expression of a
  window once. Before, the matcher read each expression up to four times for
  each window: once for the window, once more for its diff, and again as the
  earlier expression of the next window. The redesignation step read each
  expression two times.
- The expressions of a work are read at the same time, each on a connection of
  its own. The windows of a work are diffed at the same time, and each document
  is indexed by section once, at the same time as the others. The documents of
  a work are freed at the same time too.
- These commands use more memory at their peak: `validate` 2.8 GB (was 1.6 GB),
  `residue` 2.3 GB (was 1.9 GB).
- New: `DocumentReader::get_expressions` reads several expressions in one call.
  The default reads them one after the other. `SqliteStorage` reads a file on
  one connection for each expression. `Dataset::diffs_of` diffs windows whose
  documents the caller has read, and `window_documents` reads the documents of
  a list of windows, each once.
- Method versions do not change. No answer of a method changes.
