# CourtListener / Free Law Project case law: text formats and recoverable structure

Research date: 2026-09-04.
Measurements use the bulk snapshot `2026-06-30`, which was the most recent one at that date.

Scope: this note answers seven questions about how much document structure the
CourtListener corpus gives you for free, and how much you must build. Every claim has
an inline link to the source that owns it. Where a source is unclear or missing, the
note says so.

Two general cautions about sources:

- The CourtListener help pages moved. `courtlistener.com/help/api/rest/` now redirects
  (HTTP 301) to `wiki.free.law`. Old URLs in other documents are stale.
- The Django models are the authority for field names and behaviour. The help pages
  simplify and are sometimes out of date with the code.

---

## 1. Which fields carry opinion text, and in what order of preference

### The fields

All text fields live on the `Opinion` model in
[`cl/search/models.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/search/models.py).
The `help_text` strings below are verbatim from that file:

| Field | `help_text` in the model |
|---|---|
| `plain_text` | "Plain text of the document after extraction using pdftotext, wpd2txt, etc." |
| `html` | "HTML of the document, if available in the original" |
| `html_lawbox` | "HTML of Lawbox documents" |
| `html_columbia` | "HTML of Columbia archive" |
| `html_anon_2020` | "HTML of 2020 anonymous archive" |
| `xml_harvard` | "XML of Harvard CaseLaw Access Project opinion" |
| `xml_scan` | "XML of Scanning Project" |
| `html_with_citations` | "HTML of the document with citation links and other post-processed markup added" |

Note that `xml_scan` exists in the model and in the bulk SQL schema but is **not**
documented on the API help page
([`wiki.free.law/.../v4/case-law`](https://wiki.free.law/c/courtlistener/help/api/rest/v4/case-law)).
It relates to `OpinionCluster.filepath_xml_scan`, whose `help_text` is "The XML obtained
from LLM containing all available metadata and opinion(s)."

### There are three different preference orders in the codebase

This matters, and it is a genuine inconsistency, not a documentation slip.

**(a) The documented order.** The help page says: "In general, the best field for the
text of a decision is `html_with_citations`, in which each citation has been identified
and linked." and "The `html_with_citations` field contains the raw text of the decision,
and is the most reliable field for most purposes."
([case-law API docs](https://wiki.free.law/c/courtlistener/help/api/rest/v4/case-law))

**(b) The `best_text` order** used by `Opinion.objects.with_best_text()` and
`Opinion.clean_text`, defined as `OPINION_TEXT_SOURCE_FIELDS` in
[`cl/search/models.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/search/models.py):

```python
OPINION_TEXT_SOURCE_FIELDS = [
    "html_with_citations",
    "xml_harvard",
    "html_columbia",
    "html_lawbox",
    "html_anon_2020",
    "html",
]
```
with `plain_text` as the final fallback.

**(c) The citation-extraction order** in `make_get_citations_kwargs` in
[`cl/citations/utils.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/utils.py),
which is different again:

```python
for attr in [
    "xml_harvard", "html_anon_2020", "html_columbia",
    "html_lawbox", "html", "plain_text",
]:
```

**(d) The website render order** in
[`cl/opinion_page/templates/includes/opinion_tabs_content.html`](https://github.com/freelawproject/courtlistener/blob/main/cl/opinion_page/templates/includes/opinion_tabs_content.html),
which is: `xml_harvard AND html_with_citations` → `html_with_citations`; else
`xml_harvard`; else `html_with_citations`; else `html_columbia`; else `html_lawbox`;
else `html_anon_2020`; else `html`; else `<pre>{{ plain_text }}</pre>`.

**Practical conclusion.** `html_with_citations` is the best field for *citation links*.
It is a poor field for *structure*, because for a PDF-sourced opinion it is only the
plain text wrapped in `<pre class="inline">` (see §3). If you want structure, prefer the
original source field:

```
xml_harvard  >  html_anon_2020  >  html_columbia  >  html_lawbox  >  html  >  plain_text
```

and use `html_with_citations` only as a parallel stream for citation offsets. That is
the same order as (c), which is what Free Law Project themselves use when they need to
map markup back to plain text.

### Which is populated most often

The API docs do not give counts. I measured it directly from the bulk snapshot.

I downloaded
[`bulk-data/opinion-clusters-2026-06-30.csv.bz2`](https://com-courtlistener-storage.s3-us-west-2.amazonaws.com/list.html?prefix=bulk-data/)
(2.457 GB) and counted the `source` column over every row. `source` is a set of
one-letter provenance codes defined in
[`cl/search/cluster_sources.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/search/cluster_sources.py);
a cluster can carry several letters if records were merged.

Total clusters: **10,070,727**. Letters, counted by presence (so rows are counted more
than once when merged):

| Code | Meaning | Clusters | Share |
|---|---|---:|---:|
| `U` | Harvard, Library Innovation Lab Case Law Access Project | 6,627,464 | 65.81% |
| `C` | court website | 2,112,657 | 20.98% |
| `L` | lawbox | 1,608,032 | 15.97% |
| `Z` | columbia archive | 869,311 | 8.63% |
| `G` | recap | 739,119 | 7.34% |
| `R` | public.resource.org | 622,441 | 6.18% |
| `Q` | 2020 anonymous database | 23,163 | 0.23% |
| `D` | direct court input | 3,069 | 0.03% |
| `M` | manual input | 45 | 0.00% |

76.06% of clusters have a single-letter source. No cluster in this snapshot carries
`S` (scanning project) or `A` (internet archive).

Cross-check: exactly 6,627,464 clusters have a non-empty `filepath_json_harvard`, which
is the same as the `U` count. That is a good internal consistency signal.

Other cluster fields, same snapshot:

| Field | Non-empty | Share |
|---|---:|---:|
| `filepath_json_harvard` | 6,627,464 | 65.81% |
| `judges` | 5,665,507 | 56.26% |
| `filepath_pdf_harvard` | 5,544,922 | 55.06% |
| `attorneys` | 5,035,289 | 50.00% |
| `headmatter` | 2,292,908 | 22.77% |
| `summary` | 1,281,686 | 12.73% |
| `headnotes` | 967,107 | 9.60% |
| `disposition` | 431,605 | 4.29% |
| `syllabus` | 294,967 | 2.93% |

`precedential_status`: 82.57% `Published`, 9.74% `Unpublished`, 7.66% `Unknown`.

**Read this as:** two thirds of the corpus is Harvard CAP, so `xml_harvard` is by a
wide margin the most common structured field. Roughly one fifth comes from court
websites, which lands in `plain_text` or `html`. `html_anon_2020` is rare (0.23%)
despite being prominent in the code.

I did **not** measure per-field population across all opinions. The
`opinions-*.csv.bz2` file is 54.56 GB compressed and bzip2 is not seekable, so I could
only decode a contiguous prefix. That prefix is not a random sample and its field
shares are wrong (it is dominated by one import batch). I report only its *markup*
findings in §3, not its field shares.

---

## 2. Where each format comes from, and what quality follows

