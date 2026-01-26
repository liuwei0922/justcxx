use syn::{Type, TypePath, TypeReference};
use std::collections::HashMap;
use crate::ast::ClassModel;

pub fn extract_option_inner(ty: &Type) -> Option<Type> {
    if let Type::Path(type_path) = ty
        && type_path.path.segments.len() == 1
        && type_path.path.segments[0].ident == "Option"
        && let syn::PathArguments::AngleBracketed(args) = &type_path.path.segments[0].arguments
        && args.args.len() == 1
        && let syn::GenericArgument::Type(inner_ty) = &args.args[0]
    {
        return Some(inner_ty.clone());
    }
    None
}

pub fn extract_unique_ptr_inner(ty: &Type) -> Option<Type> {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "UniquePtr"
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && args.args.len() == 1
        && let syn::GenericArgument::Type(inner) = &args.args[0]
    {
        return Some(inner.clone());
    }
    None
}

#[derive(Debug, Clone)]
pub struct DefinedRefInfo {
    pub elem: Type,
    pub is_mut: bool,
    pub type_name: String,
}

pub fn extract_defined_ref_info(
    ty: &Type,
    models: &HashMap<String, ClassModel>,
) -> Option<DefinedRefInfo> {
    if let Type::Reference(TypeReference {
        mutability, elem, ..
    }) = ty
        && let Some(name) = get_type_ident_name(&*elem)
        && models.contains_key(&name)
    {
        return Some(DefinedRefInfo {
            elem: *(*elem).clone(),
            is_mut: mutability.is_some(),
            type_name: name,
        });
    }
    None
}

pub fn get_type_ident_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(TypePath { path, .. }) => path.segments.last().map(|s| s.ident.to_string()),
        Type::Reference(TypeReference { elem, .. }) => get_type_ident_name(elem),
        _ => None,
    }
}