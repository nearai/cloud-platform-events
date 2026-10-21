# cloud-platform-events

Usage message schema for cloud-platform. This crate is the single owner of the envelope
and payload types. Producers (inference-proxy) and the consumer (cloud-platform) depend on
it by git tag; nobody hand-writes the JSON.

- `InternalUsageReport`: the body of cloud-api's `POST /v1/internal/usage`, copied from
  cloud-api at commit a77d2e2a (serde parts only).
- `UsageEnvelope` / `parse_message`: versioned envelope and tolerant reader.
- Tests: `cargo test` (round-trip, tolerant reader, cloud-api body shape).
- Lint: `cargo clippy --all-targets -- -D warnings`.

Schema rules and the versioning process are in `.claude/rules/events-versioning.md`.
