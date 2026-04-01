//! This module is responsible for determining the preferences used when sending an email to a specific address.
//! These preferences include whether the email should be encrypted or signed, which public key to use if encryption is needed,
//! and the appropriate PGP scheme and email format (such as the MIME type of the message).
//!
//! The implementation of this module follows the guidelines detailed in the
//! [Confluence Sending Preferences](https://confluence.protontech.ch/display/MAILFE/Send+preferences+for+outgoing+email).
//!
//! It is important to note that this logic is inherently complex, involving multiple steps and numerous edge cases.
//! To ensure consistent behavior and avoid discrepancies, we have chosen to closely mirror the implementation found in the web inbox implementation.
//!
//! In future iterations, we may explore opportunities to simplify this code, keeping it in sync with potential changes in the web implementation.

use std::fmt::Display;

use proton_crypto_account::{
    keys::{
        ContactType, CryptoMailSettings, DecryptedAddressKey, EmailMimeType, EncryptionPreferences,
        PGPScheme, PinnedPublicKeys, PublicAddressKeys, RecipientPublicKeyModel,
    },
    proton_crypto::{
        crypto::{PrivateKey, PublicKey, UnixTimestamp},
        keytransparency::KTVerificationResult,
    },
};
use serde_repr::Serialize_repr;

use crate::{keys::SendPreferencesError, message::packages::PackageMimeType};

use super::CryptoPackageTypeError;

/// Contains the preferences a user can select in the composer to
/// change the behavior of email encryption.
#[derive(Debug, Default, PartialEq, Eq, Copy, Clone, Hash)]
pub struct ComposerPreference {
    /// Indicates if encrypt to outside is enabled.
    ///
    /// The Encrypt to outside (EO) mode encrypts email with a password.
    /// See [web](https://proton.me/support/password-protected-emails).
    pub encrypt_to_outside: bool,

    /// The mime type of the message in the composer.
    pub composer_body_mime_type: EmailMimeType,
}

impl ComposerPreference {
    #[must_use]
    pub fn new(composer_body_mime_type: EmailMimeType) -> Self {
        Self {
            encrypt_to_outside: false,
            composer_body_mime_type,
        }
    }
}

/// All possible encryption types as requested by the Proton API.

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Hash, Serialize_repr)]
#[repr(u8)]
pub enum PackageCryptoType {
    /// Encrypted using `ProtonMail`'s native encryption.
    ProtonMail = 1,

    /// Encrypted with a password for users outside of `ProtonMail`'s system.
    EncryptedOutside = 2,

    /// Message is not encrypted and is in plain text.
    #[default]
    Cleartext = 4,

    /// PGP encryption using inline PGP format.
    PgpInline = 8,

    /// PGP encryption using MIME format.
    PgpMime = 16,

    /// Cleartext message with MIME formatting.
    ClearMime = 32,
}

impl Display for PackageCryptoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageCryptoType::ProtonMail => f.write_str("proton mail"),
            PackageCryptoType::EncryptedOutside => f.write_str("encrypt to outside"),
            PackageCryptoType::Cleartext => f.write_str("cleartext"),
            PackageCryptoType::PgpInline => f.write_str("pgp inline"),
            PackageCryptoType::PgpMime => f.write_str("pgp mime"),
            PackageCryptoType::ClearMime => f.write_str("pgp clear mime"),
        }
    }
}

impl PackageCryptoType {
    #[must_use]
    pub fn type_value(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub fn enum_of(value: u8) -> Option<PackageCryptoType> {
        match value {
            1 => Some(Self::ProtonMail),
            2 => Some(Self::EncryptedOutside),
            4 => Some(Self::Cleartext),
            8 => Some(Self::PgpInline),
            16 => Some(Self::PgpMime),
            32 => Some(Self::ClearMime),
            _ => None,
        }
    }

    #[must_use]
    pub fn from_scheme(
        scheme: PGPScheme,
        encrypt: bool,
        sign: bool,
        eo: bool,
    ) -> PackageCryptoType {
        if eo {
            return PackageCryptoType::EncryptedOutside;
        }
        match scheme {
            PGPScheme::PGPMime => {
                if !encrypt && sign {
                    PackageCryptoType::ClearMime
                } else if encrypt {
                    PackageCryptoType::PgpMime
                } else {
                    PackageCryptoType::Cleartext
                }
            }
            PGPScheme::PGPInline => {
                if encrypt {
                    PackageCryptoType::PgpInline
                } else {
                    PackageCryptoType::Cleartext
                }
            }
        }
    }
}

impl TryFrom<u8> for PackageCryptoType {
    type Error = CryptoPackageTypeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::enum_of(value).ok_or(CryptoPackageTypeError::Parse(value))
    }
}

