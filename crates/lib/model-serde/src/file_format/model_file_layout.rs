//! Do not change the contents of this file without careful attention.
//! Make sure to write tests for format changes.

use bincode::config::{Configuration, Fixint, LittleEndian};
use bincode::{Decode, Encode};

/// All of our model files begin with the "FAKEYOU{NULL_BYTE}" magic bytes.
pub(crate) const FILE_HEADER_MAGIC_BYTES : &'static str = "FAKEYOU\0";

// NB: Do not change without taking care, as this controls our model file format.
pub(crate) const BINCODE_CONFIG : Configuration<LittleEndian, Fixint> = bincode::config::standard()
    .with_little_endian()
    .with_fixed_int_encoding()
    .write_fixed_array_length();

/// With the exception of the "inner_payload_bytes", everything here is expected to be read quickly.
#[derive(Encode, Decode, PartialEq, Debug)]
pub struct ModelFilePayload {
    /// The important metadata for each file.
    /// This should be available in under 500 bytes.
    pub fast_metadata: ModelFileFastMetadata,

    /// Variable additional metadata.
    /// Left as an option for fast file decoding that would omit these contents.
    pub unencrypted_proto_payload_bytes: UnencryptedProtoPayloadBytes,

    /// "Authoritative" encrypted information.
    /// Left as an option for fast file decoding that would omit these contents.
    pub encrypted_lightweight_proto_payload_bytes: EncryptedLightweightProtoPayloadBytes,

    /// Heavy payload contents that are encrypted (eg. model)
    /// Left as an option for fast file decoding that would omit these contents.
    pub encrypted_heavy_proto_payload_bytes: EncryptedHeavyProtoPayloadBytes,
}

/// With the exception of the "inner_payload_bytes", everything here is expected to be read quickly.
#[derive(Encode, Decode, PartialEq, Debug)]
pub struct ModelFileFastMetadata {
    /// A semver containing the file format version
    /// Serialization will respect a maximum length of 12 characters
    pub file_format_version: String,

    /// The unique primary key identifier of the model
    /// Serialization will respect a maximum length of 32 characters
    pub model_token: String,

    /// The name of the model
    /// Serialization will respect a maximum length of 255 characters
    pub model_name: String,

    /// The user that created the model
    /// Serialization will respect a maximum length of 20 characters
    pub model_author_username: String,
}

/// A zero-cost, reusable typesafe wrapper.
#[derive(Encode, Decode, PartialEq, Debug)]
pub struct UnencryptedProtoPayloadBytes {
    pub bytes: Option<Vec<u8>>,
}

/// A zero-cost, reusable typesafe wrapper.
#[derive(Encode, Decode, PartialEq, Debug)]
pub struct EncryptedLightweightProtoPayloadBytes {
    pub bytes: Option<Vec<u8>>,
}

/// A zero-cost, reusable typesafe wrapper.
#[derive(Encode, Decode, PartialEq, Debug)]
pub struct EncryptedHeavyProtoPayloadBytes {
    pub bytes: Option<Vec<u8>>,
}

/// A quick read view of a full model file will only glean the metadata.
#[derive(Encode, Decode, PartialEq, Debug)]
pub struct ModelFileQuickReadPayload {
    pub fast_metadata: ModelFileFastMetadata,
}

/// An intermediate read of the full model file will decode protos and decrypt important metadata.
#[derive(Encode, Decode, PartialEq, Debug)]
pub struct ModelFileIntermediateReadPayload {
    pub fast_metadata: ModelFileFastMetadata,
    pub unencrypted_proto_payload_bytes: UnencryptedProtoPayloadBytes,
    pub encrypted_lightweight_proto_payload_bytes: EncryptedLightweightProtoPayloadBytes,
}
