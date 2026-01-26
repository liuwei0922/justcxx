use syn::{Attribute, Ident, LitStr, Type};
use quote::format_ident;
use std::collections::{HashMap, HashSet};

pub struct BindInput {
    pub items: Vec<BindItem>,
}

pub enum BindItem {
    Include(LitStr),
    Struct(StructDef),
    Impl(ImplDef),
}

pub struct StructDef {
    pub attrs: Vec<Attribute>,
    pub name: Ident,
    pub fields: Vec<FieldDef>,
}

#[derive(Clone)]
pub struct FieldDef {
    pub name: Ident,
    pub ty: Type,
    pub kind: FieldKind,
    pub is_protected: bool,
    pub is_readonly: bool,
}

#[derive(Clone)]
pub enum FieldKind {
    Val,
    Obj,
    OptObj { ty: Type },
    OptVal { ty: Type },
    Map { key: Type, value: Type, is_value_ptr: bool },
    Vec { element: Type, is_ptr: bool },
}

pub struct ImplDef {
    pub target: Ident,
    pub methods: Vec<MethodDef>,
}

#[derive(Clone)]
pub enum MethodDef {
    Ctor(CtorDef),
    Iter(IterDef),
    Method(FnDef),
}

#[derive(Clone, PartialEq)]
pub enum MethodKind {
    Static,
    Const,
    Mutable,
}

#[derive(Clone)]
pub struct CtorDef {
    pub rust_name: Ident,
    pub args: Vec<Arg>,
    pub cpp_name: Ident,
    pub is_user_defined: bool,
}

#[derive(Clone)]
pub struct IterDef {
    pub rust_name: Ident,
    pub yield_ty: Ident,
    pub is_owned: bool,
    pub is_iter_mut: bool,
    pub is_item_mut: bool,
    pub cpp_name: String,
}

#[derive(Clone)]
pub struct FnDef {
    pub rust_name: Ident,
    pub cpp_name: String,
    pub args: Vec<Arg>,
    pub ret_ty: Option<Type>,
    pub kind: MethodKind,
    pub is_protected: bool,
}

#[derive(Clone)]
pub struct Arg {
    pub name: Ident,
    pub ty: Type,
}

pub struct ClassModel {
    pub name: Ident,
    pub is_owned: bool,
    pub fields: Vec<FieldDef>,
    pub methods: Vec<MethodDef>,
    pub needs_exposer: bool,
}

impl ClassModel {
    pub fn new(name: Ident, is_owned: bool) -> Self {
        Self {
            name,
            is_owned,
            fields: Vec::new(),
            methods: Vec::new(),
            needs_exposer: false,
        }
    }
}

#[derive(Clone)]
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

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct VecDef {
    pub elem_type: String,
    pub is_ptr: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MapDef {
    pub key_type: String,
    pub value_type: String,
    pub is_value_ptr: bool,
}

pub struct BindContext {
    pub includes: Vec<syn::LitStr>,
    pub models: HashMap<String, ClassModel>,
    pub class_names_order: Vec<String>, 
    pub vec_defs: HashSet<VecDef>, 
    pub map_defs: HashSet<MapDef>, 
}