/// Represents the preferences for sending an email, including encryption, signing, and formatting options.
///
/// This type is used to determine the Package for encrypting and sending an email to a recipient.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct SendPreferences<Pub: PublicKey> {
    /// Indicates whether the email should be encrypted (`true`) or sent unencrypted (`false`).
    ///
    /// If `true`, the email content will be encrypted using the selected public key. If `false`,
    /// the email will be sent in plaintext.
    pub encrypt: bool,

    /// Indicates whether the email should be signed (`true`) or sent unsigned (`false`).
    ///
    /// If `true`, the email will be signed with the sender's private key, allowing the recipient
    /// to verify the authenticity and integrity of the message.
    pub sign: bool,

    /// Specifies the Proton encryption scheme to be used in the email package.
    pub pgp_scheme: PackageCryptoType,

    /// Specifies the MIME type for formatting the email body in each email package.
    pub mime_type: PackageMimeType,

    /// Optionally stores the selected public key for encryption.
    ///
    /// This field contains the public key that will be used to encrypt the email content if
    /// encryption is enabled. It is `None` if encryption is not required or if no suitable
    /// public key was found.
    pub selected_key: Option<Pub>,

    /// Indicates whether the selected key is pinned.
    ///
    /// A pinned key is one that has been manually selected/trusted and, thus, the security of the key does
    /// not rely on trusting the server serving the right key.
    pub is_selected_key_pinned: bool,

    /// Indicates whether an internal user has disabled encryption explicitly.
    ///
    /// See [here](https://proton.me/support/manage-encryption) how to configure this.
    pub encryption_disabled: bool,

    /// Result of the key transparency verification process.
    pub key_transparency_verification: KTVerificationResult,
}

impl<Pub: PublicKey> Display for SendPreferences<Pub> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(encrypt={} sign={}", self.encrypt, self.sign)?;
        write!(f, " pgp_scheme=\"{}\"", self.pgp_scheme)?;
        write!(f, " mime_type={}", self.mime_type)?;
        write!(f, " has_key={}", self.selected_key.is_some())?;
        write!(f, " pinned={}", self.is_selected_key_pinned)?;
        write!(f, " encryption_disabled={})", self.encryption_disabled)
    }
}

impl<Pub: PublicKey> SendPreferences<Pub> {
    /// Creates an instance of [`SendPreferences`] based on the given encryption preferences and composer settings input.
    ///
    /// This function determines the final send preferences for an email by combining the provided encryption
    /// preferences with additional settings from the composer. It derives the package mode of encryption and the
    /// MIME type of the package body.
    /// See [confluence](https://confluence.protontech.ch/display/MAILFE/Send+preferences+for+outgoing+email) for more details on the logic.
    pub fn from_preferences(
        encryption_preferences: EncryptionPreferences<Pub>,
        composer_preferences: ComposerPreference,
    ) -> Self {
        let encrypt;
        let sign;

        if encryption_preferences.encryption_disabled_mail {
            encrypt = false;
            sign = false;
        } else {
            encrypt = encryption_preferences.encrypt || composer_preferences.encrypt_to_outside;
            sign = composer_preferences.encrypt_to_outside || encryption_preferences.sign;
        }

        // Select encryption mode for the package.
        //
        // Note that we can't dispatch `ProtonMail` packages if the encryption
        // is disabled for that recipient - that's going to be the case for
        // emails that are forwarded from a Proton address to a non-Proton one.
        //
        // (if we'd encrypted that message, the server wouldn't be able to
        // forward it, since it wouldn't know how to perform the decryption.)
        let pgp_scheme = if encryption_preferences.contact_type == ContactType::Internal
            && !encryption_preferences.encryption_disabled_mail
        {
            PackageCryptoType::ProtonMail
        } else {
            let scheme = PackageCryptoType::from_scheme(
                encryption_preferences.pgp_scheme,
                encrypt,
                sign,
                composer_preferences.encrypt_to_outside,
            );

            // Force PGP mime as inline pgp is not supported currently
            if scheme == PackageCryptoType::PgpInline {
                PackageCryptoType::PgpMime
            } else {
                scheme
            }
        };

        // Select the mime type of email body.
        let mime_type = match pgp_scheme {
            PackageCryptoType::PgpInline => PackageMimeType::Text,
            PackageCryptoType::PgpMime | PackageCryptoType::ClearMime => PackageMimeType::Multipart,
            // If sending EO, respect the MIME type of the composer, since it will be what the API returns when retrieving the message.
            PackageCryptoType::EncryptedOutside => {
                composer_preferences.composer_body_mime_type.into()
            }
            PackageCryptoType::Cleartext | PackageCryptoType::ProtonMail => {
                // composer html -> contact text = text
                // composer text -> contact html = text
                // composer html -> contact auto/none = html
                // composer text -> contact auto/none = text
                match composer_preferences.composer_body_mime_type {
                    EmailMimeType::Text => composer_preferences.composer_body_mime_type.into(), // Prefer composer preference if message body is text/plain
                    EmailMimeType::Html => encryption_preferences
                        .mime_type
                        .unwrap_or(composer_preferences.composer_body_mime_type)
                        .into(),
                }
            }
        };

        Self {
            encrypt,
            sign,
            pgp_scheme,
            mime_type,
            selected_key: encryption_preferences.selected_key,
            is_selected_key_pinned: encryption_preferences.is_selected_key_pinned,
            encryption_disabled: encryption_preferences.encryption_disabled_mail,
            key_transparency_verification: encryption_preferences.key_transparency_verification,
        }
    }

