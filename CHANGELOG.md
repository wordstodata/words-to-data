# Changelog

## Unreleased

Commands read a bill, a work, a window and a path in one way, and refuse an
input that the dataset does not hold.

### Fixes

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

### Breaking changes for agents

- `Locator`'s window is a span: a link matches when the window it was recorded
  over is inside the span. A span of one window matches as before.
- A run that gave "0 annotation(s)" or "This dataset holds no amendment link to
  check." with exit 0 for an input not held now exits 1, with the notice on
  stderr and nothing on stdout.

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
  title 26 between 2025-07-18 and 2025-07-30, one path fewer is changed (536, not
  537): the heading of § 7206(5)(B) differs only in a soft hyphen.


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
