use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashConfiguration};

#[macro_export]
macro_rules! lid {
    ($id:expr) => {{ Some($id.into()) }};
}

#[macro_export]
macro_rules! label_id {
    ($id:expr) => {{ mail_core_api::services::proton::LabelId::from($id) }};
}

#[macro_export]
macro_rules! label {
    ($($field:tt)*) => {{
        $crate::Label {
            $($field)*,
            ..$crate::Label::test_default()
        }
    }};
}

#[macro_export]
macro_rules! labels {
    ($($label:expr),*) => {{
        $crate::Labels::new(vec![$(
            $crate::label_id!($label)
        ),*])
    }};
}

pub async fn new_label_test_connection() -> Stash<UserDb> {
    let stash = Stash::new(StashConfiguration::test()).unwrap();
    let mut tether = stash.connection().await.unwrap();
    crate::db::migrate(&mut tether).await.unwrap();
    drop(tether);
    stash
}

#[must_use]
pub fn random_string(length: usize) -> String {
    use rand::distr::{Alphanumeric, SampleString};
    Alphanumeric.sample_string(&mut rand::rng(), length)
}
