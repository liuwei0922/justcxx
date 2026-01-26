use crate::ast::*;
use crate::utils::*;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::{HashMap, HashSet};
use syn::Type;

pub fn generate_ffi_block(class: &ClassModel, models: &HashMap<String, ClassModel>) -> TokenStream {
    let class_name = &class.name;
    let mut items = Vec::new();

    if class.needs_exposer {
        let exposer_name = format!("{}_Exposer", class_name);
        items.push(quote! {
            #[cxx_name = #exposer_name]
            type #class_name;
        });
    } else {
        items.push(quote! { type #class_name;  });
    }

    for field in &class.fields {
        items.push(generate_ffi_field(class, field, models));
    }

    for method in &class.methods {
        items.push(generate_ffi_method(class, method, models));
    }
    quote! { #(#items)* }
}

pub fn generate_vec_ffi(vec_defs: &HashSet<VecDef>) -> TokenStream {
    let mut items = Vec::new();

    let mut sorted_defs: Vec<&VecDef> = vec_defs.iter().collect();
    sorted_defs.sort_by(|a, b| a.elem_type.cmp(&b.elem_type).then(a.is_ptr.cmp(&b.is_ptr)));

    for def in vec_defs {
        let elem_ty_ident = format_ident!("{}", &def.elem_type);
        let elem_ty_ident_type = if &def.elem_type == "String" {
            quote! { String }
        } else {
            quote! { Pin<&mut #elem_ty_ident> }
        };
        let ffi_type_name = if def.is_ptr {
            format_ident!("Vec_Ptr_{}", &def.elem_type)
        } else {
            format_ident!("Vec_{}", &def.elem_type)
        };

        let len_fn = format_ident!("{}_len", ffi_type_name);
        let get_fn = format_ident!("{}_get", ffi_type_name);
        let _push_fn = format_ident!("{}_push", ffi_type_name);

        items.push(quote! {
            type #ffi_type_name;

            #[rust_name = #len_fn]
            fn #len_fn(obj: &#ffi_type_name) -> usize;

            #[rust_name = #get_fn]
            fn #get_fn(obj: Pin<&mut #ffi_type_name>, index: usize) -> #elem_ty_ident_type;
        });
    }
    quote! { #(#items)* }
}

pub fn generate_map_ffi(
    map_defs: &HashSet<MapDef>,
    models: &HashMap<String, ClassModel>,
) -> TokenStream {
    let mut items = Vec::new();

    let mut sorted_defs: Vec<&MapDef> = map_defs.iter().collect();
    sorted_defs.sort_by(|a, b| {
        a.key_type
            .cmp(&b.key_type)
            .then(a.value_type.cmp(&b.value_type))
            .then(a.is_value_ptr.cmp(&b.is_value_ptr))
    });

    for def in sorted_defs {
        let key_ty_ident = format_ident!("{}", &def.key_type);
        let value_ty_ident = format_ident!("{}", &def.value_type);

        let key_type = if &def.key_type == "String" {
            quote! { &str  }
        } else {
            quote! { #key_ty_ident }
        };
        let value_type = if &def.value_type == "String" {
            quote! {String }
        } else if models.contains_key(&def.value_type) {
            quote! { Pin<&mut #value_ty_ident> }
        } else {
            quote! { #value_ty_ident }
        };

        let ffi_type_name = if def.is_value_ptr {
            format_ident!("Map_Ptr_{}_{}", &def.key_type, &def.value_type)
        } else {
            format_ident!("Map_{}_{}", &def.key_type, &def.value_type)
        };

        let len_fn = format_ident!("{}_len", ffi_type_name);
        let get_fn = format_ident!("{}_get", ffi_type_name);
        let _push_fn = format_ident!("{}_push", ffi_type_name);

        items.push(quote! {
            type #ffi_type_name;

            #[rust_name = #len_fn]
            fn #len_fn(obj: &#ffi_type_name) -> usize;

            #[rust_name = #get_fn]
            fn #get_fn(obj: Pin<&mut #ffi_type_name>, key: #key_type) -> Result<#value_type>;
        });

        let iter_new_fn = format_ident!("{}_iter_new", ffi_type_name);
        let iter_key_fn = format_ident!("{}_iter_key", ffi_type_name);
        let iter_val_fn = format_ident!("{}_iter_val", ffi_type_name);
        let iter_step_fn = format_ident!("{}_iter_step", ffi_type_name);
        let iter_is_end_fn = format_ident!("{}_iter_is_end", ffi_type_name);

        let iter_ctx_name = format_ident!("{}_IterCtx", ffi_type_name);
        let key_ret_type = if &def.key_type == "String" {
            quote! { String }
        } else {
            quote! { #key_ty_ident }
        };

        let val_ret_type = if &def.value_type == "String" {
            quote! { String }
        } else {
            quote! { Pin<&mut #value_ty_ident> }
        };

        items.push(quote! {
            // 声明 Context 类型
            type #iter_ctx_name;

            #[rust_name = #iter_new_fn]
            fn #iter_new_fn(obj: Pin<&mut #ffi_type_name>) -> UniquePtr<#iter_ctx_name>;

            #[rust_name = #iter_key_fn]
            fn #iter_key_fn(ctx: Pin<&mut #iter_ctx_name>) -> #key_ret_type;

            #[rust_name = #iter_val_fn]
            fn #iter_val_fn(ctx: Pin<&mut #iter_ctx_name>) -> #val_ret_type;

            #[rust_name = #iter_step_fn]
            fn #iter_step_fn(ctx: Pin<&mut #iter_ctx_name>);

            #[rust_name = #iter_is_end_fn]
            fn #iter_is_end_fn(ctx: Pin<&mut #iter_ctx_name>) -> bool;
        });
    }
    quote! { #(#items)* }
}

fn generate_ffi_field(
    class: &ClassModel,
    field: &FieldDef,
    _models: &HashMap<String, ClassModel>,
) -> TokenStream {
    let class_name = &class.name;
    let field_name = &field.name;
    let (cxx_ffi_get_name, cxx_ffi_set_name) = if class.needs_exposer {
        (
            format_ident!("{}_Exposer_get_{}", class_name, field_name),
            format_ident!("{}_Exposer_set_{}", class_name, field_name),
        )
    } else {
        (
            format_ident!("{}_get_{}", class_name, field_name),
            format_ident!("{}_set_{}", class_name, field_name),
        )
    };
    let rust_ffi_get_name = format_ident!("{}_get_{}", class_name, field_name);
    let rust_ffi_set_name = format_ident!("{}_set_{}", class_name, field_name);
    let ty = &field.ty;

    match &field.kind {
        FieldKind::Val => {
            let get = quote! {
                #[rust_name = #rust_ffi_get_name]
                fn #cxx_ffi_get_name(obj: &#class_name) -> #ty;
            };
            let set = quote! {
                #[rust_name = #rust_ffi_set_name]
                fn #cxx_ffi_set_name(obj: Pin<&mut #class_name>, value: #ty);
            };
            if field.is_readonly {
                get
            } else {
                quote! { #get #set}
            }
        }
        FieldKind::Obj => quote! {
            #[rust_name = #rust_ffi_get_name]
            fn #cxx_ffi_get_name(obj: Pin<&mut #class_name>) -> Pin<&mut #ty>;
        },
        FieldKind::OptObj { ty } => quote! {
            #[rust_name = #rust_ffi_get_name]
            fn #cxx_ffi_get_name(obj: Pin<&mut #class_name>) -> Result<Pin<&mut #ty>>;
        },
        FieldKind::OptVal { ty } => quote! {
            #[rust_name = #rust_ffi_get_name]
            fn #cxx_ffi_get_name(obj: &#class_name) -> Result<#ty>;
        },
        FieldKind::Map {
            key,
            value,
            is_value_ptr,
        } => {
            let key_name = get_type_ident_name(key);
            let val_name = get_type_ident_name(value);

            if let (Some(k), Some(v)) = (key_name, val_name) {
                let map_type_name = if *is_value_ptr {
                    format_ident!("Map_Ptr_{}_{}", k, v)
                } else {
                    format_ident!("Map_{}_{}", k, v)
                };
                quote! {
                    #[rust_name = #rust_ffi_get_name]
                    fn #cxx_ffi_get_name(obj: Pin<&mut #class_name>) -> Pin<&mut #map_type_name>;
                }
            } else {
                quote! {}
            }
        }
        FieldKind::Vec { element, is_ptr } => {
            if let Some(elem_name) = get_type_ident_name(element) {
                let vec_type_name = if *is_ptr {
                    format_ident!("Vec_Ptr_{}", elem_name)
                } else {
                    format_ident!("Vec_{}", elem_name)
                };
                quote! {
                    #[rust_name = #rust_ffi_get_name]
                    fn #cxx_ffi_get_name(obj: Pin<&mut #class_name>) -> Pin<&mut #vec_type_name>;
                }
            } else {
                quote! {}
            }
        }
    }
}

fn generate_ffi_method(
    class: &ClassModel,
    method: &MethodDef,
    models: &HashMap<String, ClassModel>,
) -> TokenStream {
    let class_name = &class.name;

    match method {
        MethodDef::Ctor(ctor) => {
            if class.needs_exposer && ctor.is_user_defined {
                panic!(
                    "Error in struct '{}': Constructor '{}' defines a custom C++ factory ('{}'), but the struct also uses #[protected] members. Custom factories are not supported with Exposer pattern. Please remove the custom factory binding.",
                    class_name, ctor.rust_name, ctor.cpp_name
                );
            }

            let cpp_name = if class.needs_exposer {
                format_ident!("make_{}_Exposer_{}", class_name, ctor.rust_name)
            } else if ctor.is_user_defined {
                ctor.cpp_name.clone()
            } else {
                format_ident!("make_{}_{}", class_name, ctor.rust_name)
            };

            let ffi_unique_name = format_ident!("make_{}_{}", class_name, ctor.rust_name);
            let args_sig = convert_args_sig(&ctor.args, models);
            quote! {
                #[rust_name = #ffi_unique_name]
                fn #cpp_name(#(#args_sig),*) -> UniquePtr<#class_name>;
            }
        }
        MethodDef::Iter(iter) => {
            if class.needs_exposer {
                panic!("Error: Exposer pattern is not supported for iterators")
            }
            let iternames = IterNames::new(class_name, &iter.rust_name);
            let yield_ty = &iter.yield_ty;
            let ctx_name = &iternames.ctx_name;
            let new_fn = &iternames.new_fn;
            let next_fn = &iternames.next_fn;
            quote! {
                type #ctx_name;
                #[rust_name = #new_fn]
                fn #new_fn(obj: Pin<&mut #class_name>) -> UniquePtr<#ctx_name>;
                #[rust_name = #next_fn]
                fn #next_fn(ctx: Pin<&mut #ctx_name>) -> UniquePtr<#yield_ty>;
            }
        }
        MethodDef::Method(func) => {
            let rust_ffi_fn_name = format_ident!("{}_method_{}", class_name, func.rust_name);
            let cxx_ffi_fn_name = if class.needs_exposer {
                format_ident!("{}_Exposer_method_{}", class_name, func.rust_name)
            } else {
                rust_ffi_fn_name.clone()
            };

            let args_sig = convert_args_sig(&func.args, models);
            let ffi_ret = convert_return_type(&func.ret_ty, models);
            match func.kind {
                MethodKind::Static => quote! {
                    #[rust_name = #rust_ffi_fn_name]
                    fn #cxx_ffi_fn_name(#(#args_sig),*) #ffi_ret;
                },
                MethodKind::Const => quote! {
                    #[rust_name = #rust_ffi_fn_name]
                    fn #cxx_ffi_fn_name(obj: &#class_name, #(#args_sig),*) #ffi_ret;
                },
                MethodKind::Mutable => quote! {
                    #[rust_name = #rust_ffi_fn_name]
                    fn #cxx_ffi_fn_name(obj: Pin<&mut #class_name>, #(#args_sig),*) #ffi_ret;
                },
            }
        }
    }
}

fn convert_args_sig(args: &[Arg], models: &HashMap<String, ClassModel>) -> Vec<TokenStream> {
    args.iter()
        .map(|arg| {
            let n = &arg.name;
            let t = &arg.ty;
            // ref
            if let Some(info) = extract_defined_ref_info(&t, models) {
                let elem = info.elem;
                if info.is_mut {
                    return quote! { #n: Pin<&mut #elem> };
                }
                return quote! { #n: &#elem };
            }
            // owned
            if let Some(name) = get_type_ident_name(t) {
                if models.contains_key(&name) {
                    let elem = format_ident!("{}", name);                    
                    return quote! { #n: UniquePtr<#elem> };
                }
            }
            quote! { #n: #t }
        })
        .collect()
}

fn convert_return_type(ret: &Option<Type>, models: &HashMap<String, ClassModel>) -> TokenStream {
    // void
    let ty = match ret {
        Some(t) => t,
        None => return quote! {},
    };
    // ref
    if let Some(info) = extract_defined_ref_info(ty, models) {
        let elem = info.elem;
        if info.is_mut {
            return quote! { -> Pin<&mut #elem> };
        } else {
            return quote! { -> &#elem };
        }
    }
    // Option<T>
    if let Some(inner_ty) = extract_option_inner(ty) {
        // Option<ref>
        if let Some(info) = extract_defined_ref_info(&inner_ty, models) {
            let elem = info.elem;
            if info.is_mut {
                return quote! { -> Result<Pin<&mut #elem>> };
            } else {
                return quote! { -> Result<&#elem> };
            }
        }
        // Option<value>
        if let Some(name) = get_type_ident_name(&inner_ty) {
            if models.contains_key(&name) {
                let ident = format_ident!("{}", name);
                return quote! { -> UniquePtr<#ident> };
            }
        }
        // Option<primitive>
        return quote! { -> Result<#inner_ty> };
    }
    // value
    if let Some(name) = get_type_ident_name(ty) {
        if models.contains_key(&name) {
            let ident = format_ident!("{}", name);
            return quote! { ->UniquePtr<#ident> };
        }
    }
    // primitive
    quote! { -> #ty }
}
