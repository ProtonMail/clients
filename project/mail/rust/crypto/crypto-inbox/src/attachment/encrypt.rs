use std::io::{self, Write};
use std::string::FromUtf8Error;

use proton_crypto_account::keys::PrimaryUnlockedAddressKey;
use proton_crypto_account::proton_crypto::CryptoError;
use proton_crypto_account::proton_crypto::crypto::{
    AsPublicKeyRef, DataEncoding, DetachedSignatureVariant, Encryptor, EncryptorSync,
    PGPProviderSync, SigningMode, WritingMode,
};

use crate::keys::{InboxSessionKey, KeyPacket, SessionKeyError};

use super::{ArmorEncodingError, BinaryAttachmentEncryptedSignature, BinaryAttachmentSignature};

/// Type for encryption metadata belonging to a specific encrypted attachment.
///
/// The type can contain an encrypted and unencrypted signature (legacy).
/// For legacy during the transition period, both have to be provided.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct EncryptedAttachmentMetadata {
    /// Optional attachment signature.
    pub signature: Option<BinaryAttachmentSignature>,
    /// Optional encrypted attachment signature.
    pub encrypted_signature: Option<BinaryAttachmentEncryptedSignature>,
    /// The encrypted session key for each recipient key.
    pub key_packets: Vec<u8>,
}

/// Represent an attachment that is encrypted.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct EncryptedAttachment {
    /// Encryption metadata needed for decryption and verification.
    pub metadata: EncryptedAttachmentMetadata,
    /// Encrypted attachment data.
    pub data: Vec<u8>,
}

