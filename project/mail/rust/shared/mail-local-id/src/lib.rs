#[cfg(feature = "stash")]
use mail_stash::exports::{FromSql, ToSql};
#[cfg(feature = "serde")]
use serde::de::DeserializeOwned;
use std::fmt::Debug;

/// Marker trait to signal that this type was declared as a local id.
pub trait LocalIdMarker: Sized {
    type Counterpart: Clone
        + Send
        + Sync
        + ToSql
        + FromSql
        + serde::Serialize
        + DeserializeOwned
        + Debug;
}

#[cfg(feature = "mail-actions")]
pub trait LocalIdActionDepExt: Debug + Copy + Clone {
    fn to_dependency_key(&self) -> mail_action_queue::action::ActionDependencyKey;

    fn to_create_dependency_key(&self) -> mail_action_queue::action::ActionDependencyKey;

    fn to_custom_dependency_key(
        &self,
        prefix: &str,
    ) -> mail_action_queue::action::ActionDependencyKey;
}

#[cfg(feature = "stash")]
#[macro_export]
macro_rules! define_local_id_stash {
    ($name:ident) => {
        impl ::mail_stash::exports::ToSql for $name {
            fn to_sql(
                &self,
            ) -> Result<::mail_stash::exports::ToSqlOutput<'_>, ::mail_stash::exports::SqliteError>
            {
                self.0.to_sql()
            }
        }
        impl ::mail_stash::exports::FromSql for $name {
            fn column_result(
                value: ::mail_stash::exports::ValueRef<'_>,
            ) -> ::mail_stash::exports::FromSqlResult<Self> {
                u64::column_result(value).map($name)
            }
        }
    };
}

#[cfg(not(feature = "stash"))]
#[macro_export]
macro_rules! define_local_id_stash {
    ($name:ident) => {};
}

#[cfg(feature = "mail-actions")]
#[macro_export]
macro_rules! define_local_id_actions {
    ($name:ident) => {
        impl $crate::LocalIdActionDepExt for $name {
            fn to_dependency_key(&self) -> ::mail_action_queue::action::ActionDependencyKey {
                ::mail_action_queue::action::ActionDependencyKey::from(format!(
                    "dep-{}-{}",
                    stringify!($name),
                    self.0
                ))
            }

            fn to_create_dependency_key(&self) -> ::mail_action_queue::action::ActionDependencyKey {
                ::mail_action_queue::action::ActionDependencyKey::from(format!(
                    "create-{}-{}",
                    stringify!($name),
                    self.0
                ))
            }

            fn to_custom_dependency_key(
                &self,
                prefix: &str,
            ) -> ::mail_action_queue::action::ActionDependencyKey {
                ::mail_action_queue::action::ActionDependencyKey::from(format!(
                    "{prefix}-{}-{}",
                    stringify!($name),
                    self.0
                ))
            }
        }

        impl From<$name> for ::mail_action_queue::rebase::RebaseKey {
            fn from(id: $name) -> Self {
                ::mail_action_queue::rebase::RebaseKey::from(format!(
                    "{}-{}",
                    stringify!($name),
                    id.0
                ))
            }
        }
    };
}

#[cfg(not(feature = "mail-actions"))]
#[macro_export]
macro_rules! define_local_id_actions {
    ($name:ident) => {};
}

#[cfg(feature = "serde")]
#[macro_export]
macro_rules! declare_local_id_type {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Eq,
            Hash,
            Ord,
            PartialEq,
            PartialOrd,
            serde::Serialize,
            serde::Deserialize,
        )]
        pub struct $name(u64);
    };
}

#[cfg(not(feature = "serde"))]
#[macro_export]
macro_rules! declare_local_id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);
    };
}

/// Declare a new Local id type that maps to a remote Proton Id.
///
/// A local identifier should exist for every remote/proton Id for every resource we store
/// in the database that we will create/mutate.
///
/// # Example
///
/// ```ignore
/// use mail_core_api::declare_proton_id;
/// use mail_shared_types::declare_local_id;
///
/// declare_proton_id!(pub MyProtonId);
/// declare_local_id!(pub MyLocalProtonId => MyProtonId);
/// ```
#[macro_export]
macro_rules! declare_local_id {
    (
        $(#[$($attrss:tt)*])*
        $visibility:vis $name:ident => $remote_id:ident
    ) => {
        $(#[$($attrss)*])*
        $crate::declare_local_id_type!($name);

        impl $name {
            /// Represents the internal value as an unsigned 64-bit integer.
            #[must_use]
            pub const fn as_u64(&self) -> u64 {
                self.0
            }
        }

        impl AsRef<u64> for $name {
            fn as_ref(&self) -> &u64 {
                &self.0
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<u64> for $name {
            fn from(id: u64) -> Self {
                Self(id)
            }
        }


        impl $crate::LocalIdMarker for $name {
            type Counterpart = $remote_id;
        }

        $crate::define_local_id_stash!($name);


        $crate::define_local_id_actions!($name);

    };
}
