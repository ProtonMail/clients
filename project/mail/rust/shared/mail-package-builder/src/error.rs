use mail_crypto_inbox::attachment::AttachmentDecryptionError;
use mail_crypto_inbox::eo::EoError;
use mail_crypto_inbox::keys::{PackageCryptoType, SessionKeyError};
use mail_crypto_inbox::message::MessageError;

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("Failed to encrypt package: {0}")]
    PackageBodyEncrypt(#[from] MessageError),

    #[error("Attachment {0} is missing key packets")]
    AttachmentMissingKeyPackets(String),

    #[error("Attachment at position {0} has no remote id")]
    AttachmentHasNoRemoteId(usize),

    #[error("Attachment at position {0} already has a remote id")]
    AttachmentAlreadyHasRemoteId(usize),

    #[error("Failed to write mime body to buffer: {0}")]
    MimeBodyBuild(String),

    #[error("Failed to convert HTML body to plain text: {0}")]
    HtmlToTextConversion(String),

    #[error("Failed to extract attachment info for address: {0}")]
    PackageBodyInfoReEncrypt(SessionKeyError),

    #[error("Failed to extract attachment info for address: {0}")]
    PackageAttachmentInfo(#[from] AttachmentDecryptionError),

    #[error("Failed to encrypt attachment info to recipient: {0}")]
    PackageAttachmentInfoReEncrypt(SessionKeyError),

    #[error("Failed to encrypt attachment signature to recipient: {0}")]
    PackageAttachmentInfoReEncryptSignature(
        mail_crypto_inbox::attachment::AttachmentEncryptionError,
    ),

    #[error("Package encryption type is not supported: {0}")]
    NotSupported(PackageCryptoType),

    #[error("Should encrypt but no recipient key found")]
    NoRecipientKey,

    #[error("Primary key not found")]
    PrimaryKeyNotFound,

    #[error("EO recipient present but no EoData supplied")]
    EoDataMissing,

    #[error("Failed to fetch EO SRP modulus: {0}")]
    EoModulusFetch(Box<dyn std::error::Error + Send + Sync>),

    #[error("Failed to build EO challenge or encrypt to password: {0}")]
    Eo(#[from] EoError),
}
