#![allow(dead_code)]
use std::{env, fs, path::PathBuf};

use crate::yaml_spec::{config_schema_json, testfile_schema_json};

#[path = "src/types/yaml_spec/mod.rs"]
mod yaml_spec;

fn main() {
    println!("cargo:rerun-if-changed=src/types/yaml_spec/mod.rs");
    println!("cargo:rerun-if-changed=src/types/yaml_spec/assertions.rs");
    println!("cargo:rerun-if-changed=src/types/yaml_spec/operations.rs");

    let parent_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));

    let testfile_schema = testfile_schema_json();
    let config_file_schema = config_schema_json();

    let out_path = parent_dir.join("evalt.schema.json");
    fs::write(&out_path, testfile_schema).expect("failed to write evalt.schema.json");

    let out_path = parent_dir.join("config.evalt.schema.json");
    fs::write(&out_path, config_file_schema).expect("failed to write config.evalt.schema.json");
}
