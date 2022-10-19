use anyhow::anyhow;
use bincode::config::{Configuration, Fixint, LittleEndian};
use crate::hydrated_model::{FullyHydratedModel, HydratedModelFullMetadata, HydratedModelLightMetadata};
use crate::model_file::{EncryptedHeavyProtoPayloadBytes, EncryptedLightweightProtoPayloadBytes, ModelFileFastMetadata, ModelFileIntermediateReadPayload, ModelFilePayload, ModelFileQuickReadPayload, UnencryptedProtoPayloadBytes};
use crate::protos::EncryptedHeavyPayload;
use crate::protos::EncryptedLightweightDetails;
use crate::protos::UnencryptedLightweightDetails;
use crate::time::{maybe_timestamp_to_utc_datetime, utc_datetime_to_timestamp};
use magic_crypt::generic_array::typenum::U256;
use magic_crypt::new_magic_crypt;
use magic_crypt::{MagicCryptTrait, MagicCrypt256};
use prost::Message;
use prost::bytes::BytesMut;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Cursor, Read, Write};
use std::path::Path;

/// All of our model files begin with the "FAKEYOU{NULL_BYTE}" magic bytes.
const FILE_HEADER_MAGIC_BYTES : &'static str = "FAKEYOU\0";

// NB: Do not change without taking care, as this controls our model file format.
const BINCODE_CONFIG : Configuration<LittleEndian, Fixint> = bincode::config::standard()
    .with_little_endian()
    .with_fixed_int_encoding()
    .write_fixed_array_length();

/// Utility that handles model serialization, deserialization, encryption, decryption, and file operations.
pub struct ModelSerializer {
    crypt: MagicCrypt256,
}

impl ModelSerializer {
    pub fn new(crypto_secret: &str) -> Self {
        Self {
            crypt: new_magic_crypt!(crypto_secret, 256),
        }
    }

    pub fn read_fast_model_file_metadata<P>(&self, filename: P) -> anyhow::Result<HydratedModelLightMetadata>
        where P: AsRef<Path>
    {
        let mut reader = Self::open_model_file_reader_checking_magic_bytes(filename)?;

        // NB: The important data is in the first 500 bytes.
        let mut buf = vec![0u8; 500];
        reader.read_exact(&mut buf)?;

        let (model_file_payload , _len) : (ModelFileQuickReadPayload, usize) =
            bincode::decode_from_slice(&buf, BINCODE_CONFIG)?;

        Ok(HydratedModelLightMetadata {
            file_format_version: model_file_payload.fast_metadata.file_format_version,
            model_token: model_file_payload.fast_metadata.model_token,
            model_name: model_file_payload.fast_metadata.model_name,
            model_author_username: model_file_payload.fast_metadata.model_author_username,
        })
    }

    pub fn read_encrypted_model_file_metadata<P>(&self, filename: P) -> anyhow::Result<HydratedModelFullMetadata>
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

        Ok(HydratedModelFullMetadata {
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

    pub fn read_full_model_file<P>(&self, filename: P) -> anyhow::Result<FullyHydratedModel>
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
            None => return Err(anyhow!("unexpected payload (1)")),
            Some(ref bytes) => {
                let decrypted_bytes = self.decrypt_bytes(bytes)?;
                Self::decode_proto_from_bytes::<EncryptedHeavyPayload>(&decrypted_bytes)?
            }
        };

        Ok(FullyHydratedModel {
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

            model_pipeline_version: decrypted_heavy_proto.model_pipeline_version.ok_or(anyhow!("no version"))?,
            acoustic_model_bytes: decrypted_heavy_proto.acoustic_bytes.ok_or(anyhow!("no model (1)"))?,
            vocoder_model_bytes: decrypted_heavy_proto.vocoder_bytes.ok_or(anyhow!("no model (2)"))?,
        })
    }

