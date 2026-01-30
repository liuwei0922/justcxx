use crate::ast::*;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::collections::HashSet;

pub fn generate_wrapper_block(class: &ClassModel) -> TokenStream {
    let class_name = &class.name;
    let _owned_name = format_ident!("{}Owned", class_name);

    let tag_def = quote! {
        #[derive(Clone, Copy)]
        pub struct #class_name;

        impl justcxx::CppClass for #class_name {
            type FfiType = ffi::#class_name;
        }
    };
    let type_aliases = quote! {
        impl justcxx::CppTypeAliases for #class_name {
            type Owned = CppObject<'static, #class_name, justcxx::Mut, justcxx::Owned>;
            type Ref<'a> = CppObject<'a, #class_name, justcxx::Const, justcxx::Ref>;
            type Mut<'a> = CppObject<'a, #class_name, justcxx::Mut, justcxx::Ref>;
        }
    };

    let mut common_methods = Vec::new();
    let mut const_methods = Vec::new();
    let mut mut_methods = Vec::new();
    let mut static_methods = Vec::new();
    let mut aux_items = Vec::new();

    for field in &class.fields {
        let (commons, consts, muts, aux) = generate_wrapper_field(class, field);
        common_methods.extend(commons);
        const_methods.extend(consts);
        mut_methods.extend(muts);
        if let Some(a) = aux {
            aux_items.push(a);
        }
    }

    for method in &class.methods {
        let (commons, muts, static_muts, aux) = generate_wrapper_method(class, method);
        common_methods.extend(commons);
        mut_methods.extend(muts);
        static_methods.extend(static_muts);
        if let Some(a) = aux {
            aux_items.push(a);
        }
    }

    let static_impl = if !static_methods.is_empty() {
        quote! {
            impl #class_name {
                #(#static_methods)*
            }
        }
    } else {
        quote! {}
    };

    let generic_impl = quote! {
        impl<'a, M: justcxx::Mode, S: justcxx::Storage<#class_name>> CppObject<'a, #class_name, M, S> {
            #(#common_methods)*
        }
    };

    let const_impl = if !const_methods.is_empty() {
        quote! {
            impl<'a, S: justcxx::Storage<#class_name>>
                CppObject<'a, #class_name, justcxx::Const, S>
            {
                #(#const_methods)*
            }
        }
    } else {
        quote! {}
    };

    let mut_impl = quote! {
        impl<'a, S: justcxx::Storage<#class_name>> CppObject<'a, #class_name, justcxx::Mut, S> {
            #(#mut_methods)*
        }
    };

    quote! {
        #tag_def
        #type_aliases
        #static_impl
        #generic_impl
        #const_impl
        #mut_impl
        #(#aux_items)*
    }
}

pub fn generate_vec_wrappers(vec_defs: &HashSet<TypeKind>) -> TokenStream {
    let mut items: Vec<TokenStream> = Vec::new();

    let mut sorted_defs: Vec<&TypeKind> = vec_defs.iter().collect();
    sorted_defs.sort_by(|a, b| {
        let name_a = a.get_flat_name();
        let name_b = b.get_flat_name();
        name_a.cmp(&name_b)
    });

    for def in sorted_defs {
        if let TypeKind::Vector { inner, is_ptr: _ } = def {
            let elem_ident = inner.to_rust_tag();
            let ffi_type = def.to_ffi_type_name_only();
            let ffi_type_str = def.get_flat_name();
            let rust_tag = def.to_rust_tag();
            let new_fn = format_ident!("make_{}_new", ffi_type_str);

            items.push(quote! {
                impl justcxx::CppClass for #rust_tag {
                    type FfiType = ffi::#ffi_type;
                }
                impl justcxx::CppTypeAliases for #rust_tag {
                    type Owned = CppObject<'static, #rust_tag, justcxx::Mut, justcxx::Owned>;
                    type Ref<'a> = CppObject<'a, #rust_tag, justcxx::Const, justcxx::Ref>;
                    type Mut<'a> = CppObject<'a, #rust_tag, justcxx::Mut, justcxx::Ref>;
                }

                impl #rust_tag {
                    pub fn new() -> justcxx::CppOwned<#rust_tag> {
                        unsafe {
                            let ptr = ffi::#new_fn();
                            CppObject { inner: ptr, _marker: std::marker::PhantomData}
                        }
                    }
                }
            });

            match **inner {
                TypeKind::String => generate_vec_string(&ffi_type_str, &rust_tag, &mut items),
                TypeKind::Primitive(_) => {
                    generate_vec_primitive(&ffi_type_str, &elem_ident, &rust_tag, &mut items)
                }
                TypeKind::Object(_) | TypeKind::Map { .. } | TypeKind::Vector { .. } => {
                    generate_vec_obj(&ffi_type_str, &elem_ident, &rust_tag, &mut items)
                }
                _ => {}
            }
        }
    }
    quote! { #(#items)* }
}

