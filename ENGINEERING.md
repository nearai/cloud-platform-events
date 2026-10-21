# Engineering Guideline — cloud-platform-events

This crate is the single owner of the usage message schema: the envelope, the message types, their
versions, and the payload that producers send and the consumer reads. Its consumers are
inference-proxy (producer) and cloud-platform (worker). Nothing else defines or hand-writes the JSON.

This document is the rule set for the crate. The ownership table is the source of truth; when a rule
and the table disagree, the table wins and the rule is wrong.

Requirement IDs such as `C2` name the contract rules in `.claude/rules/events-versioning.md`; every
rule that binds this crate is stated there or here.

## 1. Ownership

| Owner | Owns | Interface others use | Never does |
|---|---|---|---|
| `cloud-platform-events` (this crate) | The envelope, the `type` and `v` of each message, the producer enum, each payload type, the tolerant reader | `UsageEnvelope`, `Producer`, `ParsedMessage`, `EnvelopeError`, `parse_message`, `CURRENT_USAGE_VERSION`, `InternalUsageReport` and the other payload types | Any I/O. Business logic. Free-text fields taken from requests or responses. Dependencies beyond `chrono`, `serde`, `serde_json` (`uuid` for tests only) |
| cloud-api, the request type (`crates/api/src/routes/usage.rs`) | Its own HTTP request type and the HTTP body builder | The same JSON value (field names, types and values), checked by a contract test | Changing the payload here without the contract test |

The crate has no other owner. A second type with the same meaning elsewhere is a finding.

## 2. Envelope

Wire shape, for every message:

```json
{"type": "usage", "v": 1, "producer": "openrouter-gateway", "emitted_at": "<RFC 3339>", "usage": {}}
```

- `type` discriminates the kind of event. Each type is versioned independently by `v`.
- `producer` is a closed enum (`openrouter-gateway`, `fixtures-staging`). A value outside the enum
  deserialises to `Other`. It is not rejected. It is informational only; it is not authenticated.
- `emitted_at` is the producer's clock, in RFC 3339.
- `usage` is the payload for the type. In phase 1 it is the body of cloud-api's
  `POST /v1/internal/usage`.

The reader reports, and does not fail on, a message whose `type` it does not handle
(`ParsedMessage::UnknownType`) or whose `v` it does not support (`ParsedMessage::UnknownVersion`).
Those are for the consumer to refuse without deleting the message. A message that is not JSON, not
an object, or missing a required field is an `EnvelopeError`. Error values carry a category and a
position, never the input text.

## 3. Versioning rules

- **Tolerant reader.** Unknown fields are ignored. Required fields are validated. Adding an optional
  field needs no version bump. Removing a field, renaming it, or changing its type or meaning needs a
  new `v`.
- **Two versions.** The reader accepts the current version, and the previous one once a newer version
  exists. Each version has its own match arm in `parse_message`. A reader for a previous version is
  removed only after the consumer no longer needs it.
- **Order for a new version.** The version is tagged first (`vN`). The consumer that reads it is
  deployed next. Producers that write it are deployed last.
- **New type.** A new `type` string gets its own payload, its own `v` starting at `1`, and its own
  round-trip fixtures.
- **Protected tags.** A published schema version is a git tag (`vN`). A published tag is never moved or
  deleted. Consumers pin the release tag that contains the APIs they use before any image leaves
  the developer machine. Local builds may use a path dependency (spec C5).
- **Fixtures.** Each version has round-trip fixtures under `tests/fixtures/`. A new version adds
  fixtures; it does not replace the fixtures of an older one.

## 4. Free text and privacy

- No free-text field may be taken from a request or a response. A string that a customer or an
  upstream provider can shape is a finding at review, whatever its name. Identifiers, enums, numbers,
  timestamps and counts are the shapes allowed.
- The envelope and payload carry no prompts, completions or content keys (spec S1). A field that
  would carry one is rejected at review, not reviewed for size or wording.
- Existing string fields are `model`, `id` (the upstream request identifier, stored as
  `provider_request_id`), and the organization, workspace and API key identifiers. The consumer
  validates them (UUID parsing, model lookup). The `id` value is supplied by the upstream, and this
  crate does not limit its length; limiting it is the consumer's rule. A new string field is
  classified at review as identifier or free text before it merges.

## 5. Dependencies

- Runtime: `chrono` (with `serde`), `serde`, `serde_json`. Dev: `uuid`.
- Adding any dependency needs a sentence in the change saying what it replaces. A dependency that
  performs I/O is not accepted.

## 6. Review gate

- Every change to a type, a field or a version is reviewed against §3 and §4. The change states which
  consumer reads it and whether it is additive.
- Review feedback names the rule and the field, and cites the code.
- Plans and design options are reviewed with `/approach-audit`. Correctness and security review run as
  a separate lane (`/code-review`).

## 7. Tests

| Level | What it proves | Where |
|---|---|---|
| Round trip | Each payload serialises to the wire shape and reads back equal | `tests/roundtrip.rs` |
| Tolerant reader | Unknown fields are ignored; unknown `type` and `v` are reported, not errors; unknown producers map to `Other`; missing required fields and non-JSON are rejected | `tests/roundtrip.rs` |
| Contract with cloud-api | The payload's serialised body has the same shape as cloud-api's `/v1/internal/usage` body | `tests/roundtrip.rs` (`body_matches_cloud_api_shape`), fixtures in `tests/fixtures/` |

The gates before a commit are `cargo test` and `cargo clippy --all-targets -- -D warnings`. The change
states their result.

## 8. Definition of done

A schema change is done when its fixtures exist for each affected version, its tests pass, the
consumer's reader for the new version is in place or the change is additive, and no free-text field
was added.
