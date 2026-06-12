use mail_api::services::proton::common::AttachmentId;
use mail_crypto_inbox::attachment::{
    AttachmentEncryptedSignature, AttachmentSignature, DecryptableAttachment, KeyPackets,
};
use secrecy::SecretString;

use crate::EoModulusProvider;

/// An attachment with its encrypted content already loaded into memory.
/// PGP-typed attachments must not be passed to `build_packages()`.
#[derive(Clone, Debug)]
pub struct LoadedAttachment {
    pub filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
    pub disposition: AttachmentDisposition,
    pub content_id: Option<String>,
    pub local_id: String,
    pub remote_id: Option<AttachmentId>,
    pub key_packets: Option<KeyPackets>,
    pub signature: Option<AttachmentSignature>,
    pub enc_signature: Option<AttachmentEncryptedSignature>,
}

impl DecryptableAttachment for LoadedAttachment {
    fn attachment_key_packets(&self) -> &KeyPackets {
        // Callers in `process_attachments` / `process_attachment_cleartext`
        // check `key_packets.is_none()` and return an error before reaching here.
        self.key_packets
            .as_ref()
            .expect("attachment_key_packets called without key_packets present")
    }

    fn attachment_signature(&self) -> Option<&AttachmentSignature> {
        self.signature.as_ref()
    }

    fn attachment_encrypted_signature(&self) -> Option<&AttachmentEncryptedSignature> {
        self.enc_signature.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentDisposition {
    Attachment,
    Inline,
}

/// The message body, carried in its source format. Each variant embeds the
/// content so the format and the bytes can't be mismatched at the call site.
/// When a recipient requires the opposite format, the crate converts
/// internally via `mail-html-transformer`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BodyFormat {
    PlainText(String),
    Html(String),
}

/// Whether we are sending as a draft (message already exists on server) or
/// as a direct send (message + attachments created in one API call).
///
/// This affects how attachment key packets are indexed: by remote attachment
/// ID for drafts, by position for direct sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendType {
    Draft,
    Direct,
}

/// User-supplied inputs for an `EncryptedOutside` (EO, password-protected)
/// recipient: the password the recipient will use to decrypt, and an optional
/// hint shown alongside the password challenge.
///
/// The SRP modulus needed by the encryption is fetched lazily by the crate via
/// the caller-provided `EoModulusProvider`.
#[derive(Clone, Debug)]
pub struct EoData {
    pub password: SecretString,
    pub password_hint: Option<String>,
}

#[derive(Clone, Debug)]
pub struct EoContainer<E: EoModulusProvider> {
    pub eo_data: EoData,
    pub eo_modulus_provider: E,
}
