# Senate roll calls: can a dataset hold them?

Research date: 2026-11-05. Every request in this note was made on that date, from
the maintainer's workstation. Issue: #112. Related: #104, #105.

## The answer

1. **The Congress.gov API has no Senate vote resource.** The Senate publishes each
   roll call itself as an XML file on senate.gov. The API gives the URL of that
   file, but senate.gov refused our requests for it with HTTP 403.
2. **The senate.gov file was not read, so its identifier is not confirmed.** Other
   evidence says that it uses a Senate identifier (`lis_member_id`), not
   `bioguide_id`, and that a published crosswalk joins the two. Voteview, a
   separate dataset, gives `bioguide_id` directly for all 104 senators of the
   119th Congress.
3. **Holding a Senate vote is a `breaking-changes` item.** `SCHEMA_VERSION` goes
   from 11 to 12, each dataset is built again, and `HouseRollCall` becomes a type
   that carries its chamber. Section 3 gives the full cost.
4. **A source exists.** The open problem is access and choice of source. A person
   must decide which source to use (section 5).

## What was not tested

Read this before you use the rest of the note.

- **No Senate XML file was read.** Both requests for one stopped at the 403. We
  did not change the request, use another route to senate.gov, use a headless
  browser, or get a copy of the file from a mirror. All statements here about the
  content of that file come from third-party code, and they are not confirmed.
- **The senate.gov terms of use were not read.** We did not find a terms page, and
  the page that lists the votes gave 403.
- **The 403 was seen from one client only.** We did not test another network or
  HTTP client, so we do not know what starts the block.
- **The member list of a roll call in the Congressional Record was not read.** The
  Daily Digest gives the tally. The article we fetched at the page it names does
  not hold the names. The whole Senate section downloaded as a PDF, but this
  machine has no tool that extracts text from a PDF.
- **Voteview was not compared, name by name, with an official record.** Its tally
  for one roll call was compared with the Daily Digest. We did not find where
  Voteview gets its Senate data, or a license for it.
- **No code was written.** The cost in section 3 comes from reading the code, not
  from a trial change.

## 1. Does a source exist?

### The Congress.gov API

| Request | Result |
| --- | --- |
| `GET api.congress.gov/v3/senate-vote` | 404, `Unknown resource: senate-vote` |
| `GET api.congress.gov/v3/senate-vote/119` | 404, `Unknown resource` |
| `GET api.congress.gov/v3/senate-vote/119/1` | 404, `Unknown resource` |
| `GET api.congress.gov/v3/senate-vote/119/1/1` | 404, `Unknown resource` |
| `GET api.congress.gov/v3/senate-vote/119/1/1/members` | 404, `Unknown resource` |
| `GET api.congress.gov/v3/senate-roll-call-vote/119/1` | 404, `Unknown resource` |
| `GET api.congress.gov/v3/house-vote/119/1/1` (control) | 200, `houseRollCallVote` |

The resource names `senate-vote` and `senate-roll-call-vote` are our guesses, made
from `house-vote`. The publisher confirms the result:

