use derive_more::Display;

#[derive(Debug, Display, Clone, Copy)]
pub enum LtQuarkKeyType {
    Curve25519,
    RSA2048,
    RSA4096,
}
