use crate::ast::*;
use crate::ffi::{generate_ffi_block, generate_map_ffi, generate_vec_ffi};
use crate::utils::*;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::collections::{HashMap, HashSet};
use syn::Type;

pub fn generate_rust(bind_context: &BindContext) -> TokenStream {
    let mut extern_cpp_items = Vec::new();
    let mut rust_wrapper_items = Vec::new();

    for class_name_str in &bind_context.class_names_order {
        let class = bind_context.models.get(class_name_str).unwrap();

        extern_cpp_items.push(generate_ffi_block(class, &bind_context.models));

        rust_wrapper_items.push(generate_wrapper_block(class, &bind_context.models));
    }

    extern_cpp_items.push(generate_vec_ffi(
        &bind_context.vec_defs,
        &bind_context.models,
    ));
    rust_wrapper_items.push(generate_vec_wrappers(
        &bind_context.vec_defs,
        &bind_context.models,
    ));
    extern_cpp_items.push(generate_map_ffi(
        &bind_context.map_defs,
        &bind_context.models,
    ));
    rust_wrapper_items.push(generate_map_wrappers(
        &bind_context.map_defs,
        &bind_context.models,
    ));

    let includes = &bind_context.includes;

    quote! {
        use cxx;

        #[repr(transparent)]
        pub struct CppObject<'a, T: justcxx::CppClass, M: justcxx::Mode, S: justcxx::Storage<T>> {
            pub inner: S::Inner,
            pub _marker: std::marker::PhantomData<(&'a (), M)>,
        }

        impl<'a, T: justcxx::CppClass, M: justcxx::Mode> Clone for CppObject<'a, T, M, justcxx::Ref> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<'a, T: justcxx::CppClass, M: justcxx::Mode> Copy for CppObject<'a, T, M, justcxx::Ref> {}

        impl<'a, T, M: justcxx::Mode, S: justcxx::Storage<T>> CppObject<'a, T, M, S>
        where
            T: justcxx::CppClass + justcxx::CppTypeAliases,
        {
            pub fn as_ptr(&self) -> *mut T::FfiType {
                unsafe { S::as_ptr(&self.inner) }
            }

            pub fn as_ref(&self) -> CppObject<'a, T, justcxx::Const, justcxx::Ref> {
                unsafe {
                    CppObject {
                        inner: self.as_ptr(),
                        _marker: std::marker::PhantomData,
                    }
                }
            }
        }

        impl<'a, T, S: justcxx::Storage<T>> CppObject<'a, T, justcxx::Mut, S>
        where
            T: justcxx::CppClass + justcxx::CppTypeAliases,
        {
            pub fn as_mut(&self) -> CppObject<'a, T, justcxx::Mut, justcxx::Ref> {
                unsafe {
                    CppObject {
                        inner: self.as_ptr(),
                        _marker: std::marker::PhantomData,
                    }
                }
            }
        }

        impl<'a, T: justcxx::CppClass, M: justcxx::Mode, S: justcxx::Storage<T>> std::fmt::Debug
            for CppObject<'a, T, M, S>
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                unsafe {
                    let ptr = S::as_ptr(&self.inner);
                    write!(f, "CppObject({:p})", ptr)
                }
            }
        }

        impl<'a, T: justcxx::CppClass, M: justcxx::Mode, S: justcxx::Storage<T>> PartialEq
            for CppObject<'a, T, M, S>
        {
            fn eq(&self, other: &Self) -> bool {
                unsafe {
                    let ptr1 = S::as_ptr(&self.inner);
                    let ptr2 = S::as_ptr(&other.inner);
                    ptr1 == ptr2
                }
            }
        }
        #[repr(transparent)]
        pub struct CppVector<T>(pub std::marker::PhantomData<T>);
        #[repr(transparent)]
        pub struct CppVectorPtr<T>(pub std::marker::PhantomData<T>);

        #[repr(transparent)]
        pub struct CppMap<K,V>(pub std::marker::PhantomData<(K,V)>);
        #[repr(transparent)]
        pub struct CppMapPtr<K,V>(pub std::marker::PhantomData<(K,V)>);

        #[cxx::bridge]
        mod ffi {
            unsafe extern "C++" {
                #(include!(#includes);)*
                include!("shim.hh");
                #(#extern_cpp_items)*
            }
        }
        #(#rust_wrapper_items)*
    }
}

