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

pub trait AsCppPtr<T: CppClass> {
    fn as_cpp_ptr(&self) -> *mut T::FfiType;
}

pub trait AsMutCppPtr<T: CppClass>: AsCppPtr<T> {}

pub trait CppTypeAliases {
    type Owned;
    type Ref<'a>;
    type Mut<'a>;
}

impl<T, C: CppClass> AsCppPtr<C> for &T
where
    T: AsCppPtr<C> + ?Sized,
{
    fn as_cpp_ptr(&self) -> *mut C::FfiType {
        (**self).as_cpp_ptr()
    }
}

impl<T, C: CppClass> AsCppPtr<C> for &mut T
where
    T: AsCppPtr<C> + ?Sized,
{
    fn as_cpp_ptr(&self) -> *mut C::FfiType {
        (**self).as_cpp_ptr()
    }
}

impl<T, C: CppClass> AsMutCppPtr<C> for &mut T where T: AsMutCppPtr<C> + ?Sized {}


pub type CppOwned<T> = <T as CppTypeAliases>::Owned;
pub type CppRef<'a, T> = <T as CppTypeAliases>::Ref<'a>;
pub type CppMut<'a, T> = <T as CppTypeAliases>::Mut<'a>;

pub use cxx;
pub use justcxx_macro::bind;