fn generate_vec_primitive(
    type_prefix: &str,
    elem_ident: &TokenStream,
    rust_tag: &TokenStream,
    items: &mut Vec<TokenStream>,
) {
    let len_fn = format_ident!("{}_len", type_prefix);
    let get_fn = format_ident!("{}_get", type_prefix);
    let get_mut_fn = format_ident!("{}_get_mut", type_prefix);
    let push_fn = format_ident!("{}_push", type_prefix);
    let slice_fn = format_ident!("{}_as_slice", type_prefix);
    let mut_slice_fn = format_ident!("{}_as_mut_slice", type_prefix);

    let common_methods = quote! {
        pub fn len(&self) -> usize {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                ffi::#len_fn(&*ptr)
            }
        }
        pub fn get(&self, index: usize) -> Option<#elem_ident> {
            unsafe{
                let ptr = S::as_ptr(&self.inner);
                match ffi::#get_fn(&*ptr, index) {
                    Ok(n) => Some(n),
                    Err(_) => None,
                }
            }
        }
        pub fn as_slice(&self) -> &[#elem_ident] {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                ffi::#slice_fn(&*ptr)
            }
        }

        pub fn iter(&self) -> impl Iterator<Item = &#elem_ident>{
            self.as_slice().iter()
        }
    };

    let mut_methods = quote! {
        pub fn get_mut(&mut self, index: usize) -> Option<&mut #elem_ident> {
            unsafe{
                let ptr = S::as_ptr(&self.inner);
                let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                match ffi::#get_mut_fn(pin_self, index) {
                    Ok(ret) => Some(
                        ret.get_unchecked_mut()
                    ),
                    Err(_) => None,
                }
            }
        }

        pub fn push(&mut self, val: #elem_ident) {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                ffi::#push_fn(pin_self, val);
            }
        }

        pub fn as_mut_slice(&self) -> &mut [#elem_ident] {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                ffi::#mut_slice_fn(pin_self)
            }
        }
        pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut #elem_ident>{
            self.as_mut_slice().iter_mut()
        }
    };

    items.push(quote! {
        impl<'a, M: justcxx::Mode, S: justcxx::Storage<#rust_tag>> CppObject<'a, #rust_tag, M, S> {
            #common_methods
        }
        impl<'a, S: justcxx::Storage<#rust_tag>> CppObject<'a, #rust_tag, justcxx::Mut, S> {
            #mut_methods
        }
    });
}

fn generate_vec_string(type_prefix: &str, rust_tag: &TokenStream, items: &mut Vec<TokenStream>) {
    let len_fn = format_ident!("{}_len", type_prefix);
    let set_fn = format_ident!("{}_set", type_prefix);
    let get_fn = format_ident!("{}_get", type_prefix);
    let push_fn = format_ident!("{}_push", type_prefix);

    let common_methods = quote! {
        pub fn len(&self) -> usize {
            unsafe {
                let ptr = S::as_ptr(&self.inner);

                ffi::#len_fn(&*ptr)
            }
        }
        pub unsafe fn get(&self, index: usize) -> Option<String> {
            unsafe{
                let ptr = S::as_ptr(&self.inner);
                match ffi::#get_fn(&*ptr, index) {
                    Ok(s) => Some(s),
                    Err(_) => None,
                }
            }
        }

        pub fn iter(&self) -> impl Iterator<Item = String> {
             let this = self.as_ref();
            (0..this.len()).map(move |i| unsafe{this.get(i).unwrap()})
        }
    };

    let mut_methods = quote! {
        pub fn push(&mut self, val: &str) {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                ffi::#push_fn(pin_self, val);
            }
        }

        pub fn set(&mut self,index: usize, val: &str) {
            if index >= self.len() {
                panic!("index out of bounds: the len is {} but the index is {}", self.len(), index);
            }
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                ffi::#set_fn(pin_self,index, val);
            }
        }
    };

    items.push(quote! {
        impl<'a, M: justcxx::Mode, S: justcxx::Storage<#rust_tag>>
            CppObject<'a, #rust_tag, M, S>
        {
            #common_methods
        }
        impl<'a, S: justcxx::Storage<#rust_tag>>
            CppObject<'a, #rust_tag, justcxx::Mut, S>
        {
            #mut_methods
        }

    });
}