fn generate_wrapper_block(class: &ClassModel, models: &HashMap<String, ClassModel>) -> TokenStream {
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
    let mut mut_methods = Vec::new();
    let mut static_methods = Vec::new();
    let mut aux_items = Vec::new();

    for field in &class.fields {
        let (commons, muts, aux) = generate_wrapper_field(class, field, models);
        common_methods.extend(commons);
        mut_methods.extend(muts);
        if let Some(a) = aux {
            aux_items.push(a);
        }
    }

    for method in &class.methods {
        let (commons, muts, static_muts, aux) = generate_wrapper_method(class, method, models);
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
        #mut_impl
        #(#aux_items)*
    }
}

fn generate_vec_wrappers(
    vec_defs: &HashSet<VecDef>,
    models: &HashMap<String, ClassModel>,
) -> TokenStream {
    let mut items = Vec::new();

    let mut sorted_defs: Vec<&VecDef> = vec_defs.iter().collect();
    sorted_defs.sort_by(|a, b| a.elem_type.cmp(&b.elem_type).then(a.is_ptr.cmp(&b.is_ptr)));

    for def in sorted_defs {
        let elem_str = &def.elem_type;
        let elem_ident = format_ident!("{}", elem_str);

        let (ffi_type, rust_tag) = if def.is_ptr {
            let ffi = format_ident!("Vec_Ptr_{}", elem_str);
            (quote! { ffi::#ffi }, quote! { CppVectorPtr<#elem_ident> })
        } else {
            let ffi = format_ident!("Vec_{}", elem_str);
            (quote! { ffi::#ffi }, quote! { CppVector<#elem_ident> })
        };

        items.push(quote! {
            impl justcxx::CppClass for #rust_tag {
                type FfiType = #ffi_type;
            }
            impl justcxx::CppTypeAliases for #rust_tag {
                type Owned = ();
                type Ref<'a> = CppObject<'a, #rust_tag, justcxx::Const, justcxx::Ref>;
                type Mut<'a> = CppObject<'a, #rust_tag, justcxx::Mut, justcxx::Ref>;
            }
        });

        let type_prefix = if def.is_ptr {
            format!("Vec_Ptr_{}", elem_str)
        } else {
            format!("Vec_{}", elem_str)
        };
        if elem_str == "String" {
            generate_vec_string(&type_prefix, &rust_tag, &mut items);
        } else if models.contains_key(elem_str) {
            generate_vec_obj(&type_prefix, &elem_ident, &rust_tag, &mut items);
        } else {
            generate_vec_primitive(&type_prefix, &elem_ident, &rust_tag, &mut items)
        }
    }
    quote! { #(#items)* }
}

fn generate_vec_primitive(
    type_prefix: &str,
    elem_ident: &Ident,
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
    elem_ident: &Ident,
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
        pub fn get(&self, index: usize) -> Option<CppObject<'a, #elem_ident, justcxx::Const, justcxx::Ref>> {
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

        pub fn iter(&self) -> impl Iterator<Item = CppObject<'a, #elem_ident, justcxx::Const, justcxx::Ref>> + 'a where M: 'a {
             let this = self.as_ref();
            (0..self.len()).map(move |i| this.get(i).unwrap().as_ref())
        }
    };

    let mut_methods = quote! {
        pub fn get_mut(&mut self, index: usize) -> Option<CppObject<'a, #elem_ident, justcxx::Mut, justcxx::Ref>>{
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

        pub fn iter_mut(&mut self) -> impl Iterator<Item = CppObject<'a, #elem_ident, justcxx::Mut, justcxx::Ref>> + 'a {
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

fn generate_map_wrappers(
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
        let key_str = &def.key_type;
        let val_str = &def.value_type;
        let is_ptr = def.is_value_ptr;

        let key_ident = format_ident!("{}", key_str);
        let val_ident = format_ident!("{}", val_str);

        let (type_prefix, rust_tag) = if is_ptr {
            (
                format!("Map_Ptr_{}_{}", key_str, val_str),
                quote! { CppMapPtr<#key_ident, #val_ident> },
            )
        } else {
            (
                format!("Map_{}_{}", key_str, val_str),
                quote! { CppMap<#key_ident, #val_ident> },
            )
        };

        let ffi_type = format_ident!("{}", type_prefix);
        items.push(quote! {
            impl justcxx::CppClass for #rust_tag {
                type FfiType = ffi::#ffi_type;
            }
            impl justcxx::CppTypeAliases for #rust_tag {
                type Owned = ();
                type Ref<'a> = CppObject<'a, #rust_tag, justcxx::Const, justcxx::Ref>;
                type Mut<'a> = CppObject<'a, #rust_tag, justcxx::Mut, justcxx::Ref>;
            }
        });

        generate_map_common(
            &type_prefix,
            &rust_tag,
            key_str,
            val_str,
            models,
            &mut items,
        );
    }

    quote! { #(#items)* }
}

struct MapTypeConfig {
    val_ret_ty: TokenStream,
    get_val_mapper: TokenStream,
    iter_val_mapper: TokenStream,
    is_obj: bool,
}

fn compute_map_config(val_ty_str: &str, models: &HashMap<String, ClassModel>) -> MapTypeConfig {
    let val_ident = format_ident!("{}", val_ty_str);

    if models.contains_key(val_ty_str) {
        MapTypeConfig {
            val_ret_ty: quote! { CppObject<'a, #val_ident, M, justcxx::Ref> },
            get_val_mapper: quote! {
                let ret_ptr = ret_pin.get_unchecked_mut() as *mut _;
                CppObject { inner: ret_ptr, _marker: std::marker::PhantomData }
            },
            iter_val_mapper: quote! {
                let v_ptr = v_raw.get_unchecked_mut() as *mut _;
                CppObject { inner: v_ptr, _marker: std::marker::PhantomData }
            },
            is_obj: true,
        }
    } else if val_ty_str == "String" {
        MapTypeConfig {
            val_ret_ty: quote! { String },
            get_val_mapper: quote! { ret_val },
            iter_val_mapper: quote! { v_raw },
            is_obj: false,
        }
    } else {
        MapTypeConfig {
            val_ret_ty: quote! { #val_ident },
            get_val_mapper: quote! { ret_val },
            iter_val_mapper: quote! { v_raw },
            is_obj: false,
        }
    }
}

fn generate_map_common(
    prefix: &str,
    rust_tag: &TokenStream,
    key_ty_str: &str,
    val_ty_str: &str,
    models: &HashMap<String, ClassModel>,
    items: &mut Vec<TokenStream>,
) {
    let config = compute_map_config(val_ty_str, models);
    let key_ident = format_ident!("{}", key_ty_str);
    let _val_ident = format_ident!("{}", val_ty_str);

    let len_fn = format_ident!("{}_len", prefix);
    let get_fn = format_ident!("{}_get", prefix);
    let iter_new_fn = format_ident!("{}_iter_new", prefix);
    let iter_struct_name = format_ident!("{}_Iter", prefix);

    let rust_key_ty = if key_ty_str == "String" {
        quote! { String }
    } else {
        quote! { #key_ident }
    };
    let key_arg_ty = if key_ty_str == "String" {
        quote! { &str }
    } else {
        quote! { #key_ident }
    };

    let get_impl = generate_map_get_impl(&get_fn, &config);

    let val_ret_ty = &config.val_ret_ty;
    let common_methods = quote! {
        pub fn len(&self) -> usize {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                ffi::#len_fn(&*ptr)
            }
        }

        pub fn get(&self, key: #key_arg_ty) -> Option<#val_ret_ty> {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                #get_impl
            }
        }

        pub fn iter(&self) -> #iter_struct_name<'a, justcxx::Const> {
            unsafe {
                let ptr = S::as_ptr(&self.inner);
                let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                let ctx = ffi::#iter_new_fn(pin_self);
                #iter_struct_name::<'a,justcxx::Const> { ctx, _marker: std::marker::PhantomData }
            }
        }
    };

    let iter_impl = generate_map_iter_struct(prefix, &rust_key_ty, &config, &iter_struct_name);

    items.push(iter_impl);
    items.push(quote! {
        impl<'a, M: justcxx::Mode, S: justcxx::Storage<#rust_tag>> CppObject<'a, #rust_tag, M, S> {
            #common_methods
        }
    });
}

fn generate_map_get_impl(get_fn: &Ident, config: &MapTypeConfig) -> TokenStream {
    let mapper = &config.get_val_mapper;
    if config.is_obj {
        quote! {
            match ffi::#get_fn(pin_self, key) {
                Ok(ret_pin) => { let val = { #mapper }; Some(val) },
                Err(_) => None
            }
        }
    } else {
        quote! {
            match ffi::#get_fn(pin_self, key) {
                Ok(ret_val) => { let val = { #mapper }; Some(val) },
                Err(_) => None
            }
        }
    }
}

fn generate_map_iter_struct(
    prefix: &str,
    key_ty: &TokenStream,
    config: &MapTypeConfig,
    struct_name: &Ident,
) -> TokenStream {
    let iter_ctx_name = format_ident!("{}_IterCtx", prefix);
    let iter_key_fn = format_ident!("{}_iter_key", prefix);
    let iter_val_fn = format_ident!("{}_iter_val", prefix);
    let iter_step_fn = format_ident!("{}_iter_step", prefix);
    let iter_is_end_fn = format_ident!("{}_iter_is_end", prefix);

    let val_ty = &config.val_ret_ty;
    let mapper = &config.iter_val_mapper;

    quote! {
        pub struct #struct_name<'a, M: justcxx::Mode> {
            ctx: cxx::UniquePtr<ffi::#iter_ctx_name>,
            _marker: std::marker::PhantomData<(&'a (), M)>,
        }

        impl<'a, M: justcxx::Mode> Iterator for #struct_name<'a, M> {
            type Item = (#key_ty, #val_ty);

            fn next(&mut self) -> Option<Self::Item> {
                unsafe {
                    if ffi::#iter_is_end_fn(self.ctx.pin_mut()) {
                        return None;
                    }
                    let k = ffi::#iter_key_fn(self.ctx.pin_mut());
                    let v_raw = ffi::#iter_val_fn(self.ctx.pin_mut());
                    let v = { #mapper };

                    ffi::#iter_step_fn(self.ctx.pin_mut());
                    Some((k, v))
                }
            }
        }
    }
}

fn generate_wrapper_field(
    class: &ClassModel,
    field: &FieldDef,
    _models: &HashMap<String, ClassModel>,
) -> (Vec<TokenStream>, Vec<TokenStream>, Option<TokenStream>) {
    let class_name = &class.name;
    let field_name = &field.name; // Ident

    let ffi_get_name = format_ident!("{}_get_{}", class_name, field_name);
    let ffi_set_name = format_ident!("{}_set_{}", class_name, field_name);

    let mut common_methods = Vec::new();
    let mut mut_methods = Vec::new();

    let prepare_ptr = quote! {
        let ptr = S::as_ptr(&self.inner);
    };

    match &field.kind {
        FieldKind::Val => {
            let ty = &field.ty;
            common_methods.push(quote! {
                pub fn #field_name(&self) -> #ty {
                    unsafe {
                        #prepare_ptr
                        ffi::#ffi_get_name(&*ptr)
                    }
                }
            });

            if !field.is_readonly {
                let set_name = format_ident!("set_{}", field_name);
                mut_methods.push(quote! {
                    pub fn #set_name(&mut self, value: #ty) {
                        unsafe {
                            #prepare_ptr
                            let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                            ffi::#ffi_set_name(pin_self, value)
                        }
                    }
                });
            }
        }
        FieldKind::Obj => generate_field_obj(
            field_name,
            &field.ty,
            field.is_readonly,
            &prepare_ptr,
            &ffi_get_name,
            &ffi_set_name,
            &mut common_methods,
            &mut mut_methods,
        ),
        FieldKind::OptVal { ty } => {
            common_methods.push(quote! {
                pub fn #field_name(&self) -> Option<#ty> {
                    unsafe {
                        #prepare_ptr
                        match ffi::#ffi_get_name(&*ptr) {
                            Ok(ret) => Some(ret),
                            Err(_) => None
                        }
                    }
                }
            });
        }
        FieldKind::OptObj { ty } => {
            common_methods.push(quote! {
                pub fn #field_name(&self) -> Option<CppObject<'a, #ty, M, justcxx::Ref>> {
                    unsafe {
                        #prepare_ptr
                        let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                        match ffi::#ffi_get_name(pin_self) {
                            Ok(ret_pin) => {
                                let ret_ptr = ret_pin.get_unchecked_mut() as *mut _;
                                Some(CppObject {
                                    inner: ret_ptr,
                                    _marker: std::marker::PhantomData
                                })
                            },
                            Err(_) => None
                        }
                    }
                }
            });
        }
        FieldKind::Vec { element, is_ptr } => {
            if let Some(elem_name) = get_type_ident_name(element) {
                let elem_ident = format_ident!("{}", elem_name);
                let vec_tag = if *is_ptr {
                    quote! { CppVectorPtr<#elem_ident> }
                } else {
                    quote! { CppVector<#elem_ident> }
                };

                common_methods.push(quote! {
                    pub fn #field_name(&self) -> CppObject<'a, #vec_tag, M, justcxx::Ref> {
                        unsafe {
                            #prepare_ptr
                            let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);

                            let ret_pin = ffi::#ffi_get_name(pin_self);
                            let ret_ptr = ret_pin.get_unchecked_mut() as *mut _;

                            CppObject {
                                inner: ret_ptr,
                                _marker: std::marker::PhantomData
                            }
                        }
                    }
                });
            }
        }
        FieldKind::Map {
            key,
            value,
            is_value_ptr,
        } => {
            let key_name = get_type_ident_name(key);
            let val_name = get_type_ident_name(value);

            if let (Some(k_str), Some(v_str)) = (key_name, val_name) {
                let k_ident = format_ident!("{}", k_str);
                let v_ident = format_ident!("{}", v_str);
                let map_tag = if *is_value_ptr {
                    quote! { CppMapPtr<#k_ident, #v_ident> }
                } else {
                    quote! { CppMap<#k_ident, #v_ident> }
                };
                common_methods.push(quote! {
                    pub fn #field_name(&self) -> CppObject<'a, #map_tag, M, justcxx::Ref> {
                        unsafe {
                            #prepare_ptr
                            let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);

                            let ret_pin = ffi::#ffi_get_name(pin_self);
                            let ret_ptr = ret_pin.get_unchecked_mut() as *mut _;

                            CppObject {
                                inner: ret_ptr,
                                _marker: std::marker::PhantomData
                            }
                        }
                    }
                });
            }
        }
    }

    (common_methods, mut_methods, None)
}

fn generate_field_obj(
    field_name: &Ident,
    ty: &Type,
    is_readonly: bool,
    prepare_ptr: &TokenStream,
    ffi_get_name: &Ident,
    ffi_set_name: &Ident,
    common_methods: &mut Vec<TokenStream>,
    mut_methods: &mut Vec<TokenStream>,
) {
    if is_readonly {
        common_methods.push(quote! {
            pub fn #field_name(&self) -> CppObject<'a, #ty, justcxx::Const, justcxx::Ref> {
                unsafe {
                    #prepare_ptr
                    let ret_ref = ffi::#ffi_get_name(&*ptr);
                    let ret_ptr = (ret_ref as *const _) as *mut _;

                    CppObject {
                        inner: ret_ptr,
                        _marker: std::marker::PhantomData
                    }
                }
            }
        });
    } else {
        common_methods.push(quote! {
            pub fn #field_name(&self) -> CppObject<'a, #ty, M, justcxx::Ref> {
                unsafe {
                    #prepare_ptr
                    let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                    let ret_pin = ffi::#ffi_get_name(pin_self);
                    let ret_ptr = ret_pin.get_unchecked_mut() as *mut _;

                    CppObject {
                        inner: ret_ptr,
                        _marker: std::marker::PhantomData
                    }
                }
            }
        });

        let set_name = format_ident!("set_{}", field_name);
        mut_methods.push(quote! {
            pub fn #set_name(&mut self, val: CppObject<'static, #ty, justcxx::Mut, justcxx::Owned>) {
                unsafe {
                    #prepare_ptr
                    let pin_self = std::pin::Pin::new_unchecked(&mut *ptr);
                    ffi::#ffi_set_name(pin_self, val.inner)
                }
            }
        });
    }
}

