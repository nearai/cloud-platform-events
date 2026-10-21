//! Usage message schema for cloud-platform.
//!
//! This crate is the only owner of the envelope and payload types. Producers and the
//! consumer use it; nobody hand-writes the JSON.

mod envelope;
mod usage;

pub use envelope::{
    parse_message, EncodeError, EnvelopeError, FailureClass, JsonFault, ParsedMessage, Producer,
    UsageEnvelope, CURRENT_USAGE_VERSION, MAX_ENVELOPE_BYTES,
};
pub use usage::{
    InternalUsageReport, ProviderAttribution, RecordUsageApiRequest, ServedProviderTier,
    ServedProviderType,
};
