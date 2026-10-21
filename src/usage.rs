//! Payload of a usage event: the body of cloud-api's `POST /v1/internal/usage`.
//!
//! Copied from cloud-api at commit a77d2e2a (read-only pin):
//! - `InternalUsageReport`: `crates/api/src/routes/usage.rs` (`RecordUsageInternalRequest`)
//! - `RecordUsageApiRequest`: `crates/services/src/usage/ports.rs`
//! - `ProviderAttribution` and the served-provider enums: `crates/services/src/usage/provider_attribution.rs`
//!
//! Only the serde-relevant parts are copied. Field names, tags, renames and flatten
//! are identical, so the JSON wire format is the cloud-api body. Deserialization is
//! tolerant: unknown fields are ignored.

use serde::{Deserialize, Serialize};

/// Request body of `POST /v1/internal/usage`, with the usage payload flattened in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InternalUsageReport {
    /// UUID of the organization to bill.
    pub organization_id: String,
    /// UUID of the workspace the usage belongs to.
    pub workspace_id: String,
    /// UUID of the API key the usage is attributed to.
    pub api_key_id: String,
    /// Optional provider-side discount as a fraction of list price (`0.2` = 20% off).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount_to_user: Option<f64>,
    /// The usage payload.
    #[serde(flatten)]
    pub usage: RecordUsageApiRequest,
}

/// Usage payload, a tagged union on `type`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RecordUsageApiRequest {
    /// Token-based chat completion usage.
    ChatCompletion {
        model: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        input_tokens: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output_tokens: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_read_tokens: Option<i32>,
        id: String,
        #[serde(flatten)]
        provider_attribution: ProviderAttribution,
    },
    /// Image generation or editing.
    ImageGeneration {
        model: String,
        image_count: i32,
        id: String,
        #[serde(flatten)]
        provider_attribution: ProviderAttribution,
    },
    /// Input-token-billed, output-less inference.
    Embedding {
        model: String,
        input_tokens: i32,
        id: String,
        #[serde(flatten)]
        provider_attribution: ProviderAttribution,
    },
    /// Document reranking (billed like [`Self::Embedding`]).
    Rerank {
        model: String,
        input_tokens: i32,
        id: String,
        #[serde(flatten)]
        provider_attribution: ProviderAttribution,
    },
    /// Text similarity scoring (billed like [`Self::Embedding`]).
    Score {
        model: String,
        input_tokens: i32,
        id: String,
        #[serde(flatten)]
        provider_attribution: ProviderAttribution,
    },
    /// Privacy/PII classification (billed like [`Self::Embedding`]).
    PrivacyClassify {
        model: String,
        input_tokens: i32,
        id: String,
        #[serde(flatten)]
        provider_attribution: ProviderAttribution,
    },
}

/// Which provider served a request. Serialized as `served_provider_*` keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAttribution {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub served_provider_tier: Option<ServedProviderTier>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub served_provider_type: Option<ServedProviderType>,
    #[serde(default)]
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub served_via_fallback: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServedProviderTier {
    Near,
    #[serde(rename = "attested_3p")]
    Attested3p,
    NonAttested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServedProviderType {
    Vllm,
    External,
    Chutes,
    Tinfoil,
}
