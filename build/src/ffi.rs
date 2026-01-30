use crate::ast::*;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashSet;
use syn::Ident;

fn convert_args(args: &[Arg]) -> Vec<TokenStream> {
    args.iter()
        .map(|arg| {
            let n = &arg.name;
            let t = arg.ty.to_ffi_type(false);
            quote! { #n: #t }
        })
        .collect()
}

fn convert_ret(ret: &Option<TypeKind>) -> TokenStream {
    match ret {
        Some(ty) => {
            let t = ty.to_ffi_type(true);
            quote! { -> #t }
        }
        None => quote! {},
    }
}

fn get_method_names(
    class_name: &Ident,
    rust_name: &Ident,
    needs_exposer: bool,
    is_ctor: bool,
) -> (Ident, Ident) {
    let base_name = if is_ctor {
        format!("make_{}_{}", class_name, rust_name)
    } else {
        format!("{}_method_{}", class_name, rust_name)
    };

    let ffi_rust_name = format_ident!("{}", base_name);

    let ffi_cpp_name = if needs_exposer {
        if is_ctor {
            format_ident!("make_{}_Exposer_{}", class_name, rust_name)
        } else {
            format_ident!("{}_Exposer_method_{}", class_name, rust_name)
        }
    } else {
        ffi_rust_name.clone()
    };

    (ffi_rust_name, ffi_cpp_name)
}

impl CtorDef {
    fn generate_ffi(&self, class_name: &Ident, needs_exposer: bool) -> TokenStream {
        let (ffi_rust, ffi_cpp) = if self.is_user_defined {
            if needs_exposer {
                panic!("Custom factories not supported with Exposer");
            }
            (
                format_ident!("{}_{}", class_name, self.rust_name),
                self.cpp_name.clone(),
            )
        } else {
            get_method_names(class_name, &self.rust_name, needs_exposer, true)
        };

        let args = convert_args(&self.args);

        quote! {
            #[rust_name = #ffi_rust]
            fn #ffi_cpp(#(#args),*) -> UniquePtr<#class_name>;
        }
    }
}

impl FnDef {
    fn generate_ffi(&self, class_name: &Ident, needs_exposer: bool) -> TokenStream {
        let (ffi_rust, ffi_cpp) =
            get_method_names(class_name, &self.rust_name, needs_exposer, false);

        let args = convert_args(&self.args);
        let ret = convert_ret(&self.ret_ty);

        match self.kind {
            MethodKind::Static => quote! {
                #[rust_name = #ffi_rust]
                fn #ffi_cpp(#(#args),*) #ret;
            },
            MethodKind::Const => quote! {
                #[rust_name = #ffi_rust]
                fn #ffi_cpp(obj: &#class_name, #(#args),*) #ret;
            },
            MethodKind::Mutable => quote! {
                #[rust_name = #ffi_rust]
                fn #ffi_cpp(obj: Pin<&mut #class_name>, #(#args),*) #ret;
            },
        }
    }
}

impl IterDef {
    fn generate_ffi(&self, class_name: &Ident, needs_exposer: bool) -> TokenStream {
        if needs_exposer {
            panic!("Exposer pattern is not supported for iterators");
        }

        let iternames = IterNames::new(class_name, &self.rust_name);
        let ctx_name = &iternames.ctx_name;
        let new_fn = &iternames.new_fn;
        let next_fn = &iternames.next_fn;
        let yield_ffi_ty = self.yield_ty.to_ffi_type_name_only();

        let self_arg = if self.is_iter_mut {
            quote! { Pin<&mut #class_name> }
        } else {
            quote! { &#class_name }
        };

        quote! {
            type #ctx_name;

            #[rust_name = #new_fn]
            fn #new_fn(obj: #self_arg) -> UniquePtr<#ctx_name>;

            #[rust_name = #next_fn]
            fn #next_fn(ctx: Pin<&mut #ctx_name>) -> UniquePtr<#yield_ffi_ty>;
        }
    }
}

pub fn generate_ffi_method(class: &ClassModel, method: &MethodDef) -> TokenStream {
    let class_name = &class.name;
    let needs_exposer = class.needs_exposer;

    match method {
        MethodDef::Ctor(ctor) => ctor.generate_ffi(class_name, needs_exposer),
        MethodDef::Iter(iter) => iter.generate_ffi(class_name, needs_exposer),
        MethodDef::Method(func) => func.generate_ffi(class_name, needs_exposer),
    }
}

fn generate_ffi_field(class: &ClassModel, field: &FieldDef) -> TokenStream {
    let class_name = &class.name;
    let field_name = &field.name;

    let (cxx_get, cxx_set) = if class.needs_exposer {
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
    let rust_get = field.get_ffi_get_name(class_name);
    let rust_set = field.get_ffi_set_name(class_name);

    let make_getter = |ret_ty, is_const_self| {
        if is_const_self {
            quote! {
                #[rust_name = #rust_get]
                fn #cxx_get(obj: &#class_name) -> #ret_ty;
            }
        } else {
            quote! {
                #[rust_name = #rust_get]
                fn #cxx_get(obj: Pin<&mut #class_name>) -> #ret_ty;
            }
        }
    };

    let make_setter = |arg_ty: TokenStream| {
        quote! {
            #[rust_name = #rust_set]
            fn #cxx_set(obj: Pin<&mut #class_name>, val: #arg_ty);
        }
    };

    match &field.ty {
        TypeKind::Primitive(_) | TypeKind::String => {
            let ret_ty = field.ty.to_ffi_type(true);
            let get = make_getter(ret_ty, true);

            if field.is_readonly {
                get
            } else {
                let arg_ty = field.ty.to_ffi_type(false);
                let set = make_setter(arg_ty);
                quote! { #get #set }
            }
        }

        TypeKind::Object(_) | TypeKind::Vector { .. } | TypeKind::Map { .. } => {
            if field.is_readonly {
                let ret_kind = TypeKind::new_const_ref(field.ty.clone());
                make_getter(ret_kind.to_ffi_type(true), true)
            } else {
                let ret_kind = TypeKind::new_mut_ref(field.ty.clone());
                let get = make_getter(ret_kind.to_ffi_type(true), false);

                let arg_kind = TypeKind::new_unique_ptr(field.ty.clone());

                let set = make_setter(arg_kind.to_ffi_type(false));

                quote! { #get #set }
            }
        }
        TypeKind::Option(inner) => {
            let is_obj = inner.is_object_value();
            let inner_ty = inner.as_ref().clone(); // 安全克隆内部类型

            let (ret_ty_tokens, is_const_getter) = match (is_obj, field.is_readonly) {
                (true, true) => {
                    let ret = TypeKind::new_result(TypeKind::new_const_ref(inner_ty.clone()));
                    (ret.to_ffi_type(true), true)
                }
                (true, false) => {
                    let ret = TypeKind::new_result(TypeKind::new_mut_ref(inner_ty.clone()));
                    (ret.to_ffi_type(true), false)
                }
                (false, _) => {
                    let ret = TypeKind::new_result(inner_ty.clone());
                    (ret.to_ffi_type(true), true)
                }
            };
            let get = make_getter(ret_ty_tokens, is_const_getter);

            if field.is_readonly {
                get
            } else {
                let arg_ty_tokens = match is_obj {
                    true => TypeKind::new_unique_ptr(inner_ty).to_ffi_type(false),
                    false => inner_ty.to_ffi_type(false),
                };
                let set = make_setter(arg_ty_tokens);
                quote! { #get #set }
            }
        }

        _ => quote! {},
    }
}

pub fn generate_ffi_block(class: &ClassModel) -> TokenStream {
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
        items.push(generate_ffi_field(class, field));
    }

    for method in &class.methods {
        items.push(generate_ffi_method(class, method));
    }
    quote! { #(#items)* }
}

pub fn generate_vec_ffi(vec_defs: &HashSet<TypeKind>) -> TokenStream {
    let mut items = Vec::new();

    let mut sorted_defs: Vec<&TypeKind> = vec_defs.iter().collect();
    sorted_defs.sort_by(|a, b| {
        let name_a = a.get_flat_name();
        let name_b = b.get_flat_name();
        name_a.cmp(&name_b)
    });

    for def in sorted_defs {
        if let TypeKind::Vector { inner, is_ptr: _ } = def {
            let elem_ty = inner;
            let ffi_type_name = def.to_ffi_type_name_only();
            let ffi_type_str = def.get_flat_name();

            let len_fn = format_ident!("{}_len", ffi_type_str);
            let get_fn = format_ident!("{}_get", ffi_type_str);
            let get_mut_fn = format_ident!("{}_get_mut", ffi_type_str);
            let push_fn = format_ident!("{}_push", ffi_type_str);
            let set_fn = format_ident!("{}_set", ffi_type_str);
            let new_fn = format_ident!("make_{}_new", ffi_type_str);

            items.push(quote! {
                type #ffi_type_name;

                #[rust_name = #new_fn]
                fn #new_fn() -> UniquePtr<#ffi_type_name>;

                #[rust_name = #len_fn]
                fn #len_fn(obj: &#ffi_type_name) -> usize;
            });

            let get_ret_ty = if let TypeKind::String = **elem_ty {
                quote! { Result<String> }
            } else if elem_ty.is_object_value() {
                let t = elem_ty.to_ffi_type_name_only();
                quote! { Result<&#t> }
            } else {
                let t = elem_ty.to_ffi_type_name_only();
                quote! { Result<#t> }
            };

            items.push(quote! {
                #[rust_name = #get_fn]
                fn #get_fn(obj: &#ffi_type_name, index: usize) -> #get_ret_ty;
            });

            let push_arg_ty = if let TypeKind::String = **elem_ty {
                quote! { &str }
            } else {
                elem_ty.to_ffi_type(false)
            };

            items.push(quote! {
                #[rust_name = #push_fn]
                fn #push_fn(obj: Pin<&mut #ffi_type_name>, val: #push_arg_ty);
            });

            if !matches!(**elem_ty, TypeKind::String) {
                let t = elem_ty.to_ffi_type_name_only();
                let ret_ty = quote! { Result<Pin<&mut #t>> };

                items.push(quote! {
                    #[rust_name = #get_mut_fn]
                    fn #get_mut_fn(obj: Pin<&mut #ffi_type_name>, index: usize) -> #ret_ty;
                });
            } else {
                items.push(quote! {
                    #[rust_name = #set_fn]
                    fn #set_fn(obj: Pin<&mut #ffi_type_name>, index: usize, val: &str);
                });
            }

            if let TypeKind::Primitive(_) = **elem_ty {
                let slice_fn = format_ident!("{}_as_slice", ffi_type_str);
                let mut_slice_fn = format_ident!("{}_as_mut_slice", ffi_type_str);
                let t = elem_ty.to_ffi_type_name_only();

                items.push(quote! {
                    #[rust_name = #slice_fn]
                    fn #slice_fn(obj: &#ffi_type_name) -> &[#t];
                    #[rust_name = #mut_slice_fn]
                    fn #mut_slice_fn(obj: Pin<&mut #ffi_type_name>) -> &mut [#t];
                });
            }
        }
    }
    quote! { #(#items)* }
}

pub fn generate_map_ffi(
    map_defs: &HashSet<TypeKind>,
) -> TokenStream {
    let mut items = Vec::new();

    let mut sorted_defs: Vec<&TypeKind> = map_defs.iter().collect();
    sorted_defs.sort_by(|a, b| a.get_flat_name().cmp(&b.get_flat_name()));

    for def in sorted_defs {
        if let TypeKind::Map {
            key,
            value,
            is_val_ptr: _,
        } = def
        {
            let ffi_type_name = def.to_ffi_type_name_only();
            let ffi_type_str = def.get_flat_name();
            let len_fn = format_ident!("{}_len", ffi_type_str);
            let get_fn = format_ident!("{}_get", ffi_type_str);
            let _insert_fn = format_ident!("{}_insert", ffi_type_str);
            let new_fn = format_ident!("make_{}_new", ffi_type_str);

            items.push(quote! {
                type #ffi_type_name;
                #[rust_name = #new_fn]
                fn #new_fn() -> UniquePtr<#ffi_type_name>;
                #[rust_name = #len_fn]
                fn #len_fn(obj: &#ffi_type_name) -> usize;
            });

            let key_arg_ty = if let TypeKind::String = **key {
                quote! { &str }
            } else if let TypeKind::Primitive(_) = **key {
                key.to_ffi_type_name_only()
            } else {
                panic!("Now only primitive and string keys.");
            };

            let (val_ret_ty, lifetime) = if let TypeKind::String = **value {
                (quote! { String }, quote! {})
            } else if value.is_object_value() {
                let t = value.to_ffi_type_name_only();
                (quote! { Pin<&'a mut #t> }, quote! { <'a> })
            } else {
                let t = value.to_ffi_type_name_only();
                (quote! { #t }, quote! {})
            };

            let self_arg = if lifetime.is_empty() {
                quote! { obj: Pin<&mut #ffi_type_name> }
            } else {
                quote! { obj: Pin<&'a mut #ffi_type_name> }
            };

            items.push(quote! {
                #[rust_name = #get_fn]
                fn #get_fn #lifetime (#self_arg, key: #key_arg_ty) -> Result<#val_ret_ty>;
            });

            // key: String -> &str, Prim -> T
            // val: String -> &str, Prim -> T, Obj -> UniquePtr
            // let val_arg_ty = if let TypeKind::String = **value {
            //     quote! { &str }
            // } else if value.is_object_value() {
            //     let t = value.to_ffi_type_name_only();
            //     quote! { UniquePtr<#t> }
            // } else {
            //     value.to_ffi_type_name_only()
            // };

            // items.push(quote! {
            //     #[rust_name = #insert_fn]
            //     fn #insert_fn(obj: Pin<&mut #ffi_type_name>, key: #key_arg_ty, val: #val_arg_ty);
            // });

            let iter_ctx_name = format_ident!("{}_IterCtx", ffi_type_str);
            let iter_new_fn = format_ident!("{}_iter_new", ffi_type_str);
            let iter_key_fn = format_ident!("{}_iter_key", ffi_type_str);
            let iter_val_fn = format_ident!("{}_iter_val", ffi_type_str);
            let iter_step_fn = format_ident!("{}_iter_step", ffi_type_str);
            let iter_is_end_fn = format_ident!("{}_iter_is_end", ffi_type_str);

            let key_iter_ret = if let TypeKind::String = **key {
                quote! { String }
            } else {
                key.to_ffi_type_name_only()
            };

            let val_iter_ret = if let TypeKind::String = **value {
                quote! { String }
            } else if value.is_object_value() {
                let t = value.to_ffi_type_name_only();
                quote! { Pin<&mut #t> }
            } else {
                let t = value.to_ffi_type_name_only();
                quote! { #t }
            };

            items.push(quote! {
                type #iter_ctx_name;
                #[rust_name = #iter_new_fn]
                fn #iter_new_fn(obj: Pin<&mut #ffi_type_name>) -> UniquePtr<#iter_ctx_name>;
                #[rust_name = #iter_key_fn]
                fn #iter_key_fn(ctx: Pin<&mut #iter_ctx_name>) -> #key_iter_ret;
                #[rust_name = #iter_val_fn]
                fn #iter_val_fn(ctx: Pin<&mut #iter_ctx_name>) -> #val_iter_ret;
                #[rust_name = #iter_step_fn]
                fn #iter_step_fn(ctx: Pin<&mut #iter_ctx_name>);
                #[rust_name = #iter_is_end_fn]
                fn #iter_is_end_fn(ctx: Pin<&mut #iter_ctx_name>) -> bool;
            });
        }
    }
    quote! { #(#items)* }
}
