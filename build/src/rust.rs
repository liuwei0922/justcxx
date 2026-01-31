use crate::ast::*;
use crate::ffi::{generate_ffi_block, generate_map_ffi, generate_vec_ffi};
use crate::wrapper::{generate_map_wrappers, generate_vec_wrappers, generate_wrapper_block};
use proc_macro2::TokenStream;
use quote::quote;

pub fn generate_rust(bind_context: &BindContext) -> TokenStream {
    let mut extern_cpp_items = Vec::new();
    let mut rust_wrapper_items = Vec::new();

    for class_name_str in &bind_context.class_names_order {
        let class = bind_context.models.get(class_name_str).unwrap();

        extern_cpp_items.push(generate_ffi_block(class));

        rust_wrapper_items.push(generate_wrapper_block(class));
    }

    extern_cpp_items.push(generate_vec_ffi(&bind_context.vec_defs));
    rust_wrapper_items.push(generate_vec_wrappers(&bind_context.vec_defs));
    extern_cpp_items.push(generate_map_ffi(&bind_context.map_defs));
    rust_wrapper_items.push(generate_map_wrappers(&bind_context.map_defs));

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
            fn as_ptr(&self) -> *mut T::FfiType {
                unsafe { S::as_ptr(&self.inner) }
            }

            pub fn as_ref(&self) -> CppObject<'_, T, justcxx::Const, justcxx::Ref> {
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
            pub fn as_mut(&self) -> CppObject<'_, T, justcxx::Mut, justcxx::Ref> {
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
                include!("justcxx.hh");
                #(#extern_cpp_items)*
            }
        }
        #(#rust_wrapper_items)*
    }
}
