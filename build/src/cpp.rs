use crate::ast::*;
use std::collections::HashSet;

pub fn generate_cpp(bind_context: &BindContext) -> String {
    let mut lines = Vec::new();
    generate_includes(&bind_context.includes, &mut lines);
    lines.push("".to_string());

    generate_vec_shims(&bind_context.vec_defs, &mut lines);
    generate_map_shims(&bind_context.map_defs, &mut lines);

    for class_name_str in &bind_context.class_names_order {
        let class = bind_context.models.get(class_name_str).unwrap();
        generate_class_shim(class, &mut lines);
    }

    lines.join("\n")
}

fn generate_includes(includes: &Vec<syn::LitStr>, lines: &mut Vec<String>) {
    for path in includes {
        let val = path.value();
        if val.starts_with('<') && val.ends_with('>') {
            lines.push(format!("#include {}", val));
        } else {
            lines.push(format!("#include \"{}\"", val));
        }
    }
}

fn generate_class_shim(class: &ClassModel, lines: &mut Vec<String>) {
    let original_class_name = class.name.to_string();
    if class.needs_exposer {
        generate_exposer_class(class, &original_class_name, lines);
    }
    let target_class_name = class.get_cxx_name().to_string();

    for field in &class.fields {
        generate_field_shim(&target_class_name, field, lines);
    }

    for method in &class.methods {
        generate_method_shim(&target_class_name, method, lines);
    }
}

fn generate_exposer_class(class: &ClassModel, original_name: &str, lines: &mut Vec<String>) {
    let exposer_name = format!("{}_Exposer", original_name);
    lines.push(format!(
        "class {} : public {} {{",
        exposer_name, original_name
    ));
    lines.push("public:".to_string());
    lines.push(format!("using {}::{};", original_name, original_name));

    for field in &class.fields {
        if field.is_protected {
            lines.push(format!("using {}::{};", original_name, field.name));
        }
    }

    for method in &class.methods {
        if let MethodDef::Method(func) = method {
            if func.is_protected {
                lines.push(format!("using {}::{};", original_name, func.cpp_name));
            }
        }
    }

    lines.push("};".to_string());
    lines.push("".to_string());
}

fn generate_field_shim(class_name: &str, field: &FieldDef, lines: &mut Vec<String>) {
    match &field.ty {
        TypeKind::Primitive(_) | TypeKind::String => {
            lines.push(format!("DEFINE_VAL({}, {})", class_name, field.name));
            if !field.is_readonly {
                lines.push(format!("DEFINE_VAL_SET({}, {})", class_name, field.name));
            }
        }

        TypeKind::Object(_) | TypeKind::Vector { .. } | TypeKind::Map { .. } => {
            if field.is_readonly {
                lines.push(format!("DEFINE_OBJ_CONST({}, {})", class_name, field.name));
            } else {
                lines.push(format!("DEFINE_OBJ({}, {})", class_name, field.name));
                lines.push(format!("DEFINE_OBJ_SET({}, {})", class_name, field.name));
            }
        }

        TypeKind::Option(inner) => {
            match (inner.is_object_value(), field.is_readonly) {
                (true, true) => lines.push(format!(
                    "DEFINE_OPT_OBJ_CONST({}, {})",
                    class_name, field.name
                )),
                (true, false) => {
                    lines.push(format!("DEFINE_OPT_OBJ({}, {})", class_name, field.name))
                }
                (false, _) => lines.push(format!("DEFINE_OPT_VAL({}, {})", class_name, field.name)),
            }
            if !field.is_readonly {
                lines.push(format!("DEFINE_OBJ_SET({}, {})", class_name, field.name));
            }
        }

        TypeKind::Reference { inner, .. } => {
            if let TypeKind::Slice(_) = &**inner {
                lines.push(format!("DEFINE_VAL({}, {})", class_name, field.name));
            }
        }

        _ => {}
    }
}

fn generate_method_shim(class_name: &str, method: &MethodDef, lines: &mut Vec<String>) {
    match method {
        MethodDef::Iter(iter) => {
            lines.push(format!(
                "DEFINE_ITER({}, {}, {})",
                class_name,
                iter.rust_name,
                iter.yield_ty.get_flat_name()
            ));
        }
        MethodDef::Method(func) => {
            let rust_name = &func.rust_name;
            let cpp_name = &func.cpp_name;

            match func.kind {
                MethodKind::Static => {
                    lines.push(format!(
                        "DEFINE_STATIC_METHOD({}, {}, {})",
                        class_name, rust_name, cpp_name
                    ));
                }
                MethodKind::Const => {
                    let macro_name = if cpp_name == "operator()" {
                        "DEFINE_OP_CALL_CONST"
                    } else {
                        "DEFINE_METHOD_CONST"
                    };

                    if cpp_name == "operator()" {
                        lines.push(format!("{}({}, {})", macro_name, class_name, rust_name));
                    } else {
                        lines.push(format!(
                            "{}({}, {}, {})",
                            macro_name, class_name, rust_name, cpp_name
                        ));
                    }
                }
                MethodKind::Mutable => {
                    let macro_name = if cpp_name == "operator()" {
                        "DEFINE_OP_CALL"
                    } else {
                        "DEFINE_METHOD"
                    };

                    if cpp_name == "operator()" {
                        lines.push(format!("{}({}, {})", macro_name, class_name, rust_name));
                    } else {
                        lines.push(format!(
                            "{}({}, {}, {})",
                            macro_name, class_name, rust_name, cpp_name
                        ));
                    }
                }
            }
        }
        MethodDef::Ctor(ctor) => {
            if !ctor.is_user_defined {
                lines.push(format!("DEFINE_CTOR({}, {})", class_name, ctor.rust_name));
            }
        }
    }
}

fn generate_vec_shims(vec_defs: &HashSet<TypeKind>, lines: &mut Vec<String>) {
    let mut sorted_defs: Vec<&TypeKind> = vec_defs.iter().collect();
    sorted_defs.sort_by(|a, b| a.get_flat_name().cmp(&b.get_flat_name()));

    for def in sorted_defs {
        if let TypeKind::Vector { inner, is_ptr } = def {
            let elem_name = inner.get_flat_name();
            let alias = def.get_flat_name();

            if *is_ptr {
                lines.push(format!(
                    "using {} = std::vector<std::unique_ptr<{}>>;",
                    alias, elem_name
                ));
            } else {
                lines.push(format!("using {} = std::vector<{}>;", alias, elem_name));
            }

            lines.push(format!("DEFINE_VEC_OPS({}, {})", alias, elem_name));

            lines.push("".to_string());
        }
    }
}

fn generate_map_shims(map_defs: &HashSet<TypeKind>, lines: &mut Vec<String>) {
    let mut sorted_defs: Vec<&TypeKind> = map_defs.iter().collect();
    sorted_defs.sort_by(|a, b| a.get_flat_name().cmp(&b.get_flat_name()));

    for def in sorted_defs {
        if let TypeKind::Map {
            key,
            value,
            is_val_ptr,
        } = def
        {
            let key_name = key.get_flat_name();
            let val_name = value.get_flat_name();
            let alias = def.get_flat_name();

            if *is_val_ptr {
                lines.push(format!(
                    "using {} = std::unordered_map<{}, std::unique_ptr<{}>>;",
                    alias, key_name, val_name
                ));
            } else {
                lines.push(format!(
                    "using {} = std::unordered_map<{}, {}>;",
                    alias, key_name, val_name
                ));
            }

            lines.push(format!("DEFINE_MAP_OPS({})", alias));
            lines.push("".to_string());
        }
    }
}
