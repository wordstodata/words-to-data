# A case is a work, and a citation runs from a paragraph to the subdivision the court wrote

Status: accepted

Supersedes the field order of `docs/adr/0008-an-opinions-text-is-one-named-field-chosen-for-fidelity.md`. Keeps its other rules.

An opinion today is one work per CourtListener opinion record, `judicial/opinion_<id>`, with one flat node holding the whole text. A `judicial.cites` link runs from that node to a U.S. Code **section**, with no date. The written subdivision is only a payload string, and a second citation of the same section in the same opinion replaces the first (ADR 0004 identity). Ten SLEUTH runs on a dataset with opinions (2026-10-21) and three measurements of CourtListener data showed what that costs an agent:

- **It gives a dissent's words to the Court.** A search hit inside the Chief Justice's dissent in *Obergefell* shows only `judicial/opinion_2812209`. One case is often two or three records (*Boeing*: one combined record and two split ones), joined only by a payload `cluster_id`.
- **It reads whole opinions to find one sentence.** Nine of ten questions read 6k–51k characters because nothing smaller than the opinion has a path.
- **It answers about the wrong law.** *Shami* (2014) cites a § 174(e) that no held printing has; *Snow* (1974) and *Harris* (1994) cite a § 174(a)(1) whose current words are later law; *United Stationers* cites § 44F, now a "[Renumbered § 30]" stub. The link says none of this.
- **It loses citations.** In ten opinions, 71 citations become 37 links and 14 of 51 written forms are gone.

Three of the remedies each change the identity of every citation link: a new unit of work changes every subject, a citation that starts below the opinion changes every subject again, and a citation that ends below the section changes many objects. Taken one at a time they would rebuild the corpus three times. This ADR takes them together.

## The decision

### 1. The work is the case

A CourtListener cluster is the work: `judicial/case_<cluster id>`. Each writing is a child, `opinion_<opinion id>`, with its type (majority, concurrence, dissent, …) and author as node facts. A reporter citation such as 537 U.S. 437 belongs to the case. Matter that the source says is no part of the opinion (the syllabus, headnotes, a court staff summary, headmatter) is a child of the case with a node type that says so, never part of a writing's text.

### 2. Split records are the text; a combined record is a source

When a case holds both a combined record and split records of the same words, the split records are the writings, and the combined record is kept only as a provenance source. Search then never returns the same words twice. When a case has only a combined record, a rule may split it into writings (Supreme Court running heads, "JUSTICE X, dissenting" lines); a writing split by a rule is `MachineSuggested`. A combined record the rule cannot split stays one writing, and its node says so.

### 3. Structure comes first in the field order

```
xml_harvard > html_columbia > html_anon_2020 > html_lawbox > html > plain_text
```

ADR 0008 ranked for the best characters because nothing parsed structure. Structure is now the point: writings, authors, paragraphs with stable ids and page labels are in `xml_harvard` (about two thirds of cases) and `html_columbia`, and in nothing else. ADR 0008's other rules stand unchanged: the provenance names the record and the field, `xml_harvard` and any `extracted_by_ocr` record are `ocr` and `MachineSuggested`, markup is stripped, and every character reference that cannot be decoded is reported. A writing read from `plain_text` gets paragraphs by rule and says so in its provenance.

### 4. Paths inside a writing follow the official reporter's pages

```
judicial/case_<cluster>/opinion_<id>/page_<p>/paragraph_<n>
judicial/case_<cluster>/opinion_<id>/footnote_<label>
```

`p` is a page of the official reporter (U.S. Reports, F.2d, T.C., …) when the source labels it; `n` counts the paragraphs that start on that page. So a pin cite, "537 U.S. at 459", is a path. A paragraph that runs over a page break belongs to the page it starts on. The source's own paragraph id (CAP's `b693-8`) is kept as a node fact, so a later import can match paragraphs again.

An opinion with no reporter pages (a slip opinion) has no page level: `opinion_<id>/paragraph_<n>`. When it later gets reporter pages, that is a new expression of the same work, and links in the redesignation form map each old path to its new one. A path is never rewritten in place.

