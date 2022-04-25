//! Wrapper for `String`.

use crate::impl_julia_typecheck;
use crate::memory::global::Global;
use crate::wrappers::ptr::{private::Wrapper as WrapperPriv, value::Value, StringRef};
use crate::{convert::unbox::Unbox, private::Private};
use crate::{
    error::{JlrsError, JlrsResult},
    memory::{output::Output, scope::PartialScope},
};
use jl_sys::{jl_pchar_to_string, jl_string_type};
use std::{ffi::CStr, ptr::NonNull};
use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    marker::PhantomData,
    mem, str,
};

/// A Julia string.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct JuliaString<'scope>(*const u8, PhantomData<&'scope ()>);

impl<'scope> JuliaString<'scope> {
    /// Create a new Julia string.
    pub fn new<'target, V, S>(scope: S, string: V) -> JlrsResult<JuliaString<'target>>
    where
        V: AsRef<str>,
        S: PartialScope<'target>,
    {
        unsafe {
            let global = scope.global();
            JuliaString::new_unrooted(global, string).root(scope)
        }
    }

    /// Create a new Julia string. Unlike [`JuliaString::new`] this method doesn't root the
    /// allocated value.
    pub fn new_unrooted<'global, V>(_: Global<'global>, string: V) -> StringRef<'global>
    where
        V: AsRef<str>,
    {
        unsafe {
            let str_ref = string.as_ref();
            let ptr = str_ref.as_ptr().cast();
            let len = str_ref.len();
            let s = jl_pchar_to_string(ptr, len);
            debug_assert!(!s.is_null());
            StringRef::wrap(s.cast())
        }
    }

    /// Returns the length of the string.
    pub fn len(self) -> usize {
        unsafe { *self.0.cast() }
    }

    /// Returns the string as a `CStr`.
    pub fn as_c_str(self) -> &'scope CStr {
        unsafe {
            let str_begin = self.0.add(mem::size_of::<usize>());
            CStr::from_ptr(str_begin.cast())
        }
    }

    /// Returns the string as a slice of bytes without the terminating `\0`.
    pub fn as_slice(self) -> &'scope [u8] {
        self.as_c_str().to_bytes()
    }

    /// Returns the string as a string slice, or an error if it the string contains
    /// invalid characters
    pub fn as_str(self) -> JlrsResult<&'scope str> {
        Ok(str::from_utf8(self.as_slice()).or(Err(JlrsError::NotUTF8))?)
    }

    /// Returns the string as a string slice without checking if the string is properly encoded.
    ///
    /// Safety: the string must be properly encoded.
    pub unsafe fn as_str_unchecked(self) -> &'scope str {
        str::from_utf8_unchecked(self.as_slice())
    }

    /// Use the `Output` to extend the lifetime of this data.
    pub fn root<'target>(self, output: Output<'target>) -> JuliaString<'target> {
        unsafe {
            let ptr = self.unwrap_non_null(Private);
            output.set_root::<JuliaString>(ptr);
            JuliaString::wrap_non_null(ptr, Private)
        }
    }
}

impl_julia_typecheck!(JuliaString<'scope>, jl_string_type, 'scope);

unsafe impl<'scope> Unbox for JuliaString<'scope> {
    type Output = Result<String, Vec<u8>>;
    unsafe fn unbox(value: Value) -> Self::Output {
        let slice = value.cast_unchecked::<JuliaString>().as_slice();
        str::from_utf8(slice)
            .map(String::from)
            .map_err(|_| slice.into())
    }
}

unsafe impl Unbox for String {
    type Output = Result<String, Vec<u8>>;
    unsafe fn unbox(value: Value) -> Self::Output {
        JuliaString::unbox(value)
    }
}

impl Debug for JuliaString<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(self.as_str().unwrap_or("<Non-UTF8 string>"))
    }
}

impl<'scope> WrapperPriv<'scope, '_> for JuliaString<'scope> {
    type Wraps = u8;
    const NAME: &'static str = "String";

    #[inline(always)]
    unsafe fn wrap_non_null(inner: NonNull<Self::Wraps>, _: Private) -> Self {
        JuliaString(inner.as_ptr(), PhantomData)
    }

    #[inline(always)]
    fn unwrap_non_null(self, _: Private) -> NonNull<Self::Wraps> {
        unsafe { NonNull::new_unchecked(self.0 as *mut _) }
    }
}

impl_root!(JuliaString, 1);
