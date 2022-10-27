use crate::file_format::crypto::Crypto;
use crate::file_format::model_file_layout::{BINCODE_CONFIG, EncryptedLightweightProtoPayloadBytes, FILE_HEADER_MAGIC_BYTES, ModelFileIntermediateReadPayload, ModelFilePayload, ModelFileQuickReadPayload, UnencryptedProtoPayloadBytes};
use crate::file_format::protos::decode_proto_from_bytes;
use crate::model::{Model, ModelFullMetadata, ModelLightMetadata};
use crate::protos::EncryptedHeavyPayload;
use crate::protos::EncryptedLightweightDetails;
use crate::protos::UnencryptedLightweightDetails;
use crate::serde::errors::DeserializerError;
use crate::serde::time::maybe_timestamp_to_utc_datetime;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub struct ModelDeserializer {
    crypto: Crypto,
}

impl ModelDeserializer {

    pub fn new(crypto_secret: &str) -> Self {
        Self {
            crypto: Crypto::new(crypto_secret),
        }
    }

    pub fn read_fast_model_file_metadata<P>(&self, filename: P) -> Result<ModelLightMetadata, DeserializerError>
        where P: AsRef<Path>
    {
        let mut reader = Self::open_model_file_reader_checking_magic_bytes(filename)?;

        // NB: The important data is in the first 500 bytes.
        let mut buf = vec![0u8; 500];
        reader.read_exact(&mut buf)?;

        let (model_file_payload , _len) : (ModelFileQuickReadPayload, usize) =
            bincode::decode_from_slice(&buf, BINCODE_CONFIG)?;

        Ok(ModelLightMetadata {
            file_format_version: model_file_payload.fast_metadata.file_format_version,
            model_token: model_file_payload.fast_metadata.model_token,
            model_name: model_file_payload.fast_metadata.model_name,
            model_author_username: model_file_payload.fast_metadata.model_author_username,
        })
    }

    pub fn read_encrypted_model_file_metadata<P>(&self, filename: P) -> Result<ModelFullMetadata, DeserializerError>
        where P: AsRef<Path>
    {
        let mut reader = Self::open_model_file_reader_checking_magic_bytes(filename)?;

        let mut buf = Vec::new();
        let _len = reader.read_to_end(&mut buf)?;

        let (model_file_payload , _len) : (ModelFileIntermediateReadPayload, usize) =
            bincode::decode_from_slice(&buf, BINCODE_CONFIG)?;

        let unencrypted_details_proto = self.hydrate_unencrypted_details_proto(
            &model_file_payload.unencrypted_proto_payload_bytes)?;

        let decrypted_details_proto = self.decrypt_and_hydrate_encrypted_details_proto(
            &model_file_payload.encrypted_lightweight_proto_payload_bytes)?;

        let model_created_at = maybe_timestamp_to_utc_datetime(unencrypted_details_proto.model_created_at);
        let model_updated_at = maybe_timestamp_to_utc_datetime(unencrypted_details_proto.model_updated_at);
        let maybe_model_invalid_before = maybe_timestamp_to_utc_datetime(decrypted_details_proto.invalid_before_timestamp);
        let maybe_model_expires_at = maybe_timestamp_to_utc_datetime(decrypted_details_proto.expiry_timestamp);

        Ok(ModelFullMetadata {
            file_format_version: model_file_payload.fast_metadata.file_format_version,
            model_token: model_file_payload.fast_metadata.model_token,
            model_name: model_file_payload.fast_metadata.model_name,
            model_author_username: model_file_payload.fast_metadata.model_author_username,

            model_version: unencrypted_details_proto.model_version,
            model_created_at,
            model_updated_at,

            maybe_model_invalid_before,
            maybe_model_expires_at,
            paid_plan_can_bypass_expiry: decrypted_details_proto.paid_plan_can_bypass_expiry,
            maybe_model_password: decrypted_details_proto.maybe_model_password,
        })
    }

