//! Support for Julia `Union`s and union-fields.

use super::Value;
use crate::layout::bits_union::{Align, BitsUnion as BU, Flag};
use crate::{
    convert::cast::Cast,
    error::{JlrsError, JlrsResult},
};
use crate::{impl_julia_typecheck, impl_valid_layout};
use jl_sys::{jl_islayout_inline, jl_uniontype_t, jl_uniontype_type};
use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    marker::PhantomData,
    ptr::NonNull,
};

/// A struct field can have a type that's a union of several types. In this case, the type of this
/// field is an instance of `Union`.
#[derive(Copy, Clone, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct Union<'frame>(NonNull<jl_uniontype_t>, PhantomData<&'frame ()>);

impl<'frame> Union<'frame> {
    pub(crate) unsafe fn wrap(union: *mut jl_uniontype_t) -> Self {
        debug_assert!(!union.is_null());
        Union(NonNull::new_unchecked(union), PhantomData)
    }

    #[doc(hidden)]
    pub unsafe fn inner(self) -> NonNull<jl_uniontype_t> {
        self.0
    }

    /// Returns true if all the isbits union optimization applies to this union type.
    pub fn is_bits_union(self) -> bool {
        unsafe {
            let v: Value = self.into();
            jl_islayout_inline(v.inner().as_ptr(), &mut 0, &mut 0) != 0
        }
    }

    /// Returns true if the isbits union optimization applies to this union type and calculates
    /// the size and aligment if it does. If this method returns false, the calculated size and
    /// alignment are invalid.
    pub fn isbits_size_align(self, size: &mut usize, align: &mut usize) -> bool {
        unsafe {
            let v: Value = self.into();
            jl_islayout_inline(v.inner().as_ptr(), size, align) != 0
        }
    }

    /// Returns the size of a field that is of this `Union` type excluding the flag that is used
    /// in bits unions.
    pub fn size(self) -> usize {
        let mut sz = 0;
        if !self.isbits_size_align(&mut sz, &mut 0) {
            return std::mem::size_of::<usize>();
        }

        sz
    }

    /*
    for (a, b) in zip(fieldnames(Union), fieldtypes(Union))
        println(a, ": ", b)
    end
    a: Any
    b: Any
    */

    /// Unions are stored as binary trees, the arguments are stored as its leaves. This method
    /// returns one of its branches.
    pub fn a(self) -> Value<'frame, 'static> {
        unsafe { Value::wrap((&*self.inner().as_ptr()).a) }
    }

    /// Unions are stored as binary trees, the arguments are stored as its leaves. This method
    /// returns one of its branches.
    pub fn b(self) -> Value<'frame, 'static> {
        unsafe { Value::wrap((&*self.inner().as_ptr()).b) }
    }

    /// Convert `self` to a `Value`.
    pub fn as_value(self) -> Value<'frame, 'static> {
        self.into()
    }
}

impl<'scope> Debug for Union<'scope> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("Union").finish()
    }
}

impl<'frame> Into<Value<'frame, 'static>> for Union<'frame> {
    fn into(self) -> Value<'frame, 'static> {
        unsafe { Value::wrap_non_null(self.inner().cast()) }
    }
}

unsafe impl<'frame, 'data> Cast<'frame, 'data> for Union<'frame> {
    type Output = Self;
    fn cast(value: Value<'frame, 'data>) -> JlrsResult<Self::Output> {
        if value.is::<Self::Output>() {
            return unsafe { Ok(Self::cast_unchecked(value)) };
        }

        Err(JlrsError::NotAUnion)?
    }

    unsafe fn cast_unchecked(value: Value<'frame, 'data>) -> Self::Output {
        Self::wrap(value.inner().as_ptr().cast())
    }
}

impl_julia_typecheck!(Union<'frame>, jl_uniontype_type, 'frame);

impl_valid_layout!(Union<'frame>, 'frame);

/// Ensures the next field is aligned to 1 byte.
#[repr(C, align(1))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Align1;

unsafe impl Align for Align1 {
    const ALIGNMENT: usize = 1;
}

/// Ensures the next field is aligned to 2 bytes.
#[repr(C, align(2))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Align2;

unsafe impl Align for Align2 {
    const ALIGNMENT: usize = 2;
}

/// Ensures the next field is aligned to 4 bytes.
#[repr(C, align(4))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Align4;

unsafe impl Align for Align4 {
    const ALIGNMENT: usize = 4;
}

/// Ensures the next field is aligned to 8 bytes.
#[repr(C, align(8))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Align8;

unsafe impl Align for Align8 {
    const ALIGNMENT: usize = 8;
}

/// Ensures the next field is aligned to 16 bytes.
#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Align16;

unsafe impl Align for Align16 {
    const ALIGNMENT: usize = 16;
}

/// When a `Union` is used as a field type in a struct, there are two possible representations.
/// Which representation is chosen depends on its arguments.
///
/// In the general case the `Union` is simply represented as a `Value`. If all of the are isbits*
/// types an inline representation is used. In this case, the value is essentially stored in an
/// array of bytes that is large enough to contain the largest-sized value, followed by a single,
/// byte-sized flag. This array has the same alignment as the value with the largest required
/// alignment.
///
/// In order to take all of this into account, when mapping a Julia struct that has one of these
/// optimized unions as a field, they are translated to three distinct fields. The first is a
/// zero-sized type with a set alignment, the second a `BitsUnion`, and finally a `u8`. The
/// generic parameter of `BitsUnion` must always be `[MaybeUninit<u8>; N]` with N explicitly equal
/// to the size of the largest possible value. The previous, zero-sized, field ensures the
/// `BitsUnion` is properly aligned, the flag indicates the type of the stored value.
///
/// Currently, even though a struct that contains an optimized union is supported by the
/// `JuliaStruct` macro, these fields can't be used from Rust. If you want to access the value,
/// you can use `Value::get_field` which will essentially convert it to the general representation.
///
/// *The types that are eligible for the optimization is actually not limited to just isbits
/// types. In particular, a struct which contains an optimized union as a field is no longer an
/// isbits type but the optimization still applies.
#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
pub struct BitsUnion<T>(T);

unsafe impl<T> BU for BitsUnion<T> {}

pub unsafe fn correct_layout_for<A: Align, B: BU, F: Flag>(u: Union) -> bool {
    let mut jl_sz = 0;
    let mut jl_align = 0;
    if !u.isbits_size_align(&mut jl_sz, &mut jl_align) {
        return false;
    }

    A::ALIGNMENT == jl_align && std::mem::size_of::<B>() == jl_sz
}

pub(crate) fn nth_union_component<'frame, 'data>(
    v: Value<'frame, 'data>,
    pi: &mut i32,
) -> Option<Value<'frame, 'data>> {
    match v.cast::<Union>() {
        Ok(un) => {
            let a = nth_union_component(un.a(), pi);
            if a.is_some() {
                a
            } else {
                *pi -= 1;
                return nth_union_component(un.b(), pi);
            }
        }
        Err(_) => {
            if *pi == 0 {
                Some(v)
            } else {
                None
            }
        }
    }
}

pub(crate) fn find_union_component(haystack: Value, needle: Value, nth: &mut u32) -> bool {
    unsafe {
        match haystack.cast::<Union>() {
            Ok(hs) => {
                if find_union_component(hs.a(), needle, nth) {
                    true
                } else if find_union_component(hs.b(), needle, nth) {
                    true
                } else {
                    false
                }
            }
            Err(_) => {
                if needle.inner() == haystack.inner() {
                    return true;
                } else {
                    *nth += 1;
                    false
                }
            }
        }
    }
}
