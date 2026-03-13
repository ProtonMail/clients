#![allow(dead_code)]
#![allow(unused_imports)]

mod auth;
mod muon;

pub use auth::*;
pub use muon::*;

#[macro_export]
macro_rules! assert_api_err {
    ($lhs:expr, $pat:pat $(if $guard:expr)?) => {
        match &$lhs {
            Err(::lattice::LatticeError::ApiError(_, e)) if matches!(e.as_ref(), $pat $( if $guard )?) => {}
            other => panic!("Expected {:?}, found {other:?}", stringify!($pat $(if $guard)?)),
        }
    };
}

#[macro_export]
macro_rules! assert_api_ok {
    ($res:expr, $pattern:pat $(if $guard:expr)?) => {
        {
            let res = &$res;
            assert!(matches!(&res, Ok($pattern) $(if $guard)?), "Expected {:?}, found {res:?}", stringify!($pat $(if $guard)?));
        };
    };
}
