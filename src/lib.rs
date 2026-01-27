use cxx::UniquePtr;

pub trait Mode {}
pub struct Const;
pub struct Mut;
impl Mode for Const {}
impl Mode for Mut {}

pub trait CppClass {
    type FfiType;
}
pub trait Storage<T: CppClass> {
    type Inner;
    unsafe fn as_ptr(inner: &Self::Inner) -> *mut T::FfiType;
}

#[derive(Clone, Copy)]
pub struct Ref;
impl<T: CppClass> Storage<T> for Ref {
    type Inner = *mut T::FfiType;
    unsafe fn as_ptr(inner: &Self::Inner) -> *mut T::FfiType {
        *inner
    }
}

pub struct Owned;
impl<T: CppClass> Storage<T> for Owned
where
    T::FfiType: cxx::memory::UniquePtrTarget,
{
    type Inner = UniquePtr<T::FfiType>;

    unsafe fn as_ptr(inner: &Self::Inner) -> *mut T::FfiType {
        // assert owned must not be null
        let r = inner.as_ref().unwrap();
        (r as *const T::FfiType) as *mut T::FfiType
    }
}

pub trait CppTypeAliases {
    type Owned;
    type Ref<'a>;
    type Mut<'a>;
}

pub type CppOwned<T> = <T as CppTypeAliases>::Owned;
pub type CppRef<'a, T> = <T as CppTypeAliases>::Ref<'a>;
pub type CppMut<'a, T> = <T as CppTypeAliases>::Mut<'a>;

pub use cxx;
pub use justcxx_macro::bind;