# Events versioning

- **C1** Envelope and payload types live only in this crate. Producers and the consumer use
  the crate; nobody hand-writes the JSON. cloud-api's own request type is untouched.
- **C2** Envelope: `{"type":"usage","v":<u16>,"producer":<enum>,"emitted_at":<RFC 3339>,"usage":<payload>}`.
  The phase 1 payload is today's `/v1/internal/usage` body.
  - `type` discriminates event kinds. Each type is versioned by its own `v`.
  - `producer` is a closed enum (`openrouter-gateway`, `fixtures-staging`). Unknown values map
    to `Other`. Informational only; not authenticated.
  - `emitted_at` is the producer's clock.
- **C3** Tolerant reader: unknown fields are ignored; required fields are validated. Adding an
  optional field needs no version bump. Removing or changing a field does.
- **C4** The consumer accepts the current and previous version. Deploy order for a change:
  consumer first, then producers. Only the versions that have a reader are accepted; any other `v` is reported as `UnknownVersion`.
- **C5** This crate is consumed by protected git tags. A schema version is a tag. Review rule: no free-text fields taken from
  requests or responses.

## Adding a version or a type

1. Add the new layout beside the old one. Keep the old reader until the consumer has
   dropped the previous version.
2. Add round-trip fixtures under `tests/fixtures/` for each version.
3. Tag the crate (`vN`). Never move a published tag. Consumers depend on that tag (C5).
4. Deploy the consumer that reads the new version. Then deploy producers that write it.
