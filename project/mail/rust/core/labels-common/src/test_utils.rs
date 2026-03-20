use mail_stash::UserDb;
use mail_stash::stash::{Stash, StashConfiguration};

pub async fn new_label_test_connection() -> Stash<UserDb> {
    let stash = Stash::new(StashConfiguration::test()).unwrap();
    let mut tether = stash.connection().await.unwrap();
    crate::db::migrate(&mut tether).await.unwrap();
    drop(tether);
    stash
}

#[must_use]
pub fn random_string(length: usize) -> String {
    use rand::{Rng, distributions::Uniform};
    let charset: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| charset[rng.sample(Uniform::new(0, charset.len()))] as char)
        .collect()
}