fn generate_vec_obj(
    type_prefix: &str,
    elem_ident: &TokenStream,
    rust_tag: &TokenStream,
    items: &mut Vec<TokenStream>,
) {
    let len_fn = format_ident!("{}_len", type_prefix);
    let get_fn = format_ident!("{}_get", type_prefix);
    let get_mut_fn = format_ident!("{}_get_mut", type_prefix);
    let push_fn = format_ident!("{}_push", type_prefix);
    let common_methods = quote! {
        pub fn len(&self) -> usize {
            unsafe {
                let ptr = S::as_ptr(&self.inner);

                ffi::#len_fn(&*ptr)
            }
        }
        pub fn get(&self, index: usize) -> Option<justcxx::CppRef<'a, #elem_ident>> {
            unsafe{
                let ptr = S::as_ptr(&self.inner);
                match ffi::#get_fn(&*ptr, index) {
                    Ok(ret_ref) => {
                        let ret_ptr = (ret_ref as *const _) as *mut _;
                        Some(CppObject {
                            inner: ret_ptr,
                            _marker: std::marker::PhantomData
                        })
                    },
                    Err(_) => None,
                }
            }

        }

        pub fn iter(&self) -> impl Iterator<Item = justcxx::CppRef<'a,#elem_ident>> + 'a where M: 'a {
             let this = self.as_ref();
            (0..self.len()).map(move |i| this.get(i).unwrap().as_ref())
        }
    };

    let mut_methods = quote! {
        pub fn get_mut(&mut self, index: usize) -> Option<justcxx::CppMut<'a, #elem_ident>>{
            unsafe{
                let ptr = S::as_ptr(&self.inner);
                let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                match ffi::#get_mut_fn(pin_self, index) {
                    Ok(ret_ref) => {
                        let ret_ptr = ret_ref.get_unchecked_mut() as *mut _;
                        Some(CppObject {
                            inner: ret_ptr,
                            _marker: std::marker::PhantomData
                        })
                    },
                    Err(_) => None,
                }
            }
        }

        pub fn push(&mut self, val: justcxx::CppOwned<#elem_ident>){
            unsafe{
                let ptr = S::as_ptr(&self.inner);
                let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                ffi::#push_fn(pin_self, val.inner);
            }
        }

        pub fn iter_mut(&mut self) -> impl Iterator<Item = justcxx::CppMut<'a, #elem_ident>> + 'a {
             let mut this = self.as_mut();
            (0..self.len()).map(move |i| this.get_mut(i).unwrap())
        }
    };

    items.push(quote! {
        impl<'a, M: justcxx::Mode, S: justcxx::Storage<#rust_tag>>
            CppObject<'a, #rust_tag, M, S>
        {
            #common_methods
        }
        impl<'a, S: justcxx::Storage<#rust_tag>>
            CppObject<'a, #rust_tag, justcxx::Mut, S>
        {
            #mut_methods
        }
    });
}

pub fn generate_map_wrappers(map_defs: &HashSet<TypeKind>) -> TokenStream {
    let mut items: Vec<TokenStream> = Vec::new();

    let mut sorted_defs: Vec<&TypeKind> = map_defs.iter().collect();
    sorted_defs.sort_by(|a, b| a.get_flat_name().cmp(&b.get_flat_name()));

    for def in sorted_defs {
        if let TypeKind::Map {
            key,
            value,
            is_val_ptr: _,
        } = def
        {
            let rust_tag = def.to_rust_tag();
            let ffi_type = def.to_ffi_type_name_only();
            let flat_name = def.get_flat_name();
            let new_fn = format_ident!("make_{}_new", &flat_name);

            items.push(quote! {
                impl justcxx::CppClass for #rust_tag {
                    type FfiType = ffi::#ffi_type;
                }
                impl justcxx::CppTypeAliases for #rust_tag {
                    type Owned = CppObject<'static, #rust_tag, justcxx::Mut, justcxx::Owned>;
                    type Ref<'a> = CppObject<'a, #rust_tag, justcxx::Const, justcxx::Ref>;
                    type Mut<'a> = CppObject<'a, #rust_tag, justcxx::Mut, justcxx::Ref>;
                }

                impl #rust_tag {
                    pub fn new() -> justcxx::CppOwned<#rust_tag> {
                        unsafe {
                            let ptr = ffi::#new_fn();
                            CppObject { inner: ptr, _marker: std::marker::PhantomData}
                        }
                    }
                }
            });

            generate_map_functions(&flat_name, rust_tag, &**key, &**value, &mut items);
        }
    }
    quote! { #(#items)* }
}

