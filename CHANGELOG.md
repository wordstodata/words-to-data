# Changelog

## Unreleased

### Opinion plain text: the first item of a quoted list starts its own paragraph ([#430](https://github.com/wordstodata/words-to-data/issues/430))

In *Polselli* the Court quotes 26 U.S.C. § 7609(c)(2)(D): the lead-in
"“issued in aid of the collection of—" and then the item "“(i) an assessment
made…". The watermark that #413 removed stood between the two lines and split
them. Without it, the lead-in and the first line of item (i) were one
paragraph, and the rest of item (i) was a paragraph of its own.

Now an indented line that opens with a list designation, "(i)" or "“(i)",
after a line that ends with "—" or ":" starts a paragraph, and the wrapped
lines of the item stay with it. At the margin the rule does not apply: in the
Tax Court opinions "means services consisting of— / (i) engaging in…" goes on
in the sentence.

#### Measured on the opinions of `evals/opinions.txt`

Added to two copies of `dataset-demo.sqlite` from the cache, by the build
before this change and by the build after it. Of the 29 writings read from
plain text, 2 change, and no words change:

- *Polselli* (opinion 10516288-1): 2 paragraphs change. The lead-in is a
  paragraph of its own, and item (i) is one paragraph. The writing keeps 42
  paragraphs.
- *Medina v. Planned Parenthood* (opinion 11084231-1): "…including each of the
  following rights:" and "“(i) Free choice" are now two paragraphs
  (`paragraph_57` and `paragraph_58`), so the writing has 80 paragraphs, not
  79, and the 22 paragraphs after them are numbered again. Its 9 footnotes do
  not change.
- **`judicial.cites`** stays at 441 links and 488 places. One link changes
  only its subject path: the citation of 42 U.S.C. § 1395a in *Medina* moves
  from `paragraph_68` to `paragraph_69`, where the paragraph number moved.
- No other writing changes.

#### Breaking changes for agents

- The method stays at `plain-text-paragraphs@3`. Version 3 changed for #413
  and is not released yet, so this change goes in the same version.

### One rule for "not held", and "matched nothing" said as that ([#425](https://github.com/wordstodata/words-to-data/issues/425))

An input the dataset does not hold stops the run with exit 1, the notice on
stderr and nothing on stdout, as #392 chose. A filter that matches nothing in
held data exits 0 and says that the filter matched nothing.

### Fixes

