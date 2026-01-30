use crate::ast::*;
use crate::utils::*;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Attribute, Ident, LitStr, Result, Token, Type, braced, parenthesized};

impl TryFrom<&syn::Type> for TypeKind {
    type Error = syn::Error;

    fn try_from(ty: &syn::Type) -> Result<Self> {
        match ty {
            syn::Type::Path(p) => parse_type_path(p),
            syn::Type::Reference(r) => parse_type_reference(r),
            syn::Type::Slice(s) => parse_type_slice(s),
            _ => Err(syn::Error::new_spanned(ty, "Unsupported type syntax")),
        }
    }
}

fn parse_type_path(p: &syn::TypePath) -> Result<TypeKind> {
    let segment = p
        .path
        .segments
        .last()
        .ok_or_else(|| syn::Error::new_spanned(p, "Empty type path"))?;
    let ident = segment.ident.to_string();

    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
        return match ident.as_str() {
            "Vec" => parse_vec(args),
            "Map" => parse_map(args),
            "Option" => parse_option(args),
            "UniquePtr" => parse_unique_ptr(args),
            _ => Err(syn::Error::new_spanned(p, "Unknown generic type")),
        };
    }

    match ident.as_str() {
        "String" => Ok(TypeKind::String),
        s if is_primitive(s) => Ok(TypeKind::Primitive(s.to_string())),
        s => Ok(TypeKind::Object(s.to_string())),
    }
}

fn parse_type_reference(r: &syn::TypeReference) -> Result<TypeKind> {
    let inner = TypeKind::try_from(&*r.elem)?;
    Ok(TypeKind::Reference {
        inner: Box::new(inner),
        is_mut: r.mutability.is_some(),
    })
}

fn parse_vec(args: &syn::AngleBracketedGenericArguments) -> Result<TypeKind> {
    let inner_ty = get_single_arg(args)?;
    let (real_inner, is_ptr) = extract_unique_ptr_info(inner_ty)?;
    Ok(TypeKind::Vector {
        inner: Box::new(real_inner),
        is_ptr,
    })
}

fn parse_map(args: &syn::AngleBracketedGenericArguments) -> Result<TypeKind> {
    let (k, v) = get_double_args(args)?;
    let (real_val, is_val_ptr) = extract_unique_ptr_info(&v)?;
    let key_kind = TypeKind::try_from(k)?;
    Ok(TypeKind::Map {
        key: Box::new(key_kind),
        value: Box::new(real_val),
        is_val_ptr,
    })
}

fn parse_option(args: &syn::AngleBracketedGenericArguments) -> Result<TypeKind> {
    let inner_ty = get_single_arg(args)?;
    let inner_ty_kind = TypeKind::try_from(inner_ty)?;
    Ok(TypeKind::Option(Box::new(inner_ty_kind)))
}

fn parse_unique_ptr(args: &syn::AngleBracketedGenericArguments) -> Result<TypeKind> {
    let inner_ty = get_single_arg(args)?;
    let inner_ty_kind = TypeKind::try_from(inner_ty)?;
    Ok(TypeKind::UniquePtr(Box::new(inner_ty_kind)))
}

fn parse_type_slice(s: &syn::TypeSlice) -> Result<TypeKind> {
    let inner_kind = TypeKind::try_from(&*s.elem)?;
    Ok(TypeKind::Slice(Box::new(inner_kind)))
}

pub fn extract_dsl(source: &str) -> Option<&str> {
    let start_idx = source.find("bind!")?;
    let open_brace_idx = source[start_idx..].find('{')? + start_idx;

    let mut depth = 0;
    let content_start = open_brace_idx + 1;

    for (rel_idx, b) in source[content_start..].bytes().enumerate() {
        match b {
            b'{' => depth += 1,
            b'}' if depth == 0 => return Some(&source[content_start..content_start + rel_idx]),
            b'}' => depth -= 1,
            _ => {}
        }
    }
    None
}

mod kw {
    syn::custom_keyword!(include);
}

impl Parse for BindInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut items = Vec::new();
        while !input.is_empty() {
            if input.peek(kw::include) {
                input.parse::<kw::include>()?;
                input.parse::<Token![!]>()?;
                let content;
                parenthesized!(content in input);
                let path: LitStr = content.parse()?;
                input.parse::<Token![;]>()?;
                items.push(BindItem::Include(path));
            } else if input.peek(Token![struct]) || input.peek(Token![#]) {
                items.push(BindItem::Struct(input.parse()?));
            } else if input.peek(Token![impl]) {
                items.push(BindItem::Impl(input.parse()?));
            } else {
                return Err(input.error("Expected include!, struct, or impl"));
            }
        }
        Ok(BindInput { items })
    }
}

impl Parse for StructDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        input.parse::<Token![struct]>()?;
        let name: Ident = input.parse()?;

        let content;
        braced!(content in input);

        let fields_parsed: Punctuated<FieldDef, Token![,]> =
            content.parse_terminated(FieldDef::parse, Token![,])?;

        Ok(StructDef {
            attrs,
            name,
            fields: fields_parsed.into_iter().collect(),
        })
    }
}

impl Parse for FieldDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let is_protected = attrs.iter().any(|attr| attr.path().is_ident("protected"));
        let is_readonly = attrs.iter().any(|attr| attr.path().is_ident("readonly"));

        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;

        let raw_ty: Type = input.parse()?;
        let ty = TypeKind::try_from(&raw_ty)?;

        Ok(FieldDef {
            name,
            ty,
            is_protected,
            is_readonly,
        })
    }
}

impl Parse for ImplDef {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<Token![impl]>()?;
        let target: Ident = input.parse()?;

        let content;
        braced!(content in input);

