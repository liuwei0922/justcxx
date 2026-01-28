use crate::ast::*;
use crate::utils::*;
use std::collections::{HashMap, HashSet};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Attribute, Ident, LitStr, Result, Token, Type, braced, parenthesized};
use quote::format_ident;

pub fn extract_dsl(source: &str) -> Option<String> {
    let start_keyword = "bind!";
    let start_idx = source.find(start_keyword)?;
    let open_brace_idx = source[start_idx..].find('{')? + start_idx;
    let mut depth = 0;
    let mut end_idx = 0;
    let mut found = false;
    for (i, c) in source[open_brace_idx..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = open_brace_idx + i;
                    found = true;
                    break;
                }
            }
            _ => {}
        }
    }
    if found {
        Some(source[open_brace_idx + 1..end_idx].to_string())
    } else {
        None
    }
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
        let ty: Type = input.parse()?;

        let kind = deduce_container_kind(&ty);

        Ok(FieldDef {
            name,
            ty,
            kind,
            is_protected,
            is_readonly,
        })
    }
}

fn deduce_container_kind(ty: &Type) -> FieldKind {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
    {
        match segment.ident.to_string().as_str() {
            "Map" if args.args.len() == 2 => {
                if let (syn::GenericArgument::Type(k), syn::GenericArgument::Type(v)) =
                    (&args.args[0], &args.args[1])
                {
                    if let Some(real_value) = extract_unique_ptr_inner(v) {
                        return FieldKind::Map {
                            key: k.clone(),
                            value: real_value,
                            is_value_ptr: true,
                        };
                    } else {
                        return FieldKind::Map {
                            key: k.clone(),
                            value: v.clone(),
                            is_value_ptr: false,
                        };
                    }
                }
            }
            "Vec" if args.args.len() == 1 => {
                if let syn::GenericArgument::Type(inner) = &args.args[0] {
                    if let Some(real_element) = extract_unique_ptr_inner(inner) {
                        return FieldKind::Vec {
                            element: real_element,
                            is_ptr: true,
                        };
                    } else {
                        return FieldKind::Vec {
                            element: inner.clone(),
                            is_ptr: false,
                        };
                    }
                }
            }
            "Option" if args.args.len() == 1 => {
                if let syn::GenericArgument::Type(inner) = &args.args[0] {
                    return FieldKind::OptVal { ty: inner.clone() };
                }
            }
            _ => {}
        }
    }
    FieldKind::Val
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
            let ty: Type = stream.parse()?;
            Ok(Arg { name, ty })
        },
        Token![,],
    )?;

    Ok((args_list.into_iter().collect(), kind))
}

struct IterAttrConfig {
    yield_ty: Ident,
    is_owned: bool,
    item_is_mut: bool,
}

impl Parse for MethodDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        
        let iter_config = parse_iter_attr(&attrs)?;
        let is_protected = attrs.iter().any(|attr| attr.path().is_ident("protected"));

        input.parse::<Token![fn]>()?;
        let rust_name: Ident = input.parse()?;
        
        let args_content;
        parenthesized!(args_content in input);
        let (args, kind) = parse_args_and_kind(&args_content)?;

        if let Some(config) = iter_config {
            return IterDef::parse_rest(input, rust_name, args, kind, config).map(MethodDef::Iter);
        }

        let ret_ty = if input.peek(Token![->]) {
            input.parse::<Token![->]>()?;
            Some(input.parse::<Type>()?)
        } else {
            None
        };

        let is_return_self = if let Some(syn::Type::Path(p)) = &ret_ty {
            p.path.is_ident("Self")
        } else {
            false
        };

        if is_return_self {
            return CtorDef::parse_rest(input, rust_name, args, kind).map(MethodDef::Ctor);
        }

        FnDef::parse_rest_with_ret(input, rust_name, args, kind, is_protected, ret_ty)
            .map(MethodDef::Method)
    }
}

