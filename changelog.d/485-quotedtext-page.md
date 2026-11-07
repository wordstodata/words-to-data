### A page mark inside a quoted string does not stop the string from matching ([#485](https://github.com/wordstodata/words-to-data/issues/485))

The Public Law print puts a page mark of the Statutes at Large, such as "139 STAT. 245", where a page ends. The evidence matcher removed these marks from an enacted block (`<quotedContent>`), but not from a quoted string (`<quotedText>`). A string with a mark inside it then held words that the Code does not hold, and it gave no evidence. Pub. L. 119-21 § 70434(d) strikes "qualified film or television production or any qualified live theatrical 139 STAT. 245 production" from 26 U.S.C. 181(c)(1). Its link gave only the inserted string as evidence. Now the matcher removes the mark from a quoted string before it compares the string with the Code, and the struck string shows in 26 U.S.C. 181(c)(1). The evidence quotes the string without the mark. The stored bill text does not change.

Method version: `address, window and quoted words@11` (was 10).
