use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::boxed::Box;
use std::collections::{HashMap, HashSet};
use syn::{Attribute, Ident, LitStr};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TypeKind {
    Primitive(String),
    String,
    Object(String),
    Vector {
        inner: Box<TypeKind>,
        is_ptr: bool,
    },
    Map {
        key: Box<TypeKind>,
        value: Box<TypeKind>,
        is_val_ptr: bool,
    },
    Option(Box<TypeKind>),
    Result(Box<TypeKind>),
    Reference {
        inner: Box<TypeKind>,
        is_mut: bool,
    },
    UniquePtr(Box<TypeKind>),
    Slice(Box<TypeKind>),
}

impl TypeKind {
    pub fn new_ref(inner: TypeKind, is_mut: bool) -> Self {
        TypeKind::Reference {
            inner: Box::new(inner),
            is_mut,
        }
    }

    pub fn new_mut_ref(inner: TypeKind) -> Self {
        Self::new_ref(inner, true)
    }

    pub fn new_const_ref(inner: TypeKind) -> Self {
        Self::new_ref(inner, false)
    }

    pub fn new_unique_ptr(inner: TypeKind) -> Self {
        TypeKind::UniquePtr(Box::new(inner))
    }

    pub fn new_result(inner: TypeKind) -> Self {
        TypeKind::Result(Box::new(inner))
    }

