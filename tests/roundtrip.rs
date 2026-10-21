use cloud_platform_events::{
    parse_message, EncodeError, EnvelopeError, FailureClass, InternalUsageReport, ParsedMessage,
    Producer, UsageEnvelope, CURRENT_USAGE_VERSION, MAX_ENVELOPE_BYTES,
};
use serde_json::{json, Value};

const CHAT: &str = include_str!("fixtures/usage_v1_chat_completion.json");
const IMAGE: &str = include_str!("fixtures/usage_v1_image_generation.json");
const EMBEDDING: &str = include_str!("fixtures/usage_v1_embedding.json");
const RERANK: &str = include_str!("fixtures/usage_v1_rerank.json");
const SCORE: &str = include_str!("fixtures/usage_v1_score.json");
const PRIVACY_CLASSIFY: &str = include_str!("fixtures/usage_v1_privacy_classify.json");

const EMITTED_AT: &str = "2026-10-20T12:00:00Z";

fn report(body: &str) -> InternalUsageReport {
    serde_json::from_str(body).expect("fixture deserializes into InternalUsageReport")
}

fn envelope(usage: InternalUsageReport) -> UsageEnvelope {
    UsageEnvelope {
        v: CURRENT_USAGE_VERSION,
        producer: Producer::OpenrouterGateway,
        emitted_at: EMITTED_AT.parse().unwrap(),
        usage,
    }
}

fn expect_usage(bytes: &[u8]) -> UsageEnvelope {
    match parse_message(bytes).expect("message parses") {
        ParsedMessage::Usage(env) => env,
        other => panic!("expected Usage, got {other:?}"),
    }
}

/// Serialize an envelope, parse it back, and require the same value.
fn assert_roundtrip(body: &str) {
    let original = envelope(report(body));
    let parsed = expect_usage(original.try_to_json().expect("envelope encodes").as_bytes());
    assert_eq!(parsed, original);
    assert_eq!(parsed.usage, report(body));
}

#[test]
fn roundtrip_chat_completion() {
    assert_roundtrip(CHAT);
}

#[test]
fn roundtrip_image_generation() {
    assert_roundtrip(IMAGE);
}

#[test]
fn roundtrip_embedding() {
    assert_roundtrip(EMBEDDING);
}

#[test]
fn roundtrip_rerank() {
    assert_roundtrip(RERANK);
}

#[test]
fn roundtrip_score() {
    assert_roundtrip(SCORE);
}

#[test]
fn roundtrip_privacy_classify() {
    assert_roundtrip(PRIVACY_CLASSIFY);
}

#[test]
fn roundtrip_provider_attribution() {
    let body = r#"{
        "organization_id": "7d1c3b8e-2f4a-4e5b-9a61-0c2d8e4f1a01",
        "workspace_id": "3a9e6f10-5b7c-4d2e-8f13-9b0a1c2d3e02",
        "api_key_id": "c4b2a0f9-8e7d-4c6b-a5f4-1e2d3c4b5a03",
        "discount_to_user": 0.2,
        "type": "chat_completion",
        "model": "Qwen/Qwen3-30B-A3B-Instruct-2507",
        "input_tokens": 100,
        "output_tokens": 50,
        "id": "chatcmpl-attr-001",
        "served_provider_tier": "attested_3p",
        "served_provider_type": "tinfoil",
        "served_via_fallback": true
    }"#;
    assert_roundtrip(body);
}

#[test]
fn unknown_fields_ignored() {
    let clean = json!({
        "type": "usage", "v": 1, "producer": "openrouter-gateway",
        "emitted_at": EMITTED_AT,
        "usage": serde_json::from_str::<Value>(CHAT).unwrap(),
    });
    let mut noisy = clean.clone();
    noisy["future_top_level"] = json!({"nested": [1, 2, 3]});
    noisy["usage"]["future_field"] = json!("ignored");
    noisy["usage"]["input_tokens_breakdown"] = json!({"text": 100});

    let a = expect_usage(clean.to_string().as_bytes());
    let b = expect_usage(noisy.to_string().as_bytes());
    assert_eq!(a, b);
}

#[test]
fn missing_optional_discount_ok() {
    let mut value: Value = serde_json::from_str(CHAT).unwrap();
    assert!(value.get("discount_to_user").is_none());
    value["discount_to_user"] = json!(0.2);
    let with = expect_usage(
        json!({"type": "usage", "v": 1, "producer": "openrouter-gateway",
               "emitted_at": EMITTED_AT, "usage": value})
        .to_string()
        .as_bytes(),
    );
    assert_eq!(with.usage.discount_to_user, Some(0.2));

    let without = expect_usage(
        envelope(report(CHAT))
            .try_to_json()
            .expect("envelope encodes")
            .as_bytes(),
    );
    assert_eq!(without.usage.discount_to_user, None);
}

