### `cases-citing` names the year of an edition the opinion cites ([#470](https://github.com/wordstodata/words-to-data/issues/470))

*United Stationers, Inc. v. United States* (1997) cites "26 U.S.C. § 41
(1986)". `cases-citing` put the words in the brackets in front of "of the
Code", and when the court writes only a year, a word was missing:

```
the opinion cites the 1986 of the Code. This dataset holds no printing of title 26 from 1986, so that law is out of scope.
```

Now each line gives the year first, and then the words in the brackets as the
court wrote them:

```
the opinion cites the Code of 1986, written "(1986)" after the citation. This dataset holds no printing of title 26 from 1986, so that law is out of scope.
no window is given: the opinion cites the Code of 1976, written "(1976 ed.)" after the citation, and this dataset holds no printing of title 8 from that year. A section number can be used again, so the provision now at that number can be a different provision from the one the court read.
```

The line with no window said "holds no printing of it". It now says which
title and that no printing is from that year, which is the test it applies.
The JSON output and the reading of a citation do not change.
