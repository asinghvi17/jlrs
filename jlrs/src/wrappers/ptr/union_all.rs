//! Wrapper for `UnionAll`, A union of types over all values of a type parameter.

use crate::{
    impl_julia_typecheck,
    memory::target::Target,
    private::Private,
    wrappers::ptr::{
        datatype::DataType, datatype::DataTypeRef, private::WrapperPriv, type_var::TypeVar,
        type_var::TypeVarRef, value::Value, value::ValueRef,
    },
};
use cfg_if::cfg_if;
use jl_sys::{
    jl_abstractarray_type, jl_anytuple_type_type, jl_array_type, jl_densearray_type,
    jl_llvmpointer_type, jl_namedtuple_type, jl_pointer_type, jl_ref_type, jl_type_type,
    jl_type_unionall, jl_unionall_t, jl_unionall_type,
};

cfg_if! {
    if #[cfg(feature = "lts")] {
        use jl_sys::jl_vararg_type;
    }else {
        use jl_sys::jl_opaque_closure_type;
    }
}

use std::{marker::PhantomData, ptr::NonNull};

#[cfg(not(all(target_os = "windows", feature = "lts")))]
use super::value::ValueResult;
use super::{value::ValueData, Ref};

/// An iterated union of types. If a struct field has a parametric type with some of its
/// parameters unknown, its type is represented by a `UnionAll`.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct UnionAll<'scope>(NonNull<jl_unionall_t>, PhantomData<&'scope ()>);

impl<'scope> UnionAll<'scope> {
    /// Create a new `UnionAll`. If an exception is thrown, it's caught and returned.
    #[cfg(not(all(target_os = "windows", feature = "lts")))]
    pub fn new<'target, T>(
        target: T,
        tvar: TypeVar,
        body: Value<'_, 'static>,
    ) -> ValueResult<'target, 'static, T>
    where
        T: Target<'target>,
    {
        use crate::catch::catch_exceptions;
        use jl_sys::jl_value_t;
        use std::mem::MaybeUninit;

        // Safety: if an exception is thrown it's caught, the result is immediately rooted
        unsafe {
            let mut callback = |result: &mut MaybeUninit<*mut jl_value_t>| {
                let res = jl_type_unionall(tvar.unwrap(Private), body.unwrap(Private));
                result.write(res);
                Ok(())
            };

            let res = match catch_exceptions(&mut callback).unwrap() {
                Ok(ptr) => Ok(NonNull::new_unchecked(ptr)),
                Err(e) => Err(NonNull::new_unchecked(e.ptr())),
            };

            target.result_from_ptr(res, Private)
        }
    }

    /// Create a new `UnionAll`. If an exception is thrown it isn't caught
    ///
    /// Safety: an exception must not be thrown if this method is called from a `ccall`ed
    /// function.
    pub unsafe fn new_unchecked<'target, T>(
        target: T,
        tvar: TypeVar,
        body: Value<'_, 'static>,
    ) -> ValueData<'target, 'static, T>
    where
        T: Target<'target>,
    {
        let ua = jl_type_unionall(tvar.unwrap(Private), body.unwrap(Private));
        target.data_from_ptr(NonNull::new_unchecked(ua), Private)
    }

    /// The type at the bottom of this `UnionAll`.
    pub fn base_type(self) -> DataTypeRef<'scope> {
        let mut b = self;

        // Safety: pointer points to valid data
        unsafe {
            while b.body().value_unchecked().is::<UnionAll>() {
                b = b.body().value_unchecked().cast_unchecked::<UnionAll>();
            }
        }

        // Safety: type at the base must be a DataType
        unsafe {
            DataTypeRef::wrap(
                b.body()
                    .value_unchecked()
                    .cast::<DataType>()
                    .unwrap()
                    .unwrap(Private),
            )
        }
    }

    /*
    for (a,b) in zip(fieldnames(UnionAll), fieldtypes(UnionAll))
        println(a,": ", b)
    end
    var: TypeVar
    body: Any
    */

    /// The body of this `UnionAll`. This is either another `UnionAll` or a `DataType`.
    pub fn body(self) -> ValueRef<'scope, 'static> {
        // Safety: pointer points to valid data
        unsafe { ValueRef::wrap(self.unwrap_non_null(Private).as_ref().body) }
    }

    /// The type variable associated with this "layer" of the `UnionAll`.
    pub fn var(self) -> TypeVarRef<'scope> {
        // Safety: pointer points to valid data
        unsafe { TypeVarRef::wrap(self.unwrap_non_null(Private).as_ref().var) }
    }

    /// Use the target to reroot this data.
    pub fn root<'target, T>(self, target: T) -> UnionAllData<'target, T>
    where
        T: Target<'target>,
    {
        // Safety: the data is valid.
        unsafe { target.data_from_ptr(self.unwrap_non_null(Private), Private) }
    }
}

