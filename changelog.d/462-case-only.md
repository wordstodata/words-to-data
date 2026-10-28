### The matcher sees a change of case only ([#462](https://github.com/wordstodata/words-to-data/issues/462))

The evidence matcher compares words in lower case. A change that changes only the case of words was then no change at all, and the amendment that made it got no link. Pub. L. 119-60 § 1701(a)(3)(A) changes the heading of 10 U.S.C. 132a from "Performance improvement officer" to "Performance Improvement Officer"; since #460 took the footnote out of the words, that change had no link. Now a quoted string that shows nothing in lower case is also compared with the case the bill prints it in. A struck "The purpose" now shows in 42 U.S.C. 1397aa(a), where the inserted words begin "Except with respect to …, the purpose".

Method version: `address, window and quoted words@10` (was 9).
