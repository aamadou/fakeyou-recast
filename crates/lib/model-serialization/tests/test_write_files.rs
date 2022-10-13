use std::fs::File;
use magic_crypt::generic_array::typenum::U256;
use magic_crypt::new_magic_crypt;
use magic_crypt::{MagicCryptTrait, MagicCrypt256};
use model_serialization::hydrated_model::FullyHydratedModel;
use model_serialization::model_serializer::ModelSerializer;
use prost::Message;
use prost::bytes::BytesMut;
use std::io::{BufReader, Cursor, Read};
use std::path::PathBuf;
use tempfile::NamedTempFile;

const PRETEND_ENCRYPTION_SECRET : &'static str = "TESTS_ARE_GREAT";

#[test]
fn test_write_file_magic_bytes() -> anyhow::Result<()> {
    let mut file = NamedTempFile::new()?;
    let filename = file.path();

    let serializer = ModelSerializer::new(PRETEND_ENCRYPTION_SECRET);

    let model = FullyHydratedModel::default();
    serializer.write_model_to_file(model, true, filename).unwrap();

    let mut file = File::open(filename)?;
    let mut reader = BufReader::new(file);

    let mut buffer = vec![0; 8];
    reader.read_exact(&mut buffer)?;

    assert_eq!(buffer, "FAKEYOU\0".as_bytes());

    Ok(())
}

#[test]
fn test_write_proto_round_trip() -> anyhow::Result<()> {
    let mut file = NamedTempFile::new()?;
    let filename = file.path();

    let serializer = ModelSerializer::new(PRETEND_ENCRYPTION_SECRET);

    // Write file
    {
        let model = FullyHydratedModel {
            file_format_version: "1.0.0".to_string(),
            model_token: "vcm_WRITTEN_MODEL".to_string(),
            model_name: "Bob Ross (angry)".to_string(),
            model_author_username: "Vegito".to_string(),

            model_version: Some("1.0".to_string()),
            model_created_at: Some(100),
            model_updated_at: Some(200),

            expiry_timestamp_millis: Some(1000),

            model_pipeline_version: 1,
            acoustic_model_bytes: "AAAA".repeat(1000).as_bytes().to_vec(),
            vocoder_model_bytes: "VVVV".repeat(1000).as_bytes().to_vec(),
        };

        serializer.write_model_to_file(model, true, filename).unwrap();
    }

    // Verify written contents
    let model = serializer.read_full_model_file(filename).unwrap();

    assert_eq!(model.file_format_version, "1.0.0".to_string());
    assert_eq!(model.model_token, "vcm_WRITTEN_MODEL".to_string());
    assert_eq!(model.model_name, "Bob Ross (angry)".to_string());
    assert_eq!(model.model_author_username, "Vegito".to_string());

    assert_eq!(model.model_version, Some("1.0".to_string()));
    assert_eq!(model.model_created_at, Some(100));
    assert_eq!(model.model_updated_at, Some(200));

    assert_eq!(model.expiry_timestamp_millis, Some(1000));
    assert_eq!(model.model_pipeline_version, 1);
    assert!(model.acoustic_model_bytes.starts_with("AAAA".as_bytes()));
    assert!(model.vocoder_model_bytes.starts_with("VVVV".as_bytes()));

    Ok(())
}
