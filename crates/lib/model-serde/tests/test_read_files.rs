use chrono::{DateTime, Utc};
use model_serde::serde::deserializer::ModelDeserializer;
use model_serde::serde::errors::DeserializerError;

const PRETEND_ENCRYPTION_SECRET : &'static str = "FAKE_SECRET";

#[test]
fn test_read_empty_file() {
    let deserializer = ModelDeserializer::new(PRETEND_ENCRYPTION_SECRET);
    let result = deserializer.read_fast_model_file_metadata("tests/files/empty_file.txt");
    match result {
        Err(DeserializerError::IOError(_)) => {}, // This error is what we expect
        _ => panic!("expected IO error"),
    }
}

#[test]
fn test_read_invalid_magic_bytes() {
    let deserializer = ModelDeserializer::new(PRETEND_ENCRYPTION_SECRET);
    let result = deserializer.read_fast_model_file_metadata("tests/files/small_file.txt");

    match result {
        Err(DeserializerError::InvalidMagicBytes) => {}, // This error is what we expect
        _ => panic!("expected invalid magic bytes error"),
    }
}

#[test]
fn test_read_fast_model_metadata() {
    let deserializer = ModelDeserializer::new(PRETEND_ENCRYPTION_SECRET);
    let result = deserializer.read_fast_model_file_metadata("tests/files/bill_nye.fakeyou");
    let hydrated  = result.unwrap();

    assert_eq!(hydrated.file_format_version, "1.0.0".to_string());
    assert_eq!(hydrated.model_author_username, "Vegito".to_string());
    assert_eq!(hydrated.model_token, "vcm_ABCDEF012345".to_string());
    assert_eq!(hydrated.model_name, "Bill Nye (sad)".to_string());
}

#[test]
fn test_read_model_encrypted_metadata() {
    let deserializer = ModelDeserializer::new(PRETEND_ENCRYPTION_SECRET);
    let result = deserializer.read_encrypted_model_file_metadata("tests/files/bill_nye.fakeyou");
    let hydrated  = result.unwrap();

    assert_eq!(hydrated.file_format_version, "1.0.0".to_string());
    assert_eq!(hydrated.model_author_username, "Vegito".to_string());
    assert_eq!(hydrated.model_token, "vcm_ABCDEF012345".to_string());
    assert_eq!(hydrated.model_name, "Bill Nye (sad)".to_string());

    assert_eq!(hydrated.model_created_at, Some(date("2022-10-19T04:18:14Z")));
    assert_eq!(hydrated.model_updated_at, Some(date("2022-10-19T04:18:14Z")));
    assert_eq!(hydrated.maybe_model_expires_at, Some(date("2022-11-18T04:18:14Z")));
    assert_eq!(hydrated.maybe_model_invalid_before, None);
    assert_eq!(hydrated.paid_plan_can_bypass_expiry, Some(true));
}

#[test]
fn test_read_full_model_file() {
    let deserializer = ModelDeserializer::new(PRETEND_ENCRYPTION_SECRET);
    let result = deserializer.read_full_model_file("tests/files/bill_nye.fakeyou");
    let hydrated  = result.unwrap();

    assert_eq!(hydrated.file_format_version, "1.0.0".to_string());
    assert_eq!(hydrated.model_author_username, "Vegito".to_string());
    assert_eq!(hydrated.model_token, "vcm_ABCDEF012345".to_string());
    assert_eq!(hydrated.model_name, "Bill Nye (sad)".to_string());

    assert_eq!(hydrated.model_created_at, Some(date("2022-10-19T04:18:14Z")));
    assert_eq!(hydrated.model_updated_at, Some(date("2022-10-19T04:18:14Z")));
    assert_eq!(hydrated.maybe_model_expires_at, Some(date("2022-11-18T04:18:14Z")));
    assert_eq!(hydrated.maybe_model_invalid_before, None);
    assert_eq!(hydrated.paid_plan_can_bypass_expiry, Some(true));

    assert_eq!(hydrated.model_pipeline_version, 1);
    assert!(hydrated.acoustic_model_bytes.starts_with("AAAA".as_bytes()));
    assert!(hydrated.vocoder_model_bytes.starts_with("VVVV".as_bytes()));
}


fn date(timestamp: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(timestamp)
        .unwrap()
        .with_timezone(&Utc)
}
