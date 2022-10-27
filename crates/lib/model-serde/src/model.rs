use chrono::{DateTime, Utc};

/// Decoded model file with no decryption.
/// The fastest processing pass we can take.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct ModelLightMetadata {
    pub file_format_version: String,
    pub model_token: String,
    pub model_name: String,
    pub model_author_username: String,
}

/// Decoded model file with metadata fields decrypted.
/// Faster than a full model file read and decrypt pass.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct ModelFullMetadata {
    pub file_format_version: String,
    pub model_token: String,
    pub model_name: String,
    pub model_author_username: String,

    pub model_version: Option<String>,
    pub model_created_at: Option<DateTime<Utc>>,
    pub model_updated_at: Option<DateTime<Utc>>,

    pub maybe_model_invalid_before: Option<DateTime<Utc>>,
    pub maybe_model_expires_at: Option<DateTime<Utc>>,

    /// If there's a model expiration date (in the past or future), can paid plans bypass this
    /// for the given model? (Some models we may always want to have self-destruct.)
    pub paid_plan_can_bypass_expiry: Option<bool>,

    /// Some models may have a short string password blocking their use. This lets us easily share
    /// certain protected models without server-based authentication.
    pub maybe_model_password: Option<String>,

    // TODO: Future fields - model creation time, model details, etc.
}


/// The full set of model file information.
/// This takes longer to read in, hydrate, and decrypt, so should not be used for enumerating a
/// directory's contents.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub file_format_version: String,
    pub model_token: String,
    pub model_name: String,
    pub model_author_username: String,

    pub model_version: Option<String>,
    pub model_created_at: Option<DateTime<Utc>>,
    pub model_updated_at: Option<DateTime<Utc>>,

    pub maybe_model_invalid_before: Option<DateTime<Utc>>,
    pub maybe_model_expires_at: Option<DateTime<Utc>>,

    /// If there's a model expiration date (in the past or future), can paid plans bypass this
    /// for the given model? (Some models we may always want to have self-destruct.)
    pub paid_plan_can_bypass_expiry: Option<bool>,

    /// Some models may have a short string password blocking their use. This lets us easily share
    /// certain protected models without server-based authentication.
    pub maybe_model_password: Option<String>,

    /// Changes how the pipeline executes.
    ///  Current supported versions:
    ///   1 - Default model execution pipeline.
    pub model_pipeline_version: u32,
    pub acoustic_model_bytes: Vec<u8>,
    pub vocoder_model_bytes: Vec<u8>,
}