    pub fn write_model_to_file<P>(&self, model: FullyHydratedModel, replace_file: bool, filename: P) -> anyhow::Result<()>
        where P: AsRef<Path>
    {
        let unencrypted_lightweight_proto_bytes = {
            let mut model_proto = UnencryptedLightweightDetails::default();
            model_proto.model_version = model.model_version;

            model_proto.model_created_at = model.model_created_at
                .map(|datetime| utc_datetime_to_timestamp(datetime));

            model_proto.model_updated_at = model.model_updated_at
                .map(|datetime| utc_datetime_to_timestamp(datetime));

            let proto_bytes = Self::encode_proto_to_bytes(model_proto)?;
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

            let proto_bytes = Self::encode_proto_to_bytes(model_proto)?;
            self.encrypt_bytes(&proto_bytes)?
        };

        let encrypted_heavy_proto_bytes = {
            let mut model_proto = EncryptedHeavyPayload::default();
            model_proto.model_pipeline_version = Some(model.model_pipeline_version);
            model_proto.acoustic_bytes = Some(model.acoustic_model_bytes);
            model_proto.vocoder_bytes = Some(model.vocoder_model_bytes);

            // TODO: Entropy

            let proto_bytes = Self::encode_proto_to_bytes(model_proto)?;
            self.encrypt_bytes(&proto_bytes)?
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

    fn open_model_file_reader_checking_magic_bytes<P>(filename: P) -> anyhow::Result<BufReader<File>> where P: AsRef<Path> {
        let file = File::open(&filename)?;
        let mut reader = BufReader::new(file);

        Self::check_starting_magic_bytes(&mut reader)?;

        Ok(reader)
    }

    fn check_starting_magic_bytes(reader: &mut BufReader<File>) -> anyhow::Result<()> {
        let mut magic_bytes_buffer : [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        reader.read_exact(&mut magic_bytes_buffer)?;

        if magic_bytes_buffer != FILE_HEADER_MAGIC_BYTES.as_bytes() {
            return Err(anyhow!("magic bytes do not match"));
        }
        Ok(())
    }

    fn encode_proto_to_bytes<T>(proto: T) -> anyhow::Result<BytesMut>
        where T : Message
    {
        let mut buffer = BytesMut::with_capacity(1024);
        proto.encode(&mut buffer)?;
        Ok(buffer)
    }

    fn decode_proto_from_bytes<T>(bytes: &[u8]) -> anyhow::Result<T>
        where T: Message + Default
    {
        // TODO: What's the cost here? Is the iterator read twice, and if so, can we reduce to one read?
        let bytes_mut = BytesMut::from_iter(bytes.iter());
        let decoded_proto = T::decode(bytes_mut)?;
        Ok(decoded_proto)
    }

    fn encrypt_bytes(&self, unencrypted_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut reader = Cursor::new(unencrypted_bytes);
        let mut encrypted_bytes = Vec::new();
        self.crypt.encrypt_reader_to_writer2::<U256>(&mut reader, &mut encrypted_bytes)?;
        Ok(encrypted_bytes)
    }

    fn decrypt_bytes(&self, encrypted_bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
        let decrypted_bytes = self.crypt.decrypt_bytes_to_bytes(&encrypted_bytes)?;
        Ok(decrypted_bytes)
    }

    fn hydrate_unencrypted_details_proto(&self, unencrypted_proto_payload: &UnencryptedProtoPayloadBytes) -> anyhow::Result<UnencryptedLightweightDetails> {
        match unencrypted_proto_payload.bytes {
            None => return Err(anyhow!("unexpected payload (1)")),
            Some(ref bytes) => {
                Self::decode_proto_from_bytes::<UnencryptedLightweightDetails>(bytes)
            }
        }
    }
    fn decrypt_and_hydrate_encrypted_details_proto(&self, encrypted_lightweight_proto_payload: &EncryptedLightweightProtoPayloadBytes) -> anyhow::Result<EncryptedLightweightDetails> {
        match encrypted_lightweight_proto_payload.bytes {
            None => return Err(anyhow!("unexpected payload (1)")),
            Some(ref bytes) => {
                let decrypted_bytes = self.decrypt_bytes(bytes)?;
                Self::decode_proto_from_bytes::<EncryptedLightweightDetails>(&decrypted_bytes)
            }
        }
    }
}
