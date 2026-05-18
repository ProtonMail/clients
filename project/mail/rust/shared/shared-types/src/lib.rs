pub mod action;
pub mod init_key;
pub mod map_vec;
pub mod model_ext;
pub mod timestamp;

pub use action::Action;
pub use init_key::InitializationKey;
pub use mail_local_id::{LocalIdActionDepExt, LocalIdMarker};
pub use map_vec::MapVec;
pub use model_ext::{ModelExtension, ModelIdExtension};
pub use timestamp::UnixTimestamp;

#[macro_export]
macro_rules! declare_local_id {
    (
        $(#[$($attrss:tt)*])*
        $visibility:vis $name:ident => $remote_id:ident
    ) => {
        $(#[$($attrss)*])*
        ::mail_local_id::declare_local_id!($name => $remote_id);

        ::mail_local_id::declare_local_id_stash!($name);

        ::mail_local_id::declare_local_id_actions!($name);
    };
    (
      $(#[$($attrss:tt)*])*
        $visibility:vis $name:ident
    ) => {
        $(#[$($attrss)*])*
        ::mail_local_id::declare_local_id!($name);

        ::mail_local_id::declare_local_id_stash!($name);

        ::mail_local_id::declare_local_id_actions!($name);
    };
}
