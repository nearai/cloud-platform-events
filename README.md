# cloud-platform-events

Shared Rust types for usage reports and versioned message envelopes.

- `InternalUsageReport` carries usage facts and optional admission correlation.
- `admission_id` identifies a reservation; the provider identifier remains separate.
- `AudioTranscription` carries a rounded-up duration in seconds.
- `UsageEnvelope` and `parse_message` provide a versioned envelope and tolerant reader.

## Compatibility

Version 2 adds the optional admission field. Rust callers must initialize the added
field, so this is a major source API change. The JSON envelope remains version 1. The optional field is omitted when
absent, preserving existing report serialization.

Transcription events use the variant introduced in version 1.2. Older readers can
ignore the optional admission field, but cannot use it to correlate reservations.

## Validation

Run `cargo test` for round-trip, reader, and duration checks, and
`cargo clippy --all-targets -- -D warnings` for lint validation.