impl IterDef {
    fn parse_rest(
        input: ParseStream,
        rust_name: Ident,
        args: Vec<Arg>,
        kind: MethodKind,
        config: IterAttrConfig,
    ) -> Result<Self> {
        if !args.is_empty() {
            return Err(input.error("Iterator methods cannot take arguments"));
        }

        let is_iter_mut = match kind {
            MethodKind::Mutable => true,
            MethodKind::Const => false,
            MethodKind::Static => return Err(input.error("Iterator must take &self or &mut self")),
        };

        if config.item_is_mut && !is_iter_mut {
            return Err(
                input.error("Iterator producing mutable references (&mut T) must take &mut self")
            );
        }

        let cpp_name = parse_cpp_mapping_str(input, &rust_name)?;
        input.parse::<Token![;]>()?;

        Ok(IterDef {
            rust_name,
            yield_ty: config.yield_ty,
            is_owned: config.is_owned,
            is_iter_mut,
            is_item_mut: config.item_is_mut,
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
        ret_ty: Option<Type>,
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

fn parse_iter_attr(attrs: &[Attribute]) -> Result<Option<IterAttrConfig>> {
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
                return parse_iter_item_type(&nv.value);
            }
        }
        return Err(syn::Error::new_spanned(attr, "Missing Item in #[iter]"));
    }
    Ok(None)
}

fn parse_iter_item_type(expr: &syn::Expr) -> Result<Option<IterAttrConfig>> {
    match expr {
        syn::Expr::Path(p) => {
            if let Some(ident) = p.path.get_ident() {
                return Ok(Some(IterAttrConfig {
                    yield_ty: ident.clone(),
                    is_owned: true,
                    item_is_mut: false,
                }));
            }
        }
        syn::Expr::Reference(r) => {
            if let syn::Expr::Path(p) = &*r.expr
                && let Some(ident) = p.path.get_ident()
            {
                return Ok(Some(IterAttrConfig {
                    yield_ty: ident.clone(),
                    is_owned: false,
                    item_is_mut: r.mutability.is_some(),
                }));
            }
        }
        _ => {}
    }
    Err(syn::Error::new_spanned(
        expr,
        "Invalid Item type. Expected T, &T or &mut T",
    ))
}

pub fn preprocess(input: &BindInput) -> BindContext {
    let mut includes = Vec::new();
    let mut models: HashMap<String, ClassModel> = HashMap::new();
    let mut class_names_order = Vec::new();

    for item in &input.items {
        match item {
            BindItem::Include(path) => includes.push(path.clone()),
            BindItem::Struct(def) => {
                let name = def.name.clone();
                let name_str = name.to_string();
                let mut model = ClassModel::new(name);

                if def.fields.iter().any(|f| f.is_protected) {
                    model.needs_exposer = true;
                }
                model.fields = def.fields.clone();
                models.insert(name_str.clone(), model);
                class_names_order.push(name_str);
            }
            BindItem::Impl(def) => {
                let target = def.target.to_string();
                if let Some(model) = models.get_mut(&target) {
                    if def.methods.iter().any(|m| match m {
                        MethodDef::Method(f) => f.is_protected,
                        _ => false,
                    }) {
                        model.needs_exposer = true;
                    }
                    model.methods.extend(def.methods.clone());
                } else {
                    panic!("Impl block found for undefined struct '{}'", target);
                }
            }
        }
    }

    check_field_kinds(&mut models);
    inject_default_ctors(&mut models);

    let vec_defs = collect_vec_defs(&models);
    let map_defs = collect_map_defs(&models);

    BindContext {
        includes,
        models,
        class_names_order,
        vec_defs,
        map_defs,
    }
}

fn check_field_kinds(models: &mut HashMap<String, ClassModel>) {
    let known_classes: std::collections::HashSet<String> = models.keys().cloned().collect();

    for model in models.values_mut() {
        for field in &mut model.fields {
            if let FieldKind::Val = field.kind
                && let Some(type_name) = get_type_ident_name(&field.ty)
                && known_classes.contains(&type_name)
            {
                field.kind = FieldKind::Obj;
            }
            if let FieldKind::OptVal { ty } = &field.kind
                && let Some(type_name) = get_type_ident_name(ty)
                && known_classes.contains(&type_name)
            {
                field.kind = FieldKind::OptObj { ty: ty.clone() };
            }
        }
    }
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

fn collect_vec_defs(models: &HashMap<String, ClassModel>) -> HashSet<VecDef> {
    let mut vec_defs = HashSet::new();

    for model in models.values() {
        for field in &model.fields {
            if let FieldKind::Vec { element, is_ptr } = &field.kind
                && let Some(name) = get_type_ident_name(element)
            {
                vec_defs.insert(VecDef {
                    elem_type: name,
                    is_ptr: *is_ptr,
                });
            }
        }
    }
    vec_defs
}

fn collect_map_defs(models: &HashMap<String, ClassModel>) -> HashSet<MapDef> {
    let mut map_defs = HashSet::new();

    for model in models.values() {
        for field in &model.fields {
            if let FieldKind::Map { key, value,is_value_ptr } = &field.kind
                && let (Some(key_name), Some(value_name)) =
                    (get_type_ident_name(key), get_type_ident_name(value))
            {
                map_defs.insert(MapDef {
                    key_type: key_name,
                    value_type: value_name,
                    is_value_ptr: *is_value_ptr,
                });
            }
        }
    }
    map_defs
}


fn inject_default_ctors(models: &mut HashMap<String, ClassModel>) {
    for (_name, model) in models.iter_mut() {
        let has_ctor = model.methods.iter().any(|m| matches!(m, MethodDef::Ctor(_)));
        
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