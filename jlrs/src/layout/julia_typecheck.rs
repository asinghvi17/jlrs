//! Trait for Julia type properties.

use crate::value::datatype::DataType;
use std::ffi::c_void;

/// This trait is used in combination with [`Value::is`] and [`DataType::is`]; types that
/// implement this trait can be used to check many properties of a Julia `DataType`.
///
/// This trait is implemented for a few types that implement [`ValidLayout`], eg `String`,
/// [`Array`], and `u8`. In these cases, if the check returns `true` the value can be successfully
/// cast to that type with [`Value::cast`].
///
/// [`Value::is`]: crate::value::Value::is
/// [`Value::cast`]: crate::value::Value::cast
/// [`Array`]: crate::value::array::Array
/// [`ValidLayout`]: crate::layout::valid_layout::ValidLayout
pub unsafe trait JuliaTypecheck {
    #[doc(hidden)]
    unsafe fn julia_typecheck(t: DataType) -> bool;
}

#[doc(hidden)]
#[macro_export]
macro_rules! impl_julia_typecheck {
    ($type:ty, $jl_type:expr, $($lt:lifetime),+) => {
        unsafe impl<$($lt),+> crate::layout::julia_typecheck::JuliaTypecheck for $type {
            unsafe fn julia_typecheck(t: $crate::value::datatype::DataType) -> bool {
                t.inner().as_ptr() == $jl_type
            }
        }
    };
    ($type:ty, $jl_type:expr) => {
        unsafe impl crate::layout::julia_typecheck::JuliaTypecheck for $type {
            unsafe fn julia_typecheck(t: $crate::value::datatype::DataType) -> bool {
                t.inner().as_ptr() == $jl_type
            }
        }
    };
    ($type:ty) => {
        unsafe impl crate::layout::julia_typecheck::JuliaTypecheck for $type {
            unsafe fn julia_typecheck(t: crate::value::datatype::DataType) -> bool {
                t.inner().as_ptr() == <$type as $crate::convert::into_julia::IntoJulia>::julia_type()
            }
        }
    };
}

impl_julia_typecheck!(i8);
impl_julia_typecheck!(i16);
impl_julia_typecheck!(i32);
impl_julia_typecheck!(i64);
impl_julia_typecheck!(isize);
impl_julia_typecheck!(u8);
impl_julia_typecheck!(u16);
impl_julia_typecheck!(u32);
impl_julia_typecheck!(u64);
impl_julia_typecheck!(usize);
impl_julia_typecheck!(f32);
impl_julia_typecheck!(f64);
impl_julia_typecheck!(bool);
impl_julia_typecheck!(char);
impl_julia_typecheck!(*mut c_void);
