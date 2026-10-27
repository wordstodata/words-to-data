### One amendment that acts on many sections is matched for each item ([#434](https://github.com/wordstodata/words-to-data/issues/434))

Some amending instructions name an Act, and not a section, in their own line,
and each item of the list below the line names the section it acts on:

> The National Security Act of 1947 (50 U.S.C. 3001 et seq.) is amended—
> (i) in section 103(c)(15) (50 U.S.C. 3025(c)(15)), by striking …
> (ii) in section 112(c)(1), 50 USC 3047. by striking …

Other lines cite several sections: *"Sections 351(a), 352(a), 353(a), and
353(b) of such title are amended by striking …"*. Such an instruction had no
address, and no link held its changes.

Now the address of such an instruction has a new list, `items`: one item for
each level of the list that names a section, or for each section the line
cites, with the item's place in the bill, its section and the designations
below it. The publisher's reference in the item's own words places the
section; a parenthesis such as *"(50 U.S.C. 98d(a)(2), 98e(d)(1),
98e(d)(2))"* places one section for each citation. Where the markup places
no section, the OLRC's table can: the row for the item's own place in the
law, or, for a section that a line cites, the row whose section has the
cited number. The instruction stays one amendment with one id. `section` and
`unresolved` do not change.

The matcher then matches each item as one amendment, with only the words the
item quotes, so the words of one item cannot take a change in another item's
section. A link of an item says so in its evidence.

Not changed: an instruction that adds, moves or rewrites a whole chapter or
part. Its items name no section, so it has no items, and it stays in
`residue` as before. *"of such title"* is still not read as a title.

On a fresh build of `eval.sqlite`, `legislature.amended_by` goes from 3,452
to 3,506 links: 54 added, 0 removed, each read by hand against the bill and
the Code. Of the 81 amendments that stopped with "no section under amendment
was named", 6 now have links (Pub. L. 119-21 § 71401(b); Pub. L. 119-60
§§ 601(c)(4), 1411(c), 6402(c)(2)(A), 7701(g), 8402(a)). 1397aa(a) of title
42 stays without a link: the Code prints the Act's § 2105(h) that item (1)
quotes as § 1397ee(h). `section-agreement` finds 0 disagreements.

The method is now `address, window and quoted words@9`.

#### Upgrade

- **`amendment-addresses --json`** can give a new field, `items`. The other
  fields do not change.
- **`residue --json` can give one amendment in more than one row**, one for
  each item that no standing link holds, each with `"item": true` and the
  item's own `address`. An item row goes away when a standing link of the
  amendment changes the item's section. On `eval.sqlite` 3 amendments have
  item rows (10 rows), and the rows go from 508 to 509.
  With `residue --partial` (#271), a linked item is read as linked in part
  against its own address: its row lists the changes under that address that
  no link claims.
- **`link-by-evidence`** counts each item as one answer and says how many
  there are.
- **Rust library:** `AmendmentAddress::items` and `ItemAddress` are new, and
  so are `AmendmentMatch::item` and `Unlinked::item`. A struct literal of
  `AmendmentAddress` must give `items`.
