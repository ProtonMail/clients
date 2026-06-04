#![allow(dead_code)]

mod auth;
pub mod device_approval;
mod muon;
mod org_member_error;
mod org_members;
pub mod sso_login;
pub mod sso_member_setup;
pub mod sso_setup;
mod test_transport;
pub mod unprivatize_admin;

pub use auth::*;
pub use muon::*;

#[macro_export]
macro_rules! assert_api_err {
    ($lhs:expr, $pat:pat $(if $guard:expr)?) => {
        match &$lhs {
            Err(::lattice_muon2::LtTransportError::Lattice(::lattice::LatticeError::ApiError(_, e))) if matches!(e.as_ref(), $pat $( if $guard )?) => {}
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
