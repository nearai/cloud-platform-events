# AGENTS.md — working agreement for agents in this repository

Start with [`ENGINEERING.md`](ENGINEERING.md). It is the rule set for this schema crate: ownership,
versioning, the review gate. This file governs what may exist in the repository at all.

## The documentation rule

This repository carries documentation that describes **what is**. It does not carry plans.

**Belongs here**

| Kind | Examples |
|---|---|
| Rules and working agreements | `ENGINEERING.md`, this file, `.claude/rules/*` |
| Entry points | `README.md` |
| Descriptions of the current schema | module docs (`//!`), the README's list of types |
| Explicitly dated historical records | dated, past-tense records, such as the compatibility check behind a tag |

**Does not belong here**

- Implementation plans, task breakdowns, step-by-step build instructions
- Design specs for schema versions that have not been built
- Handoff notes, roadmaps, TODO lists, "next steps" sections
- Any document whose accuracy decays as the code changes

**The test:** *if a document would become wrong the moment someone merges a change, it does not
belong in the repository.* A schema version is described by its code and its round-trip fixtures.
The reasoning for a version belongs in the pull request that introduces it.

## Consequences for an agent working here

- Do not write a plan file into this repository. Write it to scratch space and put the summary in
  the pull request body.
- Do not move or re-create a published tag. A schema version is a tag; a new layout is a new version.
- Do not add a dependency. The crate depends on `chrono`, `serde` and `serde_json`, and on `uuid`
  for tests only.
- The repository has no remote. Do not push, and do not add a remote.