#[test]
fn unknown_type_reported() {
    let msg = json!({"type": "other", "v": 1, "producer": "openrouter-gateway",
                     "emitted_at": EMITTED_AT, "usage": {}});
    match parse_message(msg.to_string().as_bytes()).unwrap() {
        ParsedMessage::UnknownType(t) => assert_eq!(t, "other"),
        other => panic!("expected UnknownType, got {other:?}"),
    }
}

#[test]
fn unknown_version_reported() {
    let msg = json!({"type": "usage", "v": 99, "producer": "openrouter-gateway",
                     "emitted_at": EMITTED_AT, "usage": {}});
    match parse_message(msg.to_string().as_bytes()).unwrap() {
        ParsedMessage::UnknownVersion { v } => {
            assert_eq!(v, 99);
        }
        other => panic!("expected UnknownVersion, got {other:?}"),
    }
}

#[test]
fn any_other_integer_version_is_unknown_version() {
    for v in [json!(0), json!(2), json!(70000), json!(u64::MAX), json!(-1)] {
        let msg = json!({"type": "usage", "v": v, "producer": "openrouter-gateway",
                         "emitted_at": EMITTED_AT, "usage": {}});
        match parse_message(msg.to_string().as_bytes()) {
            Ok(ParsedMessage::UnknownVersion { .. }) => {}
            other => panic!("v={v}: expected UnknownVersion, got {other:?}"),
        }
    }
}

#[test]
fn non_integer_version_is_malformed() {
    for v in [json!("1"), json!(1.0), json!(true), json!(null)] {
        let msg = json!({"type": "usage", "v": v, "producer": "openrouter-gateway",
                         "emitted_at": EMITTED_AT, "usage": {}});
        let err = parse_message(msg.to_string().as_bytes()).unwrap_err();
        assert!(
            matches!(err, EnvelopeError::InvalidField("v")),
            "v={v}: {err:?}"
        );
        assert_eq!(err.class(), FailureClass::Malformed);
    }
}

#[test]
fn unknown_producer_is_other() {
    let msg = json!({"type": "usage", "v": 1, "producer": "some-new-producer",
                     "emitted_at": EMITTED_AT, "usage": serde_json::from_str::<Value>(CHAT).unwrap()});
    let env = expect_usage(msg.to_string().as_bytes());
    assert_eq!(env.producer, Producer::Other);
}

#[test]
fn fixtures_producer_is_known() {
    let msg = json!({"type": "usage", "v": 1, "producer": "fixtures-staging",
                     "emitted_at": EMITTED_AT, "usage": serde_json::from_str::<Value>(CHAT).unwrap()});
    let env = expect_usage(msg.to_string().as_bytes());
    assert_eq!(env.producer, Producer::FixturesStaging);
}

#[test]
fn missing_required_field_rejected() {
    let msg = json!({"type": "usage", "v": 1, "emitted_at": EMITTED_AT,
                     "usage": serde_json::from_str::<Value>(CHAT).unwrap()});
    assert!(parse_message(msg.to_string().as_bytes()).is_err());
}

#[test]
fn not_json_rejected() {
    assert!(matches!(
        parse_message(b"not json"),
        Err(EnvelopeError::Json(_))
    ));
}

/// The fixture is a body cloud-api's `/v1/internal/usage` accepts. Re-serializing it
/// must yield exactly the same keys and values: no optional or default-valued extras.
#[test]
fn body_matches_cloud_api_shape() {
    for body in [CHAT, IMAGE, EMBEDDING] {
        let fixture: Value = serde_json::from_str(body).unwrap();
        let out = serde_json::to_value(report(body)).unwrap();
        assert_eq!(out, fixture);
    }
}

#[test]
fn identity_fields_are_uuids() {
    for body in [CHAT, IMAGE, EMBEDDING] {
        let v: Value = serde_json::from_str(body).unwrap();
        for key in ["organization_id", "workspace_id", "api_key_id"] {
            let s = v[key].as_str().unwrap();
            uuid::Uuid::parse_str(s).unwrap_or_else(|e| panic!("{key}={s}: {e}"));
        }
    }
}

