### `residue` counts amendments, not rows ([#469](https://github.com/wordstodata/words-to-data/issues/469))

Since #434, an amendment that acts on many sections has one row for each item
of its list. The text output of `residue` printed the number of rows under the
label "amendment(s)", and each category count was a count of rows. So the
headline of 119-s-1071 said "390 amendment(s)" for 388 amendments, and
`bills` LINKED plus the headline was more than the amendments of the bill.

Now every count in the text output is of amendments. The output an agent reads
changes in these places, and only when an amendment has more than one row:

- The headline: `390 row(s) for 388 amendment(s) that no standing link names`.
  When each amendment has one row, the headline is as before:
  `N amendment(s) that no standing link names`.
- A category or reason count gives the rows after its name when they are more
  than the amendments: `180  work (181 rows)`, `57  quiet (62 rows): …`.
- A new section after the counts names each amendment whose rows fall in more
  than one category, because each of those categories counts it:
  `1 amendment(s) have rows in more than one category, and each of those categories counts them:`
  then one line for each, such as `8b200cc5e353  work, quiet`.
- `residue --help` says that one amendment can have one row for each item.

`--json` does not change: it gives one entry for each row, as before.
