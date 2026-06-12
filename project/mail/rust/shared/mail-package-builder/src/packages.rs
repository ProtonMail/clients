use crate::attachment_entries::PackageAttachmentEntries;
use crate::body_convert::html_to_text;
use crate::eo::EoModulusProvider;
use crate::error::PackageError;
use crate::types::{
    AttachmentDisposition, BodyFormat, EoContainer, EoData, LoadedAttachment, SendType,
};
use mail_api::services::proton::prelude::AuthInput;
use mail_api::services::proton::request_data::{AddressSubPackage, Package, PackageSignaturesMode};
use mail_crypto_inbox::attachment::DecryptableAttachment;
use mail_crypto_inbox::eo::Challenge;
use mail_crypto_inbox::keys::{InboxSessionKey, PackageCryptoType, SendPreferences};
use mail_crypto_inbox::mail_crypto_inbox_mime::write::InboxMimeBuilder;
use mail_crypto_inbox::message::packages::{
    EncryptedPackageBody, PackageMimeType, package_body_encrypt,
};
use mail_crypto_inbox::proton_crypto::ProtonSRP;
use mail_proton_ids::PrivateEmail;
use proton_crypto_account::keys::{
    PrimaryUnlockedAddressKey, UnlockedAddressKey, UnlockedAddressKeys,
};
use proton_crypto_account::proton_crypto::crypto::PGPProviderSync;
use secrecy::ExposeSecret;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use tracing::{debug, instrument};