- A path that no expression holds, where the command found nothing, is
  refused as `diff` refuses it, with the nearest held path: `path`,
  `annotations --path`, `search --path`, `section-agreement --path` and
  `cases-citing --path` (#425). Before, each one exited 0 with the notice on
  stdout. What a link, a citation or a classification names at such a path is
  still listed, with the notice. `cases-citing --path` in a work the dataset
  does not carry still answers "out of scope", as `--cites` does.
- `search --work` and `search --at` with a work or a date not held are refused
  as `expressions --work` refuses them (#425).
- `section-agreement` with a filter that matches no link says "No amendment
  link matches --bill 119-hr-2215." Before, it said "This dataset holds no
  amendment link to check." on a dataset that holds 3,224 (#425).
- `redesignation-report --bill-id` with a held bill that states no renumbering
  says "Bill 119-hr-42 states no renumbering." Before, it said that the
  dataset holds no bill that states one (#425).
- `amendment-addresses` with a held bill that has no public-law document says
  that a bill that did not become law has none. Before, it gave only "built
  before #196" as the reason (#425).
- `votes` with a held bill that has no House roll call exits 0, as
  `annotations --bill` does for a held bill with no link. `--json` gives `[]`
  (#425).
- `cases-citing --cites "26 U.S.C. § 174 and 26 U.S.C. § 162"` answers each
  citation. Before, it answered § 174 alone, with no notice
  ([#426](https://github.com/wordstodata/words-to-data/issues/426)).
- `section-agreement --json` honors an explicit `--limit`, and gives
  `rows_total` beside the rows (#426).
- After `settle --verdict refuted`, `cases-citing` leaves the refuted citation
  out, so it does not count its opinion, and `section-agreement` leaves the
  refuted link out of its counts and its queue. `cases-citing` prints the
  status a review gives, and who gave it: "link 1a2956ce5231 is Verified,
  confirmed by agent:x on 2026-10-26"
  ([#415](https://github.com/wordstodata/words-to-data/issues/415)).

#### Breaking changes for agents

- `path`, `annotations --path`, `search --path`, `section-agreement --path`
  and `cases-citing --path`: a path not held, where nothing was found, exits 1
  with the notice on stderr. Before, exit 0 and the notice on stdout. The
  `absent_path` field of `--json` is given only when a link, a citation or a
  classification names the path (`path`, `annotations`), and `search --json`
  no longer has `absent_path`.
- `search --work` and `search --at` with a work or a date not held exit 1 with
  the notice on stderr. Before, exit 0. `search --json` no longer has
  `absent_scope`.
- `votes` with a held bill and no House roll call exits 0 (before, 1). The
  notice is on stdout, and `--json` gives `[]`.
- `section-agreement --json --limit N` gives N rows and `rows_total`.
- `cases-citing --json`: each place has `status` and `review` (`record`,
  `verdict`, `reviewer`, `reasoning`, `at`, or `null`). A refuted citation is
  not in the answer.

### The renumbering reader reads "Such section" ([#431](https://github.com/wordstodata/words-to-data/issues/431))

A bill can name the section a renumbering acts in as *"Such section"*: Pub.
L. 119-60 § 6402(c)(1) says "Section 102a.—Such section is further amended—(A)
by redesignating subsections (f) through (z) as subsections (e) through (y)".
#364 taught the address reader of `legislature.amended_by` this phrase, but
not the renumbering reader. That reader read the bare "Section 102a" in the
heading, found no title for it, and did not record the renumbering.

Now both readers read the section in one function
(`Scope::section_cited_by`), so they cannot come to different answers again.
*"Such section"*, *"Such subsection"* and *"Subsection (h) of such section"*
place a renumbering where the address reader places the amendment. The
reader of the bill's XML (`redesignations_stated`), which only the tests use,
still does not read the phrase.

#### Measured on a fresh build of the 7 bills from the caches

| | before | after |
|---|---|---|
| renumbering statements placed (all bills) | 118 of 230 | 123 of 230 |
| `legislature.redesignated_as` links | 307 | 335 |
| `legislature.amended_by` links | 3,405 | 3,412 |
| amendments with a link | 1,142 | 1,144 |
| `diff` of 50 U.S.C. 3024, 2026-01-23 → 2026-07-12 | 69 changed, 61 added, 80 removed, 0 moved | 7 changed, 0 added, 3 removed, 21 moved |
| changes in 50 U.S.C. 3024 with no link | 190 | 0 |

- The 5 statements placed: § 6402(c)(1)(A) (50 U.S.C. 3024), § 141(b)(2)
  (10 U.S.C. 9062(j)), and three more in 10 U.S.C. 115a, 181(h) and 321(b).
- The address of each amendment does not change. The `amended_by` links
  change through the new renumbering links: 26 go and 33 come. The 5 wrong
  links that #431 lists go. The strike of 3024(e) (§ 6402(b)(1)) loses 14
  wrong links in the new (e), and is now in `residue`. § 6402(b)(2) names
  3024(f)(2), which § 6402(c)(1)(A) then moves to (e)(2). Its change at
  (e)(2) is now given to the renumbering, and § 6402(b)(2) is in `residue`.
- Method versions: `amendingAction type=redesignate@6` stays at 6, because
  version 6 (#387) is not released yet. `address, window and quoted words@7`
  does not change. Run `redesignations` and `link-by-evidence` again.

### Opinion plain text: the watermark goes, star breaks stay in the body, a glued footnote mark is no part of a section ([#413](https://github.com/wordstodata/words-to-data/issues/413))

Three faults in the text of opinions that CourtListener gives only as
`plain_text`:

- **The watermark.** A preliminary print of the United States Reports prints
  "Page Proof Pending Publication" on each page. It stayed in the words, and
  in *Polselli* it broke Justice Jackson's citation "26 U. S. C. Page Proof
  Pending Publication § 7609(c)(2)(D)(i)". Now the plain-text rule removes
  it. Where pdftotext puts its words at the ends of the first lines of a
  paragraph ("Page     Proof", " To pursue         Pending"), the rule removes
  those words too, and the lines go on as one paragraph.
- **Stars.** A centred line of stars, "***" or "* * *", is a break between
  two parts of an opinion. The rule read it as a footnote mark, and all the
  body text after it on the page went into a false footnote. Now the line is
  a paragraph of its own, as Harvard's XML keeps it, and the text after it
  stays in the body. Stars followed by more stars ("* * *.”") are an
  ellipsis, not a mark. A star against the first word of a footnote
  ("*There is much…") is its label, so the footnote is `footnote_*`, not
  `footnote_`. In a double-spaced text, two blank lines continue a footnote
  only after a footnote has started, so a caption stays in the body.
- **A glued footnote mark.** "5 U. S. C. §706.3" was read as section "706.3".
  No section of the Code has a full stop in its number. Now, when the writing
  has a footnote with the label after the full stop, the citation reader
  reads § 706, and the 3 is the mark. When the writing has no such footnote,
  the number is read as written, as before. The default-code reader does the
  same: "IRC section 179.17" is § 179.

#### Measured on the opinions of `evals/opinions.txt`

Added to two copies of `dataset-demo.sqlite` from the cache, by the build
before this change and by the build after it:

- **Paths change in 11 writings of 8 cases.** A path change is a break. 23 paths go and 29 come: the false footnotes `footnote_***`,
  `footnote_*` and `footnote_` go, and their text is in paragraphs again; 4
  star footnotes go from `footnote_` to `footnote_*`; and the paragraphs
  after each change are numbered again. *Polselli* (opinion 10516288-1) has
  42 paragraphs, not 50, because the watermark split paragraphs. The text of
  412 passages changes, most of it because the paragraph numbers moved.
- **Words.** Per writing, the only words that go are the watermark (102 times)
  and the star of 6 footnotes, which is now their label. A word broken at a
  hyphen across the watermark now joins (`owner-` and `ship` give
  `owner-ship`). The 4 section breaks and the ellipsis in *Khalaf* get back
  the star that the false label took. A caption of 12 lines in *District of
  Columbia v. USDA* is in the body again, one paragraph to a line.
- **`judicial.cites`** goes from 438 links to 441, and from 484 places to
  488. The 4 new places: § 7609(c)(2)(D)(i) in Justice Jackson's concurrence
  in *Polselli*, § 706 in *Loper Bright*, § 5311 in *Bittner* and § 179 in
  *Khalaf*. No place is lost. Other links change only their subject path,
  where the paragraph number moved.
- One paragraph is worse: in *Polselli* the watermark stood between the
  lead-in "“issued in aid of the collection of—" and the item "“(i) an
  assessment made…", and split them. Now the two lines join, and the rest of
  item (i) is a paragraph of its own.

#### Breaking changes for agents

- Method version: `plain-text-paragraphs@3` (was 2).
- The text method stays at version 2 and the citation rules stay at
  `reporters-db laws.json U.S.C. patterns@6` and `bare section under the
  opinion's default code@4`. These versions changed for #412 and #410 and
  are not released yet, so this change goes in the same versions.

### `diff` over a span of several windows pairs the moves of each window ([#423](https://github.com/wordstodata/words-to-data/issues/423))

A renumbering link is recorded for one window of two adjacent release points.
`diff` and `coverage` over a span of several windows read the links of that
exact pair of dates only, found none, and paired by position. Each
renumbering became a "changed" or a removal and an addition. `path` over the
same span read the move correctly, so the two did not agree.

Now `diff` and `coverage` read the renumbering links of every window inside
the span, in date order, and chain them as `path` does:

- A provision moved (R) to (S) in one window and not moved later is moved
  (R) to (S) over the span.
- A provision moved (R) to (S) and later (S) to (T) is moved (R) to (T).
- A provision under a container that moved is carried with it.
- A provision moved and then changed is a move with a change, as in one
  window.

A span of one window gives the same output as before.

#### Measured on `eval.sqlite`

Title 26, 2025-07-18 to 2026-07-12 (4 windows):

| | before | after | windows one by one |
|---|---|---|---|
| `diff` changed | 543 | 467 | 460 + 0 + 14 + 3 |
| `diff` added | 365 | 320 | 315 + 0 + 3 + 2 |
| `diff` removed | 161 | 104 | 104 + 0 + 0 + 0 |
| `diff` moved | 0 | 48 | 48 + 0 + 0 + 0 |
| `coverage` changed paths | 1,063 | 885 | 874 + 0 + 16 + 5 |

The 48 moves over the span are the 48 moves of the first window. The added
and removed paths over the span are the union of those of the windows. The
changed paths are the union less 7 paths that a window adds and a later
window rewords, which the span gives as added. The `diff`
output of each window alone does not change. Title 7 over the same span: 6
moves, before 0.

#### Breaking changes for agents

- `diff`, `coverage` and the library's `Dataset::compute_diff` over a span of
  several windows give moves, and fewer changed, added and removed paths.
- New: `Redesignations::then` and `Redesignations::new_path_of` are public.

### A tag inside a line no longer breaks the line ([#412](https://github.com/wordstodata/words-to-data/issues/412))

The markup reader put a newline in place of every tag. In Harvard's XML,
`5 U.S.C. §§ 8901</extracted-citation>(5), 8905` became `8901` and `(5)` on
two lines, and the U.S.C. reader does not read a subdivision after a line
break. Italic words broke lines too, so a search for "Amici Curiae" failed.

Now only a tag that ends a block, such as `<p>`, `<div>` or `<br>`, gives a
newline:

- A tag inside a line gives nothing: the citation tags (`extracted-citation`,
  `cross_reference`), the emphasis tags (`em`, `i`, `b`, `strong`, `s`), `a`
  and `span`. Where such a tag is the only thing between two words, as in
  `<s>tens of</s>thousands`, a space goes in.
- A footnote or page mark is a word of its own, with a space at each side:
  `footnotemark`, `page-number`, `sup`, and an `a` or a `span` of class
  `footnote`, `page-label` or `star-pagination`. `§ 183<footnotemark>1` reads
  `§ 183 1`, not `§ 1831`.
- A tag this reader does not name still gives a newline.

The text method goes to version 2. Paragraphs, pages and paths do not change.

Measured on the opinions of `evals/opinions.txt`, added to two copies of
`dataset-demo.sqlite`: the words of 204 of 3,428 passages change, and no
path. Each change is a newline that becomes a space (1,531) or goes (278, all
next to punctuation, such as `530⏎,` to `530,`). `judicial.cites` goes from
434 links to 435, all in the *Windsor* majority: § 2056 becomes § 2056(a),
§ 8901 becomes § 8901(5), § 115 becomes § 115(a)(1)(A), § 208 becomes
§ 208(a), and § 8905 is new.

### A citation reads a subdivision of more than one character ([#410](https://github.com/wordstodata/words-to-data/issues/410))

The U.S.C. reader and the default-code reader read a subdivision of one
letter or one or two digits only. They stopped before a longer designation,
with no report. `7 U.S.C. § 2015(o)(4)(A)(ii)` was a link to (A), not to
clause (ii).

Now both readers also read:

- a roman numeral of more than one letter: `(ii)`, `(xiv)`, `(II)`;
- a doubled letter (an item): `(cc)`;
- a number and a capital letter: `(14A)`.

A single letter is read as before. `(i)` can be a subsection or a clause, and
the printing decides. The ends of a range of subdivisions take the same
designations.

#### Measured on `eval.sqlite`

The citations of all opinions were read again from the cache, by the build
before this change and by the build after it: 434 `judicial.cites` links
before, 437 after. The 15 links that #410 lists are all changed:

- 12 links to a provision one level too high are replaced by links to the
  provision written: 10 to a clause (ii), 1 to a clause (iii)
  (`IRC section 280F(d)(4)(A)(iii)`) and 1 to a subclause (II)
  (`20 U. S. C. §1091b(b)(2)(C)(i)(II)`, before a link to (i)).
- 3 links stay, with one place fewer: the same passage also cites
  `2015(o)(4)(A)` alone. The place that writes `(A)(ii)` is now in a new link
  to clause (ii).
- So 15 links are new. All 15 were checked by hand: each ends at the
  provision its words write, and none has a part that does not resolve.
- 5 of the changed links are made by the default-code reader
  (`I.R.C. § 199A(c)(3)(A)(ii)`, `section 199A(g)(1)(B)(ii)`), and 10 by the
  U.S.C. reader. No other link changes.

#### Breaking changes for agents

- Method versions: `reporters-db laws.json U.S.C. patterns@6` (was 5) and
  `bare section under the opinion's default code@4` (was 3). A link records
  the version that made it. Link ids do not change, because an id does not
  hash the method.

### A few words do not show that an enacted block made a change ([#389](https://github.com/wordstodata/words-to-data/issues/389))

A provision that prints three words or fewer, such as the heading "In
general" or the chapeau "support—", is in many enacted blocks. Before, an
enacted block that held those words was evidence that it made the change, as
strong as a quoted string. On `eval.sqlite`, 22 U.S.C. 9613(g)(2)(E) had four
causes, and only § 8725 of Pub. L. 119-60 was right.

Now an enacted block that a change shows by three words or fewer, and that
holds more words than that, is weak evidence:

- It is not counted where other words show in the change. 9613(g)(2)(E) and
  the heading of 9613(g)(2) go to § 8725 alone.
- Where nothing else shows, the change goes with the provision around it,
  when step 2 gives that provision's changes. 26 U.S.C. 163(j)(12)(A) prints
  "In general" and moved with (12), which § 70341(c) of Pub. L. 119-21
  renumbered. It is no longer § 70341(a)'s.
- Where neither is so, the block decides, as before.

### Only a provision that was rewritten carries the changes inside it ([#390](https://github.com/wordstodata/words-to-data/issues/390))

Step 2 of the resolver gives an amendment the changes inside a provision it
was given. Before, this was so also when the amendment only edited some words
of the provision. § 70305(a) of Pub. L. 119-21 strikes "No deduction" in 26
U.S.C. 274(o), and it was given (o)(1) and (o)(2), which another law added in
the same window.

Now a provision carries every change inside it only when its amendment enacted
it anew, renumbered it, or inserted words that open a new provision, such as
*"(I) In general.—Subparagraphs"*. A provision whose words the amendment only
edited carries only the removal of a provision inside it: *striking "which"
and all that follows* can strike the clauses below.

### Measured on `eval.sqlite`

`link-by-evidence` at version 7 against version 6: 3,329 links become 3,288.
50 links are removed and 9 are added. 6 of the added links move a change to
another amendment.

- 45 removed links were given by step 2, and 5 by a weak block.
- 31 of the 45 are in 50 U.S.C. 3024, where an address fault (#387) puts the
  amendment of (c)(7) at the whole section. They were wrong.
- Of 21 removed links checked by hand, 17 were wrong before. 4 were right,
  and are lost: three edits in 16 U.S.C. 973i(a)(2), where the OLRC
  translated the struck cross-reference, so only the units the words name can
  place them, and the editors' note in the heading of 22 U.S.C. 9412(a).
- Of 9 added links, 6 were checked by hand. 5 are right. 1 is wrong: elimination now gives 22 U.S.C. 9613(b)(6) to
  § 8729 of Pub. L. 119-60, because step 2 now gives two other changes under
  its address to § 8725.

With the address fix below (#387) too, all links: 3,329 before, 3,224 after, on
a fresh build of `eval.sqlite`.

### An added provision is matched with the provisions below it ([#432](https://github.com/wordstodata/words-to-data/issues/432))

The diff lists a provision that a window added as one path, and nothing below
it, so the change holds every provision below it (#336). Before, the matcher
compared an enacted block with the added provision's own words only:

- A provision with no words of its own could not be shown. 10 U.S.C.
  9062(n) prints all its words in its paragraphs, and had no link.
- A few own words matched a wrong block. 10 U.S.C. 9062(j)(4) prints "In
  this subsection:", and the block of § 1632(a) of Pub. L. 119-60 holds those
  words, so § 1632(a) was given (j)(4) and not (n).
- A provision whose subparagraphs stand between its chapeau and its
  continuation was not one run of the block: 26 U.S.C. 904(b)(5) and (6).
- An amendment that only strikes could take an added provision with no own
  words, because such a change brought no new words.

Now an added provision's words are the words of each field of it and of every
provision below it, in the order the Code prints them. A block shows the
provision when each field is a run of the block, in that order. The Code
does not always print a block's words as the bill does (a marginal note
inside the words, "July 4, 2025" for "the date of enactment"), so a block
also shows it when the provision's own words are one run of the block, as
before. Where the own words are three or fewer, the block must also hold,
after them, a field below of more than three words.

#### Measured on a fresh build of `eval.sqlite`

`link-by-evidence` against `main` at 1b77f66, both on a copy of one fresh
build: 3,405 links become 3,417. 3 links are removed and 15 are added; 2 of
the added links move a change to another amendment of the same section.
Amendments stopped at resolving: 69 before, 63 after.

- The 3 removed links were wrong: 10 U.S.C. 9062(j)(4) to § 1632(a); 22 U.S.C.
  9613(b)(6) to § 8729 of Pub. L. 119-60 (the wrong link the #389 measure
  above names), which now has 9613(k), the subsection it adds; and 50 U.S.C.
  3024(e)(7) to an amendment that only strikes subsection (e).
- All 15 added links were checked by hand. 14 are right: 10 U.S.C. 9062(n),
  139(l), 806(a), 1563a(a)(4) and 4174(d); 22 U.S.C. 2651a(b)(4), 2651a(c)(5)
  and 9613(k); 26 U.S.C. 170(d)(2)(D), 904(b)(5), 904(b)(6), 1202(b)(5) and
  6041(h); 50 U.S.C. 3024(e)(7), to the amendment whose struck and inserted
  words are in it. 1 is partly right: 22 U.S.C. 2651a(q) to § 5162 of Pub. L.
  119-60. Ten amendments of the law build (q). § 5162 adds (q)(10), and
  § 5114 adds (q) itself. An added provision has one cause, and before, (q)
  had none.

### The address keeps the units the line names before a section of an Act ([#387](https://github.com/wordstodata/words-to-data/issues/387))

- **An amending line of the form "Subsection (c)(7) of section 102A of the
  National Security Act of 1947 (50 U.S.C. 3024)" now has the address
  50 U.S.C. 3024(c)(7).** Before, the address reader kept only the section from
  the publisher's reference and dropped the units that the line names. The
  citation's own designations stay too: "Section 1413(g) of the … Act of 2018
  (22 U.S.C. 9613)" is 9613(g). Where the reference gives designations that do
  not agree with the line, the reference is the authority, as before.
- On `eval.sqlite`, 21 amendments get a narrower address. Pub. L. 119-60
  § 6402(a) (`4bc796d6b54a`) had 120 `legislature.amended_by` links, and 117
  were wrong. Now it has the 3 under 3024(c)(7). § 8725 (`a518c65a04b0`) loses
  its 3 wrong links in 9613(f). The provisions they free go by elimination to
  the amendments that do act there (18 new links). All links: 3,329 before,
  3,227 after this fix alone. The renumbering reader records 11 more moves (296 to 307), in
  15 U.S.C. 278s(i)(2) and 22 U.S.C. 9613(g)(2).
- Method versions: `address, window and quoted words@7` (was 6; #387, #389,
  #390 and #432 share version 7) and `amendingAction type=redesignate@6` (was 5). Run `link-by-evidence` and
  `redesignations` again; `residue` then lists the links that only an older
  version made, for review.

Commands read a bill, a work, a window and a path in one way, and refuse an
input that the dataset does not hold.

### Fixes

- Inputs that the #392 and #395 checks did not reach
  ([#424](https://github.com/wordstodata/words-to-data/issues/424)):
  - `redesignations --between` refuses a first date that is not older than
    the second, and a date that no expression holds (the notice that `search
    --at` gives). Before, `--between 2025-07-30 2025-07-18` exited 0 and
    recorded a method run.
  - A date that is not zero-padded is read as the date the dataset holds:
    `2025-7-18` is `2025-07-18`, in `--at`, `--between` and every
    `<work>@<date>`. Before, `search --at 2025-7-18` said that the dataset
    holds no expression on that date, and named false dates around it.
  - `link-amendment --path` reads a path as the other commands do: a trailing
    `/` goes, and an ASCII hyphen in a section number is the en dash.
  - `link-amendment --amendment` and `amendment-addresses --amendment` take
    the start of an id, such as the 12 characters that `annotations` prints.
    A start that names more than one amendment is refused with the count, as
    in `settle`. When the bill holds no such amendment, the notice names each
    other bill that does. Before, `amendment-addresses --amendment 8` listed
    43 amendments.
  - `redesignations --bill-id` and `link-amendment --bill` refuse a bill not
    held with the notice that `residue` gives.
  - An id in upper case is read as the same id, in `settle --link`,
    `link-amendment --amendment` and `amendment-addresses --amendment`.
- `settle --explain` on a `judicial.cites` link prints no "Causes" line. A
  citation's subject is a passage, not a change, and the line counted the
  other provisions that passage cites as amendments: 219 of the 434 citation
  links in `eval.sqlite` printed "Causes: one of N amendments linked to this
  change" ([#411](https://github.com/wordstodata/words-to-data/issues/411)).
- `annotations --bill`, `section-agreement --bill` and `amendment-addresses
  --bill` refuse a bill that the dataset does not hold, with exit 1 and the
  notice that `residue` gives. `annotations` and `section-agreement` accept a
  bill that the dataset holds no record of when its links name the bill
  ([#392](https://github.com/wordstodata/words-to-data/issues/392)).
- `expressions --work` refuses a work not held, with the notice that `search
  --work` gives ([#392](https://github.com/wordstodata/words-to-data/issues/392)).
- `annotations`, `section-agreement`, `path`, `diff`, `coverage` and
  `redesignations` refuse a window with an expression not held (the notice
  names the held dates of the work), a window of two works, and a window with
  `--from` not older than `--to`
  ([#392](https://github.com/wordstodata/words-to-data/issues/392)).
- A span of more than one window gives the links of every window inside it.
  `annotations --from A@2025-07-18 --to A@2026-07-12` listed 0 links, and
  `coverage` over that span gave 0.0 %
  ([#388](https://github.com/wordstodata/words-to-data/issues/388)).
- A path argument loses a trailing `/`, and an ASCII hyphen in a section number
  is read as the en dash the dataset holds (`section_1400Z-1` is
  `section_1400Z–1`), in `path`, `annotations`, `search`, `diff`,
  `section-agreement` and `cases-citing`. `cases-citing --path …/section_174/`
  said that no opinion cites § 174 itself
  ([#395](https://github.com/wordstodata/words-to-data/issues/395)).
- `residue --bill` matches only the titles of the Code that the bill acts in,
  with the amendments of every law in those titles. Before, it matched every
  title and then kept the bill's rows. The output does not change. On
  `eval.sqlite`: `119-hr-42` 40 s to 1.0 s, `119-hr-618` 40 s to 0.45 s,
  `119-s-1071` 40 s to 31 s, `119-hr-1` 40 s to 37 s (it acts in most
  titles). The whole `residue` is 41 s, as before
  ([#398](https://github.com/wordstodata/words-to-data/issues/398)).
- The readers report the newest review of a link. After `settle --verdict
  refuted`, `annotations` and `path` printed the link as `[Pending]`, and
  `contradictions`, `coverage` and the "1 of N causes" count still read it as
  a standing link. On a copy of `eval.sqlite`, with 5ecc26e86618 refuted and
  § 8722 of `119-s-1071` linked at 22 U.S.C. 9613(b)(6), `contradictions`
  reported a disagreement there and told the reviewer to settle the link that
  was already refuted. Now:
  - `annotations` and `path` print the status of the newest review
    (refuted is `[Rejected]`, confirmed is `[Verified]`, disputed is
    `[Disputed]`), and each link line starts with the verdict, the reviewer
    and the date: `[refuted by agent:x on 2026-10-26, elimination]`.
  - `contradictions` leaves a refuted link out of every group, and says how
    many it left out ("Refuted: N link(s) left out").
  - `coverage` lists a change again when its only links are refuted, as
    `residue` lists their amendment again.
  - The cause count, in `annotations` and `settle --explain`, does not count a
    refuted link
  ([#415](https://github.com/wordstodata/words-to-data/issues/415)).

### Breaking changes for agents

- `Locator`'s window is a span: a link matches when the window it was recorded
  over is inside the span. A span of one window matches as before.
- A run that gave "0 annotation(s)" or "This dataset holds no amendment link to
  check." with exit 0 for an input not held now exits 1, with the notice on
  stderr and nothing on stdout.
- `annotations --json` and `path --json`: each entry in `links` has `status`
  and `review` (`record`, `verdict`, `reviewer`, `reasoning`, `at`, or
  `null`). The record's `status` is the status its links report, and `"Mixed"`
  when they differ. Before, it was the maker's stored state. The
  `contradictions --json` totals have `refuted_left_out` (#415).

### `contradictions` reads a citation at the place it was written ([#397](https://github.com/wordstodata/words-to-data/issues/397))

- **A passage that cites several provisions is not a disagreement.** A
  `judicial.cites` link says that the words at one place in a passage lead to a
  provision. `contradictions` now groups citation links by that place, and not
  by the passage alone. Two citation links disagree only when they read the same
  place and name different provisions. Before, paragraph 88 of *Torres* (T.C.
  2025), which cites 15 U.S.C. § 78u-6 and 26 U.S.C. § 199A and its parts, was
  a disagreement because two rules made its links. On the benchmark dataset the
  disagreements go from 8 to 7.
- A group of citation links names the words at its place: the text output has
  an `at the words "..."` line, and `--json` has the field `occurrence`. Groups
  of other kinds do not change.
- An `amended_by` link that is outdated (#185) still counts. It stands until a
  reviewer refutes it, so a disagreement between it and a newer link is a true
  report about the dataset. On the benchmark dataset all 7 disagreements are of
  this type: a link of `evidence_matching@4` against one of `@6`.

### `show-bill` says why it found no bill for a public law ([#394](https://github.com/wordstodata/words-to-data/issues/394))

- **Two refusals in place of one.** `show-bill <dataset> "Pub. L. 119-25"` said
  "This dataset holds no document of Pub. L. 119-25" also when the dataset held
  that document (`publiclawdocument_119-25`). Now it gives the true reason, with
  exit 1 in both cases:
  - The dataset holds no document of the law.
  - The dataset holds the document of the law, which the message names by its
    work, but no held bill became that law. A bill is joined to a law when the
    law's document holds the bill's amendments.
- **Correction to 0.6.0.** The 0.6.0 entry says "A law whose document the dataset
  does not hold is refused". The condition was wrong: `show-bill` refuses a law
  when no held bill became it, and that is also so for some laws whose document
  the dataset holds.

### `settle` shows a citation's evidence, and names the record it writes ([#393](https://github.com/wordstodata/words-to-data/issues/393))

- **`settle --explain` on a citation link shows its evidence.** Before, it said
  "This dataset holds the provision at neither end" for every `judicial.cites`
  link, also when the dataset held the cited provision. Now it shows the words
  the court wrote at each place in the passage, and the words of the cited
  provision in the printing the link was resolved in (`resolved_in`). When the
  dataset does not hold that printing, it says so.
- A link whose subject is not a change and that is not a citation gets a true
  message: its subject is not a change to a provision, so there are no words at
  two ends to show. "Neither end" is now said only for a change.
- **`settle --verdict` prints the id of the record it writes**, as
  `link-amendment` does: `Recorded a review: confirmed by human:jesse on
  2026-10-25: record <id>`. The id finds the review in `settle --explain` and
  in the readers.
- In the library, `inspect::EvidenceWords` has a new variant, `Cited`.

## 0.6.0

Better addresses for the laws that amend the Code, and bill commands that tell
the truth. The address reader reads "Such section" ([#364](https://github.com/wordstodata/words-to-data/issues/364))
and splits a list of instructions under a whole title into its items ([#365](https://github.com/wordstodata/words-to-data/issues/365)).
A citation of a range of subdivisions links each one ([#366](https://github.com/wordstodata/words-to-data/issues/366)).
`diff` no longer reports a change in format characters alone ([#363](https://github.com/wordstodata/words-to-data/issues/363)).
`bills` and `show-bill` name the public law, and their counts are true ([#297](https://github.com/wordstodata/words-to-data/issues/297)).

### Upgrade from 0.5.0

- **The stored format does not change** (`SCHEMA_VERSION` 10). A dataset built
  before this change opens and reads as before.
- **Run `link-by-evidence` again** to get the evidence links of version 6, and
  `redesignations` for each bill and window to record the renumberings of version 5.
  `residue` then lists each link that only an older version made, for review.
- **Add the opinions again with `add-opinions`** to get the citation links of
  version 5 (a cached response costs no request).
- **Change programs that read `bills --json` or `show-bill --json`**: two fields
  have new names (see "Breaking changes for agents").

### Breaking changes for agents

- `bills --json`: `amendments_with_changes` is now `amendments_linked`. The text
  column WITH CHANGES is now LINKED.
- `show-bill --json`: `change_count` is now `link_count`. The text "(N change(s))"
  is now "(N standing link(s))".
- Method versions: `address, window and quoted words@6` (was 5),
  `amendingAction type=redesignate@5` (was 4), the citation rule version 5 and the
  default-code rule version 3. A link records the version that made it.
- In a new build, 15 amendment ids of Pub. L. 119-60 are replaced by 113 (#365).

### `bills` and `show-bill` name the public law, and their counts are true ([#297](https://github.com/wordstodata/words-to-data/issues/297))

- `bills` has a PUBLIC LAW column (`Pub. L. 119-21`, or `not enacted / not held`),
  and `show-bill` has a `Public law:` line. In `--json`, both have the field
  `public_law` (`"119-21"`, or `null`). A bill is joined to its law by the
  amendments the law document states, so a bill that states no amendment shows
  no law.
- `show-bill` takes a public law in place of a bill id: `Pub. L. 119-21`,
  `Public Law 119–21`, `PL 119-21`, `P.L. 119-21` or `119-21`. A law whose
  document the dataset does not hold is refused, with exit 1.
- The false change counts are replaced. They counted word-level changes that no
  step writes since #252, so they were 0 for every bill and every amendment.
  - `bills`: the WITH CHANGES column is now LINKED, the amendments that at least
    one standing `legislature.amended_by` link names. In `--json`,
    `amendments_with_changes` is now `amendments_linked`.
  - `show-bill`: "(N change(s))" is now "(N standing link(s))", the standing
    links that name the amendment. In `--json`, `change_count` is now
    `link_count`.
  - A link stands unless its newest review refutes it, as for `residue`.

### "Such section" has an address ([#364](https://github.com/wordstodata/words-to-data/issues/364))

- **The address reader reads "Such section" and "Such subsection".** The phrase
  names the unit that the nearest earlier amending line in the same section of the
  bill names. Pub. L. 119-60 § 1702(b)(1), "Such section is further amended, in
  subsection (c)— (1) in paragraph (1) …", now has the address 17 U.S.C. 105(c)(1)
  from the bill's markup. Before, it had no section, and only the OLRC table gave
  it one.
  - "Such section" keeps the designations of the earlier citation: after "Section
    9062(j) of title 10", it is 9062(j).
  - "Such subsection" is the subsection that the earlier line names: after
    "Subsection (a) of section 1142", it is 1142(a).
  - A unit before the phrase is a step below it: "Subsection (f)(2) of such
    section" reaches (f)(2).
  - A line that ends with the phrase names the phrase's unit, also when its heading
    cites a section ("(1) Section 102a.—Such section is further amended—").
  - When no earlier line names a section, the reason is new: `"Such section" had no
    antecedent` (`no_antecedent` in `--json`).
- For 119-s-1071, `amendment-addresses` now addresses 504 of 977 amendments (was
  469). No amendment of 119-s-1071 or 119-hr-1 lost or changed an address.
- **The evidence method is now version 6** (`address, window and quoted words@6`).
  Run `link-by-evidence` again. `residue` then lists each link that only version 5
  made, for review.
- The redesignation readers do not read the phrase yet. A renumbering under "Such
  section" stays unplaced, as before.

### A list under a whole title is one amendment for each item ([#365](https://github.com/wordstodata/words-to-data/issues/365))

- **A list of instructions under a line that amends a whole title is one
  amendment for each item** ([#365](https://github.com/wordstodata/words-to-data/issues/365)).
  Pub. L. 119-60 § 1701(a) says "Title 10, United States Code, is amended as
  follows:", and each of its 43 paragraphs names the section it acts on. Before,
  the whole list was one amendment with no address. Now each item is an amendment,
  and its address is the title of the line and the section of the item:
  § 1701(a)(10), "In chapter 40, … by redesignating section 711 … as section
  710a", acts on `/us/usc/t10/s711`. The list splits only when its line names a
  title of the Code (or a part of one) and no section, and at least one item
  names a section. An item that names no section is still an amendment, with the
  reason "no section under amendment was named".
- `redesignations` records a renumbered **section**: § 711 of chapter 40 →
  § 710a. Title 10 holds a second § 711, in chapter 41, and the item's "In
  chapter 40" says which one moved.
- An item's own bare `Section 2101(2) is amended` takes its title from the list
  above it.
- A marginal note inside the words "is amended" (`is 10 USC prec. 101.
  amended`) no longer hides the amending line. § 3111(a) of Pub. L. 119-60 now
  reports "no section under amendment was named" (it adds a whole subpart),
  not "no level above the clause says what it amends".
- `In section 179—` ends the section number at the dash: § 179, not § 179-.
- **Amendment ids change for the lists that now split, in a new build only.** In
  Pub. L. 119-60, 15 amendments are replaced by 113. The 15 old ids stay in a
  dataset that already holds the bill: `add-bills` does not read a bill again
  that the dataset holds. No amendment of Pub. L. 119-21 (`119-hr-1`) changes id
  or address. A link on an old id is not carried to a new build;
  `carry-reviews` carries reviews of citation links only (#295).
- The renumbering reader is `amendingAction type=redesignate@5` (was 4), so
  `residue` lists the moves that only version 4 recorded, for review ([#365](https://github.com/wordstodata/words-to-data/issues/365)).

### A range of subdivisions is a citation of each one ([#366](https://github.com/wordstodata/words-to-data/issues/366))

- **A range of subdivisions after one section is a citation of each subdivision
  in it** ([#366](https://github.com/wordstodata/words-to-data/issues/366)).
  *Boeing*'s "26 U. S. C. §§994(a)(1)-(3)" is now three links, to § 994(a)(1),
  (a)(2) and (a)(3), and each place holds the whole written form. Before, the
  reader read "§§994(a)(1)" and lost (2) and (3). The ends can be numbers,
  letters or roman numerals, joined by a dash or by "through"
  ("(b)(2) through (b)(7)"). When the ends are at different levels, or the parent
  is not held, the link ends at the provision that holds both ends and the
  `unresolved` field names the range; a member that the printing does not hold
  is named in the same way. No member is guessed. A range of sections,
  "§§ 991-997", still links its two ends only (#309). The citation rule is now
  version 5, and the default-code rule version 3. On the benchmark dataset the
  citation links go from 427 to 434. Add the opinions again with `add-opinions`
  to get the new links. `cases-citing --cites` takes a range too, and asks about
  each member.

### Format characters alone are not a change ([#363](https://github.com/wordstodata/words-to-data/issues/363))

- **A difference in format characters alone is not a change**
  ([#363](https://github.com/wordstodata/words-to-data/issues/363)). A field whose
  two texts differ only in the characters that search does not read (the soft
  hyphen U+00AD, the zero-width characters U+200B, U+200C, U+200D and U+2060, and
  U+FEFF) is not listed as changed. Before, `diff` listed 13 U.S.C. § 9(b) as
  changed between 2025-07-18 and 2025-07-30 for one soft hyphen inside the word
  "records" alone, and `cases-citing` said § 9 CHANGED in that window. The
  rule is in the one comparison that every reader uses, so `diff`, `path`,
  `cases-citing`, `residue` and `coverage` agree. A difference in letters, digits,
  punctuation or white space is still a change. `diff --help` states the rule, and
  `diff --json` names the characters in a new field, `ignored_characters`. Over
  title 26 between 2025-07-18 and 2025-07-30, the library's `TreeDiff` finds one
  path fewer changed (536, not 537): the heading of § 7206(5)(B) differs only in a
  soft hyphen. `TreeDiff` does not pair a moved provision with its old place.
  `diff` pairs them, so over the same window it prints `Changed (460)`, with 315
  added, 104 removed and 48 moved.


## 0.5.0

Case law that an agent can use ([#312](https://github.com/wordstodata/words-to-data/issues/312),
[ADR 0014](docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md)).
A court opinion is now a case with its writings, pages and paragraphs. A citation
runs from the paragraph that holds it to the subdivision the court wrote. The
readers also say when a question names a path, a bill, a work or a date that the
dataset does not hold.

### Upgrade from 0.4.0

- **The stored format does not change** (`SCHEMA_VERSION` 10). A 0.4.0 dataset opens
  with 0.5.0, and its U.S. Code, bills, links and reviews read as before.
- **Rebuild a dataset that holds court opinions.** In a 0.4.0 dataset an opinion is
  `judicial/opinion_<id>`, and a citation link runs from the opinion to the section.
  0.5.0 does not migrate them, and `add-opinions` does not remove them. Every
  citation link id changes. So build a new dataset with 0.5.0, add the opinions
  again with `add-opinions` (a cached response costs no request), then run:

  ```
  words_to_data carry-reviews <new dataset> --from <old dataset>
  ```

  It carries each review of a citation link to each new link that says the same
  thing: the same opinion record, the same section and the same written form. The
  carried review keeps the verdict, the reviewer, the reason and the time, and
  names the old link and the old review. A review that it cannot carry is listed
  with the reason, and nothing is written for it. The old dataset is only read.
  Reviews of other links are not carried yet ([#295](https://github.com/wordstodata/words-to-data/issues/295)) ([#319](https://github.com/wordstodata/words-to-data/issues/319), [#350](https://github.com/wordstodata/words-to-data/issues/350)).
- **A dataset with no opinions does not need a rebuild.** Run `redesignations` and
  `link-by-evidence` again to make the links of the new method versions. `residue`
  then lists each link that only an older version made, for review.

### Release binaries

- **Each release has the `words_to_data` command** for Linux (x86_64 and aarch64,
  static), macOS (Apple silicon and Intel) and Windows (x86_64), as an archive and as
  one `.gz` file. No Rust toolchain is needed ([#359](https://github.com/wordstodata/words-to-data/pull/359), [#360](https://github.com/wordstodata/words-to-data/pull/360)).

### Case law

- **A case is a work** ([#313](https://github.com/wordstodata/words-to-data/issues/313), [#329](https://github.com/wordstodata/words-to-data/issues/329)). A CourtListener cluster is
  `judicial/case_<cluster>`, and each writing is a child,
  `judicial/case_<cluster>/opinion_<id>`, with its type and author. The case holds
  the case name, the reporter citations, the panel and the docket. Head matter,
  a syllabus, a staff summary and a Reporter's Note are children of the case, not
  part of a writing. `add-opinions` reads each case whole, so a dissent is never
  stored without the Court's opinion.
- **Split records are the text.** A combined record is kept only as a source when
  the case has split records ([#313](https://github.com/wordstodata/words-to-data/issues/313), [#329](https://github.com/wordstodata/words-to-data/issues/329)).
- **Pages and paragraphs** ([#314](https://github.com/wordstodata/words-to-data/issues/314), [#332](https://github.com/wordstodata/words-to-data/issues/332)). The text comes first from `xml_harvard`, then
  `html_columbia`, and the other fields after them. A writing read from those two
  fields holds `page_<p>/paragraph_<n>` and `footnote_<label>`, where `p` is a page
  of the official reporter. When the text marks the pages of two reporters in one
  run, only the official reporter's pages are paths ([#341](https://github.com/wordstodata/words-to-data/issues/341), [#353](https://github.com/wordstodata/words-to-data/issues/353)).
- **Combined and plain-text records are split by rule** ([#315](https://github.com/wordstodata/words-to-data/issues/315), [#334](https://github.com/wordstodata/words-to-data/issues/334), [#335](https://github.com/wordstodata/words-to-data/issues/335), [#352](https://github.com/wordstodata/words-to-data/issues/352)).
  `justice-headings`, `running-heads` and `judge-headings` find the writings of a
  case that CourtListener holds as one record. `plain-text-paragraphs` reads
  paragraphs and footnotes from `plain_text`. `staff-summary` keeps a Ninth Circuit
  staff summary out of the writing. Each result is `MachineSuggested`, and
  `opinions` and `path` say which rule found it.
- **A citation runs from its paragraph to the subdivision the court wrote** ([#316](https://github.com/wordstodata/words-to-data/issues/316),
  [#355](https://github.com/wordstodata/words-to-data/issues/355)). The link starts at the paragraph or footnote that holds the citation, and
  it ends at the deepest path the written subdivision resolves to. A part that does
  not resolve is named, not repaired. The payload holds every place in the passage
  where the court wrote the citation.
- **A citation carries its date** ([#317](https://github.com/wordstodata/words-to-data/issues/317), [#356](https://github.com/wordstodata/words-to-data/issues/356)). The payload holds the decision date
  and the printing that the citation was resolved in: the latest printing held on
  or before the decision, or else the earliest. `cases-citing` reads its windows
  from the decision date, and says when the law as it stood on that date is out of
  scope.
- **Bare section references** ([#318](https://github.com/wordstodata/words-to-data/issues/318), [#354](https://github.com/wordstodata/words-to-data/issues/354)). When a case states that its references
  are to the Internal Revenue Code, "section 199A" and "I.R.C. § 199A" are read as
  title 26 citations. The evidence names the sentence and the writing that states it.
- **The citation reader reads more forms** ([#309](https://github.com/wordstodata/words-to-data/issues/309), [#324](https://github.com/wordstodata/words-to-data/issues/324), [#341](https://github.com/wordstodata/words-to-data/issues/341), [#353](https://github.com/wordstodata/words-to-data/issues/353)): spaced
  subdivisions ("§ 174 (a) (1)"), ranges ("§§ 991-997" links each end), "U.S.C"
  with no last point, a section with a subdivision and no "§", and the OCR form
  "26 V.S.C.". The evidence of a repaired link says
  `(OCR repair: "V.S.C." read as "U.S.C.")`.
- **A citation of a stub is flagged** ([#310](https://github.com/wordstodata/words-to-data/issues/310), [#325](https://github.com/wordstodata/words-to-data/issues/325)). When the cited section is now a
  Repealed, Renumbered, Transferred or Omitted stub, `cases-citing`, `path` and
  `annotations` give its heading.
- **`opinions`** lists the cases and writings a dataset holds ([#308](https://github.com/wordstodata/words-to-data/issues/308), [#321](https://github.com/wordstodata/words-to-data/issues/321)). **`path`**
  on a case or a writing gives every stored fact, and `not recorded` for a fact the
  publisher did not give ([#327](https://github.com/wordstodata/words-to-data/issues/327), [#330](https://github.com/wordstodata/words-to-data/issues/330)).
- **`cases-citing --json`** is new ([#302](https://github.com/wordstodata/words-to-data/issues/302), [#303](https://github.com/wordstodata/words-to-data/issues/303)). `cases-citing` reads the subdivision
  the court wrote, and puts a citation of a provision above the asked path in its
  own group. Its windows can no longer give a wrong answer from a true line: it
  names a chapeau that changed above an unchanged subdivision, marks a window that
  holds the decision date, and says why there is no window ([#340](https://github.com/wordstodata/words-to-data/issues/340), [#344](https://github.com/wordstodata/words-to-data/issues/344)).

### Readers and agents

- **`path` and `annotations` show citations from both ends** ([#301](https://github.com/wordstodata/words-to-data/issues/301), [#304](https://github.com/wordstodata/words-to-data/issues/304)): on a writing,
  the sections it cites; on a Code path, the writings that cite it.
- **A path that is not held is named** ([#305](https://github.com/wordstodata/words-to-data/issues/305), [#320](https://github.com/wordstodata/words-to-data/issues/320), [#326](https://github.com/wordstodata/words-to-data/issues/326), [#328](https://github.com/wordstodata/words-to-data/issues/328)). `diff`, `search`,
  `annotations` and `path` say so, and give the nearest held path.
- **A bill, work, date or file that is not held is named** ([#337](https://github.com/wordstodata/words-to-data/issues/337)). `votes`,
  `show-bill`, `residue --bill` and `redesignation-report --bill-id` refuse a bill
  the dataset does not hold, and name the held bills with the same number ([#343](https://github.com/wordstodata/words-to-data/issues/343)).
  `search` names a work or a date that is not held, and the held dates around it
  ([#346](https://github.com/wordstodata/words-to-data/issues/346)). A command refuses a dataset path that names no file, and makes no file
  ([#342](https://github.com/wordstodata/words-to-data/issues/342)).
- **`search` counts each occurrence** ([#306](https://github.com/wordstodata/words-to-data/issues/306), [#322](https://github.com/wordstodata/words-to-data/issues/322)). `--occurrences` lists each one with
  its offset, and `--offset` reads a window at an offset.
- **`search` matches the forms the text holds** ([#307](https://github.com/wordstodata/words-to-data/issues/307), [#323](https://github.com/wordstodata/words-to-data/issues/323), [#338](https://github.com/wordstodata/words-to-data/issues/338), [#348](https://github.com/wordstodata/words-to-data/issues/348)). A run of
  white space matches one space. "U.S.C." matches "U. S. C.", "U.S." matches "U. S.",
  and "§7" matches "§ 7". Soft hyphens and zero-width characters are not read,
  ligatures read as their letters, and a hyphen between two letters is not read, so
  "substantially" finds "substan-tially".
- **`release-points`** lists the dates the mirror offers and the cache holds ([#292](https://github.com/wordstodata/words-to-data/issues/292),
  [#299](https://github.com/wordstodata/words-to-data/issues/299)). **`declare`** sets the declaration of an existing dataset ([#293](https://github.com/wordstodata/words-to-data/issues/293), [#298](https://github.com/wordstodata/words-to-data/issues/298)).
- **A read command opens a SQLite dataset with no write**, so it does not stop with
  `database is locked` while another reader holds the file ([#331](https://github.com/wordstodata/words-to-data/issues/331), [#333](https://github.com/wordstodata/words-to-data/issues/333)).
- Tests and documents name the bill `119-hr-1`, the id that `build-dataset` gives
  ([#215](https://github.com/wordstodata/words-to-data/issues/215), [#300](https://github.com/wordstodata/words-to-data/issues/300)).

### Linking

- **An address inside an added provision** ([#336](https://github.com/wordstodata/words-to-data/issues/336)). `residue` no longer calls such an
  amendment quiet, and `diff --path` names the added provision that holds the path
  ([#345](https://github.com/wordstodata/words-to-data/issues/345)). `link-by-evidence` links an amendment that only adds to the added
  provision that holds its address ([#347](https://github.com/wordstodata/words-to-data/issues/347)).
- **The address reader keeps every container the bill states** ([#339](https://github.com/wordstodata/words-to-data/issues/339), [#349](https://github.com/wordstodata/words-to-data/issues/349)): "in
  subsection (c)", "Section 6213(b)(1) of the Internal Revenue Code", "Paragraph (2)
  of subsection (f) of section 149". A provision that the quoted words placed no
  longer takes a change that another amendment is addressed to.

### Breaking changes for agents

- **Court paths.** `judicial/opinion_<id>` is now `judicial/case_<cluster>/opinion_<id>`,
  and the words are in `page_<p>/paragraph_<n>`, `paragraph_<n>` and
  `footnote_<label>` below it. A writing that a rule found is `opinion_<id>-<n>`.
- **New node types**: `judicial.case`, `judicial.page`, `judicial.paragraph`,
  `judicial.footnote`, `judicial.headmatter`, `judicial.syllabus`,
  `judicial.summary` and `judicial.reporters_note`.
- **The citation payload.** The `section` key is replaced by `occurrences[]`, one
  entry for each place: `start`, `written`, `words`, and `unresolved` when a part
  does not resolve. New keys: `decided`, `resolved_in`, `resolved_after_decision`.
  The subject is a paragraph or a footnote, and the object can be below the section.
  Every citation link id changes.
- **`cases-citing` output.** The text groups changed. `--json` is new in this
  release, and its shape changed before the release: `opinions_held` is
  `cases_held`, and `cites[].citation_text` and `cites[].verification` are in
  `cites[].places[]`. Read the 0.5.0 shape.
- **`search` output.** The last line is `N fields, M occurrences`, not `N match(es)`.
  The new match rule finds more fields. `search --at` takes a date only.
- **Exit codes.** `diff --path` on a path that neither end holds exits 1. A bill that
  is not held exits 1 for `votes`, `show-bill`, `residue --bill` and
  `redesignation-report --bill-id`. A dataset path that names no file exits 1 for
  every command except `build-dataset` and `convert-dataset`.
- **Method versions.** The citation rule is `reporters-db laws.json U.S.C. patterns@4`
  (was 1). The evidence method is `address, window and quoted words@5` (was 4). The
  renumbering reader is `amendingAction type=redesignate@4` (was 3). New:
  `bare section under the opinion's default code@2`, `citation-review-carry@2`,
  `justice-headings@1`, `running-heads@2`, `plain-text-paragraphs@2`,
  `judge-headings@1` and `staff-summary@1`.
- **Library.** `DocumentReader` has a new method, `holds_any`. A storage search
  returns `SearchAnswer`, not `Answer<SearchResult>`. `ReleaseSource::new` replaces
  `from_mirror` and `cached_only`. `Dataset::open_sqlite` and `Dataset::load` return
  `DatasetError::NotFound` for a path that names no file.

### Known limits

- `carry-reviews` carries only the reviews of citation links ([#295](https://github.com/wordstodata/words-to-data/issues/295)). `settle
  --explain` and `path` do not yet show the link a review was carried from.
- The reader reads "§§ 994(a)(1)-(3)" as "994(a)(1)", so the range is lost.
- `time_basis` is set only on bare references read under a default sentence.
- The court of a case is not recorded ([#53](https://github.com/wordstodata/words-to-data/issues/53)).
- In Tax Court `plain_text`, some footnotes stay in the body. The watermark "Page
  Proof Pending Publication" is in the words of a preliminary print.
- `diff` reports a change when the only difference is a soft hyphen ([#338](https://github.com/wordstodata/words-to-data/issues/338)).
- The address reader does not resolve "such section" or "such subsection", and a
  clause that names its own chapter and section gets no address ([#339](https://github.com/wordstodata/words-to-data/issues/339)).

## 0.4.0

A new foundation. Almost everything changed since 0.3.0, and **nothing from 0.3.0
carries over**: rebuild your datasets with this version.

### What words-to-data is now

**Your agent's favorite legal database.** The U.S. Code and the laws that change it, as a dataset with evidence behind every
link. A dataset records which amendment made which change, who said so, by which
method at which version, and who has checked it. People and agents can query it,
check it and correct it with the command alone.

### Highlights

- **Matching makes no model call.** An amendment is placed from the bill's own
  markup, the OLRC's classification tables, the first window after the law's
  enactment in which its address changed, and the words it quotes. The model-based
  commands `extract-changes`, `score-amendments` and `match-amendments` are removed
  ([ADR 0013](docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md)).
  On Pub. L. 119-21 it links 478 of 603 amendments; the model pipeline linked 435, and
  it made errors this method does not.
- **Datasets grow.** `add-release-points` adds release points and `add-bills` adds
  laws to an existing dataset. A grown dataset gives the same links as one built from
  scratch.
- **Every link can be checked and corrected.** `settle --explain` shows the words, the
  OLRC's classification, and how the link was made. `settle` records a verdict as its
  own record ([ADR 0012](docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md)).
  `link-amendment` records a link or a no-link conclusion, and refuses a path that did
  not change.
- **The work list.** `residue` lists the amendments that no standing link names, with
  the reason each one stopped. It also lists the links the current version of a method
  no longer makes.
- **For agents.** [`docs/agents/working-a-dataset.md`](docs/agents/working-a-dataset.md)
  is a playbook for any agent. It was tested with fresh agents that had only the
  playbook, the binary and a dataset.
- **SLEUTH, the official agent.** [SLEUTH-pi](https://github.com/wordstodata/sleuth-pi)
  is a package for the pi coding agent that follows the playbook, with a tool for
  each command and a read-only default.

### Also new

- A SQLite store beside compact JSON. `build-dataset` writes SQLite when the
  output is named `.sqlite` or `.db`, and `convert-dataset` moves between the two.
- Progress bars, colour, and a time for each step in the command line. No long step
  runs in silence.
- Court opinions from CourtListener, with their U.S.C. citations as links
  (`add-opinions`, `cases-citing`).
- Readers: `search` (scoped, bounded, with snippets), `diff`, `path`, `annotations`,
  `votes`, `show-bill`, `amendment-addresses`, `info`, `validate`,
  `section-agreement`, `contradictions`, `redesignation-report`, `coverage`.
- Each method has a name and a version. A dataset records which method ran over
  which window.
- A bill's member records are fetched eight at a time. A rate-limit answer stops
  the download, so no member is left out with no warning.
- Network requests retry over IPv4 when an IPv6 route is unreachable. After one
  such answer, every later request goes over IPv4 at once: on some networks the
  error for parallel IPv6 connections comes only once a second.

### Breaking

- The stored format is new (`SCHEMA_VERSION` 10). Datasets from 0.3.0 cannot be read.
- The Python bindings and the annotator prototype are removed.
- `extract-changes`, `score-amendments` and `match-amendments` are removed. Datasets
  made with them still read, and their links still show.

### Known limits

- No command gives a single member's vote by name. `votes` counts by party.
- An amendment that is only partly linked does not show in `residue`. `coverage`
  shows its unlinked change.
