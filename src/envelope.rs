//! Versioned envelope around a usage payload, and the tolerant reader.
//!
//! Wire shape: `{"type":"usage","v":1,"producer":"...","emitted_at":"<RFC 3339>","usage":{...}}`.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::usage::InternalUsageReport;

/// Version of the `usage` message type produced by this crate.
pub const CURRENT_USAGE_VERSION: u16 = 1;

/// Largest message the producer may send: the SQS message body limit, in bytes.
pub const MAX_ENVELOPE_BYTES: usize = 262_144;

/// Name of the usage message type on the wire.
const USAGE_TYPE: &str = "usage";

/// Producer identity. Closed set; unknown strings deserialize to [`Producer::Other`]
/// (informational only until event signing).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Producer {
    OpenrouterGateway,
    FixturesStaging,
    #[serde(other)]
    Other,
}

/// A usage event: envelope metadata plus the cloud-api usage body.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageEnvelope {
    pub v: u16,
    pub producer: Producer,
    pub emitted_at: DateTime<Utc>,
    pub usage: InternalUsageReport,
}

/// Result of reading one message.
#[derive(Clone, Debug, PartialEq)]
pub enum ParsedMessage {
    Usage(UsageEnvelope),
    /// The `type` is not handled by this crate.
    UnknownType(String),
    /// The `type` is handled, but the `v` has no reader in this crate. `v` is any JSON
    /// integer other than the current version (it may exceed `u16`).
    UnknownVersion {
        v: i128,
    },
}

/// Whether a parse failure is a message this reader does not know, or a malformed one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureClass {
    /// Well-formed JSON with a value outside the known set (for example an unknown
    /// `served_provider_type` or usage `type`). A newer producer may have sent it.
    Unknown,
    /// Not JSON, not an object, a missing required field, or a wrong-typed field.
    Malformed,
}

/// Why a message could not be read at all. Messages carry payload values, so errors
/// report a category and a position, never the input text (privacy: IDs and counts only).
#[derive(Debug)]
pub enum EnvelopeError {
    /// Bytes are not JSON, or a field has the wrong shape.
    Json(JsonFault),
    /// Well-formed JSON carrying a value outside the known set. Classified as
    /// [`FailureClass::Unknown`].
    UnknownValue(JsonFault),
    /// The message is JSON but not an object.
    NotAnObject,
    /// A required envelope or payload field is absent.
    MissingField(&'static str),
    /// A required envelope or payload field has the wrong type or range.
    InvalidField(&'static str),
}

/// Why a message could not be encoded. Carries a category or a length, never payload text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The envelope could not be serialized.
    Serialize,
    /// The encoded message is longer than [`MAX_ENVELOPE_BYTES`]. Carries its length in bytes.
    EnvelopeTooLarge(usize),
}

/// Category and position of a JSON decoding failure, without the offending text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonFault {
    /// `io`, `syntax`, `data` (wrong type or value) or `eof`.
    pub kind: &'static str,
    pub line: usize,
    pub column: usize,
}

impl From<serde_json::Error> for JsonFault {
    fn from(e: serde_json::Error) -> Self {
        use serde_json::error::Category;
        let kind = match e.classify() {
            Category::Io => "io",
            Category::Syntax => "syntax",
            Category::Data => "data",
            Category::Eof => "eof",
        };
        JsonFault {
            kind,
            line: e.line(),
            column: e.column(),
        }
    }
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvelopeError::Json(fault) => write!(
                f,
                "invalid JSON ({}) at line {} column {}",
                fault.kind, fault.line, fault.column
            ),
            EnvelopeError::UnknownValue(fault) => write!(
                f,
                "unknown value ({}) at line {} column {}",
                fault.kind, fault.line, fault.column
            ),
            EnvelopeError::NotAnObject => write!(f, "message is not a JSON object"),
            EnvelopeError::MissingField(field) => write!(f, "missing field `{field}`"),
            EnvelopeError::InvalidField(field) => write!(f, "invalid field `{field}`"),
        }
    }
}

impl std::error::Error for EnvelopeError {}