### 5. A citation starts at the paragraph that holds it

The subject of a `judicial.cites` link is the paragraph or footnote whose text holds the citation. Which writing cited what, and the passage around it, are then readable without reading the opinion. A reader at the writing or the case still sees every citation, because readers already report the links beneath a path.

### 6. A citation ends at the subdivision the court wrote

The object is the deepest path that the written citation resolves to: § 174(a)(1) is `…/section_174/subsection_a/paragraph_1`. Each written subdivision is its own link, so a reviewer can refute `41(d)(l)(B)(i)` without refuting § 41. Every place in the paragraph where the court wrote it goes into the payload (offset and the words), so nothing is lost when one paragraph cites the same subdivision twice. A written subdivision that does not resolve in the printing used ends at the deepest part that does, and the payload says which part did not.

### 7. A citation carries the date the court read the law

Each citation carries the decision date and, when the opinion states one, the year the court applied ("as in effect for 1986"). The object is resolved against the printing held nearest that date, and readers say what has changed since and which period no printing covers (the `UncoveredPeriod` that `cases-citing` already reports). The mirror's printings start in 2020, so an older citation is resolved against the earliest printing held and says so. Binding a citation from 2020 or later to a printing of its own date is allowed later and needs no new decision.

### 8. An opinion's own default may name the title of a bare reference

Tax opinions write "section 174", "sec. 174" or "I.R.C. § 174", and state once that "section references are to the Internal Revenue Code … in effect for 1986". When an opinion has such a sentence, those references are read as citations to that title, with that year as their time basis. They are `MachineSuggested`, and their evidence is the citation and the sentence. An opinion without such a sentence gets no bare reads.

### 9. The rebuild carries reviews across

These changes give every citation link a new id once. The rebuild writes a map from each old link to the new links that say the same thing (old opinion, section and written form → new paragraph and subdivision), and carries each review across as a review of the new link whose provenance names the old one (ADR 0012). This is the first use of the remedy #295 asks for in general.

## Consequences

**Every stored opinion and citation link changes.** `judicial/opinion_<id>` paths, the `judicial.cites` subject and object, and their ids. Datasets are rebuilt, never migrated, and decision 9 is what keeps reviews from being lost in that rebuild. An External reference to an opinion the dataset does not hold, `judicial.opinion:<id>`, keeps its form.

**Text quality goes down where structure goes up.** Harvard's text is OCR. A citation inside it can read `(l)` for `(1)`; the citation reader must report what it repairs and never repair silently.

**There are more links.** One for each written subdivision in each paragraph, not one for each section in each opinion. Readers group by section when asked about a section.

**`cases-citing` reads dated links.** The windows it reports start at the date each citation carries, not at the opinion's date read from elsewhere.

## Considered and rejected

**Keep the opinion record as the work and add structure inside it.** No link ids change. A case stays split across works, a reporter citation has no single home, and the duplicate copies of one writing stay searchable twice.

**Keep every record CourtListener has as text.** Loses nothing, and every search and citation count says the same words twice.

**Take structure from `xml_harvard` as a map onto the cleaner text of another field.** The best characters and the structure together, at the cost of an alignment that fails on some records with no clean way to say where.

**Use any pagination the source carries.** The same pin cite would mean different paths in different cases.

**Start a citation at the writing or the case.** Fewer links. At the case, a dissent's citation reads as the Court's.

**Keep one link per section and put every written form in its payload.** No ids change and nothing is lost, but a subdivision is never a path, so it cannot be reviewed, diffed or asked about.

**Resolve against the latest printing and say nothing about time.** What the dataset does now, and how *Shami*'s § 174(e) and *Obergefell*'s 1 U.S.C. § 7 came to point at law the court never read.

**Accept the loss of reviews in the rebuild.** There are few human reviews of citations today. There will be more, and a rule that lets a rebuild drop them is the rule #295 exists to remove.
