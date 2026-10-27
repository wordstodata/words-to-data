### An amendment that adds one section is not linked to a whole new unit above it ([#450](https://github.com/wordstodata/words-to-data/issues/450))

The diff lists a provision added whole as one path, and nothing beneath it
(#336). Where nothing under an amendment's address changed, but a window
added a unit that holds the address, the method gave the whole unit to the
amendment that only adds. That is right where the amendment enacted the unit,
as § 70204(a)(1) of Pub. L. 119-21 enacts part IX with § 530A. It is wrong
where the window added the unit for another reason. § 3117(a) of Pub. L.
119-60 adds § 4816 of the Atomic Energy Defense Act, 10 U.S.C. 6340, in the
window where the OLRC moved the Act into a new subpart B of title 10. The
method gave it the whole subpart (2,344 fields).

Now such an amendment takes the added unit only when its quoted words show
the unit. Otherwise it takes the provision at its address, which the window
added too, as a change of its own. § 3117(a) is now linked to
`…/subpart_B/chapter_608/subchapter_II/section_6340`.

The method is now `address, window and quoted words@9`.

#### Measured on a fresh build of `eval.sqlite`

`link-by-evidence` against `main` at 8e4a3eb (0.6.2), both on a copy of one
fresh build (`build-dataset --offline` and `add-classifications --offline`,
as `scripts/build-eval` does): 3,452 links stay 3,452. 3 links are removed
and 3 are added, each for the same amendment. Each was checked by hand:

- 10 U.S.C. part VI subpart B to § 3117(a) of Pub. L. 119-60 (wrong) becomes
  10 U.S.C. 6340 (right).
- 14 U.S.C. chapter 3 subchapter II was linked to two amendments of
  § 7201(e) of Pub. L. 119-60, (5) and (6) (wrong: the law reorganizes
  chapter 3 into subchapters, and neither amendment enacts subchapter II).
  Now (5) is linked to 14 U.S.C. 310, which it inserts, and (6) to 14 U.S.C.
  316, which it inserts (right, both by the block). (6) also inserts § 317,
  which still has no link: the address of (6) is § 316.
