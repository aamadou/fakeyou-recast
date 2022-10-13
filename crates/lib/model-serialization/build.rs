use prost_build::Config;
use std::io::Result;

// NB: This forces a recompile if the proto source file changes.
// Without this, only Rust changes will trigger a rebuild.
// AFAIK, this is the best way.
const _: &'static str = include_str!("../../../protos/model_file_format.proto");

fn main() -> Result<()> {
    // NB: As of prost-build 0.11, they no longer bundle a protoc [1] with the library.
    //  Instead, it's recommended to grab a precompiled binary from Google [2] (search for a
    //  protoc-* binary for your platform, not a library binding).
    //  [1] https://github.com/tokio-rs/prost#protoc
    //  [2] https://github.com/protocolbuffers/protobuf/releases
    Config::new()
        //.protoc_arg("--experimental_allow_proto3_optional");
        .compile_protos(&["../../../protos/model_file_format.proto"], &["../../../protos/"])
        .unwrap();

    Ok(())
}