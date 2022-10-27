use crate::file_format::crypto::Crypto;
use crate::file_format::model_file_layout::{BINCODE_CONFIG, EncryptedHeavyProtoPayloadBytes, EncryptedLightweightProtoPayloadBytes, FILE_HEADER_MAGIC_BYTES, ModelFileFastMetadata, ModelFilePayload, UnencryptedProtoPayloadBytes};
use crate::file_format::protos::encode_proto_to_bytes;
use crate::model::Model;
use crate::protos::EncryptedHeavyPayload;
use crate::protos::EncryptedLightweightDetails;
use crate::protos::UnencryptedLightweightDetails;
use crate::serde::errors::SerializerError;
use crate::serde::time::utc_datetime_to_timestamp;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub struct ModelSerializer {
    crypto: Crypto,
}

impl ModelSerializer {

    pub fn new(crypto_secret: &str) -> Self {
        Self {
            crypto: Crypto::new(crypto_secret),
        }
    }

    pub fn write_model_to_file<P>(&self, model: Model, replace_file: bool, filename: P) -> Result<(), SerializerError>
        where P: AsRef<Path>
    {
        let unencrypted_lightweight_proto_bytes = {
            let mut model_proto = UnencryptedLightweightDetails::default();
            model_proto.model_version = model.model_version;

            model_proto.model_created_at = model.model_created_at
                .map(|datetime| utc_datetime_to_timestamp(datetime));

            model_proto.model_updated_at = model.model_updated_at
                .map(|datetime| utc_datetime_to_timestamp(datetime));

            let proto_bytes = encode_proto_to_bytes(model_proto)?;
            proto_bytes
        };

        let encrypted_lightweight_proto_bytes = {
            let mut model_proto = EncryptedLightweightDetails::default();

            model_proto.expiry_timestamp = model.maybe_model_expires_at
                .map(|datetime| utc_datetime_to_timestamp(datetime));

            model_proto.invalid_before_timestamp = model.maybe_model_invalid_before
                .map(|datetime| utc_datetime_to_timestamp(datetime));

            model_proto.paid_plan_can_bypass_expiry = model.paid_plan_can_bypass_expiry;
            model_proto.maybe_model_password = model.maybe_model_password;

            // TODO: Entropy

            let proto_bytes = encode_proto_to_bytes(model_proto)?;
            self.crypto.encrypt_bytes(&proto_bytes)?
        };

        let encrypted_heavy_proto_bytes = {
            let mut model_proto = EncryptedHeavyPayload::default();
            model_proto.model_pipeline_version = Some(model.model_pipeline_version);
            model_proto.acoustic_bytes = Some(model.acoustic_model_bytes);
            model_proto.vocoder_bytes = Some(model.vocoder_model_bytes);

            // TODO: Entropy

            let proto_bytes = encode_proto_to_bytes(model_proto)?;
            self.crypto.encrypt_bytes(&proto_bytes)?
        };

        let file_contents = ModelFilePayload {
            fast_metadata: ModelFileFastMetadata {
                file_format_version: model.file_format_version,
                model_token: model.model_token,
                model_name: model.model_name,
                model_author_username: model.model_author_username,
            },
            unencrypted_proto_payload_bytes: UnencryptedProtoPayloadBytes {
                bytes: Some(unencrypted_lightweight_proto_bytes.to_vec()),
            },
            encrypted_lightweight_proto_payload_bytes: EncryptedLightweightProtoPayloadBytes {
                bytes: Some(encrypted_lightweight_proto_bytes.to_vec()),
            },
            encrypted_heavy_proto_payload_bytes: EncryptedHeavyProtoPayloadBytes {
                bytes: Some(encrypted_heavy_proto_bytes.to_vec()),
            },
        };

        let file_bytes = bincode::encode_to_vec(file_contents, BINCODE_CONFIG)?;

        let mut options = OpenOptions::new();

        if replace_file {
            options.write(true)
                .create(true)
                .truncate(true);
        } else {
            // NB: This will fail if the file already exists.
            options.write(true)
                .create_new(true);
        }

        let mut file = options.open(filename)?;

        let _len = file.write(FILE_HEADER_MAGIC_BYTES.as_bytes())?;
        let _len = file.write_all(&file_bytes)?;
        file.flush()?;

        Ok(())
    }
}
