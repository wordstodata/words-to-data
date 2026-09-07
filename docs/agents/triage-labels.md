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
