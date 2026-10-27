### A block shows a provision that the Code prints with a space lost ([#452](https://github.com/wordstodata/words-to-data/issues/452))

- **An enacted block shows an added provision also when the Code prints two
  of its words with no space between them.** The Code prints a table cell of
  two lines as two paragraphs with nothing between them. 26 U.S.C. 1202(a)(5)
  reads "Years stock held:Applicablepercentage:", and the bill reads
  "Applicable percentage:". So § 70431(a)(2) of Pub. L. 119-21 had no link
  ("the words it quotes show in none of the 10 change(s) under its address").
  Now it links to 1202(a)(5) by the block it enacts. The words are compared
  with the spaces taken out only when no other reading shows the block, and
  each field must start and end at the edge of a word of the block.
- **Why version 4 linked it and versions 5 to 8 did not.** Version 4 linked
  1202(a)(5) by elimination: no other amendment addressed to 1202(a) took it.
  #339 (version 5) gave § 70431(a)(5)(B) its full address, 1202(a)(4)(A). The
  OLRC put its words in the lead-in of 1202(a)(4), outside that address, so
  the change to (a)(4) became a second change that § 70431(a)(2) could take
  by elimination. An amendment that quotes words takes a change by
  elimination only when one change is left to it, so it took none. The
  address of #339 stays.
- On a fresh build of the benchmark dataset, `link-by-evidence` makes 3,453
  evidence links (was 3,452). 1 link is added, the one above, and none is
  removed. 3 links of § 10101(a) of Pub. L. 119-21 to 7 U.S.C. 2012(u)(1),
  (u)(4)(A) and (u)(4)(B) stay on the same change and amendment; their
  evidence now says the block shows the change, not that the change sits
  inside 2012(u).

Method version: `address, window and quoted words@9` (was 8). Run
`link-by-evidence` again; `residue` then lists the links that only version 8
made, for review.