    pub fn read_full_model_file<P>(&self, filename: P) -> Result<Model, DeserializerError>
        where P: AsRef<Path> {

        let reader = Self::open_model_file_reader_checking_magic_bytes(filename)?;

        let model_file_payload : ModelFilePayload = bincode::decode_from_reader(reader, BINCODE_CONFIG)?;

        let unencrypted_details_proto = self.hydrate_unencrypted_details_proto(
            &model_file_payload.unencrypted_proto_payload_bytes)?;

        let decrypted_details_proto = self.decrypt_and_hydrate_encrypted_details_proto(
            &model_file_payload.encrypted_lightweight_proto_payload_bytes)?;

        let model_created_at = maybe_timestamp_to_utc_datetime(unencrypted_details_proto.model_created_at);
        let model_updated_at = maybe_timestamp_to_utc_datetime(unencrypted_details_proto.model_updated_at);
        let maybe_model_invalid_before = maybe_timestamp_to_utc_datetime(decrypted_details_proto.invalid_before_timestamp);
        let maybe_model_expires_at = maybe_timestamp_to_utc_datetime(decrypted_details_proto.expiry_timestamp);

        let decrypted_heavy_proto = match model_file_payload.encrypted_heavy_proto_payload_bytes.bytes {
            None => return Err(DeserializerError::UnexpectedPayload),
            Some(ref bytes) => {
                let decrypted_bytes = self.crypto.decrypt_bytes(bytes)?;
                decode_proto_from_bytes::<EncryptedHeavyPayload>(&decrypted_bytes)?
            }
        };

        Ok(Model {
            file_format_version: model_file_payload.fast_metadata.file_format_version,
            model_token: model_file_payload.fast_metadata.model_token,
            model_name: model_file_payload.fast_metadata.model_name,
            model_author_username: model_file_payload.fast_metadata.model_author_username,

            model_version: unencrypted_details_proto.model_version,
            model_created_at,
            model_updated_at,

            maybe_model_invalid_before,
            maybe_model_expires_at,
            paid_plan_can_bypass_expiry: decrypted_details_proto.paid_plan_can_bypass_expiry,
            maybe_model_password: decrypted_details_proto.maybe_model_password,

            model_pipeline_version: decrypted_heavy_proto.model_pipeline_version.ok_or(DeserializerError::InvalidVersion)?,
            acoustic_model_bytes: decrypted_heavy_proto.acoustic_bytes.ok_or(DeserializerError::AbsentAcousticModel)?,
            vocoder_model_bytes: decrypted_heavy_proto.vocoder_bytes.ok_or(DeserializerError::AbsentVocoderModel)?,
        })
    }

    fn open_model_file_reader_checking_magic_bytes<P>(filename: P) -> Result<BufReader<File>, DeserializerError>
        where P: AsRef<Path>
    {
        let file = File::open(&filename)?;
        let mut reader = BufReader::new(file);

        Self::check_starting_magic_bytes(&mut reader)?;

        Ok(reader)
    }

    fn check_starting_magic_bytes(reader: &mut BufReader<File>) -> Result<(), DeserializerError> {
        let mut magic_bytes_buffer : [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        reader.read_exact(&mut magic_bytes_buffer)?;

        if magic_bytes_buffer != FILE_HEADER_MAGIC_BYTES.as_bytes() {
            return Err(DeserializerError::InvalidMagicBytes);
        }
        Ok(())
    }

    fn hydrate_unencrypted_details_proto(&self, unencrypted_proto_payload: &UnencryptedProtoPayloadBytes) -> Result<UnencryptedLightweightDetails, DeserializerError> {
        match unencrypted_proto_payload.bytes {
            None => return Err(DeserializerError::UnexpectedPayload),
            Some(ref bytes) => {
                Ok(decode_proto_from_bytes::<UnencryptedLightweightDetails>(bytes)?)
            }
        }
    }

    fn decrypt_and_hydrate_encrypted_details_proto(&self, encrypted_lightweight_proto_payload: &EncryptedLightweightProtoPayloadBytes) -> Result<EncryptedLightweightDetails, DeserializerError> {
        match encrypted_lightweight_proto_payload.bytes {
            None => return Err(DeserializerError::UnexpectedPayload),
            Some(ref bytes) => {
                let decrypted_bytes = self.crypto.decrypt_bytes(bytes)?;
                Ok(decode_proto_from_bytes::<EncryptedLightweightDetails>(&decrypted_bytes)?)
            }
        }
    }
}
