// ============================================================================
// integration_tests.rs — Integration test suite for ffmpeg-skill-rs
// ============================================================================

use ffmpeg_skill::contract::generate_contract_json;
use ffmpeg_skill::cut::parse_time_to_seconds;
use ffmpeg_skill::doctor::inspect_capabilities;
use ffmpeg_skill::probe::FieldSelector;
use ffmpeg_skill::speedramp::build_atempo_chain;
use serde_json::json;

#[test]
fn test_contract_schema() {
    let contract = generate_contract_json();
    assert_eq!(contract["contract_version"], "1.0");
    assert_eq!(contract["skill_name"], "ffmpeg-skill-rs");
    assert_eq!(contract["engine"], "native-rust");

    let tools = contract["tools"].as_array().expect("tools array");
    assert!(tools.len() >= 15);

    let tool_names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();

    assert!(tool_names.contains(&"doctor"));
    assert!(tool_names.contains(&"probe"));
    assert!(tool_names.contains(&"cut"));
    assert!(tool_names.contains(&"fit"));
    assert!(tool_names.contains(&"crop"));
    assert!(tool_names.contains(&"cropdetect"));
    assert!(tool_names.contains(&"export"));
    assert!(tool_names.contains(&"join"));
    assert!(tool_names.contains(&"loudness"));
    assert!(tool_names.contains(&"silence"));
    assert!(tool_names.contains(&"speedramp"));
    assert!(tool_names.contains(&"reverse"));
    assert!(tool_names.contains(&"waveform"));
    assert!(tool_names.contains(&"scenes"));
    assert!(tool_names.contains(&"check"));
}

#[test]
fn test_time_parsing() {
    assert_eq!(parse_time_to_seconds("15.5"), Some(15.5));
    assert_eq!(parse_time_to_seconds("01:30"), Some(90.0));
    assert_eq!(parse_time_to_seconds("00:01:30.500"), Some(90.5));
    assert_eq!(parse_time_to_seconds("02:00:00"), Some(7200.0));
    assert_eq!(parse_time_to_seconds("invalid"), None);
}

#[test]
fn test_atempo_chain_builder() {
    assert_eq!(build_atempo_chain(1.0), "atempo=1.0000");
    assert_eq!(build_atempo_chain(1.5), "atempo=1.5000");
    assert_eq!(build_atempo_chain(2.0), "atempo=2.0000");
    assert_eq!(build_atempo_chain(4.0), "atempo=2.0,atempo=2.0000");
    assert_eq!(build_atempo_chain(0.5), "atempo=0.5000");
    assert_eq!(build_atempo_chain(0.25), "atempo=0.5,atempo=0.5000");
}

#[test]
fn test_field_selector() {
    let doc = json!({
        "format": "mp4",
        "duration": 42.5,
        "video": {
            "codec": "h264",
            "resolution": [1920, 1080]
        }
    });

    let sel_dur = FieldSelector::new("duration");
    assert_eq!(sel_dur.extract(&doc), json!(42.5));

    let sel_codec = FieldSelector::new("video.codec");
    assert_eq!(sel_codec.extract(&doc), json!("h264"));

    let sel_none = FieldSelector::new("nonexistent.field");
    assert_eq!(sel_none.extract(&doc), json!(null));
}

#[tokio::test]
async fn test_doctor_inspection() {
    let report = inspect_capabilities().await.expect("doctor report");
    assert!(!report.version.is_empty());
    // Since ffmpeg is installed on this dev system:
    assert!(report.ffmpeg.is_some());
    assert!(report.ffprobe.is_some());
}
