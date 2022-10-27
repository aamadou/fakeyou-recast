use anyhow::{anyhow, bail};
use chrono::{DateTime, Duration, TimeZone, Utc};
use clap::Parser;
use std::fs::File;
use std::io::{BufReader, Read};
use std::ops::Add;
use std::path::{Path, PathBuf};
use model_serde::model::Model;
use model_serde::serde::serializer::ModelSerializer;

/// Package FakeYou realtime voice conversion models
#[derive(Parser, Debug)]
struct Args {
    /// Name of the model, eg. "Joe Biden (angry)"
    #[arg(long)]
    model_name: String,

    /// Foreign key / database identifier for the model. Starts with "vcm_".
    #[arg(long)]
    model_token: String,

    /// FakeYou username of the model creator (with case).
    #[arg(long)]
    author_username: String,

    /// Crypto secret used to encrypt the file
    #[arg(long)]
    crypto_secret: String,

    /// Location of the raw acoustic model file
    #[arg(long)]
    acoustic_file: String,

    /// Location of the raw vocoder model file
    #[arg(long)]
    vocoder_file: String,

    /// Name of the final output file to write
    #[arg(long)]
    output_file: String,

    /// Model creation timestamp (RFC3339, eg. "1999-09-09T23:55:55.50Z")
    /// If absent, it will be "now".
    #[arg(long)]
    created_at: Option<String>,

    /// Model update timestamp (RFC3339, eg. "1999-09-09T23:55:55.50Z")
    /// If absent, it will be "created_at" or its default.
    #[arg(long)]
    updated_at: Option<String>,

    /// Model expiration timestamp (RFC3339, eg. "1999-09-09T23:55:55.50Z")
    /// Mutually exclusively set with this or the "expires_in_days" arg.
    /// If both of these arguments are absent, it will be treated as never-expiring.
    #[arg(long)]
    expires_at: Option<String>,

    /// Model expiration in days
    /// Mutually exclusively set with this or the "expires_at" arg.
    /// If both of these arguments are absent, it will be treated as never-expiring.
    #[arg(long)]
    expires_in_days: Option<i64>,
}

fn read_file_bytes<P>(filename: P) -> anyhow::Result<Vec<u8>> where P: AsRef<Path> {
    let file = File::open(filename)?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.expires_at.is_some() && args.expires_in_days.is_some() {
        return Err(anyhow!("You cannot set both expires_at and expires_in_days"));
    }

    let created_at = match &args.created_at {
        Some(created_at) => DateTime::parse_from_rfc3339(created_at)
            .map_err(|e| anyhow!("could not parse created_at timestamp: {:?}", e))?
            .naive_utc(),
        None => Utc::now()
            .naive_utc(),
    };

    let updated_at = match &args.updated_at {
        Some(created_at) => DateTime::parse_from_rfc3339(created_at)
            .map_err(|e| anyhow!("could not parse updated_at timestamp: {:?}", e))?
            .naive_utc(),
        None => created_at,
    };

    let mut expires_at = None;

    if let Some(expires) = &args.expires_at {
        expires_at = Some(DateTime::parse_from_rfc3339(expires)
            .map_err(|e| anyhow!("could not parse updated_at timestamp: {:?}", e))?
            .naive_utc());
    }

    if let Some(expires_in_days) = args.expires_in_days {
        let days = Duration::days(expires_in_days);
        expires_at = Some(Utc::now()
            .naive_utc()
            .add(days));
    }


    let created_at = Utc.from_utc_datetime(&created_at);
    let updated_at = Utc.from_utc_datetime(&updated_at);
    let expires_at = expires_at.map(|naive| Utc.from_utc_datetime(&naive));

    let acoustic_file = PathBuf::from(args.acoustic_file);
    let vocoder_file = PathBuf::from(args.vocoder_file);

    if !acoustic_file.exists() {
        bail!("Acoustic model file does not exist: {:?}", acoustic_file);
    }

    if !vocoder_file.exists() {
        bail!("Vocoder model file does not exist: {:?}", acoustic_file);
    }

    let acoustic_file_bytes = read_file_bytes(acoustic_file)?;
    let vocoder_file_bytes = read_file_bytes(vocoder_file)?;

    let serializer = ModelSerializer::new(&args.crypto_secret);

    let model = Model {
        file_format_version: "1.0.0".to_string(),
        model_token: args.model_token,
        model_name: args.model_name,
        model_author_username: args.author_username,
        model_version: None,
        model_created_at: Some(created_at),
        model_updated_at: Some(updated_at),
        maybe_model_expires_at: expires_at,
        maybe_model_invalid_before: None,
        paid_plan_can_bypass_expiry: Some(true),
        maybe_model_password: None,
        model_pipeline_version: 1,
        acoustic_model_bytes: acoustic_file_bytes,
        vocoder_model_bytes: vocoder_file_bytes,
    };

    serializer.write_model_to_file(model, true, args.output_file).unwrap();

    println!("Success!");

    Ok(())
}
