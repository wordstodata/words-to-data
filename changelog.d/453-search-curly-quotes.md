### `search` reads a curly quote mark as the straight one ([#453](https://github.com/wordstodata/words-to-data/issues/453))

The Code is stored with straight quote marks. Many court opinions keep the
curly marks of their source, as "the court’s reason". Text copied from a PDF
or a web page has curly marks too. `search` compared the marks as they are, so
`“mathematical or clerical error” means` found nothing in the Code, and
`court's` found only the opinions that write a straight apostrophe.

Now `search` reads `“` and `”` as `"`, and `‘` and `’` as `'`, in the query
and in the text. The rule is in the library (`words_to_data::search`,
`CURLY_QUOTE_MARKS`), so the library search and the command read a query the
same way. The rule of #439 does not change: one pair of double quote marks
around the whole query is removed, with the same notice. Snippets show the
text as it is stored. No link changes: no link method uses search.

#### Measured on `data/eval.sqlite`

Fields found, before (a release build of 0.6.2) and after (a release build
of this change):

| Query | Before | After |
|---|---|---|
| `“mathematical or clerical error” means` (title 26 at 2026-07-12) | 0 | 1 |
| `"mathematical or clerical error" means` (title 26 at 2026-07-12) | 1 | 1 |
| `taxpayer’s` (title 26 at 2026-07-12) | 0 | 511 |
| `taxpayer's` (title 26 at 2026-07-12) | 511 | 511 |
| `court’s` (opinions) | 144 | 224 |
| `court's` (opinions) | 80 | 224 |
| `the “ordinary and necessary”` (opinions) | 1 | 1 |
| `the "ordinary and necessary"` (opinions) | 0 | 1 |

The one field of the first query is the definition in § 6213(g)(2). The 224
fields of `court's` are the 144 that write a curly apostrophe and the 80 that
write a straight one.