fn generate_map_functions(
    type_prefix: &str,
    rust_tag: TokenStream,
    key_kind: &TypeKind,
    val_kind: &TypeKind,
    items: &mut Vec<TokenStream>,
) {
    let len_fn = format_ident!("{}_len", type_prefix);
    let get_fn = format_ident!("{}_get", type_prefix);

    let iter_new_fn = format_ident!("{}_iter_new", type_prefix);
    let iter_struct_name = format_ident!("{}_Iter", type_prefix);

    let (key_arg_ty, key_pass_code) = if let TypeKind::String = key_kind {
        (quote! { &str }, quote! { key })
    } else {
        let t = key_kind.to_rust_tag();
        (quote! { #t }, quote! { key })
    };

    let val_tag = val_kind.to_rust_tag();
    let is_obj = val_kind.is_object_value();

    let (common_ret_ty, common_mapper) = if is_obj {
        (
            quote! { justcxx::CppRef<'a, #val_tag> },
            quote! {
                let ptr = ret.get_unchecked_mut() as *mut _;
                CppObject { inner: ptr, _marker: std::marker::PhantomData }
            },
        )
    } else {
        (
            if let TypeKind::String = val_kind {
                quote! { String }
            } else {
                quote! { #val_tag }
            },
            quote! { ret },
        )
    };

    // let mut_methods_impl = if is_obj {
    //     let ret_ty = quote! { justcxx::CppMut<'a, #val_tag> };
    //     let mapper = quote! {
    //         // ret 是 Pin<&mut T>
    //         let ptr = ret.get_unchecked_mut() as *mut _;
    //         CppObject { inner: ptr, _marker: std::marker::PhantomData }
    //     };

    //     quote! {
    //         pub fn get_mut(&mut self, key: #key_arg_ty) -> Option<#ret_ty> {
    //             unsafe {
    //                 let ptr = S::as_ptr(&self.inner);
    //                 let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
    //                 match ffi::#get_fn(pin_self, #key_pass_code) {
    //                     Ok(ret) => {
    //                         let val = { #mapper };
    //                         Some(val)
    //                     },
    //                     Err(_) => None,
    //                 }
    //             }
    //         }
    //     }
    // } else {
    //     quote! {}
    // };

    let common_methods = quote! {
        pub fn len(&self) -> usize {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                ffi::#len_fn(&*ptr)
            }
        }

        pub fn get(&self, key: #key_arg_ty) -> Option<#common_ret_ty> {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                let mut_ptr = ptr as *mut _;
                let pin_self = std::pin::Pin::new_unchecked(&mut *mut_ptr);

                match ffi::#get_fn(pin_self, #key_pass_code) {
                    Ok(ret) => {
                        let val = { #common_mapper };
                        Some(val)
                    },
                    Err(_) => None,
                }
            }
        }

        pub fn iter(&self) -> #iter_struct_name<'a, justcxx::Const> {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                let mut_ptr = ptr as *mut _;
                let pin_self = std::pin::Pin::new_unchecked(&mut *mut_ptr);

                let ctx = ffi::#iter_new_fn(pin_self);
                #iter_struct_name::<'a, justcxx::Const> { ctx, _marker: std::marker::PhantomData }
            }
        }
    };

    items.push(quote! {
        impl<'a, M: justcxx::Mode, S: justcxx::Storage<#rust_tag>> CppObject<'a, #rust_tag, M, S> {
            #common_methods
        }
        // impl<'a, S: justcxx::Storage<#rust_tag>> CppObject<'a, #rust_tag, justcxx::Mut, S> {
        //     #mut_methods_impl
        // }
    });

    generate_map_iter_struct(type_prefix, key_kind, val_kind, &iter_struct_name, items);
}

