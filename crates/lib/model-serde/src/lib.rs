// Never allow these
#![forbid(private_in_public)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

// These are nitpicks and can be commented out if necessary
#![forbid(non_snake_case)]
#![forbid(unused_imports)]
#![forbid(unused_mut)]

// These are nitpicks and can be commented out if necessary, and are overridden in codegen
#![deny(dead_code)]
#![deny(unused_variables)]

pub (crate) mod protos {
    #![allow(dead_code)] // NB: Codegen violates our lint levels
    #![allow(unused_variables)] // NB: Codegen violates our lint levels
    include!(concat!(env!("OUT_DIR"), "/model_file_format.rs"));
}

pub (crate) mod file_format;

pub mod model;
pub mod serde;