#[test]
fn to_json_carries_type_usage_and_version() {
    let v: Value = serde_json::from_str(
        &envelope(report(IMAGE))
            .try_to_json()
            .expect("envelope encodes"),
    )
    .unwrap();
    assert_eq!(v["type"], "usage");
    assert_eq!(v["v"], 1);
    assert_eq!(v["producer"], "openrouter-gateway");
    assert_eq!(v["emitted_at"], EMITTED_AT);
    assert_eq!(v["usage"]["type"], "image_generation");
}

/// The body inference-proxy sends: `build_usage_body` (proxy.rs:587-680) plus
/// `complete_usage_body` (proxy.rs:817-846) with the identity fields injected.
/// No discount is set, so `discount_to_user` is absent.
#[test]
fn serialized_body_equals_proxy_body() {
    let proxy_chat = json!({
        "organization_id": "7d1c3b8e-2f4a-4e5b-9a61-0c2d8e4f1a01",
        "workspace_id": "3a9e6f10-5b7c-4d2e-8f13-9b0a1c2d3e02",
        "api_key_id": "c4b2a0f9-8e7d-4c6b-a5f4-1e2d3c4b5a03",
        "type": "chat_completion",
        "model": "Qwen/Qwen3-30B-A3B-Instruct-2507",
        "input_tokens": 100,
        "output_tokens": 50,
        "cache_read_tokens": 0,
        "id": "chatcmpl-test-001"
    });
    let proxy_image = json!({
        "organization_id": "7d1c3b8e-2f4a-4e5b-9a61-0c2d8e4f1a01",
        "workspace_id": "3a9e6f10-5b7c-4d2e-8f13-9b0a1c2d3e02",
        "api_key_id": "c4b2a0f9-8e7d-4c6b-a5f4-1e2d3c4b5a03",
        "type": "image_generation",
        "model": "Qwen/Qwen-Image-2512",
        "image_count": 3,
        "id": "img-test-001"
    });
    let proxy_embedding = json!({
        "organization_id": "7d1c3b8e-2f4a-4e5b-9a61-0c2d8e4f1a01",
        "workspace_id": "3a9e6f10-5b7c-4d2e-8f13-9b0a1c2d3e02",
        "api_key_id": "c4b2a0f9-8e7d-4c6b-a5f4-1e2d3c4b5a03",
        "type": "embedding",
        "model": "Qwen/Qwen3-Embedding-0.6B",
        "input_tokens": 42,
        "id": "emb-test-001"
    });
    for (body, proxy) in [
        (CHAT, proxy_chat),
        (IMAGE, proxy_image),
        (EMBEDDING, proxy_embedding),
    ] {
        let env = envelope(report(body));
        let wire: Value =
            serde_json::from_str(&env.try_to_json().expect("envelope encodes")).unwrap();
        assert_eq!(wire["usage"], proxy);
    }
}

/// Bodies copied verbatim from cloud-api's e2e tests (crates/api/tests/e2e_all/usage_recording.rs,
/// at a77d2e2a). Identity fields are the placeholders the test helper would inject.
const CLOUD_API_BODIES: &[(&str, &str)] = &[
    (
        "chat_completion",
        include_str!("fixtures/cloud_api_chat_completion.json"),
    ),
    (
        "chat_completion_cache",
        include_str!("fixtures/cloud_api_chat_completion_cache.json"),
    ),
    (
        "chat_completion_discount",
        include_str!("fixtures/cloud_api_chat_completion_discount.json"),
    ),
    (
        "chat_completion_discount_zero",
        include_str!("fixtures/cloud_api_chat_completion_discount_zero.json"),
    ),
    (
        "image_generation",
        include_str!("fixtures/cloud_api_image_generation.json"),
    ),
    (
        "embedding_zero",
        include_str!("fixtures/cloud_api_embedding_zero.json"),
    ),
];

/// Compares exactly: `0` and `0.0` differ, and `null` differs from an absent key.
#[test]
fn cloud_api_bodies_deserialize_and_roundtrip() {
    for (name, body) in CLOUD_API_BODIES {
        let mut fixture: Value = serde_json::from_str(body).unwrap();
        let report: InternalUsageReport = serde_json::from_str(body)
            .unwrap_or_else(|e| panic!("{name} does not deserialize: {e}"));
        let mut out = serde_json::to_value(&report).unwrap();
        if *name == "chat_completion_discount_zero" {
            // Known mismatch, pinned by `discount_zero_serializes_as_float_not_integer`:
            // cloud-api's body has `0`, this crate writes `0.0`. Both read back the same.
            // Changing it changes the wire bytes, so it is held out of the exact check here.
            assert_eq!(fixture["discount_to_user"], json!(0));
            assert_eq!(out["discount_to_user"], json!(0.0));
            fixture.as_object_mut().unwrap().remove("discount_to_user");
            out.as_object_mut().unwrap().remove("discount_to_user");
        }
        assert_eq!(out, fixture, "{name} round trip differs");
    }
}

