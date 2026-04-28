#![allow(dead_code)]
#![allow(unused_imports)]

mod auth;
mod muon;
pub mod sso_setup;
pub mod unprivatize_admin;

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
            match res {
                Ok($pattern) $(if $guard)? => {}
                other => panic!("Expected Ok({}), found {other:?}", stringify!($pattern $(if $guard)?)),
            }
        };
    };
}