/// Errors that can be returned in attachment encryption.
#[derive(Debug, thiserror::Error)]
pub enum AttachmentEncryptionError {
    #[error("Failed to encrypt and sign attachment: {0}")]
    Encryption(CryptoError),
    #[error("Failed to to convert to utf-8 string: {0}")]
    Encoding(#[from] FromUtf8Error),
    #[error("Failed to encrypt attached signature: {0}")]
    SignatureEncryption(CryptoError),
    #[error("No encryption keys provided")]
    NoKeys,
    #[error("No signing keys provided")]
    NoSigningKeys,
    #[error("Session key generation failed: {0}")]
    SessionKeyGeneration(CryptoError),
    #[error("Session key encryption failed: {0}")]
    SessionKeyEncryption(CryptoError),
    #[error("Invalid session key: {0}")]
    SessionKeyProblem(#[from] SessionKeyError),
    #[error("Encrypted signature armor: {0}")]
    Armor(#[from] ArmorEncodingError),
}

pub trait EncryptableAttachment {
    /// Returns the plaintext attachment data.
    fn attachment_data(&self) -> &[u8];

    /// Encrypts and signs an attachment with the primary address key.
    ///
    /// The output [`EncryptedAttachment`] consists of the encrypted attachment and the [`EncryptedAttachmentMetadata`]
    /// containing the key packets, signatures, and encrypted signature.
    fn attachment_encrypt_and_sign<P>(
        &self,
        pgp: &P,
        primary_address_key: &PrimaryUnlockedAddressKey<P::PrivateKey, P::PublicKey>,
    ) -> Result<EncryptedAttachment, AttachmentEncryptionError>
    where
        P: PGPProviderSync,
    {
        encrypt(pgp, primary_address_key, self.attachment_data())
    }
}

/// Encrypts and signs an attachment with the primary address key.
///
/// The output [`EncryptedAttachment`] consists of the encrypted attachment and the [`EncryptedAttachmentMetadata`]
/// containing the key packets, signatures, and encrypted signature.
pub fn encrypt<P>(
    pgp: &P,
    primary_address_key: &PrimaryUnlockedAddressKey<P::PrivateKey, P::PublicKey>,
    attachment_data: impl AsRef<[u8]>,
) -> Result<EncryptedAttachment, AttachmentEncryptionError>
where
    P: PGPProviderSync,
{
    encrypt_helper(
        pgp,
        &[primary_address_key.for_encryption()],
        primary_address_key.for_signing(),
        attachment_data,
    )
}

/// Encrypts an attachment to each key in `encryption_keys` and produces a signature for each key in `signing_keys`.
///
/// The output [`EncryptedAttachment`] consists of the encrypted attachment and the [`EncryptedAttachmentMetadata`]
/// containing the key packets, signatures, and encrypted signature.
/// If no signing keys are provided, i.e., a zero length slice, no signatures are produced.
fn encrypt_helper<P>(
    pgp: &P,
    encryption_keys: &[impl AsPublicKeyRef<P::PublicKey>],
    signing_keys: &[impl AsRef<P::PrivateKey>],
    attachment_data: impl AsRef<[u8]>,
) -> Result<EncryptedAttachment, AttachmentEncryptionError>
where
    P: PGPProviderSync,
{
    if encryption_keys.is_empty() {
        return Err(AttachmentEncryptionError::NoKeys);
    }

    // Generate a fresh PGP session key for encrypting the data and the signature.
    let session_key = pgp
        .new_encryptor()
        .with_encryption_key_refs(encryption_keys)
        .generate_session_key()
        .map_err(AttachmentEncryptionError::SessionKeyGeneration)?;

    // Encrypt the session key with all the provide encryption_keys.
    // The encrypted session key packets are called key packets for brevity.
    let key_packets = pgp
        .new_encryptor()
        .with_encryption_key_refs(encryption_keys)
        .encrypt_session_key(&session_key)
        .map_err(AttachmentEncryptionError::SessionKeyEncryption)?;

    // Encrypt the data with the session key and produce the detached signature.
    let (data, detached_signature_bytes) =
        encrypt_attachment_and_sign_detached(pgp, &session_key, signing_keys, attachment_data)?;

    // Encrypt the detached signature with the session key and attach the key packets.
    let encrypted_detached_signature =
        encrypt_detached_signature(pgp, &key_packets, &session_key, &detached_signature_bytes)?;

    let metadata = EncryptedAttachmentMetadata {
        signature: Some(detached_signature_bytes),
        encrypted_signature: Some(encrypted_detached_signature),
        key_packets,
    };

    Ok(EncryptedAttachment { metadata, data })
}

/// Encrypts and signs an attachment using streaming, reading the input from the specified source and writing the encrypted output to the given destination.
///
/// Returns metadata for the attachment, including the detached signature and key packets.
pub fn encrypt_and_sign_to_writer<'a, P, R, W>(
    pgp: &'a P,
    primary_address_key: &'a PrimaryUnlockedAddressKey<P::PrivateKey, P::PublicKey>,
    attachment_source: R,
    encrypted_dest: W,
) -> Result<EncryptedAttachmentMetadata, AttachmentEncryptionError>
where
    P: PGPProviderSync,
    W: Write + 'a,
    R: io::Read,
{
    encrypt_and_sign_to_writer_helper(
        pgp,
        primary_address_key.for_encryption(),
        primary_address_key.for_signing(),
        attachment_source,
        encrypted_dest,
    )
}

/// Streaming attachment encryption helper.
fn encrypt_and_sign_to_writer_helper<'a, P, R, W>(
    pgp: &'a P,
    encryption_key: &'a impl AsPublicKeyRef<P::PublicKey>,
    signing_keys: &'a [impl AsRef<P::PrivateKey>],
    attachment_source: R,
    encrypted_dest: W,
) -> Result<EncryptedAttachmentMetadata, AttachmentEncryptionError>
where
    P: PGPProviderSync,
    W: Write + 'a,
    R: io::Read,
{
    if signing_keys.is_empty() {
        return Err(AttachmentEncryptionError::NoSigningKeys);
    }

    // Generate a fresh PGP session key for encrypting the data and the signature.
    let session_key = pgp
        .new_encryptor()
        .with_encryption_key(encryption_key.as_public_key())
        .generate_session_key()
        .map_err(AttachmentEncryptionError::SessionKeyGeneration)?;

    // Encrypt the session key with all the provided encryption_keys.
    // The encrypted session key packets are called key packets for brevity.
    let key_packets = pgp
        .new_encryptor()
        .with_encryption_key(encryption_key.as_public_key())
        .encrypt_session_key(&session_key)
        .map_err(AttachmentEncryptionError::SessionKeyEncryption)?;

    let detached_message_data = pgp
        .new_encryptor()
        .with_session_key(session_key.clone())
        .with_signing_key_refs(signing_keys)
        .encrypt_to_writer(
            attachment_source,
            DataEncoding::Bytes,
            SigningMode::Detached(DetachedSignatureVariant::Plaintext),
            WritingMode::All,
            encrypted_dest,
        )
        .map_err(AttachmentEncryptionError::Encryption)?;

    let detached_signature_bytes = detached_message_data
        .detached_signature
        .ok_or(AttachmentEncryptionError::NoSigningKeys)?;

    // Encrypt the detached signature with the session key and attach the key packets.
    let encrypted_detached_signature =
        encrypt_detached_signature(pgp, &key_packets, &session_key, &detached_signature_bytes)?;

    Ok(EncryptedAttachmentMetadata {
        signature: Some(BinaryAttachmentSignature(detached_signature_bytes)),
        encrypted_signature: Some(encrypted_detached_signature),
        key_packets,
    })
}

fn encrypt_attachment_and_sign_detached<P>(
    pgp: &P,
    session_key: &P::SessionKey,
    signing_keys: &[impl AsRef<P::PrivateKey>],
    attachment_data: impl AsRef<[u8]>,
) -> Result<(Vec<u8>, BinaryAttachmentSignature), AttachmentEncryptionError>
where
    P: PGPProviderSync,
{
    let mut data = Vec::with_capacity(attachment_data.as_ref().len());

    let detached_signature_bytes = {
        // Encrypt the data and produce a detached signature.
        let detached_message_data = pgp
            .new_encryptor()
            .with_session_key_ref(session_key)
            .with_signing_key_refs(signing_keys)
            .encrypt_to_writer(
                attachment_data.as_ref(),
                DataEncoding::Bytes,
                SigningMode::Detached(DetachedSignatureVariant::Plaintext),
                WritingMode::All,
                &mut data,
            )
            .map_err(AttachmentEncryptionError::Encryption)?;

        detached_message_data
            .detached_signature
            .ok_or(AttachmentEncryptionError::NoSigningKeys)?
    };

    Ok((data, BinaryAttachmentSignature(detached_signature_bytes)))
}

// Encrypt the detached signature with the session key and attach the key packets.
fn encrypt_detached_signature<P>(
    pgp: &P,
    key_packets: &[u8],
    session_key: &P::SessionKey,
    detached_signature_bytes: &[u8],
) -> Result<BinaryAttachmentEncryptedSignature, AttachmentEncryptionError>
where
    P: PGPProviderSync,
{
    let mut encrypted_signature_bytes =
        Vec::with_capacity(key_packets.len() + detached_signature_bytes.len());

    // The encrypted pgp message consists of `key_packets || encrypted_data`
    encrypted_signature_bytes.extend_from_slice(key_packets);

    // Write encrypted_data.
    let _ = pgp
        .new_encryptor()
        .with_session_key_ref(session_key)
        .encrypt_to_writer(
            detached_signature_bytes,
            DataEncoding::Bytes,
            SigningMode::Inline,
            WritingMode::All,
            &mut encrypted_signature_bytes,
        )
        .map_err(AttachmentEncryptionError::SignatureEncryption)?;

    Ok(BinaryAttachmentEncryptedSignature(
        encrypted_signature_bytes,
    ))
}

/// Represents decrypted attachment information that can be re-encrypted for new recipients.
pub struct ExtractedAttachmentInfo {
    /// The decrypted session key used to encrypt the attachment.
    ///
    /// [`InboxSessionKey`] provides methods to re-encrypt the session key for new recipients.
    pub session_key: InboxSessionKey,
    /// Internal attachment detached signature bytes.
    pub(crate) detached_signature_bytes: Option<BinaryAttachmentSignature>,
}

impl ExtractedAttachmentInfo {
    /// Creates a key packet for the provided recipient public key.
    ///
    /// Encrypts the internal symmetric session key with the provided public key
    /// using `OpenPGP`. The output is an `OpenPGP` PKESK packet (referred to as a key packet in the Proton context).
    pub fn encrypt_session_key_to_recipient<P>(
        &self,
        pgp: &P,
        recipient_key: &impl AsPublicKeyRef<P::PublicKey>,
    ) -> Result<KeyPacket, SessionKeyError>
    where
        P: PGPProviderSync,
    {
        self.session_key.encrypt_to_recipient(pgp, recipient_key)
    }