#[test]
fn discount_zero_serializes_as_float_not_integer() {
    let body = json!({
        "organization_id": "7d1c3b8e-2f4a-4e5b-9a61-0c2d8e4f1a01",
        "workspace_id": "3a9e6f10-5b7c-4d2e-8f13-9b0a1c2d3e02",
        "api_key_id": "c4b2a0f9-8e7d-4c6b-a5f4-1e2d3c4b5a03",
        "discount_to_user": 0,
        "type": "embedding",
        "model": "m",
        "input_tokens": 1,
        "id": "x"
    });
    let report: InternalUsageReport = serde_json::from_value(body).unwrap();
    assert_eq!(report.discount_to_user, Some(0.0));
    let out = serde_json::to_string(&report).unwrap();
    assert!(out.contains(r#""discount_to_user":0.0"#), "{out}");
}

#[test]
fn none_token_fields_are_omitted() {
    let body = json!({
        "organization_id": "7d1c3b8e-2f4a-4e5b-9a61-0c2d8e4f1a01",
        "workspace_id": "3a9e6f10-5b7c-4d2e-8f13-9b0a1c2d3e02",
        "api_key_id": "c4b2a0f9-8e7d-4c6b-a5f4-1e2d3c4b5a03",
        "type": "chat_completion",
        "model": "Qwen/Qwen3-30B-A3B-Instruct-2507",
        "id": "chatcmpl-no-tokens"
    });
    let report: InternalUsageReport = serde_json::from_value(body.clone()).unwrap();
    let out = serde_json::to_value(&report).unwrap();
    assert_eq!(out, body);
    let wire: Value =
        serde_json::from_str(&envelope(report).try_to_json().expect("envelope encodes")).unwrap();
    for key in ["input_tokens", "output_tokens", "cache_read_tokens"] {
        assert!(wire["usage"].get(key).is_none(), "{key} must be omitted");
    }
}

#[test]
fn error_messages_do_not_echo_payload_values() {
    const SENTINEL: &str = "SENTINEL-PAYLOAD-VALUE-7f3a";
    // A wrong-typed field: serde's own message would quote the value.
    let wrong_type = json!({
        "type": "usage", "v": 1, "producer": "openrouter-gateway",
        "emitted_at": EMITTED_AT,
        "usage": {
            "organization_id": "7d1c3b8e-2f4a-4e5b-9a61-0c2d8e4f1a01",
            "workspace_id": "3a9e6f10-5b7c-4d2e-8f13-9b0a1c2d3e02",
            "api_key_id": "c4b2a0f9-8e7d-4c6b-a5f4-1e2d3c4b5a03",
            "type": "chat_completion", "model": "m", "id": "x",
            "input_tokens": SENTINEL
        }
    });
    let err = parse_message(wrong_type.to_string().as_bytes()).unwrap_err();
    let shown = format!("{err} / {err:?}");
    assert!(!shown.contains(SENTINEL), "leaked payload: {shown}");

    // A syntax error next to the sentinel.
    let bad = format!("{{\"type\": \"usage\", \"x\": {SENTINEL}}}");
    let err = parse_message(bad.as_bytes()).unwrap_err();
    let shown = format!("{err} / {err:?}");
    assert!(!shown.contains(SENTINEL), "leaked payload: {shown}");
    assert!(
        err.to_string().contains("line 1"),
        "position reported: {err}"
    );
}

fn message_with(overrides: &[(&str, Value)], remove: &[&str]) -> Vec<u8> {
    let mut msg = json!({
        "type": "usage", "v": 1, "producer": "openrouter-gateway",
        "emitted_at": EMITTED_AT,
        "usage": serde_json::from_str::<Value>(CHAT).unwrap(),
    });
    for (key, value) in overrides {
        msg[*key] = value.clone();
    }
    for key in remove {
        msg.as_object_mut().unwrap().remove(*key);
    }
    msg.to_string().into_bytes()
}

#[test]
fn missing_envelope_fields_are_named() {
    for field in ["producer", "emitted_at", "usage"] {
        let bytes = message_with(&[], &[field]);
        match parse_message(&bytes) {
            Err(EnvelopeError::MissingField(name)) => assert_eq!(name, field),
            other => panic!("{field}: expected MissingField, got {other:?}"),
        }
        assert_eq!(
            parse_message(&bytes).unwrap_err().class(),
            FailureClass::Malformed
        );
    }
}

#[test]
fn wrong_typed_envelope_fields_are_invalid_not_json_errors() {
    let bytes = message_with(&[("emitted_at", json!(12345))], &[]);
    assert!(matches!(
        parse_message(&bytes),
        Err(EnvelopeError::InvalidField("emitted_at"))
    ));
    let bytes = message_with(&[("producer", json!(["x"]))], &[]);
    assert!(matches!(
        parse_message(&bytes),
        Err(EnvelopeError::InvalidField("producer"))
    ));
}

#[test]
fn unknown_enum_value_is_classified_unknown() {
    let mut usage: Value = serde_json::from_str(CHAT).unwrap();
    usage["served_provider_type"] = json!("some-future-provider");
    let bytes = message_with(&[("usage", usage)], &[]);
    let err = parse_message(&bytes).unwrap_err();
    assert!(matches!(err, EnvelopeError::UnknownValue(_)), "{err:?}");
    assert_eq!(err.class(), FailureClass::Unknown);
    // The value is not echoed.
    assert!(!format!("{err} / {err:?}").contains("some-future-provider"));
}

#[test]
fn served_provider_tier_is_still_strict() {
    let mut usage: Value = serde_json::from_str(CHAT).unwrap();
    usage["served_provider_tier"] = json!("unknown_tier");
    let bytes = message_with(&[("usage", usage)], &[]);
    assert!(
        parse_message(&bytes).is_err(),
        "unknown tier must be rejected"
    );
}

#[test]
fn unknown_usage_type_is_classified_unknown() {
    let mut usage: Value = serde_json::from_str(CHAT).unwrap();
    usage["type"] = json!("future_kind");
    let bytes = message_with(&[("usage", usage)], &[]);
    assert_eq!(
        parse_message(&bytes).unwrap_err().class(),
        FailureClass::Unknown
    );
}

#[test]
fn malformed_is_classified_malformed() {
    assert_eq!(
        parse_message(b"not json").unwrap_err().class(),
        FailureClass::Malformed
    );
    assert_eq!(
        parse_message(b"[1]").unwrap_err().class(),
        FailureClass::Malformed
    );
    let bytes = message_with(&[("usage", json!({"type": "chat_completion"}))], &[]);
    assert_eq!(
        parse_message(&bytes).unwrap_err().class(),
        FailureClass::Malformed
    );
}

#[test]
fn unknown_type_and_version_are_unknown_messages() {
    let typed = json!({"type": "other", "v": 1}).to_string();
    assert!(matches!(
        parse_message(typed.as_bytes()).unwrap(),
        ParsedMessage::UnknownType(_)
    ));
    let versioned = json!({"type": "usage", "v": 2}).to_string();
    assert!(matches!(
        parse_message(versioned.as_bytes()).unwrap(),
        ParsedMessage::UnknownVersion { .. }
    ));
    let usage = message_with(&[], &[]);
    assert!(matches!(
        parse_message(&usage).unwrap(),
        ParsedMessage::Usage(_)
    ));
}

#[test]
fn version_two_is_not_read_as_version_one() {
    let msg = json!({"type": "usage", "v": 2, "producer": "openrouter-gateway",
                     "emitted_at": EMITTED_AT, "usage": {}});
    assert!(matches!(
        parse_message(msg.to_string().as_bytes()).unwrap(),
        ParsedMessage::UnknownVersion { v: 2, .. }
    ));
}

#[test]
fn oversized_envelope_is_refused_with_length_only() {
    let mut report = report(CHAT);
    report.usage = serde_json::from_value(json!({
        "type": "chat_completion",
        "model": "m".repeat(MAX_ENVELOPE_BYTES),
        "id": "x"
    }))
    .unwrap();
    let env = envelope(report);
    // Key order does not change the length, so this is the length of the wire message.
    let expected_len = serde_json::to_string(&json!({
        "type": "usage", "v": 1, "producer": "openrouter-gateway",
        "emitted_at": EMITTED_AT,
        "usage": serde_json::to_value(&env.usage).unwrap(),
    }))
    .unwrap()
    .len();
    assert!(expected_len > MAX_ENVELOPE_BYTES);
    assert_eq!(
        env.try_to_json(),
        Err(EncodeError::EnvelopeTooLarge(expected_len))
    );
}
