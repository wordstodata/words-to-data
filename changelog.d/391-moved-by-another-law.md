### The quoted words find an address that another law moved in the window ([#391](https://github.com/wordstodata/words-to-data/issues/391))

Two laws can change one section in one window. 26 U.S.C. 25C: another law
redesignated (h) as (i) and inserted a new (h). § 70505(a) of Pub. L. 119-21
amends "Section 25C(h)" by striking "placed in service" and all that follows
through "December 31, 2032" and inserting "placed in service after December
31, 2025". The Code after the window prints those words at (i). The matcher
read the address by its number alone, and linked the amendment to the new
(h) and (h)(2), which are the other law's.

The maintainer decided the order of the evidence: first the quoted words,
then the OLRC notes (#380). The effective date alone does not decide. Now the
address is read at another provision only when the quoted words say so:

- the provision at the address before the window holds every string the
  amendment strikes or quotes to mark a place;
- no change under the address shows the words the amendment inserts or
  enacts;
- one provision, and only one, that the window added whole beside the
  address (the same provision above it, the same level) shows all of them.

The dataset does not hold the OLRC notes (#380 is open), so they are not used.

On a fresh build of `eval.sqlite`, `legislature.amended_by` goes from 3,452
to 3,451 links. One amendment changes, and no other link changes:

- Removed, 2 wrong links of § 70505(a) (`f1e4088d276d`): 26 U.S.C. 25C(h)
  and 25C(h)(2).
- Added, 1 link of § 70505(a): 26 U.S.C. 25C(i), which now prints "This
  section shall not apply with respect to any property placed in service
  after December 31, 2025."

Linked amendments stay at 1,175, and the residue does not change.

Method version: `address, window and quoted words@9` (was 8). Run
`link-by-evidence` again; `residue` then lists the links that only version 8
made, for review.
