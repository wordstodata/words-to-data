# `AmendingAction::Move`

There is no `move` amending action. A bill cannot state one, so this build will never read one, and no sample of one can be found.

## Why this is out of scope

The publisher's schema settles it. `uslm-2.0.17.xsd:610-714` defines `AmendingActionTypeEnum` with twelve values and no more:

```
enact  add  amend  substitute  redesignate  repeal
repealAndReserve  insert  delete  conform  noChange  unknown
```

The string `"move"` appears **zero times** in the whole schema. The five public laws in the cache use six of the twelve — `insert` 1302, `delete` 1216, `amend` 1155, `add` 391, `redesignate` 116, `repeal` 26 — and a conforming document cannot carry a thirteenth.

`AmendingAction::Move` was therefore ours. It could only ever be filled from `match_amendments.rs`, which parses the `operation` string of a **model's reply** and defaults to `Amend`. A variant that only a model can fill is not a fact from a source.

#143 asked for a real bill carrying one, so that a move could be recorded against a real fixture rather than an invented one. That was the right instinct under `CLAUDE.md` — never invent data — and the request cannot be satisfied.

## What is real, and is not this

**A provision does get relocated.** A section is transferred to another chapter; a paragraph moves under a different subsection. The law does this, and `docs/adr/0001-structural-paths-locate-not-identify.md` is right that it breaks a path.

What is not real is the *markup value*. A bill writes a relocation the same way it writes anything else: in words, usually as a redesignation next to an addition and a repeal, or as prose — "is transferred to". So reading a relocation is a **words** problem, of the same kind as the 13 redesignation statements this build reports rather than places (#153), and of the same kind the model reader exists for (#154).

If relocation is wanted, it starts from a real clause in the corpus and a reader for those words. It does not start from an action type, and it does not wait for a sample that cannot arrive.

## The link model already permits it

Worth recording, because it is the one part of the old plan that survives: a link's two ends carry a work each, so an edge may cross from one work to another. Nothing in the core has to change for a relocation to be recorded once somebody can read one. The gap was never the model.

## Related

- #143, closed by this entry
- #156, the enum values that do and do not exist
- #153, unplaced statements — where an unreadable relocation clause would surface
- `docs/adr/0001-structural-paths-locate-not-identify.md`, which cites `Move` as motivation. The motivation stands on `Redesignate` alone.
