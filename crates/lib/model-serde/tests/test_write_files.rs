use chrono::{DateTime, Utc};
use model_serde::model::Model;
use model_serde::serde::deserializer::ModelDeserializer;
use model_serde::serde::serializer::ModelSerializer;
use std::fs::File;
use std::io::{BufReader, Read};
use tempfile::NamedTempFile;

const PRETEND_ENCRYPTION_SECRET : &'static str = "TESTS_ARE_GREAT";

#[test]
fn test_write_file_magic_bytes() {
    let file = NamedTempFile::new().unwrap();
    let filename = file.path();

    let serializer = ModelSerializer::new(PRETEND_ENCRYPTION_SECRET);

    let model = Model::default();
    serializer.write_model_to_file(model, true, filename).unwrap();

    let file = File::open(filename).unwrap();
    let mut reader = BufReader::new(file);

    let mut buffer = vec![0; 8];
    reader.read_exact(&mut buffer).unwrap();

    assert_eq!(buffer, "FAKEYOU\0".as_bytes());
}

#[test]
fn test_write_proto_round_trip() {
    let file = NamedTempFile::new().unwrap();
    let filename = file.path();

    let serializer = ModelSerializer::new(PRETEND_ENCRYPTION_SECRET);
    let deserializer = ModelDeserializer::new(PRETEND_ENCRYPTION_SECRET);

    // Write file
    {
        let model = Model {
            file_format_version: "1.0.0".to_string(),
            model_token: "vcm_WRITTEN_MODEL".to_string(),
            model_name: "Bob Ross (angry)".to_string(),
            model_author_username: "Vegito".to_string(),

            model_version: Some("1.0".to_string()),
            model_created_at: Some(date("2022-09-01T01:02:03Z")),
            model_updated_at: Some(date("2022-10-17T04:04:00Z")),

            maybe_model_invalid_before: Some(date("2000-01-01T10:00:00Z")),
            maybe_model_expires_at: Some(date("2024-12-30T12:00:00Z")),

            paid_plan_can_bypass_expiry: Some(true),
            maybe_model_password: Some("secret_model_password".to_string()),

            model_pipeline_version: 1,
            acoustic_model_bytes: "AAAA".repeat(1000).as_bytes().to_vec(),
            vocoder_model_bytes: "VVVV".repeat(1000).as_bytes().to_vec(),
        };

        serializer.write_model_to_file(model, true, filename).unwrap();
    }

    // Verify written contents
    let model = deserializer.read_full_model_file(filename).unwrap();

    assert_eq!(model.file_format_version, "1.0.0".to_string());
    assert_eq!(model.model_token, "vcm_WRITTEN_MODEL".to_string());
    assert_eq!(model.model_name, "Bob Ross (angry)".to_string());
    assert_eq!(model.model_author_username, "Vegito".to_string());

    assert_eq!(model.model_version, Some("1.0".to_string()));
    assert_eq!(model.model_created_at, Some(date("2022-09-01T01:02:03Z")));
    assert_eq!(model.model_updated_at, Some(date("2022-10-17T04:04:00Z")));

    assert_eq!(model.maybe_model_invalid_before, Some(date("2000-01-01T10:00:00Z")));
    assert_eq!(model.maybe_model_expires_at, Some(date("2024-12-30T12:00:00Z")));

    assert_eq!(model.paid_plan_can_bypass_expiry, Some(true));
    assert_eq!(model.maybe_model_password, Some("secret_model_password".to_string()));

    assert_eq!(model.model_pipeline_version, 1);
    assert!(model.acoustic_model_bytes.starts_with("AAAA".as_bytes()));
    assert!(model.vocoder_model_bytes.starts_with("VVVV".as_bytes()));
}

fn date(timestamp: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(timestamp)
        .unwrap()
        .with_timezone(&Utc)
}
