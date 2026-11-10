# cloud-platform-events

Usage message schema for cloud-platform. This crate is the single owner of the envelope
and payload types. Producers (inference-proxy) and the consumer (cloud-platform) depend on
it by git tag; nobody hand-writes the JSON.

- `InternalUsageReport`: the body of cloud-api's `POST /v1/internal/usage`, copied from
  cloud-api at commit a77d2e2a (serde parts only).
- `InternalUsageReport.admission_id`: optional execution UUID for reservation settlement; provider `id` remains correlation data.
- `AudioTranscription`: rounded-up `duration_seconds` at the audio model rate.
- `UsageEnvelope` / `parse_message`: versioned envelope and tolerant reader.
- Tests: `cargo test` (round-trip, tolerant reader, cloud-api body shape).
- Lint: `cargo clippy --all-targets -- -D warnings`.

Schema rules and the versioning process are in `.claude/rules/events-versioning.md`.

## Compatibility and rollout

Version 2 adds the optional `admission_id` field and the `AudioTranscription` variant.
Rust callers must initialize the new field (usually `None` for legacy reports) and update
exhaustive matches. This is a source API change, so the crate's major version increases.
The JSON envelope remains v1: the new field is omitted when absent, and existing payloads
keep their meaning and shape.

Upgrade the cloud-platform consumer before producers emit admissions or audio receipts.
An older reader ignores `admission_id` and cannot settle the associated reservation; it
retains an unknown audio variant for replay after upgrade. Pin the reviewed commit for
companion testing, and publish an immutable v2.0.0 tag before production rollout.
