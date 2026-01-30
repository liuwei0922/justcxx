use crate::ast::*;
use syn::Result;

pub fn extract_unique_ptr_info(ty: &syn::Type) -> Result<(TypeKind, bool)> {
    let kind = TypeKind::try_from(ty)?;
    if let TypeKind::UniquePtr(inner) = kind {
        Ok((*inner, true))
    } else {
        Ok((kind, false))
    }
}

pub fn get_single_arg(args: &syn::AngleBracketedGenericArguments) -> Result<&syn::Type> {
    if args.args.len() != 1 {
        return Err(syn::Error::new_spanned(
            args,
            "Expected exactly 1 generic argument",
        ));
    }
    if let syn::GenericArgument::Type(ty) = &args.args[0] {
        Ok(ty)
    } else {
        Err(syn::Error::new_spanned(
            &args.args[0],
            "Expected type argument",
        ))
    }
}

pub fn get_double_args(
    args: &syn::AngleBracketedGenericArguments,
) -> Result<(&syn::Type, &syn::Type)> {
    if args.args.len() != 2 {
        return Err(syn::Error::new_spanned(
            args,
            "Expected exactly 2 generic arguments",
        ));
    }
    match (&args.args[0], &args.args[1]) {
        (syn::GenericArgument::Type(k), syn::GenericArgument::Type(v)) => Ok((k, v)),
        _ => Err(syn::Error::new_spanned(args, "Expected type arguments")),
    }
}

pub fn is_primitive(s: &str) -> bool {
    matches!(
        s,
        "i8" | "u8"
            | "i16"
            | "u16"
            | "i32"
            | "u32"
            | "i64"
            | "u64"
            | "f32"
            | "f64"
            | "bool"
            | "usize"
            | "isize"
    )
}
