use std::env;
use std::fs;
use std::path::Path;

pub mod ast;
pub mod ffi;
pub mod wrapper;
pub mod parser;
pub mod utils;
mod cpp;
mod macros; 


pub fn bridge(rust_source_file: impl AsRef<Path>) -> cc::Build {
    let source_path = rust_source_file.as_ref();
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_dir_path = Path::new(&out_dir);

    println!("cargo:rerun-if-changed={}", source_path.display());

    let generated_rust = out_dir_path.join("cxxgen_bridge.rs");
    let generated_cpp = out_dir_path.join("shim.hh");
    generate_artifacts(
        source_path,
        &generated_rust,
        &generated_cpp,
    );
    let mut build = cxx_build::bridge(&generated_rust);
    build.include(out_dir_path);
    build
}

fn generate_artifacts(src: &Path, rust_out: &Path, cpp_out: &Path) {
    let source_code =
        fs::read_to_string(src).expect(&format!("Failed to read source file: {:?}", src));

    let dsl_content =
        parser::extract_dsl(&source_code).expect("No bind! { ... } block found in source file");

    let ast: ast::BindInput = syn::parse_str(&dsl_content).expect("Failed to parse bind! DSL");

    let bind_context = parser::preprocess(&ast);

    let rust_tokens = wrapper::generate_rust(&bind_context);
    let rust_code = rust_tokens.to_string();

    let cpp_header = cpp::generate_cpp(&bind_context);
    let cpp_utils = macros::CONTENT;
    let combined_cpp = format!("{}\n\n{}", cpp_utils, cpp_header);
    write_if_changed(rust_out, &rust_code);
    write_if_changed(cpp_out, &combined_cpp);

}

fn write_if_changed(path: &Path, content: &str) {
    if path.exists() {
        if let Ok(existing) = fs::read_to_string(path) {
            if existing == content {
                return;
            }
        }
    }
    fs::write(path, content).expect(&format!("Failed to write to {:?}", path));
}