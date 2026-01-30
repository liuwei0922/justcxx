use std::env;
use std::fs;
use std::path::Path;

pub mod ast;
mod cpp;
pub mod ffi;
mod macros;
pub mod parser;
pub mod rust;
pub mod utils;
mod wrapper;

use crate::ast::*;
use quote::format_ident;
use std::collections::{HashMap, HashSet};

fn collect_containers(
    models: &HashMap<String, ClassModel>,
) -> (HashSet<TypeKind>, HashSet<TypeKind>) {
    let mut vecs = HashSet::new();
    let mut maps = HashSet::new();

    let mut visit = |ty: &TypeKind| {
        collect_recursive(ty, &mut vecs, &mut maps);
    };

    for model in models.values() {
        for field in &model.fields {
            visit(&field.ty);
        }

        for method in &model.methods {
            match method {
                MethodDef::Method(f) => {
                    for arg in &f.args {
                        visit(&arg.ty);
                    }
                    if let Some(ret) = &f.ret_ty {
                        visit(ret);
                    }
                }
                MethodDef::Ctor(c) => {
                    for arg in &c.args {
                        visit(&arg.ty);
                    }
                }
                MethodDef::Iter(iter) => {
                    visit(&iter.yield_ty);
                }
            }
        }
    }

    (vecs, maps)
}

fn collect_recursive(ty: &TypeKind, vecs: &mut HashSet<TypeKind>, maps: &mut HashSet<TypeKind>) {
    match ty {
        TypeKind::Vector { inner, .. } => {
            vecs.insert(ty.clone());
            collect_recursive(inner, vecs, maps);
        }

        TypeKind::Map { key, value, .. } => {
            maps.insert(ty.clone());
            collect_recursive(key, vecs, maps);
            collect_recursive(value, vecs, maps);
        }

        TypeKind::Reference { inner, .. }
        | TypeKind::Option(inner)
        | TypeKind::Result(inner)
        | TypeKind::UniquePtr(inner)
        | TypeKind::Slice(inner) => {
            collect_recursive(inner, vecs, maps);
        }

        TypeKind::Primitive(_) | TypeKind::String | TypeKind::Object(_) => {}
    }
}

fn inject_default_ctors(models: &mut HashMap<String, ClassModel>) {
    for (_name, model) in models.iter_mut() {
        let has_ctor = model
            .methods
            .iter()
            .any(|m| matches!(m, MethodDef::Ctor(_)));

        if !has_ctor {
            let default_ctor = MethodDef::Ctor(CtorDef {
                rust_name: format_ident!("new"),
                args: vec![],
                cpp_name: format_ident!("new"),
                is_user_defined: false,
            });

            model.methods.push(default_ctor);
        }
    }
}

pub fn preprocess(input: &BindInput) -> BindContext {
    let mut includes = Vec::new();
    let mut models = HashMap::new();
    let mut class_names_order = Vec::new();

    for item in &input.items {
        match item {
            BindItem::Include(path) => includes.push(path.clone()),
            BindItem::Struct(def) => {
                let name = def.name.clone();
                let name_str = name.to_string();
                let mut model = ClassModel::new(name);

                model.needs_exposer = def.fields.iter().any(|f| f.is_protected);
                model.fields = def.fields.clone();
                models.insert(name_str.clone(), model);
                class_names_order.push(name_str);
            }
            BindItem::Impl(def) => {
                let target = def.target.to_string();
                if let Some(model) = models.get_mut(&target) {
                    if def
                        .methods
                        .iter()
                        .any(|m| matches!(m, MethodDef::Method(f) if f.is_protected))
                    {
                        model.needs_exposer = true;
                    }
                    model.methods.extend(def.methods.clone());
                } else {
                    panic!("Impl block found for undefined struct '{}'", target);
                }
            }
        }
    }

    inject_default_ctors(&mut models);

    let (vec_defs, map_defs) = collect_containers(&models);

    BindContext {
        includes,
        models,
        class_names_order,
        vec_defs,
        map_defs,
    }
}

pub fn bridge(rust_source_file: impl AsRef<Path>) -> cc::Build {
    let source_path = rust_source_file.as_ref();
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_dir_path = Path::new(&out_dir);

    println!("cargo:rerun-if-changed={}", source_path.display());

    let generated_rust = out_dir_path.join("justcxx.rs");
    let generated_cpp = out_dir_path.join("justcxx.hh");
    generate_artifacts(source_path, &generated_rust, &generated_cpp);
    let mut build = cxx_build::bridge(&generated_rust);
    build.include(out_dir_path);
    build
}

fn generate_artifacts(src: &Path, rust_out: &Path, cpp_out: &Path) {
    let source_code =
        fs::read_to_string(src).expect(&format!("Failed to read source file: {:?}", src));

    let dsl_content =
        parser::extract_dsl(&source_code).expect("No bind! { ... } block found in source file");

    let ast: BindInput = syn::parse_str(&dsl_content).expect("Failed to parse bind! DSL");

    let bind_context = preprocess(&ast);

    let rust_tokens = rust::generate_rust(&bind_context);
    let rust_code = rust_tokens.to_string();

    let cpp_header = cpp::generate_cpp(&bind_context);
    let cpp_utils = macros::CONTENT;
    let combined_cpp = format!("{}\n{}", cpp_utils, cpp_header);
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