- [api.congress.gov issue #436](https://github.com/LibraryOfCongress/api.congress.gov/issues/436),
  2026-04-02, from a Congress.gov team member: "Only House roll call votes are
  available on Congress.gov and the Congress.gov API." The same comment names
  [`senate.gov/legislative/votes_new.htm`](https://www.senate.gov/legislative/votes_new.htm)
  as the official source, and says that the team has no update on when Senate
  votes will be available.
- [api.congress.gov issue #249](https://github.com/LibraryOfCongress/api.congress.gov/issues/249),
  the feature request for Senate votes, is open.
- The [API change log](https://github.com/LibraryOfCongress/api.congress.gov/blob/main/ChangeLog.md)
  has no entry for a Senate vote endpoint, as a completed or an upcoming change.

The API does point to the Senate records. `GET /v3/bill/119/hr/1/actions` returned
20 distinct Senate entries in `recordedVotes`, for example:

```json
{"chamber":"Senate","congress":119,"date":"2025-07-01T16:03:40Z","rollNumber":372,
 "sessionNumber":1,
 "url":"https://www.senate.gov/legislative/LIS/roll_call_votes/vote1191/vote_119_1_00372.xml"}
```

So a bill can find its Senate roll calls from data that the client already reads.
`tests/test_data/congress_client_cache/bill/119/hr/1/actions.json` holds the same
kind of entry.

### senate.gov

| URL on `www.senate.gov` | Result |
| --- | --- |
| `/legislative/LIS/roll_call_votes/vote1191/vote_119_1_00372.xml` | 403 |
| `/legislative/LIS/roll_call_lists/vote_menu_119_1.xml` | 403 |
| `/legislative/votes_new.htm` | 403 |
| `/` | 403 |
| `/robots.txt` | 302 to a "file not found" page |
| `/general/contacting.htm` | 200 |

The XML 403 is a page with the title "Senate.gov - Unauthorized". It says that a
security risk was detected and that the webmaster was alerted. All requests used
one `curl` user agent, which we did not change.

This is a known block.
[api.congress.gov issue #441](https://github.com/LibraryOfCongress/api.congress.gov/issues/441),
2026-04-18, reports the same 403 on the same URLs and attributes it to the Akamai
firewall of the Senate. The Library of Congress closed it and sent the reporter to
the [Senate webmaster](https://www.senate.gov/general/contacting.htm). That page
gives `webmaster@sec.senate.gov` for questions about the function of the site.

The workarounds which that issue names are a headless browser and unofficial
mirrors. A block is a decision by the site owner, so this project must not use
such a workaround without a decision from the maintainer. The correct next step is
to ask the webmaster for access.

### Other publishers

| Source | Request | Result |
| --- | --- | --- |
| Congressional Record, through the API | `GET /v3/daily-congressional-record/171/113/articles` | 200 |
| Congressional Record text | `congress.gov/119/crec/2025/06/30/171/113/modified/CREC-2025-06-30-pt1-PgD678.htm` | 200 |
| Congressional Record, Senate section PDF | `govinfo.gov/content/pkg/CREC-2025-06-30/pdf/CREC-2025-06-30-senate.pdf` | 200, 1.6 MB |
| Voteview rolls | `voteview.com/static/data/out/rollcalls/S119_rollcalls.csv` | 200, 915 rolls to 2026-09-30 |
| Voteview votes | `voteview.com/static/data/out/votes/S119_votes.csv` | 200, 91,633 rows |
| Voteview members | `voteview.com/static/data/out/members/S119_members.csv` | 200 |
| Crosswalk | `raw.githubusercontent.com/unitedstates/congress-legislators/main/legislators-current.yaml` | 200 |

The Daily Digest for the Record of 2025-06-30, page S4074, gives the official
tally: "By 51 yeas to 50 nays, Vice President voting yea (Vote No. 372), Senate
passed H.R. 1".

[Voteview](https://voteview.com/data) asks for a citation. It is an academic
dataset, not a record of the Senate. It is a candidate source, and its use is a
decision for the maintainer. Of its 915 rolls, 457 are on measures and 458 are on
nominations (`bill_number` starts with `PN`).

[GovQL](https://govql.us) was named in issue #249. It runs on code that scrapes
Senate data, so we treated it as a mirror and did not request it.

## 2. How does a Senate record identify a member?

**The senate.gov XML (not confirmed).** The open
[`vote_info.py`](https://github.com/unitedstates/congress/blob/main/congress/tasks/vote_info.py)
parser reads these elements below `members/member`: `lis_member_id`, `state`,
`party`, `member_full`, `first_name`, `last_name` and `vote_cast`. It does not read
a Bioguide identifier. That is evidence about the form of the file, and it is not
a read of the file. The same code says that the votes 133 to 136 of the 101st
Congress, 1st session, have no `lis_member_id`, so old files need a name lookup.

**The crosswalk (tested).** `legislators-current.yaml` gives `bioguide` and `lis`
for each senator:

- 100 current senators, 100 have a `lis` value, and no value is used twice.
- For example, `bioguide: T000476` has `lis: S384`.
- The API also accepts that member: `GET /v3/member/T000476` gives 200, with
  `bioguideId` and no LIS field. So the Congress.gov API cannot give this join.

**Voteview (tested).** `S119_members.csv` has a `bioguide_id` column, so no
crosswalk is necessary:

- 104 Senate rows, and each has a `bioguide_id`.
- 100 of the 104 are in `legislators-current.yaml`. The other four (V000137,
  R000595, G000359, M001190) are in `legislators-historical.yaml`, each with a
  `lis` value.
- A vote joins on `icpsr`. For example: `S119_votes.csv` has
  `119,Senate,372,41504,6`, and `S119_members.csv` has `icpsr` 41504 =
  `TILLIS, Thomas Roland (Thom)`, `T000476`. `T000476` is the key of the member
  store (`MemberVote::bioguide_id`).

**So:** with Voteview the join to `bioguide_id` works now, and was tested. With the
senate.gov XML, the join is expected to work through the crosswalk, and was not
tested on a real file.

### What the test of roll 372 showed

Voteview has 50 rows with cast code 1, 50 with cast code 6, and one more row with
cast code 1. That row is `icpsr` 99912, the President row of the members file, not
a senator. The roll row gives `yea_count` 50 and `nay_count` 50. The official tally
is 51 to 50, with the Vice President voting yea. So:

- A reader of Voteview must leave out the President row. We did not check what the
  row means.
- The Vice President's vote is not a senator's vote. A sum of the senators gives 50
  to 50, which does not explain the result.

Voteview numbers its rolls through the whole Congress. Roll 660 is roll 1 of
session 2 (`clerk_rollnumber`). A reader must use `session` and `clerk_rollnumber`.

## 3. What type changes are necessary?

The code was read at `1230207`. No code changes in this PR.

### The format

| Item | Now | Necessary change |
| --- | --- | --- |
| `HouseRollCall` (`src/congress/vote.rs`) | no chamber | Name it `RollCall`, and add `chamber: Chamber` |
| `BillVotes.roll_calls` | `Vec<HouseRollCall>` | `Vec<RollCall>`; it is serialized in `DatasetCompact.bill_votes` (`src/compact.rs`) |
| `roll_calls` table (`src/storage/sqlite.rs`) | no `chamber` column | Add the column; `CREATE TABLE IF NOT EXISTS` does not change a table that exists |
| `SCHEMA_VERSION` (`src/storage/mod.rs`) | 11 | 12 |
| Tie vote | no field | A place for the Vice President's vote (see below) |

This is `breaking-changes`, by the rules in `docs/agents/triage-labels.md`: the
schema changes, and a serialized type changes. `src/storage/sqlite.rs` and
`src/compact.rs` refuse a file at another version, so each user builds each
dataset again. Do not use `#[serde(default)]` to make an old roll call read as
House. The version check refuses the old file before that matters, and a default
chamber is a guess.

### The cost beyond the format

These are the faults that a change of the type alone does not find:

1. **The filter.** `extract_house_votes_from_actions` (`src/congress/vote.rs`)
   drops each entry that is not `"House"`. `get_bill_house_votes` calls it.
2. **The duplicate check.** `seen_rolls` in `src/congress/client.rs` holds only
   `roll_number`. For H.R. 1, the House and the Senate each have their own number
   sequence, so a House roll and a Senate roll with one number would remove each
   other. The key must be chamber, session and number. This key is already weak
   for the House: it has no session.
3. **Dropped positions.** `from_api_response` uses `if let Ok(position)`, so a
   position that `VotePosition` does not parse is removed without a message. The
   `vote_info.py` parser handles "Present, Giving Live Pair", and "Guilty" and "Not
   Guilty". The bill case is the live pair. On read, `sqlite.rs` maps each unknown
   stored position to `NotVoting`.
4. **The tie.** See roll 372 above. `MemberVote` holds one bioguide and one
   position, and the tallies are sums. Add the Vice President as a member vote and
   the sum becomes 51, but that person is not a senator on that date. Leave the
   vote out and the sum does not explain the result. This needs a decision.
5. **The date.** `HouseRollCall::day()` reads RFC 3339 or `YYYY-MM-DD`. Any other
   date form gives `None`, and each vote on the roll goes to `unresolved`. The
   date form in the XML is not known. The `recordedVotes` date is UTC, not the day
   the chamber sat, so a late vote can have the wrong day. Voteview gives
   `YYYY-MM-DD`.
6. **The readers.** `RollCallTally` (`src/inspect.rs`) has no chamber, and
   `tallies.sort_by_key` sorts by number only, so the two chambers mix. The
   `votes` command, its help text and its empty message say "House", and so does
   the chain in `cases_citing.rs`. `CONTEXT.md` says "Only House roll calls can be
   held".
7. **The identifier.** If the source is the XML, resolve `lis_member_id` to
   `bioguide_id` when the file is read, and store only `bioguide_id`. Then `Member`
   does not change. The crosswalk is a new input that the project must keep up to
   date. Each senator on a roll also needs a member record, but `fetch_members`
   already fetches each id that a vote names, so that cost is requests only.
8. **A new parser and client.** The present parser reads JSON from the Congress.gov
   API. Each other source needs its own reader, its own cache and real test files.
   There is no Senate file in `tests/test_data/`.

### The size

- 8 files in `src/` and `tests/` name `HouseRollCall`, `BillVotes` or the two client
  methods, with 20 uses of `HouseRollCall`.
- A dataset gains approximately 100 member votes for each Senate roll. H.R. 1 alone
  has 20 Senate rolls in its actions, so approximately 2,000 rows. The dataset in
  `a-court-opinion-in-the-core.md` holds 1,297 member votes.
- This is one PR of medium size, plus one rebuild for each user. Because a rebuild
  is the larger cost, put it in one release window with the other
  `breaking-changes` items.

## 4. If no source exists

This question does not apply, because a source exists. The Senate publishes the
records. senate.gov refuses automated requests from this client, and Voteview is a
tested second source. The issue that follows this note is the type change, as the
brief says for the case where a source exists.

The `votes` output already says that it covers the House: see its help text and
its empty message. The JSON output of `votes` does not carry a chamber.

## 5. Decisions for the maintainer

1. Ask `webmaster@sec.senate.gov` for automated access to the XML files, or not.
2. Use Voteview as the source, or not. Check its license and its data lineage
   first, and compare a sample with an official record.
3. Decide how a dataset records the Vice President's vote.
