### A public law's `<footnote>` is read as a footnote ([#486](https://github.com/wordstodata/words-to-data/issues/486))

A public law prints the OLRC's classification notes as USLM `<footnote>`
elements, with a mark `<ref idref="…">*</ref>` that has no class. The footnote
reader knew only the forms of the Code, `<note type="footnote">` and
`<ref class="footnoteRef">`. So the mark and the footnote text went into the
words. In `119-s-1071` (Pub. L. 119-60), the `number_display` of § 3111(b)(1)
was "\* \* Note: Classifications for SEC. 3111(b)(1): 50 USC 2501 and note, …".

Now the reader takes the mark, the footnote and the narrow no-break spaces
around them out of the words, and puts the footnote text in the `footnotes`
field. The `number_display` of § 3111(b)(1) is "(1) ". The same change applies
to § 3111(d)(2)(A) and (B).

The `amending_text` of the instructions that hold these footnotes does not
change in this release: it is read by a different function.

Rebuild a dataset that holds `119-s-1071` to get the new records.