    /// Creates a new [`SendPreferences`] instance by first determining the recipient's public key model,
    /// then deriving the encryption preferences from it, and finally adjusting the send preferences based on
    /// additional parameters.
    ///
    /// This function orchestrates the process of generating the appropriate send preferences for an email by
    /// utilizing the recipient's public keys, the user's mail settings, and various flags related to encryption
    /// and signing.
    /// See [confluence](https://confluence.protontech.ch/display/MAILFE/Send+preferences+for+outgoing+email) for more details on the logic.
    ///
    /// # Errors
    ///
    /// An [`EncryptionPreferencesError`] if the key selection fails.
    /// An [`EncryptionPreferencesError::ApiKeyNotPinned`] is thrown if there are pinned keys, but none of the fingerprints of the pinned keys matches
    /// the fingerprint of one of the keys served by the API.
    /// In this case the client should force the user (via a modal) to trust one of the keys served by the API before sending any email.
    pub fn new(
        api_keys: PublicAddressKeys<Pub>,
        pinned_keys: Option<PinnedPublicKeys<Pub>>,
        encryption_time: UnixTimestamp,
        crypto_mail_settings: &CryptoMailSettings,
        composer_preferences: ComposerPreference,
    ) -> Result<Self, SendPreferencesError> {
        let recipient_key_model = RecipientPublicKeyModel::from_public_keys_at_time(
            api_keys,
            pinned_keys,
            encryption_time,
            true, // prefer v6/pqc keys for mail.
        );

        let encryption_preferences = EncryptionPreferences::from_key_model_and_settings(
            recipient_key_model,
            crypto_mail_settings,
        )?;

        Ok(SendPreferences::from_preferences(
            encryption_preferences,
            composer_preferences,
        ))
    }

    /// Creates a new [`SendPreferences`] instance for an internal sender by determining the encryption preferences
    /// based on the sender's address keys and the provided mail settings.
    ///
    /// This function is specifically designed for creating send preferences for users sending to themselves, where encryption and
    /// signing are generally enabled by default.
    pub fn new_for_self<Priv: PrivateKey>(
        is_address_external: bool,
        address_keys: &[DecryptedAddressKey<Priv, Pub>],
        encryption_time: UnixTimestamp,
        crypto_mail_settings: CryptoMailSettings,
        composer_preferences: ComposerPreference,
    ) -> Result<Self, SendPreferencesError> {
        let encryption_preferences =
            EncryptionPreferences::from_unlocked_address_keys_and_settings(
                is_address_external,
                address_keys,
                crypto_mail_settings,
                encryption_time,
            )?;

        Ok(SendPreferences::from_preferences(
            encryption_preferences,
            composer_preferences,
        ))
    }
}

impl<Pub: PublicKey> From<EncryptionPreferences<Pub>> for SendPreferences<Pub> {
    fn from(value: EncryptionPreferences<Pub>) -> Self {
        Self::from_preferences(value, ComposerPreference::default())
    }
}
