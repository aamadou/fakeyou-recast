use magic_crypt::MagicCryptError;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

pub enum DeserializerError {
    // External library errors
    BincodeDecodeError(bincode::error::DecodeError),
    CryptoError(MagicCryptError),
    IOError(std::io::Error),
    ProtoDecodeError(prost::DecodeError),

    // Other deserializer errors
    AbsentAcousticModel,
    AbsentVocoderModel,
    InvalidMagicBytes,
    InvalidVersion, // TODO: Actually make assertions on version so we ensure we're distributing valid versions
    UnexpectedPayload,
}

impl Error for DeserializerError {}

// NB: Debug is typically derived. We're hand-writing it to remove enum variant names and
// struct names that might leak into the binary during compile.
impl Debug for DeserializerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error") // NB: Intentionally obscure
    }
}

// NB: Display is generally hand-written, but we're intentionally being obscure here.
impl Display for DeserializerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error") // NB: Intentionally obscure
    }
}

impl From<bincode::error::DecodeError> for DeserializerError {
    #[inline]
    fn from(error: bincode::error::DecodeError) -> DeserializerError {
        DeserializerError::BincodeDecodeError(error)
    }
}

impl From<MagicCryptError> for DeserializerError {
    #[inline]
    fn from(error: MagicCryptError) -> DeserializerError {
        DeserializerError::CryptoError(error)
    }
}

impl From<std::io::Error> for DeserializerError {
    #[inline]
    fn from(error: std::io::Error) -> DeserializerError {
        DeserializerError::IOError(error)
    }
}

impl From<prost::DecodeError> for DeserializerError {
    #[inline]
    fn from(error: prost::DecodeError) -> DeserializerError {
        DeserializerError::ProtoDecodeError(error)
    }
}

pub enum SerializerError {
    // External library errors
    BincodeEncodeError(bincode::error::EncodeError),
    CryptoError(MagicCryptError),
    IOError(std::io::Error),
    ProtoEncodeError(prost::EncodeError),
}

impl Error for SerializerError {}

// NB: Debug is typically derived. We're hand-writing it to remove enum variant names and
// struct names that might leak into the binary during compile.
impl Debug for SerializerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error") // NB: Intentionally obscure
    }
}

// NB: Display is generally hand-written, but we're intentionally being obscure here.
impl Display for SerializerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error") // NB: Intentionally obscure
    }
}

impl From<bincode::error::EncodeError> for SerializerError {
    #[inline]
    fn from(error: bincode::error::EncodeError) -> SerializerError {
        SerializerError::BincodeEncodeError(error)
    }
}

impl From<MagicCryptError> for SerializerError {
    #[inline]
    fn from(error: MagicCryptError) -> SerializerError {
        SerializerError::CryptoError(error)
    }
}

impl From<std::io::Error> for SerializerError {
    #[inline]
    fn from(error: std::io::Error) -> SerializerError {
        SerializerError::IOError(error)
    }
}

impl From<prost::EncodeError> for SerializerError {
    #[inline]
    fn from(error: prost::EncodeError) -> SerializerError {
        SerializerError::ProtoEncodeError(error)
    }
}