    pub fn to_ffi_type(&self, is_return: bool) -> TokenStream {
        match self {
            TypeKind::Primitive(s) => {
                let i = format_ident!("{}", s);
                quote! { #i }
            }

            TypeKind::String => {
                if is_return {
                    quote! { String }
                } else {
                    quote! { &str }
                }
            }

            TypeKind::Object(s) => {
                let i = format_ident!("{}", s);
                quote! { UniquePtr<#i> }
            }

            TypeKind::Vector { .. } | TypeKind::Map { .. } => {
                let name = self.get_flat_name();
                let ident = format_ident!("{}", name);
                quote! { UniquePtr<#ident> }
            }

            TypeKind::Reference { inner, is_mut } => {
                if let TypeKind::Slice(elem) = &**inner {
                    let t = elem.to_ffi_type_name_only();
                    return if *is_mut {
                        quote! { &mut [#t] }
                    } else {
                        quote! { &[#t] }
                    };
                }

                if let TypeKind::String = &**inner {
                    if !*is_mut {
                        return quote! { &str };
                    }
                }

                let t = inner.to_ffi_type_name_only();
                if *is_mut {
                    quote! { Pin<&mut #t> }
                } else {
                    quote! { &#t }
                }
            }

            TypeKind::Option(inner) => {
                if !is_return {
                    panic!("Option type is not supported as function argument in FFI");
                }

                if inner.is_object_value() {
                    inner.to_ffi_type(true)
                } else {
                    let t = inner.to_ffi_type(true);
                    quote! { Result<#t> }
                }
            }

            TypeKind::UniquePtr(inner) => {
                let inner_ty = inner.to_ffi_type_name_only();
                quote! { UniquePtr<#inner_ty> }
            }

            TypeKind::Slice(inner) => {
                let inner_ty = inner.to_ffi_type_name_only();
                quote! { [#inner_ty] }
            }

            TypeKind::Result(inner) => {
                if !is_return {
                    panic!("Result type is not supported as function argument in FFI");
                }
                let inner_ty = inner.to_ffi_type(is_return);
                quote! { Result<#inner_ty> }
            }
        }
    }

    pub fn to_ffi_type_name_only(&self) -> TokenStream {
        match self {
            TypeKind::Primitive(s) => {
                let i = format_ident!("{}", s);
                quote! {#i}
            }
            TypeKind::Object(s) => {
                let i = format_ident!("{}", s);
                quote! {#i}
            }
            TypeKind::String => quote! { CxxString },
            TypeKind::Vector { .. } | TypeKind::Map { .. } => {
                let name = self.get_flat_name();
                let ident = format_ident!("{}", name);
                quote! { #ident }
            }
            _ => panic!("Unexpected type for name extraction: {:?}", self),
        }
    }

    pub fn is_object_value(&self) -> bool {
        matches!(
            self,
            TypeKind::Object(_) | TypeKind::Vector { .. } | TypeKind::Map { .. }
        )
    }

    pub fn get_flat_name(&self) -> String {
        match self {
            TypeKind::Primitive(s) | TypeKind::Object(s) => s.clone(),
            TypeKind::String => "String".to_string(),

            TypeKind::Vector { inner, is_ptr } => {
                let prefix = if *is_ptr { "Vec_Ptr" } else { "Vec" };
                format!("{}_{}", prefix, inner.get_flat_name())
            }

            TypeKind::Map {
                key,
                value,
                is_val_ptr,
            } => {
                let prefix = if *is_val_ptr { "Map_Ptr" } else { "Map" };
                format!(
                    "{}_{}_{}",
                    prefix,
                    key.get_flat_name(),
                    value.get_flat_name()
                )
            }

            TypeKind::Reference { inner, .. }
            | TypeKind::UniquePtr(inner)
            | TypeKind::Option(inner)
            | TypeKind::Result(inner) => inner.get_flat_name(),

            TypeKind::Slice { .. } => panic!("Slice cannot be used in flat name"),
        }
    }

    pub fn to_rust_wrapper_arg_type(&self) -> TokenStream {
        match self {
            TypeKind::Reference { inner, is_mut } => {
                if let TypeKind::Slice(slice_inner) = &**inner {
                    let t = slice_inner.to_rust_wrapper_arg_type();
                    return if *is_mut {
                        quote! { &mut [#t] }
                    } else {
                        quote! { &[#t] }
                    };
                }

                if let TypeKind::String = **inner {
                    return quote! { &str };
                }

                if inner.is_object_value() {
                    let tag = inner.to_rust_tag();
                    if *is_mut {
                        quote! { &mut justcxx::CppMut<'_, #tag> }
                    } else {
                        quote! { justcxx::CppRef<'_, #tag> }
                    }
                } else {
                    let t = inner.to_rust_wrapper_arg_type();
                    if *is_mut {
                        quote! { &mut #t }
                    } else {
                        quote! { &#t }
                    }
                }
            }

            TypeKind::Object(_) | TypeKind::Vector { .. } | TypeKind::Map { .. } => {
                let tag = self.to_rust_tag();
                quote! { justcxx::CppOwned<#tag> }
            }

            TypeKind::Primitive(s) => {
                let i = format_ident!("{}", s);
                quote! {#i}
            }
            TypeKind::String => quote! { &str },

            _ => panic!("Unsupported arg type: {:?}", self),
        }
    }

    pub fn to_rust_wrapper_ret_type(&self, lifetime: Option<&TokenStream>) -> TokenStream {
        let lt = lifetime.map(|l| quote! {#l}).unwrap_or(quote! {'_});

        match self {
            TypeKind::Reference { inner, is_mut } => {
                if let TypeKind::Slice(slice_inner) = &**inner {
                    let t = slice_inner.to_rust_wrapper_arg_type();
                    return if *is_mut {
                        quote! { &mut [#t] }
                    } else {
                        quote! { &[#t] }
                    };
                }

                if inner.is_object_value() {
                    let tag = inner.to_rust_tag();
                    if *is_mut {
                        quote! { justcxx::CppMut<#lt, #tag> }
                    } else {
                        quote! { justcxx::CppRef<#lt, #tag> }
                    }
                } else {
                    let t = inner.to_rust_wrapper_ret_type(None);
                    if *is_mut {
                        quote! { &mut #t }
                    } else {
                        quote! { &#t }
                    }
                }
            }

            TypeKind::Option(inner) => {
                let t = inner.to_rust_wrapper_ret_type(Some(&lt));
                quote! { Option<#t> }
            }

            TypeKind::Object(_) | TypeKind::Vector { .. } | TypeKind::Map { .. } => {
                let tag = self.to_rust_tag();
                quote! { justcxx::CppOwned<#tag> }
            }

            TypeKind::String => quote! { String },
            TypeKind::Primitive(s) => {
                let i = format_ident!("{}", s);
                quote! {#i}
            }

            _ => panic!("Unsupported return type: {:?}", self),
        }
    }

    pub fn gen_arg_conversion(&self, arg_name: &Ident) -> TokenStream {
        match self {
            TypeKind::Reference { inner, is_mut } => {
                if inner.is_object_value() {
                    if *is_mut {
                        quote! { std::pin::Pin::new_unchecked(&mut *#arg_name.as_ptr()) }
                    } else {
                        quote! { &*#arg_name.as_ptr() }
                    }
                } else {
                    quote! { #arg_name }
                }
            }
            TypeKind::Object(_) | TypeKind::Vector { .. } | TypeKind::Map { .. } => {
                quote! { #arg_name.inner }
            }
            _ => quote! { #arg_name },
        }
    }

    pub fn gen_ret_conversion(&self, ffi_expr: TokenStream) -> TokenStream {
        match self {
            TypeKind::Option(inner) => {
                let inner_conv = inner.gen_ret_conversion(quote! {val});
                if inner.is_object_value() {
                    quote! {
                        let val = #ffi_expr;
                        if val.is_null() { None } else { Some({ #inner_conv }) }
                    }
                } else {
                    quote! {
                        match #ffi_expr {
                            Ok(val) => Some({ #inner_conv }),
                            Err(_) => None
                        }
                    }
                }
            }

            TypeKind::Reference { inner, is_mut } if inner.is_object_value() => {
                let ptr_extract = if *is_mut {
                    quote! { ffi_ret.get_unchecked_mut() as *mut _ }
                } else {
                    quote! { (ffi_ret as *const _) as *mut _ }
                };
                quote! {
                    let ffi_ret = #ffi_expr;
                    let ret_ptr = #ptr_extract;
                    CppObject { inner: ret_ptr, _marker: std::marker::PhantomData }
                }
            }

            TypeKind::Object(_) | TypeKind::Vector { .. } | TypeKind::Map { .. } => {
                quote! {
                    let unique_ptr = #ffi_expr;
                    CppObject { inner: unique_ptr, _marker: std::marker::PhantomData }
                }
            }

            _ => ffi_expr,
        }
    }

    pub fn to_rust_tag(&self) -> TokenStream {
        match self {
            TypeKind::Object(s) | TypeKind::Primitive(s) => {
                let i = format_ident!("{}", s);
                quote! { #i }
            }
            
            TypeKind::Vector { inner, is_ptr } => {
                let inner_tag = inner.to_rust_tag();

                if *is_ptr {
                    quote! { CppVectorPtr<#inner_tag> }
                } else {
                    quote! { CppVector<#inner_tag> }
                }
            }

            TypeKind::Map {
                key,
                value,
                is_val_ptr,
            } => {
                let key_tag = key.to_rust_tag();
                let val_tag = value.to_rust_tag();

                if *is_val_ptr {
                    quote! { CppMapPtr<#key_tag, #val_tag> }
                } else {
                    quote! { CppMap<#key_tag, #val_tag> }
                }
            }
            TypeKind::String => quote! { String },

            _ => panic!("Type {:?} cannot be used as a Tag", self),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FieldDef {
    pub name: Ident,
    pub ty: TypeKind,
    pub is_protected: bool,
    pub is_readonly: bool,
}

impl FieldDef {
    pub fn get_ffi_get_name(&self, class_name: &Ident) -> Ident {
        format_ident!("{}_get_{}", class_name, self.name)
    }

    pub fn get_ffi_set_name(&self, class_name: &Ident) -> Ident {
        format_ident!("{}_set_{}", class_name, self.name)
    }

    pub fn get_wrapper_set_name(&self) -> Ident {
        format_ident!("set_{}", self.name)
    }
}

#[derive(Clone, Debug)]
pub struct Arg {
    pub name: Ident,
    pub ty: TypeKind,
}

#[derive(Clone, Debug)]
pub struct FnDef {
    pub rust_name: Ident,
    pub cpp_name: String,
    pub args: Vec<Arg>,
    pub ret_ty: Option<TypeKind>,
    pub kind: MethodKind,
    pub is_protected: bool,
}

#[derive(Clone, Debug)]
pub struct IterDef {
    pub rust_name: Ident,
    pub yield_ty: TypeKind,
    pub cpp_name: String,
    pub is_iter_mut: bool,
}

#[derive(Clone, Debug)]
pub struct CtorDef {
    pub rust_name: Ident,
    pub args: Vec<Arg>,
    pub cpp_name: Ident,
    pub is_user_defined: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum MethodKind {
    Static,
    Const,
    Mutable,
}

#[derive(Clone, Debug)]
pub enum MethodDef {
    Ctor(CtorDef),
    Iter(IterDef),
    Method(FnDef),
}

#[derive(Debug)]
pub struct StructDef {
    pub attrs: Vec<Attribute>,
    pub name: Ident,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug)]
pub struct ImplDef {
    pub target: Ident,
    pub methods: Vec<MethodDef>,
}

#[derive(Debug)]
pub enum BindItem {
    Include(LitStr),
    Struct(StructDef),
    Impl(ImplDef),
}

#[derive(Debug)]
pub struct BindInput {
    pub items: Vec<BindItem>,
}

#[derive(Debug)]
pub struct ClassModel {
    pub name: Ident,
    pub fields: Vec<FieldDef>,
    pub methods: Vec<MethodDef>,
    pub needs_exposer: bool,
}

impl ClassModel {
    pub fn new(name: Ident) -> Self {
        Self {
            name,
            fields: Vec::new(),
            methods: Vec::new(),
            needs_exposer: false,
        }
    }
    pub fn get_cxx_name(&self) -> Ident {
        if self.needs_exposer {
            format_ident!("{}_Exposer", self.name)
        } else {
            format_ident!("{}", self.name)
        }
    }
}

#[derive(Clone, Debug)]
pub struct IterNames {
    pub struct_name: Ident,
    pub ctx_name: Ident,
    pub new_fn: Ident,
    pub next_fn: Ident,
}

impl IterNames {
    pub fn new(class_name: &Ident, method_name: &Ident) -> Self {
        Self {
            struct_name: format_ident!("{}_{}_Iter", class_name, method_name),
            ctx_name: format_ident!("{}_{}_IterCtx", class_name, method_name),
            new_fn: format_ident!("{}_{}_iter_new", class_name, method_name),
            next_fn: format_ident!("{}_{}_iter_next", class_name, method_name),
        }
    }
}

pub struct BindContext {
    pub includes: Vec<syn::LitStr>,
    pub models: HashMap<String, ClassModel>,
    pub class_names_order: Vec<String>,
    pub vec_defs: HashSet<TypeKind>,
    pub map_defs: HashSet<TypeKind>,
}