        let mut methods = Vec::new();
        while !content.is_empty() {
            methods.push(content.parse()?);
        }

        Ok(ImplDef { target, methods })
    }
}

fn parse_args_and_kind(input: ParseStream) -> Result<(Vec<Arg>, MethodKind)> {
    let mut kind = MethodKind::Static;

    if input.peek(Token![&]) {
        input.parse::<Token![&]>()?;
        let is_mut = input.parse::<Option<Token![mut]>>()?.is_some();
        input.parse::<Token![self]>()?;
        kind = if is_mut {
            MethodKind::Mutable
        } else {
            MethodKind::Const
        };

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
    } else if input.peek(Token![self]) || input.peek(Token![mut]) {
        return Err(
            input.error("Pass-by-value `self` is not supported. Use `&self` or `&mut self`")
        );
    }

    let args_list = input.parse_terminated(
        |stream| {
            let name: Ident = stream.parse()?;
            stream.parse::<Token![:]>()?;
            let raw_ty: Type = stream.parse()?;
            let ty = TypeKind::try_from(&raw_ty)?;
            Ok(Arg { name, ty })
        },
        Token![,],
    )?;

    Ok((args_list.into_iter().collect(), kind))
}

impl Parse for MethodDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;

        let iter_ty_kind = parse_iter_attr(&attrs)?;
        let is_protected = attrs.iter().any(|attr| attr.path().is_ident("protected"));

        input.parse::<Token![fn]>()?;
        let rust_name: Ident = input.parse()?;

        let args_content;
        parenthesized!(args_content in input);
        let (args, kind) = parse_args_and_kind(&args_content)?;

        if let Some(yield_ty) = iter_ty_kind {
            return IterDef::parse_rest(input, rust_name, args, kind, yield_ty)
                .map(MethodDef::Iter);
        }

        let ret_ty_kind = if input.peek(Token![->]) {
            input.parse::<Token![->]>()?;
            let ty: Type = input.parse()?;
            Some(TypeKind::try_from(&ty)?)
        } else {
            None
        };

        let is_return_self = matches!(&ret_ty_kind, Some(TypeKind::Object(s)) if s == "Self");
        if is_return_self {
            return CtorDef::parse_rest(input, rust_name, args, kind).map(MethodDef::Ctor);
        }

        FnDef::parse_rest_with_ret(input, rust_name, args, kind, is_protected, ret_ty_kind)
            .map(MethodDef::Method)
    }
}

impl IterDef {
    fn parse_rest(
        input: ParseStream,
        rust_name: Ident,
        args: Vec<Arg>,
        kind: MethodKind,
        yield_ty: TypeKind,
    ) -> Result<Self> {
        if !args.is_empty() {
            return Err(input.error("Iterator methods cannot take arguments"));
        }

        let is_iter_mut = match kind {
            MethodKind::Mutable => true,
            MethodKind::Const => false,
            MethodKind::Static => return Err(input.error("Iterator must take &self or &mut self")),
        };

        let is_item_mut = matches!(yield_ty, TypeKind::Reference { is_mut: true, .. });

        if is_item_mut && !is_iter_mut {
            return Err(
                input.error("Iterator producing mutable references (&mut T) must take &mut self")
            );
        }

        let cpp_name = parse_cpp_mapping_str(input, &rust_name)?;
        input.parse::<Token![;]>()?;

        Ok(IterDef {
            rust_name,
            yield_ty,
            is_iter_mut,
            cpp_name,
        })
    }
}

impl CtorDef {
    fn parse_rest(
        input: ParseStream,
        rust_name: Ident,
        args: Vec<Arg>,
        kind: MethodKind,
    ) -> Result<Self> {
        if kind != MethodKind::Static {
            return Err(input.error("Constructors must be static (no self)"));
        }

        let (cpp_name, is_user) = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            (input.parse::<Ident>()?, true)
        } else {
            (rust_name.clone(), false)
        };
        input.parse::<Token![;]>()?;

        Ok(CtorDef {
            rust_name,
            args,
            cpp_name,
            is_user_defined: is_user,
        })
    }
}

impl FnDef {
    fn parse_rest_with_ret(
        input: ParseStream,
        rust_name: Ident,
        args: Vec<Arg>,
        kind: MethodKind,
        is_protected: bool,
        ret_ty: Option<TypeKind>,
    ) -> Result<Self> {
        let cpp_name = parse_cpp_mapping_str(input, &rust_name)?;
        input.parse::<Token![;]>()?;

        Ok(FnDef {
            rust_name,
            cpp_name,
            args,
            ret_ty,
            kind,
            is_protected,
        })
    }
}

fn parse_iter_attr(attrs: &[Attribute]) -> Result<Option<TypeKind>> {
    for attr in attrs {
        if !attr.path().is_ident("iter") {
            continue;
        }
        let nested = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Meta, Token![,]>::parse_terminated,
        )?;

        for meta in nested {
            if let syn::Meta::NameValue(nv) = meta
                && nv.path.is_ident("Item")
            {
                let ty: syn::Type = syn::parse2(nv.value.to_token_stream())?;
                return TypeKind::try_from(&ty).map(Some);
            }
        }
        return Err(syn::Error::new_spanned(attr, "Missing Item in #[iter]"));
    }
    Ok(None)
}

fn parse_cpp_mapping_str(input: ParseStream, default: &Ident) -> Result<String> {
    if input.peek(Token![=]) {
        input.parse::<Token![=]>()?;
        if input.peek(LitStr) {
            Ok(input.parse::<LitStr>()?.value())
        } else {
            Ok(input.parse::<Ident>()?.to_string())
        }
    } else {
        Ok(default.to_string())
    }
}