type MethodGenResult = (
    Vec<TokenStream>,
    Vec<TokenStream>,
    Vec<TokenStream>,
    Option<TokenStream>,
);

fn generate_wrapper_method(
    class: &ClassModel,
    method: &MethodDef,
    models: &HashMap<String, ClassModel>,
) -> MethodGenResult {
    let class_name = &class.name;
    let _ffi_class_name = format_ident!("{}", class_name);

    let mut common_methods = Vec::new();
    let mut mut_methods = Vec::new();
    let mut static_methods = Vec::new();
    let mut aux_items = None;

    match method {
        MethodDef::Ctor(ctor) => {
            let name = &ctor.rust_name;
            let ffi_unique_name = format_ident!("make_{}_{}", class_name, ctor.rust_name);
            let (args_def, args_call) = process_method_args(&ctor.args, models);

            static_methods.push(quote! {
                    pub fn #name(#(#args_def),*) -> CppObject<'static, #class_name, justcxx::Mut, justcxx::Owned> {
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
            generate_iterator_method(
                class_name,
                iter,
                models,
                &mut common_methods,
                &mut aux_items,
            );
        }
        MethodDef::Method(func) => {
            generate_normal_method(
                class_name,
                func,
                models,
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
    _models: &HashMap<String, ClassModel>,
    common_methods: &mut Vec<TokenStream>,
    aux_items: &mut Option<TokenStream>,
) {
    let method_name = &iter.rust_name;
    let yield_tag = &iter.yield_ty;

    let names = IterNames::new(class_name, method_name);

    let aux_struct = generate_iter_aux_struct(&names, yield_tag);

    if let Some(existing) = aux_items {
        *aux_items = Some(quote! { #existing #aux_struct });
    } else {
        *aux_items = Some(aux_struct);
    }
    let item = quote! {
        CppObject<'static, #yield_tag, justcxx::Mut, justcxx::Owned>
    };

    let wrapper_method = generate_iter_wrapper_method(method_name, &names, &item, iter.is_iter_mut);
    common_methods.push(wrapper_method);
}

fn generate_iter_aux_struct(names: &IterNames, yield_tag: &Ident) -> TokenStream {
    let struct_name = &names.struct_name;
    let ctx_name = &names.ctx_name;
    let next_fn = &names.next_fn;

    quote! {
        #[allow(non_camel_case_types)]
        pub struct #struct_name<'a, M: justcxx::Mode> {
            ctx: cxx::UniquePtr<ffi::#ctx_name>,
            _marker: std::marker::PhantomData<(&'a (), M)>,
        }

        impl<'a, M: justcxx::Mode> Iterator for #struct_name<'a, M> {
            type Item = CppObject<'static, #yield_tag, justcxx::Mut, justcxx::Owned>;

            fn next(&mut self) -> Option<Self::Item> {
                unsafe {
                    let ret_ptr = ffi::#next_fn(self.ctx.pin_mut());
                    if ret_ptr.is_null() {
                        None
                    } else {
                        Some(CppObject {
                            inner: ret_ptr,
                            _marker: std::marker::PhantomData
                        })
                    }
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
    models: &HashMap<String, ClassModel>,
    common_methods: &mut Vec<TokenStream>,
    mut_methods: &mut Vec<TokenStream>,
    static_methods: &mut Vec<TokenStream>,
) {
    let method_name = &func.rust_name;
    let ffi_name = format_ident!("{}_method_{}", class_name, func.rust_name);

    let (args_decl, args_call) = process_method_args(&func.args, models);

    let prepare_ptr = quote! {
        let ptr = S::as_ptr(&self.inner);
    };

    match func.kind {
        MethodKind::Static => {
            let ffi_call_expr = quote! {
                ffi::#ffi_name(#(#args_call),*)
            };

            let (ret_ty, body) = process_return_type_wrapper(&func.ret_ty, models, ffi_call_expr);

            static_methods.push(quote! {
                pub fn #method_name(#(#args_decl),*) #ret_ty {
                    unsafe { #body }
                }
            });
        }

        MethodKind::Const => {
            let ffi_call_expr = quote! {
                ffi::#ffi_name(&*ptr, #(#args_call),*)
            };
            let (ret_ty, body) = process_return_type_wrapper(&func.ret_ty, models, ffi_call_expr);

            common_methods.push(quote! {
                pub fn #method_name(&self, #(#args_decl),*) #ret_ty {
                    unsafe {
                        #prepare_ptr
                        #body
                    }
                }
            });
        }

        MethodKind::Mutable => {
            let ffi_call_expr = quote! {
                ffi::#ffi_name(pin_self, #(#args_call),*)
            };
            let (ret_ty, body) = process_return_type_wrapper(&func.ret_ty, models, ffi_call_expr);

            mut_methods.push(quote! {
                pub fn #method_name(&mut self, #(#args_decl),*) #ret_ty {
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

fn process_method_args(
    args: &[Arg],
    models: &HashMap<String, ClassModel>,
) -> (Vec<TokenStream>, Vec<TokenStream>) {
    args.iter()
        .map(|arg| process_single_arg(arg, models))
        .unzip()
}

fn process_single_arg(
    arg: &Arg,
    models: &HashMap<String, ClassModel>,
) -> (TokenStream, TokenStream) {
    let arg_name = &arg.name;
    let arg_ty = &arg.ty;
    // ref
    if let Some(info) = extract_defined_ref_info(arg_ty, models) {
        let class_ident = format_ident!("{}", info.type_name);
        let is_mut_arg = info.is_mut;

        let decl = if is_mut_arg {
            quote! { #arg_name: &mut justcxx::CppMut<'_, #class_ident> }
        } else {
            quote! { #arg_name: justcxx::CppRef<'_, #class_ident> }
        };

        let call = if is_mut_arg {
            quote! {
                std::pin::Pin::new_unchecked(&mut *#arg_name.as_ptr())
            }
        } else {
            quote! {
                &*#arg_name.as_ptr()
            }
        };

        return (decl, call);
    }

    // Owned
    if let Some(type_name) = get_type_ident_name(arg_ty) {
        if models.contains_key(&type_name) {
            let class_ident = format_ident!("{}", type_name);

            let decl = quote! {
                #arg_name: justcxx::CppOwned<#class_ident>
            };

            let call = quote! { #arg_name.inner };
            return (decl, call);
        }
    }

    (quote! { #arg_name: #arg_ty }, quote! { #arg_name })
}

fn process_return_type_wrapper(
    ret: &Option<Type>,
    models: &HashMap<String, ClassModel>,
    ffi_call_expr: TokenStream,
) -> (TokenStream, TokenStream) {
    // void
    let ret_ty = match ret {
        Some(t) => t,
        None => return handle_void(ffi_call_expr),
    };
    // ref
    if let Some(info) = extract_defined_ref_info(ret_ty, models) {
        return handle_cpp_obj_ref(info, ffi_call_expr);
    }
    // Option<...>
    if let Some(inner_ty) = extract_option_inner(ret_ty) {
        // Option<ref>
        if let Some(info) = extract_defined_ref_info(&inner_ty, models) {
            return handle_option_cpp_obj(info, ffi_call_expr);
        }

        // Option<value>
        if let Some(type_name) = get_type_ident_name(&inner_ty) {
            if models.contains_key(&type_name) {
                return handle_option_cpp_obj_owned(&type_name, ffi_call_expr);
            }
        }
        // Option<primitive>
        return handle_option_primitive(&inner_ty, ffi_call_expr);
    }
    // value
    if let Some(type_name) = get_type_ident_name(ret_ty) {
        if models.contains_key(&type_name) {
            return handle_cpp_obj_value(&type_name, ffi_call_expr);
        }
    }
    // primitive
    handle_primitive(ret_ty, ffi_call_expr)
}

fn handle_void(ffi_call_expr: TokenStream) -> (TokenStream, TokenStream) {
    (quote! {}, quote! { #ffi_call_expr; })
}

fn handle_primitive(ret_ty: &Type, ffi_call_expr: TokenStream) -> (TokenStream, TokenStream) {
    (quote! { -> #ret_ty }, ffi_call_expr)
}

fn handle_cpp_obj_ref(
    info: DefinedRefInfo,
    ffi_call_expr: TokenStream,
) -> (TokenStream, TokenStream) {
    let class_ident = format_ident!("{}", info.type_name);
    let is_mut = info.is_mut;

    let ret_mode = get_return_mode(is_mut);

    let sig = quote! {
        -> CppObject<'a, #class_ident, #ret_mode, justcxx::Ref>
    };

    let ffi_ret_var = format_ident!("ffi_ret");
    let ptr_conversion = get_ptr_conversion(is_mut, &ffi_ret_var);

    let body = quote! {
        let #ffi_ret_var = #ffi_call_expr;
        let ret_ptr = #ptr_conversion;

        CppObject {
            inner: ret_ptr,
            _marker: std::marker::PhantomData
        }
    };

    (sig, body)
}

fn handle_option_cpp_obj(
    info: DefinedRefInfo,
    ffi_call_expr: TokenStream,
) -> (TokenStream, TokenStream) {
    let class_ident = format_ident!("{}", info.type_name);
    let is_mut = info.is_mut;

    let ret_mode = get_return_mode(is_mut);

    let sig = quote! {
         -> Option<CppObject<'a, #class_ident, #ret_mode, justcxx::Ref>>
    };

    let val_var = format_ident!("val");
    let ptr_conversion = get_ptr_conversion(is_mut, &val_var);

    let body = quote! {
        match #ffi_call_expr {
            Ok(#val_var) => {
                let ret_ptr = #ptr_conversion;
                Some(CppObject {
                    inner: ret_ptr,
                    _marker: std::marker::PhantomData
                })
            },
            Err(_) => None,
        }
    };

    (sig, body)
}

fn handle_cpp_obj_value(type_name: &str, ffi_call_expr: TokenStream) -> (TokenStream, TokenStream) {
    let class_ident = format_ident!("{}", type_name);
    let sig = quote! {
        -> CppObject<'static, #class_ident, justcxx::Mut, justcxx::Owned>
    };

    let body = quote! {
        let unique_ptr = #ffi_call_expr;
        CppObject {
            inner: unique_ptr,
            _marker: std::marker::PhantomData
        }
    };

    (sig, body)
}

fn handle_option_cpp_obj_owned(
    type_name: &str,
    ffi_call_expr: TokenStream,
) -> (TokenStream, TokenStream) {
    let class_ident = format_ident!("{}", type_name);
    let sig = quote! {
        -> Option<CppObject<'static, #class_ident, justcxx::Mut, justcxx::Owned>>
    };
    let body = quote! {
        let unique_ptr = #ffi_call_expr;
        if unique_ptr.is_null() {
            None
        } else {
            Some(CppObject {
                inner: unique_ptr,
                _marker: std::marker::PhantomData
            })
        }
    };
    (sig, body)
}

fn handle_option_primitive(
    inner_ty: &Type,
    ffi_call_expr: TokenStream,
) -> (TokenStream, TokenStream) {
    let sig = quote! { -> Option<#inner_ty> };
    let body = quote! {
        match #ffi_call_expr {
            Ok(val) => Some(val),
            Err(_) => None,
        }
    };

    (sig, body)
}

fn get_return_mode(is_mut_ret: bool) -> TokenStream {
    if is_mut_ret {
        quote! { M }
    } else {
        quote! { justcxx::Const }
    }
}

fn get_ptr_conversion(is_mut_ret: bool, var_name: &Ident) -> TokenStream {
    if is_mut_ret {
        quote! { #var_name.get_unchecked_mut() as *mut _ }
    } else {
        quote! { (#var_name as *const _) as *mut _ }
    }
}
