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

    let target_class_name = if class.needs_exposer {
        generate_exposer_class(class, &original_class_name, lines);
        format!("{}_Exposer", original_class_name)
    } else {
        original_class_name.clone()
    };

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
    match &field.kind {
        FieldKind::Val => {
            lines.push(format!("DEFINE_VAL({}, {})", class_name, field.name));
            if !field.is_readonly {
                lines.push(format!("DEFINE_VAL_SET({}, {})", class_name, field.name));
            }
        }
        FieldKind::Obj => {            
            if field.is_readonly{
                lines.push(format!("DEFINE_OBJ_CONST({}, {})", class_name, field.name));
            }else{
                lines.push(format!("DEFINE_OBJ({}, {})", class_name, field.name));
                lines.push(format!("DEFINE_OBJ_SET({}, {})", class_name, field.name));
            }
        }
        FieldKind::OptObj { ty: _ } => {
            lines.push(format!("DEFINE_OPT_OBJ({}, {})", class_name, field.name));
        }
        FieldKind::OptVal { ty: _ } => {
            lines.push(format!("DEFINE_OPT_VAL({}, {})", class_name, field.name));
        }
        FieldKind::Map { .. } => {
            lines.push(format!("DEFINE_OBJ({}, {})", class_name, field.name));
        }
        FieldKind::Vec { .. } => {
            lines.push(format!("DEFINE_OBJ({}, {})", class_name, field.name));
        }
    }
}

fn generate_method_shim(class_name: &str, method: &MethodDef, lines: &mut Vec<String>) {
    match method {
        MethodDef::Iter(iter) => {
            lines.push(format!(
                "DEFINE_ITER({}, {}, {})",
                class_name, iter.rust_name, iter.yield_ty
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

fn generate_vec_shims(vec_defs: &HashSet<VecDef>, lines: &mut Vec<String>) {
    let mut sorted_defs: Vec<&VecDef> = vec_defs.iter().collect();
    sorted_defs.sort_by(|a, b| a.elem_type.cmp(&b.elem_type).then(a.is_ptr.cmp(&b.is_ptr)));

    for def in sorted_defs {
        let elem_type = if &def.elem_type == "String" {
            "std::string"
        } else {
            &def.elem_type
        };

        if def.is_ptr {
            let alias = format!("Vec_Ptr_{}", &def.elem_type);
            lines.push(format!(
                "using {} = std::vector<std::unique_ptr<{}>>;",
                alias, elem_type
            ));
            lines.push(format!("DEFINE_VEC_OPS({}, {})", alias, elem_type));           
        } else {
            let alias = format!("Vec_{}", &def.elem_type);
            lines.push(format!("using {} = std::vector<{}>;", alias, elem_type));
            lines.push(format!("DEFINE_VEC_OPS({}, {})", alias, elem_type));
        }
        lines.push("".to_string()); // 空行分隔
    }
}

fn generate_map_shims(map_defs: &HashSet<MapDef>, lines: &mut Vec<String>) {
    let mut sorted_defs: Vec<&MapDef> = map_defs.iter().collect();
    sorted_defs.sort_by(|a, b| {
        a.key_type
            .cmp(&b.key_type)
            .then(a.value_type.cmp(&b.value_type))
            .then(a.is_value_ptr.cmp(&b.is_value_ptr))
    });

    for def in sorted_defs {
        let key_type = if &def.key_type == "String" {
            "std::string"
        } else {
            &def.key_type
        };
        let value_type = if &def.value_type == "String" {
            "std::string"
        } else {
            &def.value_type
        };

        if def.is_value_ptr {
            let alias = format!("Map_Ptr_{}_{}", &def.key_type, &def.value_type);
            lines.push(format!(
                "using {} = std::unordered_map<{},std::unique_ptr<{}>>;",
                alias, key_type, value_type
            ));
            lines.push(format!("DEFINE_MAP_OPS({})", alias));
            lines.push(format!("DEFINE_MAP_ITER({})", alias));
        } else {
            let alias = format!("Map_{}_{}", &def.key_type, &def.value_type);
            lines.push(format!(
                "using {} = std::unordered_map<{},{}>;",
                alias, key_type, value_type
            ));
            lines.push(format!("DEFINE_MAP_OPS({})", alias));
            lines.push(format!("DEFINE_MAP_ITER({})", alias));
        }

        lines.push("".to_string()); // 空行分隔
    }
}
