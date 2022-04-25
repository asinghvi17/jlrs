//! Field index trait.

pub trait FieldIndex: private::FieldIndex {}
impl<FI: private::FieldIndex> FieldIndex for FI {}

mod private {
    use crate::{
        convert::to_symbol::private::ToSymbol,
        error::{JlrsError, JlrsResult, CANNOT_DISPLAY_TYPE},
        prelude::Wrapper,
        private::Private,
        wrappers::ptr::{
            array::{dimensions::Dims, Array},
            datatype::DataType,
            string::JuliaString,
            symbol::Symbol,
        },
    };

    pub trait FieldIndex {
        fn field_index(&self, ty: DataType, _: Private) -> JlrsResult<usize>;

        fn array_index(&self, _data: Array, _: Private) -> JlrsResult<usize> {
            Err(JlrsError::ArrayNeedsNumericalIndex)?
        }
    }

    impl FieldIndex for &str {
        fn field_index(&self, ty: DataType, _: Private) -> JlrsResult<usize> {
            unsafe { ty.field_index(self.to_symbol_priv(Private)) }
        }
    }

    impl FieldIndex for Symbol<'_> {
        fn field_index(&self, ty: DataType, _: Private) -> JlrsResult<usize> {
            ty.field_index(*self)
        }
    }

    impl FieldIndex for JuliaString<'_> {
        fn field_index(&self, ty: DataType, _: Private) -> JlrsResult<usize> {
            unsafe { ty.field_index(self.to_symbol_priv(Private)) }
        }
    }

    impl<D: Dims> FieldIndex for D {
        fn field_index(&self, ty: DataType, _: Private) -> JlrsResult<usize> {
            debug_assert!(!ty.is::<Array>());

            if self.n_dimensions() != 1 {
                Err(JlrsError::ArrayNeedsSimpleIndex)?
            }

            let n = self.size();
            if ty.n_fields() as usize <= n {
                Err(JlrsError::OutOfBounds {
                    idx: n,
                    n_fields: ty.n_fields() as usize,
                    value_type: ty.display_string_or(CANNOT_DISPLAY_TYPE),
                })?;
            }

            Ok(n)
        }

        fn array_index(&self, data: Array, _: Private) -> JlrsResult<usize> {
            data.dimensions().index_of(self)
        }
    }
}
