### An amendment of "the heading for section X" links only to that heading ([#451](https://github.com/wordstodata/words-to-data/issues/451))

*"The heading for section 951A is amended by striking "global intangible
low-taxed income" …"* (§ 70323(a)(3)(E)(i) of Pub. L. 119-21) was also linked
to the heading of 26 U.S.C. 951A(b). Its address is the whole section, and
the heading of (b) lost the same words in the same window by a renumbering
of the same law. Now an amendment whose words say *"the heading of"* or
*"the heading for section X"*, where X is the section of its address, takes
only the change to the provision at its address, and no change below it.

The method is now `address, window and quoted words@9`.

#### Measured on a fresh build of `eval.sqlite`

`link-by-evidence` against `main` at 8e4a3eb (0.6.2), both on a copy of one
fresh build (`build-dataset --offline` and `add-classifications --offline`,
as `scripts/build-eval` does): 3,452 links become 3,457. 1 link is removed
and 6 are added. Each was checked by hand:

- Removed, wrong: 951A(b) to § 70323(a)(3)(E)(i). The link of the
  renumbering to 951A(b), and the link of § 70323(a)(3)(E)(i) to the heading
  of § 951A, stay.
- Added, right: 26 U.S.C. 250(a)(1)(B)(i), (a)(2)(A), (a)(2)(A)(i),
  (a)(2)(B)(ii) and (b)(3)(A)(i)(II) to § 70323(a)(3)(A)(i), which strikes
  "global intangible low-taxed income" in those units; and 951A(d)(1)(A) to
  § 70323(a)(3)(C), which strikes the same words in 951A(d). Before, the
  heading amendments of § 250 and § 951A quoted the same words, the tie held
  these changes back, and no amendment got them.