impl<'base> UnionAll<'base> {
    /// The `UnionAll` `Type`.
    pub fn type_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_type_type, Private) }
    }

    /// `Type{T} where T<:Tuple`
    pub fn anytuple_type_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_anytuple_type_type, Private) }
    }

    /// The `UnionAll` `Vararg`.
    #[cfg(feature = "lts")]
    pub fn vararg_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_vararg_type, Private) }
    }

    /// The `UnionAll` `AbstractArray`.
    pub fn abstractarray_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_abstractarray_type, Private) }
    }

    /// The `UnionAll` `OpaqueClosure`.
    #[cfg(not(feature = "lts"))]
    pub fn opaque_closure_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_opaque_closure_type, Private) }
    }

    /// The `UnionAll` `DenseArray`.
    pub fn densearray_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_densearray_type, Private) }
    }

    /// The `UnionAll` `Array`.
    pub fn array_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_array_type, Private) }
    }

    /// The `UnionAll` `Ptr`.
    pub fn pointer_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_pointer_type, Private) }
    }

    /// The `UnionAll` `LLVMPtr`.
    pub fn llvmpointer_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_llvmpointer_type, Private) }
    }

    /// The `UnionAll` `Ref`.
    pub fn ref_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_ref_type, Private) }
    }

    /// The `UnionAll` `NamedTuple`.
    pub fn namedtuple_type<T>(_: &T) -> Self
    where
        T: Target<'base>,
    {
        // Safety: global constant
        unsafe { UnionAll::wrap(jl_namedtuple_type, Private) }
    }
}

impl_julia_typecheck!(UnionAll<'scope>, jl_unionall_type, 'scope);
impl_debug!(UnionAll<'_>);

impl<'scope> WrapperPriv<'scope, '_> for UnionAll<'scope> {
    type Wraps = jl_unionall_t;
    type TypeConstructorPriv<'target, 'da> = UnionAll<'target>;
    const NAME: &'static str = "UnionAll";

    // Safety: `inner` must not have been freed yet, the result must never be
    // used after the GC might have freed it.
    unsafe fn wrap_non_null(inner: NonNull<Self::Wraps>, _: Private) -> Self {
        Self(inner, PhantomData)
    }

    fn unwrap_non_null(self, _: Private) -> NonNull<Self::Wraps> {
        self.0
    }
}

impl_root!(UnionAll, 1);

/// A reference to a [`UnionAll`] that has not been explicitly rooted.
pub type UnionAllRef<'scope> = Ref<'scope, 'static, UnionAll<'scope>>;
impl_valid_layout!(UnionAllRef, UnionAll);
impl_ref_root!(UnionAll, UnionAllRef, 1);

use crate::memory::target::target_type::TargetType;

/// `UnionAll` or `UnionAllRef`, depending on the target type `T`.
pub type UnionAllData<'target, T> = <T as TargetType<'target>>::Data<'static, UnionAll<'target>>;

/// `JuliaResult<UnionAll>` or `JuliaResultRef<UnionAllRef>`, depending on the target type `T`.
pub type UnionAllResult<'target, T> =
    <T as TargetType<'target>>::Result<'static, UnionAll<'target>>;