Verbatim from the [case-law API docs](https://wiki.free.law/c/courtlistener/help/api/rest/v4/case-law):

- `plain_text`: "will be populated if we got the opinion from a court's website as a PDF or Microsoft Word document."
- `html`: "will be populated if we got the opinion from a court's website as a Word Perfect or HTML document, or if we got the opinion from Resource.org, which provided HTML documents."
- `html_lawbox`: "will be populated if we got the content from the Lawbox donation."
- `html_columbia`: "will be populated if we got the content from the Columbia collaboration."
- `html_anon_2020`: "will be populated if we got the content from our anonymous source in 2020."
- `xml_harvard`: "will be populated if the source was Harvard's Caselaw Access Project. This field has a lot of data but is not always perfect due to being created by OCR."

### Scraping and extraction

Scraping is done by [juriscraper](https://github.com/freelawproject/juriscraper), which
"gathers judicial opinions, oral arguments, and PACER data in the American court
system" and is "XPath-based scraping powered by lxml's html parser"
([README.rst](https://github.com/freelawproject/juriscraper/blob/main/README.rst)).
Juriscraper returns metadata and document URLs; it does not parse the interior of an
opinion.

Text extraction is done by [doctor](https://github.com/freelawproject/doctor). Its
`/extract/doc/text/` endpoint uses `pdftotext` for PDF, `antiword` for `.doc`,
`docx2txt` for `.docx`, `wpd2html` for WordPerfect, and `lxml.html.clean.Cleaner` for
HTML. OCR is optional and uses tesseract
([doctor README](https://github.com/freelawproject/doctor/blob/main/README.md)).
This confirms the `plain_text` help text: it is a flat `pdftotext` dump, with no
structure at all.

### OCR quality

CAP's own statement is direct
([case.law About page](https://case.law/about/), source in
[capstone-static](https://github.com/harvard-lil/capstone-static/blob/main/src/templates/cap-about-page.js)):

> "Harvard Law School Collection data is generated by OCR from page scans, using ABBYY
> FineReader. Case metadata, such as the party names, docket number, citation, and date,
> has received human review. Case text and general head matter has been generated by
> machine OCR and has not received human review."

and

> "A vendor then used OCR to extract the text of every case, creating case-level XML
> files. Key metadata fields, like case name, citation, court and decision date, were
> corrected for accuracy, while the text of each case was left as raw OCR output."

The CAP JSON for a case carries an OCR confidence score. For Roe v. Wade,
[`static.case.law/us/410/cases/0113-01.json`](https://static.case.law/us/410/cases/0113-01.json)
has `"analysis": {"ocr_confidence": 0.657, ...}`. That is per-case and usable as a
quality gate.

CourtListener sets `extracted_by_ocr=True` on **every** opinion imported from Harvard.
See `harvard_opinions.py`, which constructs `Opinion(..., xml_harvard=opinion_xml,
per_curiam=per_curiam, extracted_by_ocr=True)`
([source](https://github.com/freelawproject/courtlistener/blob/main/cl/corpus_importer/management/commands/harvard_opinions.py)).
So `extracted_by_ocr` is a provenance flag, not a per-document measurement, for the
Harvard two-thirds of the corpus.

CourtListener state on their coverage page that they "used machine learning and human
resources to make over a million corrections to the data set"
([data coverage: case law](https://wiki.free.law/c/courtlistener/help/data-coverage/case-law)).
They do not say which corrections, or which records they touched. That claim is not
auditable from the sources I could reach.

### Quality ranking that follows from provenance

1. **`html_anon_2020`** — richest markup of all (named divs, typed citations, structured
   footnotes, star pagination). Born-digital, not OCR. But only 0.23% of clusters.
2. **`xml_harvard`** — best *coverage* and good coarse structure (typed opinions,
   author, page numbers, footnotes). Text is raw OCR with no human review.
3. **`html_columbia`** / **`html_lawbox`** — editor-prepared HTML from donated
   collections. Clean text, moderate markup, no opinion typing inside the field.
4. **`html`** — court website HTML or Resource.org HTML. Resource.org HTML has numbered
   paragraphs; court HTML varies wildly.
5. **`plain_text`** — `pdftotext` output. Correct characters, zero structure.

---

## 3. How much internal structure survives

I measured this on real data. I byte-range downloaded the first 40 MB of
`bulk-data/opinions-2026-06-30.csv.bz2`, decoded 327 MB of CSV from it, and parsed
5,046 opinion rows. **This prefix is not a random sample** — Postgres `COPY TO` emits
heap order, and this block is dominated by one import batch. Use the per-format marker
rates below as a shape guide, not as corpus-wide statistics.

Percentage of opinions *that have the field populated* which contain each marker:

| Marker | `plain_text` | `html` | `html_lawbox` | `html_columbia` | `html_anon_2020` | `xml_harvard` | `html_with_citations` |
|---|---:|---:|---:|---:|---:|---:|---:|
| (n populated in sample) | 518 | 168 | 82 | 13 | 3725 | 850 | 5025 |
| `<page-number>` element | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 51.9% | 8.8% |
| `star-pagination` class | 0.0% | 0.0% | 85.4% | 84.6% | 100.0% | 14.8% | 76.9% |
| footnote element/class | 0.0% | 45.2% | 0.0% | 69.2% | 62.4% | 29.9% | 51.7% |
| `<blockquote>` | 0.0% | 0.6% | 52.4% | 69.2% | 0.0% | 29.9% | 5.3% |
| `<h1>`–`<h4>` | 0.0% | 0.0% | 100.0% | 0.0% | 62.4% | 0.0% | 46.7% |
| `¶ N` paragraph numbers | 13.5% | 0.0% | 4.9% | 7.7% | 0.0% | 3.6% | 1.9% |
| `class="num"` para numbers | 0.0% | 82.7% | 0.0% | 0.0% | 0.0% | 0.0% | 1.3% |
| `<opinion>` element | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 100.0% | 16.9% |
| `<author>` element | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 69.6% | 11.8% |
| `<pre class="inline">` | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 6.8% |
| `<span class="citation">` | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 94.3% |

The `<h1>`–`<h4>` hits in `html_lawbox` and `html_anon_2020` are **caption headings**
(the case name, or a "Footnotes" heading), not section headings inside the opinion body.
See the samples below.

### Concrete markup, from real records

**`xml_harvard`** (opinion 4105007, from the bulk file). The wrapper is a CAP CaseXML
fragment:

```xml
<?xml version="1.0" encoding="utf-8"?>
<opinion type="majority">
<author id="b1326-17">BARNES, J.,</author>
<p id="ATjE">FOR THE COURT:</p>
<p id="b1326-18">¶ 1. John Bell (Jack) appeals the judgment of the Chancery Court ...</p>
<p id="b1326-19">STATEMENT OF FACTS AND PROCEDURAL HISTORY</p>
```

Note the third `<p>` is a **section heading rendered as an ordinary paragraph**. There
is no heading element. This is the single biggest structural gap in the Harvard data.

Page breaks (opinion 4821739):

```xml
<page-number citation-index="1" label="986">*986</page-number>
```

Footnote marks (opinion 5658606):

```xml
<footnotemark>*</footnotemark>
```

**`html_lawbox`** (opinion 1128998):

```html
<div>
<center><b>213 Kan. 751 (1974)</b></center>
<center><b>518 P.2d 362</b></center>
<center><h1>TERRY A. HENRY, ... Appellant,<br>v.<br>THOMAS W. BAUDER, ... Appellee.</h1></center>
<center>No. 47,101</center>
<center><p><b>Supreme Court of Kansas.</b></p></center>
<center>Opinion filed January 26, 1974.</center>
<p><i>Michael G. Norris,</i> of Olathe, argued the cause, ...</p>
<p>The opinion of the court was delivered by</p>
<p>PRAGER, J.:</p>
```

and star pagination inline:

```html
... charged the defendant Bauder with ordinary <span class="star-pagination">*752</span> negligence.
```

**`html_columbia`** (opinion 4870715):

```html
<p>F.Z. ("the father") appeals ... We dismiss the appeal as being taken from a void
judgment. <span class="star-pagination">*Page 2</span> </p>
<blockquote><p> "Ex Mero Motu this cause is transferred ... Section
<cross_reference>12-15-117</cross_reference>[, Ala. Code 1975.]</p></blockquote>
```

Note the raw XML tag `<cross_reference>` leaking into the HTML. The Columbia converter
maps a fixed tag list and leaves anything else in place. From `convert_columbia_html` in
[`cl/corpus_importer/import_columbia/columbia_utils.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/corpus_importer/import_columbia/columbia_utils.py):

```python
conversions = [
    ("italic", "em"), ("block_quote", "blockquote"), ("bold", "strong"),
    ("underline", "u"), ("strikethrough", "strike"), ("superscript", "sup"),
    ("subscript", "sub"), ("heading", "h3"), ("table", "pre"),
]
...
text = re.sub("<page_number>", ' <span class="star-pagination">*', text)
text = re.sub("</page_number>", "</span> ", text)
```

So the Columbia source XML **does** have a `<heading>` element, and it is mapped to
`<h3>`. That means Columbia is the one donated collection with real section headings —
where the original archive marked them.

**`html_anon_2020`** (opinion 4588465) is the richest:

```html
<div class="courtcasedochead col-12"><div class="caseinfo">
  <div class="casename">
    <div class="fullcasename">Southern Massachusetts Oil Corporation v. Commissioner.</div>
    <div class="shortcasename" party1="Southern Massachusetts Oil Corp." party2="Commissioner">...</div>
  </div>
  <div class="docketnumber">Docket No. 3958-64.</div>
  <div class="courtinfo"><courtname>United States Tax Court</courtname>...</div>
  <div class="citations">
    <span class="citeforthisresource" pagescheme="T.C. Memo">T.C. Memo 1966-115</span>
    <span class="citeforthisresource" pagescheme="Tax Ct. Memo LEXIS">1966 Tax Ct. Memo LEXIS 170</span>
  </div>
  <span name="decisiondates"><decisiondate day="27" month="05" year="1966">May 27, 1966</decisiondate></span>
</div></div>
<div class="courtcasedocbody"><div class="representation"><div class="counsel">
  <span class="star-pagination" number="1" pagescheme="1966 Tax Ct. Memo LEXIS 170">*170 </span> Jerry M. Brown, for the petitioner. ...
```

with wired footnotes (opinion 4588461):

```html
<div class="footnotes"><h4>Footnotes</h4><ul><li><div id="fn_fnote1">
  <bodytext><p>1. SEC. 42. PERIOD IN WHICH ITEMS OF GROSS INCOME INCLUDED. ...
  <a href="#fnr_fnote1">↩</a></p></bodytext>
```

**`html`** (Resource.org, opinion 380204) — this one has numbered paragraphs:

```html
<p class="case_cite">625 F.2d 850</p>
<p class="parties">BOB'S BIG BOY FAMILY RESTAURANTS ... v. NATIONAL LABOR RELATIONS BOARD, Respondent.</p>
<p class="docket">No. 78-3609.</p>
<p class="court">United States Court of Appeals,<br>Ninth Circuit.</p>
<p class="date">Submitted Dec. 7, 1979.<br>Decided July 28, 1980.</p>
<div class="prelims">
  <p class="indent">Carlton J. Trosclair, Washington, D.C., ... for petitioner.</p>
  <p class="indent">Before GOODWIN, WALLACE and FARRIS, Circuit Judges.</p>
  <p class="indent">WALLACE, Circuit Judge.</p>
</div>
<div class="num" id="p1"><span class="num">1</span><p class="indent">This petition seeks review ...</p></div>
```

**`html_with_citations` over a PDF-only opinion** (opinion 11103682) — this is the
critical negative result:

```html
<pre class="inline">             UNITED STATES DISTRICT COURT
              EASTERN DISTRICT OF MISSOURI
                 SOUTHEASTERN DIVISION

TROY D. SENCIBAUGH,              )
...
```

The generator confirms it. From `create_cited_html` in
[`cl/citations/annotate_citations.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/annotate_citations.py):

```python
else:  # Else, present `source_text` wrapped in <pre> HTML tags...
    if single_doc:
        source_text = f'<pre class="inline">{html.escape(document.source_text)}</pre>'
```

So `html_with_citations` adds **no structure** to a PDF-derived opinion. It adds only
citation `<span>`s. It is a citation layer, not a structure layer.

### The CAP HTML alternative, which CourtListener does not import

CAP also publishes a per-case HTML render at `static.case.law`, and it is much richer
than the CaseXML that landed in `xml_harvard`. From
[`static.case.law/us/410/html/0113-01.html`](https://static.case.law/us/410/html/0113-01.html)
(Roe v. Wade):

```html
<section class="casebody" data-case-id="32044046866018_0005" data-firstpage="113" data-lastpage="178">
  <section class="head-matter">
    <h4 class="parties" id="b185-4" data-blocks='[["BL_185.3",185,[248,354,1147,94]]]'>ROE et al. <em>v. </em>WADE, ...</h4>
    <p class="docketnumber" ...>No. 70-18.</p>
    <p class="otherdate" ...>Argued December 13, 1971</p>
    <p class="decisiondate" ...>Decided January 22, 1973</p>
    <p class="judges" ...><a id="p115" href="#p115" data-label="115" data-citation-index="1" class="page-label">*115</a>Blackmun, J., delivered the opinion of the Court, ...</p>
    <p class="attorneys" ...>...<a class="footnotemark" href="#footnote_0_1" id="ref_footnote_0_1"><em>*</em></a></p>
    <aside data-label="*" class="footnote" id="footnote_0_1">...</aside>
  </section>
  <article class="opinion" data-type="majority">
    <p class="author" id="b188-4" data-blocks='[["BL_188.4",188,[300,336,502,35]]]'>...Mr. Justice Blackmun</p>
    ...
  </article>
  <article class="opinion" data-type="concurrence">...</article>
  <article class="opinion" data-type="dissent">...</article>
</section>
```

Class counts in that one file: 241 `citation`, 78 `page-label`, 74 `footnotemark`,
74 `footnote`, 3 `opinion`, 3 `author`, plus `casebody`, `head-matter`, `parties`,
`docketnumber`, `decisiondate`, `otherdate`, `judges`, `attorneys`.

Two things worth noting.

First, `data-blocks` gives the **page number and bounding box** of every block:
"Each block of the HTML also contains the region of the PDF from which it was extracted"
([CAP docs](https://case.law/docs/), source in
[`cap-docs-page.js`](https://github.com/harvard-lil/capstone-static/blob/main/src/templates/cap-docs-page.js)).

Second — and this is the important negative — **CAP HTML still does not mark section
headings**. In Roe v. Wade the roman-numeral section markers are plain paragraphs:

```html
<p id="b189-6" data-blocks='[["BL_189.5",189,[812,1162,18,33]]]'>I</p>
<p id="b195-10" data-blocks='[["BL_195.3",195,[797,329,18,33]]]'><a ... class="page-label">*123</a>III</p>
```

Body paragraphs on the same pages start at x≈254; these heading paragraphs start at
x≈797–816. The geometry that identifies them is present in `data-blocks`, but the
semantics are not. A rule can recover it. See §7.

---

## 4. How separate opinions are represented and linked

The chain is: `Court` ← `Docket` ← `OpinionCluster` ← `Opinion`.

From `cl/search/models.py`:

- `Opinion.cluster` is a `ForeignKey(OpinionCluster, related_name="sub_opinions", on_delete=CASCADE)` with help text "The cluster that the opinion is a part of".
- `OpinionCluster.docket` is a `ForeignKey(Docket, related_name="clusters")` with help text "The docket that the opinion cluster is a part of".

The API docs say "The `sub_opinions` field provides a list of the opinions that are
linked to each cluster" and, on the bulk page, "Clusters serve the purpose of grouping
dissenting and concurring opinions together"
([bulk data docs](https://wiki.free.law/c/courtlistener/help/api/bulk-data/bulk-legal-data)).

### Opinion type vocabulary

`Opinion.OPINION_TYPES` in
[`cl/search/models.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/search/models.py):

```
010combined            Combined Opinion
015unamimous           Unanimous Opinion      (note: the constant is misspelled in the source)
020lead                Lead Opinion
025plurality           Plurality Opinion
030concurrence         Concurrence Opinion
035concurrenceinpart   In Part Opinion
040dissent             Dissent
050addendum            Addendum
060remittitur          Remittitur
070rehearing           Rehearing
080onthemerits         On the Merits
090onmotiontostrike    On Motion to Strike Cost Bill
100trialcourt          Trial Court Document
```

The numeric prefixes exist so that a lexical sort is also a priority sort. The docs say
"The most common type of opinion is a 'Combined Opinion' this is what we label any
opinion that either cannot be identified as a specific type, or that contains more than
one type."
([case-law API docs](https://wiki.free.law/c/courtlistener/help/api/rest/v4/case-law))

In my (biased) 5,046-row sample: 84.30% `010combined`, 14.05% `020lead`, 1.41%
`100trialcourt`, 0.14% `040dissent`, 0.08% `030concurrence`, 0.02%
`035concurrenceinpart`. And 99.78% of the clusters in that sample had exactly one
opinion. Treat the exact numbers with suspicion, but the shape is clear and matches
what the docs say: **most clusters are a single Combined Opinion, and the
majority/dissent split is usually *not* separated at the record level.**

Where it *is* separated, it came from Harvard or Columbia:

- Harvard: CAP JSON has one entry per writing with a `type` field. For Roe v. Wade,
  [`static.case.law/us/410/cases/0113-01.json`](https://static.case.law/us/410/cases/0113-01.json)
  has three: `majority` (Mr. Justice Blackmun), `concurrence` (Mr. Justice Stewart),
  `dissent` (Mr. Justice Rehnquist). `map_opinion_type` in
  [`harvard_opinions.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/corpus_importer/management/commands/harvard_opinions.py)
  maps CAP's `majority` → `Opinion.LEAD`, `dissent` → `Opinion.DISSENT`, and so on,
  falling back to `Opinion.COMBINED` for anything unrecognised.
- Columbia: the source XML has `<opinion_text>`, `<dissent_text>`, `<concurrence_text>`
  and matching `<opinion_byline>`, `<dissent_byline>`, `<concurrence_byline>` elements.
  `extract_columbia_opinions` and `map_opinion_types` in
  [`columbia_utils.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/corpus_importer/import_columbia/columbia_utils.py)
  turn those into separate `Opinion` rows, with the first `opinion` becoming `020lead`
  and later ones `050addendum`.

### Ordering, authorship and versions

- `Opinion.ordering_key` (IntegerField, unique per `(cluster_id, ordering_key)`). Docs:
  "This field is only populated for opinions ingested from Harvard or Columbia sources."
  **Caution:** `ordering_key` is present in the API but is **not** in the bulk
  `opinions` CSV. The header of `opinions-2026-06-30.csv.bz2` is:
  `id, date_created, date_modified, author_str, per_curiam, joined_by_str, type, sha1,
  page_count, download_url, local_path, plain_text, html, html_lawbox, html_columbia,
  html_anon_2020, xml_harvard, xml_scan, html_with_citations, extracted_by_ocr,
  author_id, cluster_id`. `ordering_key` and `main_version_id` are absent. I could not
  find a source that documents this omission; it looks like the export script has not
  been updated.
- `Opinion.author` (FK to `people_db.Person`) and `Opinion.author_str` (free text
  fallback). `Opinion.joined_by` / `joined_by_str` for judges who joined.
  `Opinion.per_curiam` boolean.
- `OpinionCluster.panel` / `judges` / `non_participating_judges` at cluster level.
- `Opinion.main_version` (self-FK): "The id of another Opinion which is the updated or
  final version of this opinion", `related_name="versions"`.

### A new model you should know about: `OpinionContent`

`cl/search/models.py` now defines an `OpinionContent` model whose docstring says:

> "Stores normalized opinion text content. Each Opinion can have multiple OpinionContent
> records, one per content type. This normalized structure replaces the previous wide
> table design that had separate fields for each content source (plain_text, html,
> html_lawbox, etc.)."

It has `opinion` (FK), `content`, `source` (choices: `Got from juriscraper`, `Got from
the Harvard Case Law Access Project`, `Got from 2020 anonymous database`, `Got from
Columbia archive`, `Got from Lawbox`, `Free opinions on RECAP`, `Got from FLP Scanning
Project`), `extraction_type` (choices: `Extracted by opening the document and getting
the text`, `Extracted via OCR`, `Extracted via LLM`), and `is_main_version`.

The table `search_opinioncontent` exists in the bulk schema dump
`bulk-data/schema-2026-06-30.sql`, but there is **no** `opinion-content` CSV in the
bulk-data bucket as of the 2026-06-30 snapshot, and the API help page does not mention
it. **Plan for this migration.** The wide text fields are on their way out. The docstring
even says "Future work: Add fields from Opinion model (sha1, download_url, local_path,
etc.) so existing content can be copied from Opinion to OpinionContent, enabling full
versioning support."

---

## 5. Identifiers and citation data

### Identifiers

- `Opinion.id` — integer primary key. Used in `data-id` attributes in
  `html_with_citations` and in the citation graph.
- `OpinionCluster.id` — integer primary key. This is the id in a CourtListener opinion
  URL: `Opinion.get_absolute_url()` returns
  `reverse("view_case", args=[self.cluster.pk, self.cluster.slug])`.
- `OpinionCluster.slug` — URL slug.
- `Docket.id`, `Court.id` — upstream in the chain.
- `Opinion.sha1` — "unique ID for the document, as generated via SHA1 of the binary file
  or text data". Useful for change detection.
- `OpinionCluster.scdb_id` — Supreme Court Database id, where known.
- CAP ids: the CAP JSON `id` (for example 11957048 for Roe v. Wade) and the CAP case path
  (`/us/410/0113-01`). CourtListener stores the CAP JSON path in
  `filepath_json_harvard`, but I found no field holding the CAP numeric id directly.

### Citations of the case (parallel citations)

`Citation` is a child of `OpinionCluster` (`related_name="citations"`), with fields
`volume`, `reporter`, `page`, `type`. `type` is one of FEDERAL, STATE, STATE_REGIONAL,
SPECIALTY, SCOTUS_EARLY, LEXIS, WEST, NEUTRAL, JOURNAL. `unique_together` is
`("cluster", "volume", "reporter", "page")`. `sort_cites()` scores them into Bluebook
order (neutral first, then U.S., S. Ct., L. Ed., …).
([models.py](https://github.com/freelawproject/courtlistener/blob/main/cl/search/models.py))

`page` is a `TextField`, not an integer, and the help text explains why: "several
jurisdictions do funny things with the so-called 'page'. For example, we have seen Roman
numerals in Nebraska, 13301-M in Connecticut, and 144M in Montana." Do not model it as
an integer.

### Citations made by the case (the citation graph)

`OpinionsCited` links `citing_opinion` → `cited_opinion` with a `depth` field ("The
number of times the cited opinion was cited in the citing opinion"). It is
opinion-to-opinion, not cluster-to-cluster. A commented-out block in the model shows
`quoted` and `treatment` were considered and not built.

`OpinionsCitedByRECAPDocument` does the same for briefs and filings.

`OpinionCluster.citation_count` caches the inbound count.

### eyecite: yes, it is used, and here is exactly how

`cl/search/models.py` imports `from eyecite import get_citations` and
`from eyecite.tokenizers import HyperscanTokenizer`.

The pipeline is in
[`cl/citations/tasks.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/tasks.py),
function `store_opinion_citations_and_update_parentheticals`:

1. `make_get_citations_kwargs(opinion)` picks the first non-empty of `xml_harvard`,
   `html_anon_2020`, `html_columbia`, `html_lawbox`, `html`, `plain_text` and chunks it
   at 200,000 characters. Markup fields are passed as `markup_text` with
   `clean_steps=["xml", "html", "all_whitespace"]`; `plain_text` is passed as
   `plain_text` with `clean_steps=["all_whitespace"]`.
2. `get_citations(tokenizer=HYPERSCAN_TOKENIZER, **kwargs)` extracts.
3. `do_resolve_citations` matches each citation to an `Opinion`.
4. `create_cited_html` writes `Opinion.html_with_citations`.
5. `OpinionsCited` rows are deleted and recreated with `depth=len(_citations)`.
6. Descriptive parentheticals become `Parenthetical` rows.

The annotation markup, from
[`annotate_citations.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/annotate_citations.py):

```html
<!-- resolved -->
<span class="citation" data-id="{opinion.pk}"><a href="{opinion_url}" aria-description="Citation for case: {case_name}">…</a></span>
<!-- unresolved -->
<span class="citation no-link">…</span>
<!-- ambiguous -->
<span class="citation multiple-matches"><a href="{citation_redirector_url}">…</a></span>
```

Pin cites are appended to the URL as a fragment (`#122`). `Id.` and `supra` citations
are annotated over their `full_span()` to avoid unbalanced HTML; other citations use
`span_with_pincite()`.

`data-id` on the span is the **`Opinion` id**, not the cluster id. That is the direct
hook for building a link graph out of `html_with_citations`.

Note that `@pghistory.track(exclude=["html_with_citations"])` on the `Opinion` model
means the citation-annotated HTML is deliberately excluded from history tracking.

### Citation Lookup API

There is a separate `POST /api/rest/v4/citation-lookup/` endpoint that "can look up
either an individual citation or can parse and look up every citation in a block of
text", backed by eyecite. Limits: 60 valid citations per minute, 250 citations per
request, 64,000 characters per request
([citation lookup docs](https://wiki.free.law/c/courtlistener/help/api/rest/v4/citation-lookup)).
That page states the database holds 18,127,065 legal citations.

---

## 6. Bulk data and API limits

### Bulk data

- **Format.** "Files are generated using the [PostgreSQL `COPY TO` command]. This
  generates CSV files that correspond with the tables in our database. Files are
  provided using the CSV output format, in the UTF-8 encoding, with a header row on the
  top."
  ([bulk data docs](https://wiki.free.law/c/courtlistener/help/api/bulk-data/bulk-legal-data))
- **The CSV dialect is not standard.** The same page gives the import statement:
  `COPY public.search_opinionscited (...) FROM 'path.csv' WITH (FORMAT csv, ENCODING
  utf8, ESCAPE '\', HEADER);`. Note `ESCAPE '\'`. Embedded quotes are backslash-escaped,
  **not** doubled. A default CSV reader will mis-parse the file. In Python this needs
  `csv.reader(f, escapechar='\\', doublequote=False)`. I hit this: the default dialect
  gave 41M+ phantom rows out of a 10.07M-row file.
- **Snapshots, not deltas.** "Files are snapshots, not deltas, meaning each file
  contains everything in our database at the time of generation."
- **Cadence.** "Bulk data files are regenerated quarterly on the last day of March,
  June, September, and December beginning at 3AM PST."
- **Location.** `https://com-courtlistener-storage.s3-us-west-2.amazonaws.com/list.html?prefix=bulk-data/`.
  Objects are directly fetchable without credentials and support HTTP range requests.
- **Schema.** A `schema-YYYY-MM-DD.sql` `pg_dump` accompanies each snapshot.
- **Licence.** "Our bulk data files are free of known copyright restrictions", with a
  Public Domain Mark 1.0 badge linking to
  `https://creativecommons.org/publicdomain/mark/1.0/`.

Actual sizes in the `2026-06-30` snapshot (bytes from the S3 `ListObjectsV2` response):

| File | Size |
|---|---:|
| `opinions-2026-06-30.csv.bz2` | 54.56 GB |
| `dockets-2026-06-30.csv.bz2` | 5.01 GB |
| `opinion-clusters-2026-06-30.csv.bz2` | 2.46 GB |
| `oral-arguments-2026-06-30.csv.bz2` | 0.68 GB |
| `citation-map-2026-06-30.csv.bz2` | 0.53 GB |
| `parentheticals-2026-06-30.csv.bz2` | 0.29 GB |
| `unmatched-citations-2026-06-30.csv.bz2` | 0.22 GB |
| `citations-2026-06-30.csv.bz2` | 0.13 GB |
| `search_opinioncluster_panel-2026-06-30.csv.bz2` | 0.005 GB |
| `courts-2026-06-30.csv.bz2` | < 0.001 GB |

There is also a case law embeddings dataset at
`s3://com-courtlistener-storage/embeddings/opinions/` (~2 TB, ModernBERT), fetchable
with `aws s3 sync --no-sign-request`, per the same page.

Bulk files are cumulative, so history is available back to `2022-08-02` for dockets and
`opinions`. That gives you real diffable snapshots without any API calls.

### API rate limits

From the [API v4 overview](https://wiki.free.law/c/courtlistener/help/api/rest/v4/overview):

> "By default, authenticated users may make up to **5 requests per minute**, **50
> requests per hour**, and **125 requests per day**."

Rates use "a rolling window basis". "All throttles apply concurrently. The most
restrictive one — given your recent traffic — is what controls whether the next request
is accepted." Elevated access comes through a Free Law Project membership or a
commercial agreement.

Authentication is required. I verified this: an unauthenticated
`GET https://www.courtlistener.com/api/rest/v4/opinions/?id=2812209` returns
`HTTP 401 {"detail":"Authentication credentials were not provided."}`. Auth is
`Authorization: Token <token>`, or session cookie, or HTTP Basic.

There is a separate Usage API that reports your consumption without consuming quota.

**At 125 requests/day, the API is unusable for ingesting 10M clusters.** Bulk data is
the only realistic path for a full corpus. Use the API for incremental top-ups and for
the fields the bulk export omits (notably `ordering_key`).

### Terms of use

From the
[CourtListener Terms of Service](https://wiki.free.law/c/terms/courtlistener/courtlistenercom-terms-of-service-and-policies)
(mirrored at `https://www.courtlistener.com/terms/`):

- On copyright: "You acknowledge that while judicial opinions, motions, and other
  filings are generally in the public domain, other court filings may contain
  third-party copyrighted works, such as books and articles, that may retain copyright
  protection."
- FCRA: Free Law Project "is not a consumer reporting agency under the Fair Credit
  Reporting Act" and you agree not to use the data "as a factor in establishing an
  individual's eligibility for credit, insurance, employment, government benefits,
  housing" and similar.
- Credentials: "Do not share, resell, pool, or transfer your account credentials, API
  tokens, or OAuth access tokens."
- Limits: "Do not use multiple accounts, registered clients, or credential rotation to
  exceed the rate limits that apply to your access level."
- Attribution: "**Attribute honestly.** If you republish or display our data, do not
  present it in a way that suggests Free Law Project produced, endorsed, or verified an
  AI-generated analysis of it."
- The privacy section says the site uses an "anti-crawling challenge" and that they
  "defend the Services against attacks, abuse, and automated scraping". I confirmed
  this: `https://www.courtlistener.com/opinion/108713/roe-v-wade/` returns HTTP 202 with
  an empty body to a plain HTTP client. **Do not scrape the website.** Use bulk data.

The terms do not state a licence for the case law itself; the bulk data page's Public
Domain Mark is the closest thing to one, and it is stated only about the bulk files.

### CAP terms

CAP's own statement is only: "The CAP data is free for the public to use and access."
([case.law About](https://case.law/about/)). I could not find a formal licence
identifier (CC0, PDM or similar) on the CAP site. That is a genuine gap — the CAP data
licence is less clearly stated than CourtListener's bulk data licence. CAP does give a
suggested citation format.

CAP scope, from the same page: "CAP includes all official, book-published state and
federal United States case law through 2020". "Our earliest case is from 1658".
CAP excludes "Cases not designated as officially published, such as most lower court
decisions", "Non-published trial documents", and "Copyrighted material such as
headnotes, for cases still under copyright". A separate Fastcase donation covers a fixed
list of reporters through 2020.

---

## 7. What is genuinely needed to derive a hierarchy

### The short answer

Most of it is rule-based. Free Law Project have already proved this, at scale, in a
package you can read.

### `centralia` — the single most important finding

[freelawproject/centralia](https://github.com/freelawproject/centralia) is a BSD-2
Python package, first commit 2026-06-16, that does exactly the job words_to_data needs:
"Court PDF opinion extractor: a PDF plus a court id in, a typed document out."

It states the problem in the same terms you would:

> "Getting the *text* out of a court PDF is easy; `pdftotext` does it. What is hard is
> that an opinion is a document with **parts**, and the PDF says nothing about them.
> Nothing marks which lines are the caption, where the syllabus ends and the opinion
> begins, which paragraphs belong to the dissent rather than the majority, whose
> footnote is whose, or which line is a page number instead of a sentence. All of it has
> to be recovered from how the page is *set* — position, size, leading, indentation, and
> the rules the printer drew."
> ([README](https://github.com/freelawproject/centralia/blob/main/README.md))

It returns `cluster`, `opinions` (one per writing, with `type`, `author`, `text`,
`html`, `footnotes`), `headmatter`, `endmatter`, `sections`, `removed`, `warnings`,
`diagnostics`, `html`, and `casebody` (Harvard casebody XML).

**It uses no machine learning.** The pipeline
([DEVELOPMENT.md](https://github.com/freelawproject/centralia/blob/main/DEVELOPMENT.md))
is eleven deterministic stages:

```
load > triage > measure > classify > furniture > footnotes > segments
     > headmatter > body > finalize > emit
```

Its stated design rule is "Measure the document; never configure the threshold" —
thresholds come from each document's own geometry, not from per-court tuning. Its second
rule is "Nothing leaves the PDF silently": every row removed as furniture is returned in
`removed` with its page and bounding box.

Where things stand, from DEVELOPMENT.md §12:

| | |
|---|---|
| courts wired | 241 |
| corpus | 10,349 PDFs, 3.1 GB |
| released through the public API | 191 |
| held back | 50 (45 federal district, 5 other) |
| core engine | 20.5k lines |
| the 241 court files | 111k lines |

It is already wired into doctor as `POST /extract/opinion/structured/`
([doctor README](https://github.com/freelawproject/doctor/blob/main/README.md)):

> "Given a **digital** (text-based) court PDF and the court it came from, extract a
> structured opinion with centralia. For the courts centralia has been ported to this
> replaces pdftotext/OCR: instead of one flat string you get the case-level criteria,
> one entry per writing with its own author and text, and Harvard casebody XML."

I searched `cl/scrapers/`, `cl/lib/microservice_utils.py` and
`cl_scrape_opinions.py` in the CourtListener repo and found **no** reference to
`centralia` or `/extract/opinion/structured/`. So as of this snapshot the structured
extractor exists in the microservice but is not yet wired into the CourtListener ingest,
and none of its output is in the bulk data. That is a claim about absence, and absence is
harder to prove than presence — treat it as "I could not find it" rather than
"definitely not there".

### Where geometry works and where it does not

DEVELOPMENT.md §6 is the clearest statement anyone has published on this:

> "**Geometry works when the PDF is born-digital.** The court's word processor wrote real
> text with real coordinates, so position, size and leading mean what they appear to
> mean. Most state appellate courts and all federal circuits are here."

> "**Geometry is untrustworthy on scans.** … An OCR text layer does **not** make geometry
> trustworthy — the words may be roughly right while every coordinate is an artifact of
> the scanner."

> "**Geometry is defeated by unmapped glyphs.** Some PDFs embed fonts whose character map
> is broken or absent, and pdfminer yields `(cid:NN)` instead of letters."

And on what actually holds courts back: "Of the 50 courts held back from the public API,
**45 are federal district courts**." The reason is not scanning — `acca` and `afcca` are
"almost entirely scans, and both are released". The reason is that district dockets
"carry exhibits and third-party attachments that were never typeset by the court at
all, and those produce *wrong* readings rather than honest refusals."

### Rule-based work (regex / heuristics over reasonably clean text)

All of this is deterministic. None of it needs a model.

1. **Split into writings.** Free for `xml_harvard` (`<opinion type="…">` per record) and
   Columbia (`<opinion_text>` / `<dissent_text>` / `<concurrence_text>`). For everything
   else, a byline regex over "delivered the opinion of the Court", "X, J., dissenting",
   "concurring in part and dissenting in part" and similar. Centralia already does this
   for 241 courts.
2. **Page breaks / star pagination.** Three markers, all regular:
   `<page-number citation-index="1" label="986">*986</page-number>` (Harvard),
   `<span class="star-pagination">*752</span>` (Lawbox, Columbia, anon-2020),
   `<a class="page-label" data-label="115">*115</a>` (CAP HTML).
3. **Footnotes.** `<footnotemark>` / `<footnote>` (Harvard),
   `<div class="footnotes">…<div id="fn_fnote1">…<a href="#fnr_fnote1">↩</a>`
   (anon-2020), `<footnote_reference>` / `<footnote_body>` (Columbia source XML).
4. **Numbered paragraphs.** `¶ N` in the text (13.5% of `plain_text` in my sample), and
   `<div class="num" id="pN"><span class="num">N</span>` in Resource.org `html` (82.7%
   of that field in my sample).
5. **Section headings.** This is the one real gap in Harvard and CAP data — headings are
   plain `<p>` elements. But they are recoverable by rule:
   - By **content**: a paragraph whose whole text matches `^[IVXLC]+\.?$`,
     `^[A-Z]\.$`, `^\d+\.$`, or is short and fully upper-case.
   - By **geometry**, when using CAP HTML rather than `xml_harvard`: the `data-blocks`
     attribute gives page and bounding box. In Roe v. Wade, heading paragraphs start at
     x≈797–816 while body paragraphs start at x≈254. Centering is a clean discriminator.
   - Columbia is the exception: its source XML has a `<heading>` element, converted to
     `<h3>`. Where Columbia data exists, headings are already marked.
6. **Head matter separation.** `OpinionCluster.headmatter` is already a separate field
   (22.77% of clusters). CAP HTML separates it as `<section class="head-matter">` with
   typed rows (`parties`, `docketnumber`, `decisiondate`, `judges`, `attorneys`).
7. **Citation links.** Already done. Use `<span class="citation" data-id="…">` from
   `html_with_citations`, or run eyecite yourself.
8. **De-noising `plain_text`.** Running heads, folios, e-filing stamps, signature
   blocks. Centralia's `furniture` stage does this by repetition and band position.

### Work that truly needs a model

Much less than you would expect.

1. **OCR itself**, for scanned pages with no text layer. This is only relevant if you
   process PDFs yourself. CourtListener already ran OCR (ABBYY FineReader for CAP,
   tesseract in doctor), so the corpus text already exists. You do not need to redo it.
2. **OCR error correction.** CAP say the case text "has not received human review". If
   you want clean text — correct hyphenation, broken ligatures, `l`/`1` and `O`/`0`
   confusions, garbled reporter abbreviations — that is a genuine model task on a
   ~6.6M-document corpus. It is also **optional**: hierarchy extraction mostly needs
   layout signals, not perfect characters. Do this only if downstream diffing needs it.
   The per-case `ocr_confidence` in CAP JSON tells you which documents to prioritise.
3. **Layout analysis on scanned PDFs**, where the OCR coordinate layer is unreliable.
   Centralia routes these to a `scanned` status and declines to parse rather than
   guessing. This is where a model would add real value, and it is exactly the 45
   federal-district-court problem plus the service courts of criminal appeals.
4. **Classifying "Combined Opinion" records into their parts.** ~84% of opinions in my
   sample are `010combined`. Where the byline pattern is regular, rules win. Where the
   court did not label its writings at all, a sequence-labelling model over paragraphs
   could help — but centralia demonstrates that geometry gets you there for born-digital
   PDFs without one, so try rules first and measure the residue.
5. **Not needed at all:** any model to separate majority from dissent in the
   `xml_harvard` or Columbia subsets. That is already a data field.

Free Law Project's own LLM use in CourtListener today is narrow. The only prompts in the
repo are in
[`cl/search/llm_prompts.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/search/llm_prompts.py),
and they normalise **docket numbers** ("You are an expert assistant that cleans and
standardizes legal case docket numbers"). Nothing there touches opinion structure.
Separately, `OpinionContent.EXTRACTION_METHOD` includes an `LLM` choice ("Extracted via
LLM") and `OpinionCluster.filepath_xml_scan` is "The XML obtained from LLM containing all
available metadata and opinion(s)" — so LLM extraction is planned or in progress for the
FLP Scanning Project, but no cluster in the 2026-06-30 snapshot carries source `S`.

### One tool that is not what its name suggests

[x-ray](https://github.com/freelawproject/x-ray) finds **bad redactions** in PDFs — black
rectangles over live text. It uses PyMuPDF and returns page → bbox → hidden text. It is
useful for privacy hygiene when handling RECAP documents. It does nothing for structure.

---

## What this means for words_to_data

**Ingest path.** Take bulk data, not the API. 125 requests/day makes the API useless for
a 10M-cluster load, and the website has an anti-crawling challenge. Pull
`courts`, `dockets`, `opinion-clusters`, `opinions`, `citations`, `citation-map` for one
snapshot date. Parse with `ESCAPE '\'` semantics (`escapechar='\\', doublequote=False`) —
a stock CSV reader silently produces four times too many rows. Files are cumulative back
to 2022, so you get real historical snapshots to diff against, for free.

**Format preference, for structure.** `xml_harvard` → `html_anon_2020` → `html_columbia`
→ `html_lawbox` → `html` → `plain_text`. Use `html_with_citations` as a *parallel*
citation layer only; for PDF-sourced opinions it is just the plain text in
`<pre class="inline">` and adds nothing. Expect roughly two thirds of the corpus to come
through `xml_harvard`.

**What you get for free.** Opinion-to-cluster-to-docket-to-court linkage; opinion type
(majority/dissent/concurrence) where Harvard or Columbia supplied it; author and panel;
parallel citations with Bluebook ordering; a full opinion-to-opinion citation graph with
depth; page-break markers in four of six text formats; footnote markup in four of six;
blockquotes; and head matter as a separate cluster field.

**What you must build, by rules.** Section headings (the one consistent gap — Harvard and
CAP both leave them as plain paragraphs, recoverable by content regex or by `data-blocks`
x-position); splitting `010combined` opinions into writings by byline; normalising the
four different page-marker syntaxes into one; de-noising `plain_text`. All of this is
regex and geometry. Do not start with a model.

**What genuinely needs a model.** OCR correction over the ~6.6M Harvard documents, if
and only if your diffing needs character-perfect text — CAP say that text "has not
received human review". And layout analysis on true scans, where OCR coordinates are
artifacts. Both are optional for a first version.

**Do not build the PDF reader from scratch.** `centralia` is BSD-2, reads 241 courts,
is purely geometric, and already emits Harvard casebody XML — the same shape as
`xml_harvard`. Even if words_to_data stays Rust, centralia's DEVELOPMENT.md is the
specification you would otherwise spend a year discovering. Calling doctor's
`/extract/opinion/structured/` is a viable interim.

**Plan for `OpinionContent`.** The wide text fields (`plain_text`, `html_lawbox`, …) are
being replaced by a normalised `OpinionContent` table with explicit `source` and
`extraction_type` columns. The table is already in the bulk schema dump. Model your Rust
types against that shape, not against the six wide columns, or you will migrate twice.

### Open questions the sources could not answer

1. **Per-field population across all opinions.** I measured cluster `source` over all
   10,070,727 clusters, but not per-opinion field population. The `opinions` bulk file is
   54.56 GB of bzip2, which is not seekable, so a random sample needs bzip2 block
   recovery. My prefix sample is biased and its field shares should not be quoted.
2. **True `type` distribution.** How many opinions really are `010combined` corpus-wide?
   Same obstacle. My 84% figure is from a biased prefix.
3. **Why `ordering_key` and `main_version_id` are missing from the bulk `opinions` CSV.**
   They are in the model and in `schema.sql`, but not in the export header. No source
   documents this.
4. **When `OpinionContent` becomes the primary store**, and whether a bulk CSV for it
   will be published. No roadmap, no release note found.
5. **Whether centralia will be wired into CourtListener ingest**, and whether its output
   will reach the bulk data. I found no reference to it in the CourtListener repo.
6. **The formal licence of CAP data.** case.law says only "The CAP data is free for the
   public to use and access." No SPDX identifier, no CC0 declaration found. Contrast with
   CourtListener's bulk files, which carry Public Domain Mark 1.0.
7. **What the "over a million corrections … using machine learning" were**, on the
   CourtListener coverage page. No method, no scope, no audit trail published.
8. **Whether `xml_harvard` preserves CAP's `data-blocks` geometry.** It does not — the
   samples show a CaseXML fragment with `<p id="…">` and no coordinates. The geometry
   exists only in CAP's own HTML at `static.case.law`. So if you want heading detection
   by x-position, you must fetch CAP HTML separately and align it to CourtListener
   records via `filepath_json_harvard`. I found no documented alignment key beyond that
   path.
9. **The `html_anon_2020` source.** It is the richest markup in the corpus and its
   provenance is stated only as "our anonymous source in 2020". Its licence status is
   therefore unclear. It is only 0.23% of clusters, so this is low stakes.

---

## 8. Statute, U.S.C. and regulation citations

Added after the first draft. Sections 1–7 covered case-to-case citations only. This
section answers whether CourtListener extracts and **stores** citations from an opinion
to a statute or a regulation.

**Short answer: no. eyecite finds them. CourtListener discards them. They survive only
as unlinked display markup in `html_with_citations`, and even that is incomplete.**

### 8.1 Does eyecite recognise statute citations as a distinct type?

Yes. `FullLawCitation` is a first-class type in
[`eyecite/models.py`](https://github.com/freelawproject/eyecite/blob/main/eyecite/models.py).
Its class hierarchy is `FullLawCitation → FullCitation → ResourceCitation →
CitationBase`, so it is a `FullCitation` and takes part in resolution.

I ran eyecite locally to get exact output rather than rely on the docs. Environment:
eyecite **2.7.8** with reporters-db **3.2.66**, both installed from PyPI.

| Input text | Result |
|---|---|
| `See 26 U.S.C. § 174 (2018).` | `FullLawCitation`, matched `'26 U.S.C. § 174'`, `groups={'title': '26', 'reporter': 'U.S.C.', 'section': '174'}`, `metadata.year='2018'` |
| `under 42 U.S.C. §§ 1983, 1988` | **one** `FullLawCitation`, matched `'42 U.S.C. §§ 1983'`, `groups={'title': '42', 'reporter': 'U.S.C.', 'section': '1983'}`. **`1988` is lost.** |
| `Pub. L. No. 116-136, 134 Stat. 281 (2020).` | **one** `FullLawCitation`, matched `'Pub. L. No. 116-136, 134'`, `groups={'reporter': 'Pub. L.', 'title': '116-136', 'section': '134'}`. **The `Stat.` citation is swallowed and lost.** |
| `See 134 Stat. 281.` | `FullLawCitation`, `groups={'volume': '134', 'reporter': 'Stat.', 'page': '281'}` — correct when it stands alone |
| `40 C.F.R. § 122.2 (2019).` | `FullLawCitation`, `groups={'chapter': '40', 'reporter': 'C.F.R.', 'section': '122.2'}`, `metadata.year='2019'` |
| `See 29 C.F.R. 1910.132.` (no `§`) | **nothing found** |
| `Fed. R. Civ. P. 12(b)(6)` | **nothing found** — court rules are not in scope |
| `Mass. Gen. Laws ch. 1, § 2 (West 1999).` | `FullLawCitation`, `groups={'reporter': 'Mass. Gen. Laws', 'chapter': '1', 'section': '2'}`, `metadata.publisher='West'` |
| `Foo v. Bar, 1 U.S. 2, 3-4 (1999).` | `FullCaseCitation` (for contrast) |

The `groups` for a U.S.C. citation are already the fields you would want: title,
reporter, section. Year comes through `metadata`.

Three real limits, all observed above, all worth knowing before you plan a spike:
multi-section strings (`§§ 1983, 1988`) yield only the first section; a `Pub. L.` and a
parallel `Stat.` cite in the same sentence collapse into one mis-parsed record; and a
C.F.R. cite without a `§` is not found at all.

The vocabulary lives in
[`reporters_db/data/laws.json`](https://github.com/freelawproject/reporters-db/blob/main/reporters_db/data/laws.json)
(211,528 bytes, 371 keys). Counting entries by `cite_type`:
`leg_session` 143, `leg_statute` 103, `admin_compilation` 59, `admin_register` 58,
`admin_docket` 5, `municipal` 2, `admin_filing` 2, `leg_act` 1.

The 14 keys with `"jurisdiction": "United States"` are: `ASBCA`, `C.F.R.`, `CBCA`,
`CFPB`, `CFTC`, `FR`, `Pub. L.`, `Pvt. L.`, `Registration`, `SBA`, `Stat.`,
`Treas. Reg.`, `U.S. Patent`, `U.S.C.`

The `U.S.C.` entry, verbatim from `laws.json`:

```json
{ "cite_type": "leg_statute",
  "name": "United States Code; United States Code Annotated; United States Code Service; Gould's United States Code Unannotated",
  "jurisdiction": "United States",
  "regexes": ["(?P<title>\\d+),?\\s+$reporter,?\\s+$section_marker\\s*$law_section"],
  "variations": ["U. S. C.", "USC", "U.S.C.A.", "U.S.C.S.", "U.S.C.U.", "U.S. Code", "United States Code"],
  "examples": ["1 U.S.C. § 1", "1 U.S.C.A. § 1", "1 U.S.C.S. § 1", "1 U.S.C.U. §§ 1-2",
               "1 U.S.C. sec. 1", "1 U.S.C. Sections 1-2", "1 USC S. 1-2", "1 U.S. Code §1",
               "21, United States Code, Section 853", "18, United States Code, Section 3500",
               "18, United States Code, Section 981(a)(l)(C)"] }
```

The C.F.R. entry is `"cite_type": "admin_compilation"` with the single regex
`"(?P<chapter>\\d+) $reporter,? § $law_section"`. The literal `§` in that regex is why
`29 C.F.R. 1910.132` fails.

### 8.2 Does CourtListener persist law citations anywhere?

**No.** Four independent pieces of evidence.

**(a) The resolver refuses them.** `resolve_fullcase_citation` in
[`cl/citations/match_citations.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/match_citations.py),
verbatim:

```python
    # Case 2: FullLawCitation (TODO: implement support)
    elif type(full_citation) is FullLawCitation:
        pass

    # Case 3: FullJournalCitation (TODO: implement support)
    elif type(full_citation) is FullJournalCitation:
        pass

    # If no Opinion can be matched, just return a placeholder object
    return NO_MATCH_RESOURCE
```

So a statute citation is deliberately routed to `NO_MATCH_RESOURCE`. Support is marked
`TODO`.

**(b) The citation graph cannot hold one.** `OpinionsCited` in
[`cl/search/models.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/search/models.py)
is `citing_opinion` FK → `cited_opinion` FK, both to `Opinion`. There is no polymorphic
target and no statute table to point at.

**(c) The unmatched-citation store explicitly drops them.** From
[`cl/citations/unmatched_citations_utils.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/unmatched_citations_utils.py):

```python
def unmatched_citation_is_valid(citation, self_citations) -> bool:
    if not isinstance(citation, FullCaseCitation):
        return False
```

and the docstring of `store_unmatched_citations` in the same file:

> "Only FullCaseCitations provide useful information for resolution updates. Other types
> are discarded"

`UnmatchedCitation` in
[`cl/citations/models.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/models.py)
subclasses `BaseCitation`, so its columns are `volume`, `reporter`, `page`, `type` — a
case-reporter shape. Its `create_from_eyecite_base` is annotated
`eyecite_citation: FullCaseCitation` and calls `map_reporter_db_cite_type`, which maps
only the nine case cite types (`specialty`, `journal`, `federal`, `state`,
`state_regional`, `neutral`, `specialty_lexis`, `specialty_west`, `scotus_early`) and
would raise `KeyError` on `leg_statute`
([cl/citations/utils.py](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/utils.py)).

**(d) No table exists in the bulk schema.** The `pg_dump` at
`https://com-courtlistener-storage.s3.us-west-2.amazonaws.com/bulk-data/schema-2026-06-30.sql`
declares 126 tables. The only citation-related ones are:

```
search_citation                       (parallel citations of a cluster)
search_citationevent                  (pghistory audit of the above)
search_opinionscited                  (opinion -> opinion)
search_opinionscitedbyrecapdocument   (recap doc -> opinion)
citations_unmatchedcitation           (volume, reporter, page, type, status,
                                       citation_string, court_id, year, citing_opinion_id)
search_parenthetical
search_parentheticalgroup
```

There is no statute, law, code or regulation table, and no `law-citations` CSV in
`bulk-data/`. `search_citation.type` choices in the model are all case-reporter types
(FEDERAL, STATE, STATE_REGIONAL, SPECIALTY, SCOTUS_EARLY, LEXIS, WEST, NEUTRAL,
JOURNAL); there is no statute value.

**(e) The public Citation Lookup API also excludes them.** In
[`cl/citations/api_views.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/api_views.py)
the view declares `citation_list: list[FullCaseCitation | ShortCaseCitation]`, and that
list is populated by `CitationCountRateThrottle` in
[`cl/api/utils.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/api/utils.py):

```python
citation_objs = filter_out_non_case_law_and_non_valid_citations(
    eyecite.get_citations(text, tokenizer=HYPERSCAN_TOKENIZER)
)
view.citation_list = citation_objs
```

`filter_out_non_case_law_and_non_valid_citations` keeps only
`FullCaseCitation | ShortCaseCitation`
([cl/citations/utils.py](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/utils.py)).
So you cannot get statute parsing out of the hosted API either.

**Conclusion:** law citations are used **transiently**. They exist inside the
`get_citations` → `resolve_citations` → `create_cited_html` pass, get written into
`html_with_citations` as display markup, and are then thrown away. Nothing is queryable.

### 8.3 Is a statute reference marked up in `html_with_citations`?

**Yes, but only as an unlinked span with no identifier.**

The mechanism: eyecite's `resolve_citations` calls `resolve_full_citation` for any
`FullCitation`
([eyecite/resolve.py](https://github.com/freelawproject/eyecite/blob/main/eyecite/resolve.py):
`if isinstance(citation, FullCitation): resolution = resolve_full_citation(citation)`).
CourtListener's resolver returns `NO_MATCH_RESOURCE`, and `generate_annotations` in
[`cl/citations/annotate_citations.py`](https://github.com/freelawproject/courtlistener/blob/main/cl/citations/annotate_citations.py)
emits for that case:

```python
annotation = ['<span class="citation no-link">', "</span>"]
```

Real markup, from `bulk-data/opinions-2026-06-30.csv.bz2`:

Opinion **380213** (Resource.org HTML source):

```html
... charging him and six others with conspiracy in violation of
<span class="citation no-link">21 U.S.C. § 846</span>. The object of the conspiracy ...
```

Opinion **11103682** (PDF / `plain_text` source, so wrapped in `<pre>`):

```html
... charging Sencibaugh with being a felon in possession of a firearm, in violation of
</pre><span class="citation no-link">18 U.S.C. §  922</span><pre class="inline">(g)(1).
```

Note that `(g)(1)` falls **outside** the span. The subsection is not part of the
annotation, because eyecite's `law_section` group did not capture it here.

Opinion **4653215** (a statute and a regulation together):

```html
</pre><span class="citation no-link">38 U.S.C. § 7105</span><pre class="inline">; </pre>
<span class="citation no-link">38 C.F.R. § 20.202</span><pre class="inline">.
```

**The span carries no identifier.** Compare the three annotation shapes emitted by
`generate_annotations`:

| Resolution | Markup |
|---|---|
| matched case | `<span class="citation" data-id="{Opinion.pk}"><a href="{url}" aria-description="…">…</a></span>` |
| ambiguous case | `<span class="citation multiple-matches"><a href="{citation_redirector_url}">…</a></span>` |
| **anything unmatched, including every statute** | `<span class="citation no-link">…</span>` |

Only the display text is preserved. There is no `data-id`, no `href`, no title/section
attribute, and no way to tell a statute apart from a genuinely unresolvable case
citation except by re-parsing the span text.

**Coverage is partial.** Measured over the same 5,046-opinion prefix of
`opinions-2026-06-30.csv.bz2` used in §3 (5,025 of those have `html_with_citations`):

- Citation spans by class: **63,369** `citation` (linked, with `data-id`), **14,763**
  `citation no-link`, **5,212** `citation multiple-matches`.
- **3,189** `no-link` spans contain text matching `U.S.C.` / `C.F.R.` / `Stat.` /
  `Pub. L.`. **Zero** linked spans and **zero** multiple-matches spans do. That is the
  measurement that proves statutes never resolve.
- Of the 2,100 opinions in the sample that mention a U.S.C. cite in
  `html_with_citations`, only 382 (18.2%) have at least one of those mentions inside a
  span.

That 18.2% is misleading on its own, because it splits sharply by source field:

| Primary source field | Opinions mentioning U.S.C. | …with a U.S.C. inside a span |
|---|---:|---:|
| `xml_harvard` | 103 | 96 (93.2%) |
| `plain_text` | 123 | 118 (95.9%) |
| `html` | 15 | 15 (100.0%) |
| `html_anon_2020` | 1,859 | 153 (8.2%) |

The `html_anon_2020` collapse is almost certainly `unbalanced_tags="skip"` in
`create_cited_html`, which the code comments as "Don't risk overwriting existing tags" —
anon-2020 markup is dense, so many annotations are skipped. I did not prove that
causally; it is an inference from the code. Since `html_anon_2020` is only 0.23% of
clusters (§1), the corpus-wide picture is closer to the 93–100% rows. **This sample is
the same biased prefix as §3; treat the percentages as shape, not as corpus statistics.**

### 8.4 What would it cost us to extract law citations ourselves?

**eyecite is usable, and it is permissively licensed.**

- Package `eyecite`, version **2.7.8**, `license = "BSD-2-Clause"` in
  [`pyproject.toml`](https://github.com/freelawproject/eyecite/blob/main/pyproject.toml);
  the [LICENSE](https://github.com/freelawproject/eyecite/blob/main/LICENSE) file reads
  "BSD 2-Clause License / Copyright (c) 2020, Free Law Project".
- Runtime dependency `reporters-db>=3.2.53`. `hyperscan>=0.1.5` is in a
  `[dependency-groups]` extra, not a hard requirement — the `HyperscanTokenizer` that
  CourtListener uses is optional and only a speed optimisation.
- **It is a Python library.** There is no Rust binding and no C API. I found no Rust port
  in the Free Law Project org. From outside Python your options are a subprocess or a
  small HTTP service, PyO3 embedding, or reimplementation.

**Reimplementation in Rust is realistic, because the data is language-neutral.**

- `reporters-db` is **BSD-2-Clause**
  ([repo](https://github.com/freelawproject/reporters-db)) and ships the vocabulary as
  plain JSON in `reporters_db/data/`: `laws.json` (211,528 B), `reporters.json`
  (907,920 B), `journals.json` (265,208 B), `regexes.json` (4,658 B),
  `case_name_abbreviations.json`, `state_abbreviations.json`, `reporters.csv`. Their
  README states: "the data is in the `json` format, so you should be able to import it
  using your language of choice."
- The law regexes are templates with `$`-placeholders resolved from
  [`regexes.json`](https://github.com/freelawproject/reporters-db/blob/main/reporters_db/data/regexes.json).
  The relevant ones, verbatim:

  ```
  law.section       (?P<section>(?:\d+(?:[\-.:]\d+){,3})|(?:\d+(?:\((?:[a-zA-Z]{1}|\d{1,2})\))+))
  section_marker    ((§§?)|([Ss]((ec)(tion)?)?s?\.?))
  law.year          (?P<year>1\d{3}|20\d{2})
  ```

- **Portability check.** I scanned every `regexes` entry in `laws.json`: **zero** use
  lookahead, lookbehind or backreferences. That means the Rust `regex` crate can compile
  them. One caveat: `regexes.json` uses the Python-only `{,n}` bounded-repeat form (for
  example `{,3}` in `law.section`); Rust requires `{0,3}`. That is a mechanical rewrite.

**Estimated cost, for the statute subset only:**

- *Cheapest path:* handle federal statutes and the C.F.R. with your own rules. That is
  14 U.S. federal keys in `laws.json`, of which you probably care about four: `U.S.C.`,
  `C.F.R.`, `Stat.`, `Pub. L.`. Their regexes are a handful of lines. This is a day or
  two of Rust, and it maps directly onto the USLM hierarchy words_to_data already models
  (title → section). It also lets you fix eyecite's `§§ 1983, 1988` truncation and its
  `Pub. L. … Stat.` mis-parse, both of which matter for a linking product.
- *Fuller path:* compile all 371 `laws.json` keys plus the `$`-placeholder expansion.
  That is a data-driven loader, not per-reporter code; call it a week including tests
  against the `examples` arrays that `laws.json` ships for each key (the U.S.C. entry
  alone carries 11 worked examples, which are ready-made test fixtures).
- *Interim path:* shell out to eyecite from the existing Python bindings layer. The repo
  already uses maturin and has a `.venv`, so `pip install eyecite` costs nothing
  structurally, and you get 371 reporters and the case-citation logic for free.

Note there is nothing to *resolve against* on the CourtListener side. Even a perfect
extractor gives you a `(title, reporter, section)` triple and nothing more. Resolving
`26 U.S.C. § 174` to a document is a job for the USLM corpus words_to_data already
ingests — which is the point of doing this at all.

### 8.5 CFR and regulations specifically

Everything above applies unchanged. Additional detail:

- `C.F.R.` is in `laws.json` with `"cite_type": "admin_compilation"` and
  `"jurisdiction": "United States"`. Its one regex is
  `"(?P<chapter>\\d+) $reporter,? § $law_section"`.
- Verified: `40 C.F.R. § 122.2 (2019).` yields
  `FullLawCitation(groups={'chapter': '40', 'reporter': 'C.F.R.', 'section': '122.2'},
  metadata.year='2019')`. Note the volume is captured as `chapter`, not `title`, which is
  inconsistent with the `U.S.C.` entry's `title`. Normalise this yourself.
- Verified: `See 29 C.F.R. 1910.132.` — with no section marker — yields **nothing**. A
  large share of real regulation cites in opinions omit the `§`. Budget for this.
- `laws.json` counts 59 `admin_compilation` and 58 `admin_register` entries, so state
  administrative codes and registers are covered too.
- The Federal Register is present under the key `FR`, name "Federal Register",
  `cite_type: admin_register`, jurisdiction "United States". `Fed. Reg.` is **not** a key
  and is not among the `FR` variations I checked. **I could not test whether eyecite
  matches `85 Fed. Reg. 12,345` — the sandbox blocked further command execution before I
  ran it.** Verify this before relying on Federal Register extraction.
- CourtListener treats a C.F.R. cite exactly like a U.S.C. cite: `FullLawCitation` →
  `NO_MATCH_RESOURCE` → `<span class="citation no-link">38 C.F.R. § 20.202</span>`
  (real markup from opinion 4653215). Nothing stored.
- No source I found says anything about agency adjudications, Federal Register notices,
  or state regulations as a distinct data type in CourtListener.

### 8.6 Addendum to "What this means for words_to_data"

**Statute and regulation citations are not available from CourtListener at all.** Not as
a table, not as an API field, not as a bulk export. If a statute link graph is the
product, you build the extractor. That is not a research risk; it is scoped work.

**Do not build it on `html_with_citations`.** Coverage is 8–100% depending on source
field, the span carries no identifier, and subsections such as `(g)(1)` fall outside the
markup. Parse the source text field instead (§1 preference order), which you are already
doing for structure.

**Rules beat a model here, again.** A statute citation is a regular language. The
vocabulary is BSD-2 JSON you can vendor, none of the law regexes need lookaround, and
the `examples` arrays are ready-made fixtures. Four federal keys (`U.S.C.`, `C.F.R.`,
`Stat.`, `Pub. L.`) cover the federal case, and your own rules can fix two eyecite
defects that a wrapper cannot.

### 8.7 Additional open questions

10. **Whether eyecite matches `Fed. Reg.`** as written in opinions, given the key is
    `FR`. Untested — execution was blocked before I could check.
11. **How often law citations actually occur** corpus-wide. Same 54.56 GB bzip2 obstacle
    as questions 1 and 2. In my biased prefix, 2,100 of 5,025 opinions with
    `html_with_citations` mention a U.S.C. cite, but that number is not trustworthy.
12. **Whether the `FullLawCitation` `TODO` in `match_citations.py` is planned work.**
    The comment says "(TODO: implement support)" with no issue reference. I found no
    roadmap entry, design doc or release note about statute resolution.
13. **Why `U.S.C.` uses `title` but `C.F.R.` uses `chapter`** for the leading number in
    `reporters-db`. No source explains the choice; it looks like an inconsistency you
    must normalise around.
