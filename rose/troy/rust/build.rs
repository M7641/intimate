//! Compile the registry's .proto into Rust types at build time.
//!
//! Production path is `buf generate` (see registry/proto/buf.gen.yaml). This
//! build script is the no-system-protoc fallback: `protox` compiles the proto
//! to a FileDescriptorSet in pure Rust, then prost-build turns it into structs.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = PathBuf::from("../registry/proto");
    let proto_file = proto_root.join("contracts/customer/v1/customer.proto");

    let fds = protox::compile([&proto_file], [&proto_root])?;

    prost_build::Config::new()
        .out_dir(std::env::var("OUT_DIR")?)
        .compile_fds(fds)?;

    println!("cargo:rerun-if-changed={}", proto_root.display());
    Ok(())
}
