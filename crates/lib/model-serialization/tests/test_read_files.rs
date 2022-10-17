use model_serialization::model_serializer::ModelSerializer;

const PRETEND_ENCRYPTION_SECRET : &'static str = "FAKE_SECRET";

#[test]
fn test_read_empty_file() {
    let serializer = ModelSerializer::new(PRETEND_ENCRYPTION_SECRET);
    let result = serializer.read_fast_model_file_metadata("tests/files/empty_file.txt");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "failed to fill whole buffer".to_string());
}

#[test]
fn test_read_invalid_magic_bytes() {
    let serializer = ModelSerializer::new(PRETEND_ENCRYPTION_SECRET);
    let result = serializer.read_fast_model_file_metadata("tests/files/small_file.txt");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "magic bytes do not match".to_string());
}

#[test]
fn test_read_fast_model_metadata() {
    let serializer = ModelSerializer::new(PRETEND_ENCRYPTION_SECRET);
    let result = serializer.read_fast_model_file_metadata("tests/files/bill_nye.fakeyou");
    let hydrated  = result.unwrap();

    assert_eq!(hydrated.file_format_version, "1.0.0".to_string());
    assert_eq!(hydrated.model_author_username, "Vegito".to_string());
    assert_eq!(hydrated.model_token, "vcm_ABCDEF012345".to_string());
    assert_eq!(hydrated.model_name, "Bill Nye (sad)".to_string());
}

#[test]
fn test_read_model_encrypted_metadata() {
    let serializer = ModelSerializer::new(PRETEND_ENCRYPTION_SECRET);
    let result = serializer.read_encrypted_model_file_metadata("tests/files/bill_nye.fakeyou");
    let hydrated  = result.unwrap();

    assert_eq!(hydrated.file_format_version, "1.0.0".to_string());
    assert_eq!(hydrated.model_author_username, "Vegito".to_string());
    assert_eq!(hydrated.model_token, "vcm_ABCDEF012345".to_string());
    assert_eq!(hydrated.model_name, "Bill Nye (sad)".to_string());

    assert_eq!(hydrated.model_created_at, Some(1665976472411));
    assert_eq!(hydrated.model_updated_at, Some(1665976472411));
    assert_eq!(hydrated.expiry_timestamp_millis, Some(1668568472411));
}

#[test]
fn test_read_full_model_file() {
    let serializer = ModelSerializer::new(PRETEND_ENCRYPTION_SECRET);
    let result = serializer.read_full_model_file("tests/files/bill_nye.fakeyou");
    let hydrated  = result.unwrap();

    assert_eq!(hydrated.file_format_version, "1.0.0".to_string());
    assert_eq!(hydrated.model_author_username, "Vegito".to_string());
    assert_eq!(hydrated.model_token, "vcm_ABCDEF012345".to_string());
    assert_eq!(hydrated.model_name, "Bill Nye (sad)".to_string());

    assert_eq!(hydrated.model_created_at, Some(1665976472411));
    assert_eq!(hydrated.model_updated_at, Some(1665976472411));
    assert_eq!(hydrated.expiry_timestamp_millis, Some(1668568472411));

    assert_eq!(hydrated.model_pipeline_version, 1);
    assert!(hydrated.acoustic_model_bytes.starts_with("AAAA".as_bytes()));
    assert!(hydrated.vocoder_model_bytes.starts_with("VVVV".as_bytes()));
}