/// Per-recipient encryption strategy for re-encrypting attachment session keys
/// (and optionally signatures) inside `process_attachments`.
enum EncryptionTool<'a, P: PGPProviderSync> {
    /// Encrypt to the recipient's public key.
    PublicKey(&'a P::PublicKey),
    /// Encrypt to a shared password (Encrypted Outside / EO).
    Password(&'a str),
}

/// EO inputs after the modulus has been fetched. Crate-private.
struct ResolvedEo {
    eo_data: EoData,
    modulus: String,
    modulus_id: String,
}

/// Builds encrypted packages for a set of recipients.
///
/// The crate performs no direct I/O. When an Encrypted Outside (EO) recipient
/// is present, the SRP modulus needed for the SRP challenge is fetched via the
/// caller-provided `EoModulusProvider`; otherwise the provider is never
/// invoked and callers may pass `None`.
///
/// html↔text conversion is handled internally: callers pass the body in
/// whichever format they have (`body_format`) and the crate derives the
/// opposite format on demand for recipients that need it.
///
/// # Arguments
/// - `send_preferences`: pre-loaded encryption preferences per recipient email
///   (obtained via `mail-core-key-manager`'s `KeySelector`)
/// - `body`: the message body and its source format, bundled in `BodyFormat`
///   so the variant tag and the bytes can't drift apart.
/// - `attachments`: attachments already loaded into memory, including the
///   encrypted key packets and signatures needed for per-recipient
///   re-encryption on the `ProtonMail` and `Cleartext` paths. Taken by value
///   so the encrypted bytes can be moved into the MIME builder instead of
///   cloned per attachment.
/// - `eo_data`: required when any recipient has the `EncryptedOutside`
///   scheme. Carries the password and optional hint.
/// - `eo_modulus_provider`: required alongside `eo_data` for EO recipients.
///   The crate awaits one call to `get_auth_modulus()` when EO is in play.
///
/// # Returns
/// `Vec<Package>` ready for the Proton send API.
#[instrument(skip_all)]
pub async fn build_packages<P: PGPProviderSync, E: EoModulusProvider>(
    pgp: &P,
    send_type: SendType,
    address_keys: &UnlockedAddressKeys<P>,
    send_preferences: &HashMap<PrivateEmail, SendPreferences<P::PublicKey>>,
    body: BodyFormat,
    mut attachments: Vec<LoadedAttachment>,
    eo_container: Option<EoContainer<E>>,
) -> Result<Vec<Package>, PackageError> {
    let needs_eo = send_preferences
        .values()
        .any(|prefs| prefs.pgp_scheme == PackageCryptoType::EncryptedOutside);

    let resolved_eo = if needs_eo {
        let honest_eo_container = eo_container.ok_or(PackageError::EoDataMissing)?;
        let EoContainer {
            eo_data,
            eo_modulus_provider,
        } = honest_eo_container;

        let modulus = eo_modulus_provider
            .get_auth_modulus()
            .await
            .map_err(|e| PackageError::EoModulusFetch(Box::new(e)))?;
        Some(ResolvedEo {
            eo_data,
            modulus: modulus.modulus,
            modulus_id: modulus.modulus_id,
        })
    } else {
        None
    };

    let demanded_mime_types: HashSet<_> = send_preferences
        .values()
        .map(|pref| pref.mime_type)
        .collect();

    let primary = address_keys
        .primary_for_mail()
        .map_err(|_| PackageError::PrimaryKeyNotFound)?;

    let mut encrypted_bodies = Vec::with_capacity(demanded_mime_types.len());

    for mime_type in demanded_mime_types {
        let encrypted = match mime_type {
            PackageMimeType::Html => encrypt_html_body(pgp, &primary, &body)?,
            PackageMimeType::Text => encrypt_text_body(pgp, &primary, &body)?,
            PackageMimeType::Multipart => {
                encrypt_mime_body(pgp, &primary, &body, &mut attachments)?
            }
        };
        encrypted_bodies.push(encrypted);
    }

    let mut packages = Vec::with_capacity(encrypted_bodies.len());

    for encrypted_body in &encrypted_bodies {
        let matching_recipients: Vec<_> = send_preferences
            .iter()
            .filter(|(_, pref)| encrypted_body.mime_type == pref.mime_type)
            .inspect(|(email, _)| {
                debug!(
                    "build recipient {} top package for the {} body package",
                    email, encrypted_body.mime_type
                );
            })
            .collect();

        let package = build_top_package(
            send_type,
            pgp,
            address_keys,
            &matching_recipients,
            encrypted_body,
            &attachments,
            resolved_eo.as_ref(),
        )?;

        packages.push(package);
    }

    Ok(packages)
}

fn encrypt_html_body<P: PGPProviderSync>(
    pgp: &P,
    key: &PrimaryUnlockedAddressKey<P::PrivateKey, P::PublicKey>,
    body: &BodyFormat,
) -> Result<EncryptedPackageBody, PackageError> {
    debug!("encrypt package for html");
    let (BodyFormat::Html(bytes) | BodyFormat::PlainText(bytes)) = body;
    Ok(package_body_encrypt(
        pgp,
        key,
        PackageMimeType::Html,
        bytes.as_bytes(),
    )?)
}

fn encrypt_text_body<P: PGPProviderSync>(
    pgp: &P,
    key: &PrimaryUnlockedAddressKey<P::PrivateKey, P::PublicKey>,
    body: &BodyFormat,
) -> Result<EncryptedPackageBody, PackageError> {
    debug!("encrypt package for text");

    let text_buf;
    let text = match body {
        BodyFormat::PlainText(s) => s.as_str(),
        BodyFormat::Html(s) => {
            text_buf = html_to_text(s)?;
            text_buf.as_str()
        }
    };

    Ok(package_body_encrypt(
        pgp,
        key,
        PackageMimeType::Text,
        text.as_bytes(),
    )?)
}

fn encrypt_mime_body<P: PGPProviderSync>(
    pgp: &P,
    key: &PrimaryUnlockedAddressKey<P::PrivateKey, P::PublicKey>,
    body: &BodyFormat,
    attachments: &mut [LoadedAttachment],
) -> Result<EncryptedPackageBody, PackageError> {
    debug!("encrypt package for mime");

    let mut content = Vec::new();
    let mut builder = InboxMimeBuilder::new();

    let text_buf;
    match body {
        BodyFormat::Html(s) => {
            text_buf = html_to_text(s)?;
            builder = builder.html_body(s).text_body(text_buf.as_str());
        }
        BodyFormat::PlainText(s) => {
            builder = builder.text_body(s);
        }
    }

    for att in attachments {
        let data = std::mem::take(&mut att.data);
        match att.disposition {
            AttachmentDisposition::Attachment => {
                builder = builder.attachment(&att.filename, Some(att.mime_type.clone()), data);
            }
            AttachmentDisposition::Inline => {
                if let Some(content_id) = &att.content_id {
                    builder = builder.inline_attachment(
                        content_id,
                        &att.filename,
                        Some(att.mime_type.clone()),
                        data,
                    );
                } else {
                    builder = builder.attachment(&att.filename, Some(att.mime_type.clone()), data);
                }
            }
        }
    }

    builder
        .write_to(&mut content)
        .map_err(|err| PackageError::MimeBodyBuild(err.to_string()))?;

    Ok(package_body_encrypt(
        pgp,
        key,
        PackageMimeType::Multipart,
        &content,
    )?)
}

fn build_top_package<P: PGPProviderSync>(
    send_type: SendType,
    pgp: &P,
    sender_keys: &[UnlockedAddressKey<P>],
    recipient_preferences: &[(&PrivateEmail, &SendPreferences<P::PublicKey>)],
    encrypted_body: &EncryptedPackageBody,
    attachments: &[LoadedAttachment],
    resolved_eo: Option<&ResolvedEo>,
) -> Result<Package, PackageError> {
    let mut package = Package {
        body: Some(encrypted_body.encrypted_body.clone().into()),
        mime_type: encrypted_body.mime_type,
        addresses: HashMap::new(),
        package_type: 0,
        body_key: None,
        attachment_keys: None,
    };

    for (email, prefs) in recipient_preferences {
        build_address_sub_package(
            send_type,
            pgp,
            email,
            &mut package,
            &encrypted_body.session_key,
            attachments,
            sender_keys,
            prefs,
            resolved_eo,
        )?;
    }

    package.package_type = package
        .addresses
        .iter()
        .fold(0, |acc, (_, addr)| acc | addr.address_type.type_value());

    Ok(package)
}

#[expect(clippy::too_many_arguments)]
fn build_address_sub_package<P: PGPProviderSync>(
    send_type: SendType,
    pgp: &P,
    recipient_email: &PrivateEmail,
    top_package: &mut Package,
    body_session_key: &InboxSessionKey,
    attachments: &[LoadedAttachment],
    sender_keys: &[impl AsRef<P::PrivateKey>],
    prefs: &SendPreferences<P::PublicKey>,
    resolved_eo: Option<&ResolvedEo>,
) -> Result<(), PackageError> {
    let mut address_package = AddressSubPackage {
        address_type: prefs.pgp_scheme,
        body_key_packet: None,
        attachment_key_packets: None,
        attachment_enc_signatures: None,
        signature: None,
        token: None,
        enc_token: None,
        auth: None,
        password_hint: None,
    };

    match prefs.pgp_scheme {
        PackageCryptoType::ProtonMail | PackageCryptoType::PgpMime => {
            let recipient_key = prefs
                .selected_key
                .as_ref()
                .ok_or(PackageError::NoRecipientKey)?;

            let recipient_key_packet = body_session_key
                .encrypt_to_recipient(pgp, recipient_key)
                .map_err(PackageError::PackageBodyInfoReEncrypt)?;

            address_package.body_key_packet = Some(recipient_key_packet);

            // For ProtonMail we re-encrypt the attachment session keys towards
            // each recipient. For PgpMime, attachments are embedded in the
            // body and are not re-encrypted here.
            if prefs.pgp_scheme == PackageCryptoType::ProtonMail {
                process_attachments(
                    send_type,
                    pgp,
                    attachments,
                    sender_keys,
                    EncryptionTool::PublicKey(recipient_key),
                    prefs.sign,
                    &mut address_package,
                )?;
            }
        }

        PackageCryptoType::Cleartext => {
            top_package.body_key = Some(body_session_key.to_owned().into());

            process_attachment_cleartext(send_type, pgp, attachments, sender_keys, top_package)?;

            address_package.signature = Some(PackageSignaturesMode::None);
        }

        PackageCryptoType::ClearMime => {
            top_package.body_key = Some(body_session_key.to_owned().into());
            address_package.signature = Some(PackageSignaturesMode::from(prefs.sign));
        }

        PackageCryptoType::EncryptedOutside => {
            let eo = resolved_eo.ok_or(PackageError::EoDataMissing)?;

            build_address_package_for_eo(pgp, eo, body_session_key, &mut address_package)?;

            process_attachments(
                send_type,
                pgp,
                attachments,
                sender_keys,
                EncryptionTool::Password(eo.eo_data.password.expose_secret()),
                prefs.sign,
                &mut address_package,
            )?;
        }

        PackageCryptoType::PgpInline => {
            return Err(PackageError::NotSupported(prefs.pgp_scheme));
        }
    }

    top_package.addresses.insert(
        recipient_email.as_clear_text_str().to_owned(),
        address_package,
    );

    Ok(())
}

/// Populates the EO-specific fields on the address sub-package: SRP challenge
/// (`token`, `enc_token`, `auth`), password hint, and the body key packet
/// re-encrypted to the EO password.
fn build_address_package_for_eo<P: PGPProviderSync>(
    pgp: &P,
    eo: &ResolvedEo,
    body_session_key: &InboxSessionKey,
    address_package: &mut AddressSubPackage,
) -> Result<(), PackageError> {
    let srp = ProtonSRP::new_sync();
    let challenge =
        Challenge::generate(pgp, &srp, eo.eo_data.password.expose_secret(), &eo.modulus)?;

    address_package.password_hint = Some(eo.eo_data.password_hint.clone().unwrap_or_default());
    address_package.enc_token = Some(challenge.enc_token);
    address_package.token = Some(challenge.token.deref().to_string());
    address_package.auth = Some(AuthInput {
        version: challenge.verifier.version,
        modulus_id: eo.modulus_id.clone(),
        salt: challenge.verifier.salt,
        verifier: challenge.verifier.verifier,
    });

    address_package.body_key_packet = Some(
        body_session_key
            .encrypt_to_password(pgp, eo.eo_data.password.expose_secret())
            .map_err(PackageError::PackageBodyInfoReEncrypt)?,
    );

    Ok(())
}

/// Re-encrypts each attachment's session key (and optional signature) for one
/// recipient, building the per-address `attachment_key_packets` and
/// `attachment_enc_signatures` entries on `address_package`, and sets
/// `address_package.signature` to reflect whether attachments are signed.
///
/// Dispatches on `tool`: `PublicKey` re-encrypts to the recipient's public key
/// (ProtonMail path) and produces an `enc_signature` when the sender's signature
/// is present; `Password` re-encrypts to a shared EO password.
///
/// `sign` carries the caller's intent (`SendPreferences::sign`). It is
/// downgraded to `false` if any attachment carries no signature material at
/// all — i.e. both `signature` (plain detached signature) and `enc_signature`
/// (the same signature stored encrypted) are `None`. The two fields are
/// alternative storage forms of the same detached signature; exactly one is
/// typically populated, and either is sufficient to produce a per-recipient
/// signature here. If neither is present we can't ship one, so claiming
/// "signed attachments" on the wire would be misleading.
fn process_attachments<P: PGPProviderSync>(
    send_type: SendType,
    pgp: &P,
    attachments: &[LoadedAttachment],
    sender_keys: &[impl AsRef<P::PrivateKey>],
    tool: EncryptionTool<P>,
    mut sign: bool,
    address_package: &mut AddressSubPackage,
) -> Result<(), PackageError> {
    let mut attachment_key_packets = PackageAttachmentEntries::new(send_type);
    let mut attachment_enc_signatures = PackageAttachmentEntries::new(send_type);

    for (position, attachment) in attachments.iter().enumerate() {
        if attachment.signature.is_none() && attachment.enc_signature.is_none() {
            sign = false;
        }

        if attachment.key_packets.is_none() {
            return Err(PackageError::AttachmentMissingKeyPackets(
                attachment.local_id.clone(),
            ));
        }

        let attachment_info = attachment.decrypt_attachment_info(pgp, sender_keys)?;

        let recipient_attachment_kp = match tool {
            EncryptionTool::PublicKey(recipient_key) => {
                let kp = attachment_info
                    .encrypt_session_key_to_recipient(pgp, recipient_key)
                    .map_err(PackageError::PackageAttachmentInfoReEncrypt)?;

                if let Some(enc_signature) = attachment_info
                    .encrypt_signature_to_recipient(pgp, recipient_key)
                    .map_err(PackageError::PackageAttachmentInfoReEncryptSignature)?
                {
                    attachment_enc_signatures.insert(
                        position,
                        attachment,
                        enc_signature.encode_base64(),
                    )?;
                }

                kp
            }
            EncryptionTool::Password(password) => attachment_info
                .encrypt_session_key_to_password(pgp, password)
                .map_err(PackageError::PackageAttachmentInfoReEncrypt)?,
        };

        attachment_key_packets.insert(position, attachment, recipient_attachment_kp)?;
    }

    if !attachment_key_packets.is_empty() {
        address_package.attachment_key_packets = Some(attachment_key_packets.into());
    }
    if !attachment_enc_signatures.is_empty() {
        address_package.attachment_enc_signatures = Some(attachment_enc_signatures.into());
    }

    address_package.signature = Some(PackageSignaturesMode::from(sign));

    Ok(())
}

/// Exposes the attachment session keys at the top-level package when the
/// recipient is on the `Cleartext` path. The first cleartext recipient
/// populates `top_package.attachment_keys`; subsequent cleartext recipients
/// share the same exposed keys.
fn process_attachment_cleartext<P: PGPProviderSync>(
    send_type: SendType,
    pgp: &P,
    attachments: &[LoadedAttachment],
    sender_keys: &[impl AsRef<P::PrivateKey>],
    top_package: &mut Package,
) -> Result<(), PackageError> {
    if top_package.attachment_keys.is_some() {
        return Ok(());
    }

    let mut attachment_keys = PackageAttachmentEntries::new(send_type);

    for (position, attachment) in attachments.iter().enumerate() {
        if attachment.key_packets.is_none() {
            return Err(PackageError::AttachmentMissingKeyPackets(
                attachment.local_id.clone(),
            ));
        }

        let attachment_key = attachment
            .decrypt_attachment_info(pgp, sender_keys)?
            .session_key
            .into();

        attachment_keys.insert(position, attachment, attachment_key)?;
    }

    if !attachment_keys.is_empty() {
        top_package.attachment_keys = Some(attachment_keys.into());
    }

    Ok(())
}
