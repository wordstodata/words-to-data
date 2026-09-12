# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the actual label strings used in this repo's issue tracker.

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label string from this table.

Edit the right-hand column to match whatever vocabulary you actually use.

## Attribute labels

These are not states. An issue carries one category role and one state role from the tables above; an attribute label sits alongside them and says something extra about the work.

| Label              | Meaning                                                                  |
| ------------------ | ------------------------------------------------------------------------ |
| `breaking-changes` | Changes a stored format or an interface that existing readers depend on. |

Apply `breaking-changes` when the work does any of these:

- Changes the SQLite schema, which means `SCHEMA_VERSION` in `src/storage/mod.rs` goes up. Datasets are rebuilt, never migrated, so the break must be visible before the work starts.
- Changes a type that is serialized into a W2D file, such as `DatasetMetadata` or `Provenance`. Adding a field breaks an old reader as surely as removing one.
- Changes the JSON a CLI command gives an agent.

Adding a new type, or a new link kind, is not a breaking change.

The label groups work that should share one release window. Three breaking issues merged separately give a user three breaks; merged together they give one.

### Forcing a rebuild is not the same as breaking the format

`breaking-changes` means **an existing reader cannot read the file**. Some work forces every user to rebuild without breaking anything, and the label does not describe it. `docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md` separates the three cases:

| What changed | Example | Old reader | Label |
| --- | --- | --- | --- |
| the **format** | `#129`, the node type | cannot parse the file | `breaking-changes` |
| an **index** — a stored derivation | `#115`, readable container paths | parses fine, holds stale paths | **not** a breaking change |
| a **record** — the facts themselves | `#113`, the appendix parser | parses fine, holds different facts | **not** a breaking change |

All three need a rebuild. Only the first one makes a reader obsolete.

So when triaging, say which of the three it is, and whether a rebuild is needed — those are two questions, not one. An issue that forces a rebuild without breaking the format still has to say so in its body, because a user who does not rebuild will silently hold stale data. That is worse than a refusal, not better.

The rebuild cost is not fixed. `#123` records that `match-amendments` has no cache, so a rebuild re-buys its model calls; until that is fixed, grouping work into fewer rebuilds is worth more than the label alone suggests.
