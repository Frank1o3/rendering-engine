use include_dir::{Dir, include_dir};

pub static SHADERS: Dir = include_dir!("$CARGO_MANIFEST_DIR/../shaders_gles");
