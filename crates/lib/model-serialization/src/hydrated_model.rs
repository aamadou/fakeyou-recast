
/// Decoded model file with no decryption.
/// The fastest processing pass we can take.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct HydratedModelLightMetadata {
    pub file_format_version: String,
    pub model_token: String,
    pub model_name: String,
    pub model_author_username: String,
}

/// Decoded model file with metadata fields decrypted.
/// Faster than a full model file read and decrypt pass.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct HydratedModelFullMetadata {
    pub file_format_version: String,
    pub model_token: String,
    pub model_name: String,
    pub model_author_username: String,

    pub model_version: Option<String>,
    pub model_created_at: Option<u64>,
    pub model_updated_at: Option<u64>,

    pub expiry_timestamp_millis: Option<u64>,

    // TODO: Future fields - model creation time, model details, etc.
}


/// The full set of model file information.
/// This takes longer to read in, hydrate, and decrypt, so should not be used for enumerating a
/// directory's contents.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct FullyHydratedModel {
    pub file_format_version: String,
    pub model_token: String,
    pub model_name: String,
    pub model_author_username: String,

    pub model_version: Option<String>,
    pub model_created_at: Option<u64>,
    pub model_updated_at: Option<u64>,

    pub expiry_timestamp_millis: Option<u64>,

    /// Changes how the pipeline executes.
    ///  Current supported versions:
    ///   1 - Default model execution pipeline.
    pub model_pipeline_version: u32,
    pub acoustic_model_bytes: Vec<u8>,
    pub vocoder_model_bytes: Vec<u8>,
}
