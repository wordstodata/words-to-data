### A marginal note no longer joins the words of an amending line ([#484](https://github.com/wordstodata/words-to-data/issues/484))

- **Each edge of a marginal note is a word boundary in `amending_text`.** The
  Public Law print puts a marginal note (`<sidenote>`) between two words:
  `chapter 61<sidenote><p>26 USC</p><p>prec. 6041.</p></sidenote> <amendingAction>`.
  The stored amending text was *"chapter 6126 USCprec. 6041.is amended"*, and
  the chapter number was wrong. Now it is *"chapter 61 26 USC prec. 6041. is
  amended"*. The words of the note stay in the text. The same printing rule
  applies to each printing of a law, so two printings still mint the same ids.
- **The words a bill enacts or quotes do not change.** The matcher finds them
  in the stored fields of the bill, which still join the note to the words
  next to it.
- **Amendment ids change.** The id is a hash of the amending text, so each
  amendment whose instruction holds a marginal note gets a new id: 554 of the
  1,683 amendments of `eval.sqlite` (196 in Pub. L. 119-21, 355 in Pub. L.
  119-60, and 1 each in Pub. L. 119-22, 119-24 and 119-39). Rebuild a dataset to
  get the new ids. A link recorded by hand against an old id does not name the
  new amendment.
- **On a fresh build of `eval.sqlite`:** the links and the `residue` rows are
  the same, with the new ids. `legislature.amended_by` has 3,532 links, and
  `residue` has 106 rows for 119-hr-1 and 388 for 119-s-1071, before and after.
  The address reader reads the stored fields of the bill, not the amending
  text, so the addresses do not change. The stored fields still join the note
  (the stored-text issue, #217).