    /// Creates a key packet for the provided passphrase.
    ///
    /// Encrypts the internal symmetric session key with the provided passphrase
    /// using `OpenPGP`. The output is an `OpenPGP` SKESK packet (referred to as a key packet in the Proton context).
    pub fn encrypt_session_key_to_password<P>(
        &self,
        pgp: &P,
        passphrase: &str,
    ) -> Result<KeyPacket, SessionKeyError>
    where
        P: PGPProviderSync,
    {
        self.session_key.encrypt_to_password(pgp, passphrase)
    }

    /// Encrypts the internal signature towards a new recipient if present.
    pub fn encrypt_signature_to_recipient<P>(
        &self,
        pgp: &P,
        recipient: &P::PublicKey,
    ) -> Result<Option<BinaryAttachmentEncryptedSignature>, AttachmentEncryptionError>
    where
        P: PGPProviderSync,
    {
        let Some(signature_bytes) = &self.detached_signature_bytes else {
            return Ok(None);
        };

        let encrypted_signature = pgp
            .new_encryptor()
            .with_encryption_key(recipient)
            .encrypt_raw(&**signature_bytes, DataEncoding::Bytes)
            .map(BinaryAttachmentEncryptedSignature)
            .map_err(AttachmentEncryptionError::SignatureEncryption)?;

        Ok(Some(encrypted_signature))
    }

    /// Returns the internal signature as an binary PGP Signature without encrypting it.
    #[must_use]
    pub fn signature<Provider: PGPProviderSync>(&self) -> Option<&BinaryAttachmentSignature> {
        self.detached_signature_bytes.as_ref()
    }
}