fn generate_map_iter_struct(
    prefix: &str,
    key_kind: &TypeKind,
    val_kind: &TypeKind,
    struct_name: &syn::Ident,
    items: &mut Vec<TokenStream>,
) {
    let iter_ctx_name = format_ident!("{}_IterCtx", prefix);
    let iter_key_fn = format_ident!("{}_iter_key", prefix);
    let iter_val_fn = format_ident!("{}_iter_val", prefix);
    let iter_step_fn = format_ident!("{}_iter_step", prefix);
    let iter_is_end_fn = format_ident!("{}_iter_is_end", prefix);

    let key_tag = key_kind.to_rust_tag();
    let val_tag = val_kind.to_rust_tag();

    let (iter_key_ty, key_mapper) = if let TypeKind::String = key_kind {
        (quote! { String }, quote! { k })
    } else {
        (quote! { #key_tag }, quote! { k })
    };

    let (iter_val_ty, val_mapper) = if val_kind.is_object_value() {
        (
            quote! { justcxx::CppRef<'a, #val_tag> },
            quote! {
                let v_ptr = v_raw.get_unchecked_mut() as *mut _;
                CppObject { inner: v_ptr, _marker: std::marker::PhantomData }
            },
        )
    } else if let TypeKind::String = val_kind {
        (quote! { String }, quote! { v_raw })
    } else {
        (quote! { #val_tag }, quote! { v_raw })
    };

    items.push(quote! {
        pub struct #struct_name<'a, M: justcxx::Mode> {
            ctx: cxx::UniquePtr<ffi::#iter_ctx_name>,
            _marker: std::marker::PhantomData<(&'a (), M)>,
        }

        impl<'a, M: justcxx::Mode> Iterator for #struct_name<'a, M> {
            type Item = (#iter_key_ty, #iter_val_ty);

            fn next(&mut self) -> Option<Self::Item> {
                unsafe {
                    if ffi::#iter_is_end_fn(self.ctx.pin_mut()) {
                        return None;
                    }
                    let k = ffi::#iter_key_fn(self.ctx.pin_mut());
                    let v_raw = ffi::#iter_val_fn(self.ctx.pin_mut());

                    let final_key = { #key_mapper };
                    let final_val = { #val_mapper };

                    ffi::#iter_step_fn(self.ctx.pin_mut());
                    Some((final_key, final_val))
                }
            }
        }
    });
}

fn gen_val_field(
    class_name: &Ident,
    field: &FieldDef,
) -> (
    Vec<TokenStream>,
    Vec<TokenStream>,
    Vec<TokenStream>,
    Option<TokenStream>,
) {
    let ty = &field.ty;
    let is_readonly = field.is_readonly;
    let field_name = &field.name;
    let get_name = field.get_ffi_get_name(class_name);
    let ret_ty = ty.to_rust_wrapper_ret_type(None);
    let get_call = quote! { ffi::#get_name(&*ptr) };

    let common = vec![quote! {
        pub fn #field_name(&self) -> #ret_ty {
            unsafe { let ptr = S::as_ptr(&self.inner); #get_call }
        }
    }];

    let mut muts = Vec::new();
    if !is_readonly {
        let set_name = field.get_wrapper_set_name();
        let ffi_set = field.get_ffi_set_name(class_name);
        let arg_ty = ty.to_rust_wrapper_arg_type();

        muts.push(quote! {
            pub fn #set_name(&mut self, val: #arg_ty) {
                unsafe {
                    let ptr = S::as_ptr(&self.inner);
                    let pin = std::pin::Pin::new_unchecked(&mut *ptr);
                    ffi::#ffi_set(pin, val);
                }
            }
        });
    }

    (common, vec![], muts, None)
}

fn gen_obj_field(
    class_name: &Ident,
    field: &FieldDef,
) -> (
    Vec<TokenStream>,
    Vec<TokenStream>,
    Vec<TokenStream>,
    Option<TokenStream>,
) {
    let ty = &field.ty;
    let is_readonly = field.is_readonly;
    let field_name = &field.name;
    let get_name = field.get_ffi_get_name(class_name);

    let ref_const = TypeKind::Reference {
        inner: Box::new(ty.clone()),
        is_mut: false,
    };
    let ret_ty_const = ref_const.to_rust_wrapper_ret_type(Some(&quote! { 'a }));

    let ref_mut = TypeKind::Reference {
        inner: Box::new(ty.clone()),
        is_mut: true,
    };
    let ret_ty_mut = ref_mut.to_rust_wrapper_ret_type(Some(&quote! { 'a }));

    match is_readonly {
        true => {
            let body = ref_const.gen_ret_conversion(quote! { ffi::#get_name(&*ptr) });
            let common = vec![quote! {
                pub fn #field_name(&self) -> #ret_ty_const {
                    unsafe { let ptr = S::as_ptr(&self.inner); #body }
                }
            }];
            (common, vec![], vec![], None)
        }

        false => {
            let body_mut = ref_mut.gen_ret_conversion(
                quote! { ffi::#get_name(std::pin::Pin::new_unchecked(&mut *ptr)) },
            );
            let consts = vec![quote! {
                pub fn #field_name(&self) -> #ret_ty_const {
                    unsafe {
                        let ptr = S::as_ptr(&self.inner);
                        #body_mut
                    }
                }
            }];

            let body_mut = ref_mut.gen_ret_conversion(
                quote! { ffi::#get_name(std::pin::Pin::new_unchecked(&mut *ptr)) },
            );
            let mut muts = vec![quote! {
                pub fn #field_name(&mut self) -> #ret_ty_mut {
                    unsafe {
                        let ptr = S::as_ptr(&self.inner);
                        #body_mut
                    }
                }
            }];

            let set_name = field.get_wrapper_set_name();
            let ffi_set = field.get_ffi_set_name(class_name);
            let arg_ty = ty.to_rust_wrapper_arg_type();
            let arg_conv = ty.gen_arg_conversion(&format_ident!("val"));

            muts.push(quote! {
                pub fn #set_name(&mut self, val: #arg_ty) {
                    unsafe {
                        let ptr = S::as_ptr(&self.inner);
                        let pin = std::pin::Pin::new_unchecked(&mut *ptr);
                        ffi::#ffi_set(pin, #arg_conv);
                    }
                }
            });

            (vec![], consts, muts, None)
        }
    }
}

fn gen_opt_field(
    class_name: &Ident,
    field: &FieldDef,
    inner: &TypeKind,
) -> (
    Vec<TokenStream>,
    Vec<TokenStream>,
    Vec<TokenStream>,
    Option<TokenStream>,
) {
    let is_readonly = field.is_readonly;
    let field_name = &field.name;
    let get_name = field.get_ffi_get_name(class_name);
    let is_obj = inner.is_object_value();

    match (is_obj, is_readonly) {
        (true, true) => {
            let ret_kind = TypeKind::Option(Box::new(TypeKind::Reference {
                inner: Box::new(inner.clone()),
                is_mut: false,
            }));
            let ret_ty = ret_kind.to_rust_wrapper_ret_type(Some(&quote! { 'a }));
            let body = ret_kind.gen_ret_conversion(quote! { ffi::#get_name(&*ptr) });

            let common = vec![quote! {
                pub fn #field_name(&self) -> #ret_ty {
                    unsafe { let ptr = S::as_ptr(&self.inner); #body }
                }
            }];
            (common, vec![], vec![], None)
        }

        (true, false) => {
            let ret_kind_const = TypeKind::Option(Box::new(TypeKind::Reference {
                inner: Box::new(inner.clone()),
                is_mut: false,
            }));
            let ret_kind_mut = TypeKind::Option(Box::new(TypeKind::Reference {
                inner: Box::new(inner.clone()),
                is_mut: true,
            }));

            let ret_ty_const = ret_kind_const.to_rust_wrapper_ret_type(Some(&quote! { 'a }));

            let ret_ty_mut = ret_kind_mut.to_rust_wrapper_ret_type(Some(&quote! { 'a }));
            let body_mut = ret_kind_mut.gen_ret_conversion(
                quote! { ffi::#get_name(std::pin::Pin::new_unchecked(&mut *ptr)) },
            );
            let consts = vec![quote! {
                pub fn #field_name(&self) -> #ret_ty_const {
                    unsafe { let ptr = S::as_ptr(&self.inner); #body_mut }
                }
            }];

            let mut muts = vec![quote! {
                pub fn #field_name(&mut self) -> #ret_ty_mut {
                    unsafe { let ptr = S::as_ptr(&self.inner); #body_mut }
                }
            }];

            let set_name = field.get_wrapper_set_name();
            let ffi_set = field.get_ffi_set_name(class_name);
            let arg_ty = TypeKind::new_unique_ptr(inner.clone()).to_rust_wrapper_arg_type();
            let arg_conv = inner.gen_arg_conversion(&format_ident!("val"));

            muts.push(quote! {
                pub fn #set_name(&mut self, val: #arg_ty) {
                    unsafe {
                        let ptr = S::as_ptr(&self.inner);
                        let pin = std::pin::Pin::new_unchecked(&mut *ptr);
                        ffi::#ffi_set(pin, #arg_conv);
                    }
                }
            });

            (vec![], consts, muts, None)
        }

        (false, _) => {
            let ret_ty = field.ty.to_rust_wrapper_ret_type(Some(&quote! { 'a }));

            let body = field
                .ty
                .gen_ret_conversion(quote! { ffi::#get_name(&*ptr) });
            let common = vec![quote! {
                pub fn #field_name(&self) -> #ret_ty {
                    unsafe { let ptr = S::as_ptr(&self.inner); #body }
                }
            }];

            let muts = if !is_readonly {
                let set_name = field.get_wrapper_set_name();
                let ffi_set = field.get_ffi_set_name(class_name);
                let arg_ty = inner.to_rust_wrapper_arg_type();
                let arg_conv = inner.gen_arg_conversion(&format_ident!("val"));

                vec![quote! {
                    pub fn #set_name(&mut self, val: #arg_ty) {
                        unsafe {
                            let ptr = S::as_ptr(&self.inner);
                            let pin = std::pin::Pin::new_unchecked(&mut *ptr);
                            ffi::#ffi_set(pin, #arg_conv);
                        }
                    }
                }]
            } else {
                vec![]
            };

            (common, vec![], muts, None)
        }
    }
}

fn generate_wrapper_field(
    class: &ClassModel,
    field: &FieldDef,
) -> (
    Vec<TokenStream>,
    Vec<TokenStream>,
    Vec<TokenStream>,
    Option<TokenStream>,
) {
    let class_name = &class.name;

    match &field.ty {
        TypeKind::Primitive(_) | TypeKind::String => gen_val_field(class_name, field),

        TypeKind::Object(_) | TypeKind::Map { .. } | TypeKind::Vector { .. } => {
            gen_obj_field(class_name, field)
        }

        TypeKind::Option(inner) => gen_opt_field(class_name, field, inner),

        _ => (vec![], vec![], vec![], None),
    }
}

fn generate_wrapper_method(
    class: &ClassModel,
    method: &MethodDef,
) -> (
    Vec<TokenStream>,
    Vec<TokenStream>,
    Vec<TokenStream>,
    Option<TokenStream>,
) {
    let class_name = &class.name;
    let mut common_methods = Vec::new();
    let mut mut_methods = Vec::new();
    let mut static_methods = Vec::new();
    let mut aux_items = None;

    match method {
        MethodDef::Ctor(ctor) => {
            let name = &ctor.rust_name;
            let ffi_unique_name = format_ident!("make_{}_{}", class_name, ctor.rust_name);

            let (args_def, args_call): (Vec<_>, Vec<_>) = ctor
                .args
                .iter()
                .map(|arg| {
                    let n = &arg.name;
                    let ty = arg.ty.to_rust_wrapper_arg_type();
                    let call = arg.ty.gen_arg_conversion(n);
                    (quote! { #n: #ty }, call)
                })
                .unzip();

            static_methods.push(quote! {
                pub fn #name(#(#args_def),*) -> justcxx::CppOwned<#class_name> {
                    unsafe {
                        let unique_ptr = ffi::#ffi_unique_name(#(#args_call),*);
                        CppObject {
                            inner: unique_ptr,
                            _marker: std::marker::PhantomData,
                        }
                    }
                }
            });
        }

        MethodDef::Iter(iter) => {
            generate_iterator_method(class_name, iter, &mut common_methods, &mut aux_items);
        }

        MethodDef::Method(func) => {
            generate_normal_method(
                class_name,
                func,
                &mut common_methods,
                &mut mut_methods,
                &mut static_methods,
            );
        }
    }
    (common_methods, mut_methods, static_methods, aux_items)
}

fn generate_iterator_method(
    class_name: &Ident,
    iter: &IterDef,
    common_methods: &mut Vec<TokenStream>,
    aux_items: &mut Option<TokenStream>,
) {
    let method_name = &iter.rust_name;
    let yield_ty_kind = &iter.yield_ty;

    let names = IterNames::new(class_name, method_name);

    let aux_struct = generate_iter_aux_struct(&names, yield_ty_kind, iter.is_iter_mut);

    if let Some(existing) = aux_items {
        *aux_items = Some(quote! { #existing #aux_struct });
    } else {
        *aux_items = Some(aux_struct);
    }

    let item_ty_tokens = yield_ty_kind.to_rust_wrapper_ret_type(Some(&quote! {'a}));

    let wrapper_method =
        generate_iter_wrapper_method(method_name, &names, &item_ty_tokens, iter.is_iter_mut);

    common_methods.push(wrapper_method);
}

fn generate_iter_aux_struct(names: &IterNames, yield_ty: &TypeKind, is_owned: bool) -> TokenStream {
    let struct_name = &names.struct_name;
    let ctx_name = &names.ctx_name;
    let next_fn = &names.next_fn;

    let item_ty = yield_ty.to_rust_wrapper_ret_type(Some(&quote! {'a}));

    let conversion = yield_ty.gen_ret_conversion(quote! {ret_raw});

    let body = if is_owned {
        quote! {
            let ret_raw = ffi::#next_fn(self.ctx.pin_mut());
            if ret_raw.is_null() {
                None
            } else {
                Some({ #conversion })
            }
        }
    } else {
        quote! {
             let ret_raw = ffi::#next_fn(self.ctx.pin_mut());
             #conversion
        }
    };

    quote! {
        #[allow(non_camel_case_types)]
        pub struct #struct_name<'a, M: justcxx::Mode> {
            ctx: cxx::UniquePtr<ffi::#ctx_name>,
            _marker: std::marker::PhantomData<(&'a (), M)>,
        }

        impl<'a, M: justcxx::Mode> Iterator for #struct_name<'a, M> {
            type Item = #item_ty;

            fn next(&mut self) -> Option<Self::Item> {
                unsafe {
                    #body
                }
            }
        }
    }
}

fn generate_iter_wrapper_method(
    method_name: &Ident,
    names: &IterNames,
    yield_ty_tokens: &TokenStream,
    is_iter_mut: bool,
) -> TokenStream {
    let struct_name = &names.struct_name;
    let new_fn = &names.new_fn;

    let receiver = if is_iter_mut {
        quote! { &mut self }
    } else {
        quote! { &self }
    };
    quote! {
        pub fn #method_name(#receiver) -> impl Iterator<Item = #yield_ty_tokens> + 'a where M: 'a {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                let ctx = ffi::#new_fn(pin_self);
                #struct_name::<'a, M> {
                    ctx,
                    _marker: std::marker::PhantomData,
                }
            }
        }
    }
}

fn generate_normal_method(
    class_name: &Ident,
    func: &FnDef,
    common_methods: &mut Vec<TokenStream>,
    mut_methods: &mut Vec<TokenStream>,
    static_methods: &mut Vec<TokenStream>,
) {
    let method_name = &func.rust_name;
    let ffi_name = format_ident!("{}_method_{}", class_name, func.rust_name);

    let (args_decl, args_call): (Vec<_>, Vec<_>) = func
        .args
        .iter()
        .map(|arg| {
            let n = &arg.name;
            let ty = arg.ty.to_rust_wrapper_arg_type();
            let call = arg.ty.gen_arg_conversion(n);
            (quote! { #n: #ty }, call)
        })
        .unzip();

    let prepare_ptr = quote! {
        let ptr = S::as_ptr(&self.inner);
    };

    let ret_decl = if let Some(ret) = &func.ret_ty {
        let ty = ret.to_rust_wrapper_ret_type(Some(&quote! {'a}));
        quote! { -> #ty }
    } else {
        quote! {}
    };

    match func.kind {
        MethodKind::Static => {
            let ffi_call_expr = quote! { ffi::#ffi_name(#(#args_call),*) };
            let body = if let Some(ret) = &func.ret_ty {
                ret.gen_ret_conversion(ffi_call_expr)
            } else {
                quote! { #ffi_call_expr; }
            };

            static_methods.push(quote! {
                pub fn #method_name(#(#args_decl),*) #ret_decl {
                    unsafe { #body }
                }
            });
        }

        MethodKind::Const => {
            let ffi_call_expr = quote! { ffi::#ffi_name(&*ptr, #(#args_call),*) };
            let body = if let Some(ret) = &func.ret_ty {
                ret.gen_ret_conversion(ffi_call_expr)
            } else {
                quote! { #ffi_call_expr; }
            };

            common_methods.push(quote! {
                pub fn #method_name(&self, #(#args_decl),*) #ret_decl {
                    unsafe {
                        #prepare_ptr
                        #body
                    }
                }
            });
        }

        MethodKind::Mutable => {
            let ffi_call_expr = quote! { ffi::#ffi_name(pin_self, #(#args_call),*) };
            let body = if let Some(ret) = &func.ret_ty {
                ret.gen_ret_conversion(ffi_call_expr)
            } else {
                quote! { #ffi_call_expr; }
            };

            mut_methods.push(quote! {
                pub fn #method_name(&mut self, #(#args_decl),*) #ret_decl {
                    unsafe {
                        #prepare_ptr
                        let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                        #body
                    }
                }
            });
        }
    }
}
