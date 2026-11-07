### A public law's `bill_id` uses a hyphen, the same as its work ([#488](https://github.com/wordstodata/words-to-data/issues/488))

The parser took `document_type.bill_id` of a public law from the `<docNumber>` of its preface, as the print writes it: `119–21`, with an en dash. The work of the same law is `publiclawdocument_119-21`, with a hyphen. So one law had two spellings, and an exact search with one of them did not find the other. Now the parser writes each dash of the number as a hyphen, and `bill_id` is `119-21`. All 7 public laws of the eval dataset gave an en dash before.

This changes a record, not the format: an old reader reads the new value. Rebuild a dataset to get the new value. A dataset that is not rebuilt keeps the en dash.