impl EnvelopeError {
    /// Whether the failure is an unknown value (label `unknown`) or a malformed message.
    pub fn class(&self) -> FailureClass {
        match self {
            EnvelopeError::UnknownValue(_) => FailureClass::Unknown,
            _ => FailureClass::Malformed,
        }
    }
}

impl From<serde_json::Error> for EnvelopeError {
    fn from(e: serde_json::Error) -> Self {
        // serde's derived enums report an unrecognised variant with this prefix. The
        // message is inspected here only; it is never stored or displayed.
        let unknown = e.classify() == serde_json::error::Category::Data
            && e.to_string().starts_with("unknown variant");
        if unknown {
            EnvelopeError::UnknownValue(JsonFault::from(e))
        } else {
            EnvelopeError::Json(JsonFault::from(e))
        }
    }
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::Serialize => write!(f, "envelope could not be serialized"),
            EncodeError::EnvelopeTooLarge(len) => write!(
                f,
                "envelope is {len} bytes, over the {MAX_ENVELOPE_BYTES}-byte limit"
            ),
        }
    }
}

impl std::error::Error for EncodeError {}

impl UsageEnvelope {
    /// Serialize to the wire format, with `"type":"usage"`. Fails if the result is longer
    /// than [`MAX_ENVELOPE_BYTES`].
    pub fn try_to_json(&self) -> Result<String, EncodeError> {
        #[derive(Serialize)]
        struct Wire<'a> {
            #[serde(rename = "type")]
            ty: &'static str,
            v: u16,
            producer: Producer,
            emitted_at: &'a DateTime<Utc>,
            usage: &'a InternalUsageReport,
        }
        let json = serde_json::to_string(&Wire {
            ty: USAGE_TYPE,
            v: self.v,
            producer: self.producer,
            emitted_at: &self.emitted_at,
            usage: &self.usage,
        })
        .map_err(|_| EncodeError::Serialize)?;
        if json.len() > MAX_ENVELOPE_BYTES {
            return Err(EncodeError::EnvelopeTooLarge(json.len()));
        }
        Ok(json)
    }
}

/// Read one message. Parses once to a `Value`, then reads each envelope field. Unknown
/// types and versions are reported, not errors. Unknown fields are ignored.
pub fn parse_message(bytes: &[u8]) -> Result<ParsedMessage, EnvelopeError> {
    let value: Value = serde_json::from_slice(bytes)?;
    let obj = value.as_object().ok_or(EnvelopeError::NotAnObject)?;

    let ty = obj
        .get("type")
        .ok_or(EnvelopeError::MissingField("type"))?
        .as_str()
        .ok_or(EnvelopeError::InvalidField("type"))?;
    if ty != USAGE_TYPE {
        return Ok(ParsedMessage::UnknownType(ty.to_owned()));
    }

    let raw = obj.get("v").ok_or(EnvelopeError::MissingField("v"))?;
    let v = match (raw.as_u64(), raw.as_i64()) {
        (Some(u), _) => i128::from(u),
        (None, Some(i)) => i128::from(i),
        (None, None) => return Err(EnvelopeError::InvalidField("v")),
    };
    // The one place that decides which versions are read (spec C3/C4). Only the current
    // version has a reader; a previous version gets its own arm when it is added.
    if v == i128::from(CURRENT_USAGE_VERSION) {
        read_current(obj, CURRENT_USAGE_VERSION)
    } else {
        Ok(ParsedMessage::UnknownVersion { v })
    }
}

/// Reader for [`CURRENT_USAGE_VERSION`]. Each field is read on its own so that a missing
/// field is reported by name, and a wrong-typed one without its value.
fn read_current(obj: &Map<String, Value>, v: u16) -> Result<ParsedMessage, EnvelopeError> {
    let field = |name: &'static str| obj.get(name).ok_or(EnvelopeError::MissingField(name));
    let producer = Producer::deserialize(field("producer")?)
        .map_err(|_| EnvelopeError::InvalidField("producer"))?;
    let emitted_at = DateTime::<Utc>::deserialize(field("emitted_at")?)
        .map_err(|_| EnvelopeError::InvalidField("emitted_at"))?;
    let usage = InternalUsageReport::deserialize(field("usage")?)?;
    Ok(ParsedMessage::Usage(UsageEnvelope {
        v,
        producer,
        emitted_at,
        usage,
    }))
}
