//! Bridge between `mail-common`'s domain types and the shared
//! `mail-package-builder` crate.
//!
//! Owns the conversion in this direction so the shared crate stays free of
//! `mail-common` dependencies.

use crate::MailUserContext;
use crate::datatypes::{Disposition, MimeType};
use crate::draft::PackageError;
use crate::draft::send::{EoData, MailType};
use crate::models::Attachment;
use mail_account_api::{AccountApi, ApiError};
use mail_core_api::services::proton::PrivateEmail;
use mail_crypto_inbox::keys::SendPreferences;
use mail_crypto_inbox::message::packages::PackageMimeType;
use mail_package_builder as shared;
use mail_stash::stash::Tether;
use proton_crypto_account::proton_crypto::crypto::PGPProviderSync;
use std::collections::HashMap;
use tracing::{Instrument, debug_span, error};

impl From<MailType> for shared::SendType {
    fn from(value: MailType) -> Self {
        match value {
            MailType::Draft => Self::Draft,
            MailType::Direct => Self::Direct,
        }
    }
}

/// Wraps the message body in `shared::BodyFormat` matching the source
/// `MimeType`. mail-common feeds only `TextHtml` or `TextPlain` into
/// `build_packages`; any other value indicates the body has already been
/// encoded to HTML.
pub(super) fn to_shared_body(mime_type: MimeType, body: &str) -> shared::BodyFormat {
    match mime_type {
        MimeType::TextPlain => shared::BodyFormat::PlainText(body.to_owned()),
        _ => shared::BodyFormat::Html(body.to_owned()),
    }
}

impl From<Disposition> for shared::AttachmentDisposition {
    fn from(value: Disposition) -> Self {
        match value {
            Disposition::Attachment => Self::Attachment,
            Disposition::Inline => Self::Inline,
        }
    }
}

/// Pre-loads attachment cleartext when at least one recipient demands a
/// `Multipart` body and skips the I/O otherwise. Mirrors the implicit gating
/// that lived inside `generate_mime_top_package` before the shared-crate
/// migration.
pub(super) async fn hydrate_attachments<P>(
    context: &MailUserContext,
    tether: &mut Tether,
    attachments: &[Attachment],
    send_preferences: &HashMap<PrivateEmail, SendPreferences<P::PublicKey>>,
) -> Result<Vec<shared::LoadedAttachment>, PackageError>
where
    P: PGPProviderSync,
{
    let needs_cleartext = send_preferences
        .values()
        .any(|prefs| matches!(prefs.mime_type, PackageMimeType::Multipart));

    let mut loaded = Vec::with_capacity(attachments.len());

    for attachment in attachments {
        if attachment.attachment_type.is_pgp() {
            continue;
        }

        let data = if needs_cleartext {
            attachment
                .content_data(context, tether)
                .instrument(debug_span!(
                    "mime_package::get_attachment_content_data",
                    id = ?attachment.local_id,
                ))
                .await
                .map_err(|err| {
                    error!("Failed to read attachment file: {err:?}");
                    PackageError::AttachmentLoad(Box::new(err))
                })?
        } else {
            Vec::new()
        };

        loaded.push(shared::LoadedAttachment {
            filename: attachment.filename.clone(),
            mime_type: attachment.mime_type.to_string(),
            data,
            disposition: attachment.disposition.into(),
            content_id: attachment.content_id.as_ref().map(|c| c.to_string()),
            local_id: attachment
                .local_id
                .expect("attachment must have a local id when building send packages")
                .to_string(),
            remote_id: attachment.remote_id(),
            key_packets: attachment.key_packets.as_ref().map(|kp| kp.value.clone()),
            signature: attachment.signature.as_ref().map(|s| s.value.clone()),
            enc_signature: attachment.enc_signature.as_ref().map(|s| s.value.clone()),
        });
    }

    Ok(loaded)
}

#[async_trait::async_trait]
impl shared::EoModulusProvider for MailUserContext {
    type Error = ApiError;
    async fn get_auth_modulus(&self) -> Result<shared::EoModulus, Self::Error> {
        let resp = self.session().get_auth_modulus().await?;
        Ok(shared::EoModulus {
            modulus: resp.modulus,
            modulus_id: resp.modulus_id,
        })
    }
}

/// Converts mail-common's `EoData` to the shared crate's `EoData`.
pub(super) fn to_shared_eo_data(eo_data: EoData) -> shared::EoData {
    shared::EoData {
        password: eo_data.password,
        password_hint: eo_data.password_hint,
    }
}

/// Translates a shared-crate `PackageError` into mail-common's superset error
/// type. `AttachmentMissingKeyPackets` carries the stringified local id that
/// the integrator wrote into `LoadedAttachment.local_id`; we parse it back to
/// `LocalAttachmentId` (or fall back to `AttachmentHasNoLocalId` when the
/// hydrator had no id to record).
pub(super) fn translate_package_error(err: shared::PackageError) -> PackageError {
    use crate::datatypes::LocalAttachmentId;
    use shared::PackageError as S;

    match err {
        S::PackageBodyEncrypt(e) => PackageError::PackageBodyEncrypt(e),
        S::AttachmentMissingKeyPackets(local_id) => match local_id.parse::<u64>() {
            Ok(id) => PackageError::AttachmentMissingKeyPackets(LocalAttachmentId::from(id)),
            Err(_) => PackageError::AttachmentHasNoLocalId,
        },
        S::AttachmentHasNoRemoteId(_) => PackageError::AttachmentHasNoRemoteId,
        S::AttachmentAlreadyHasRemoteId(_) => PackageError::AttachmentAlreadyHasRemoteId,
        S::MimeBodyBuild(msg) => PackageError::MimeBodyBuild(msg),
        S::HtmlToTextConversion(msg) => PackageError::HtmlToTextConversion(msg),
        S::PackageBodyInfoReEncrypt(e) => PackageError::PackageBodyInfoReEncrypt(e),
        S::PackageAttachmentInfo(e) => PackageError::PackageAttachmentInfo(e),
        S::PackageAttachmentInfoReEncrypt(e) => PackageError::PackageAttachmentInfoReEncrypt(e),
        S::PackageAttachmentInfoReEncryptSignature(e) => {
            PackageError::PackageAttachmentInfoReEncryptSignature(e)
        }
        S::NotSupported(scheme) => PackageError::NotSupported(scheme),
        S::NoRecipientKey => PackageError::NoRecipientKey,
        S::PrimaryKeyNotFound => PackageError::PrimaryKeyNotFound,
        S::EoDataMissing => PackageError::PackageEoPasswordMissing,
        S::EoModulusFetch(e) => match e.downcast::<ApiError>() {
            Ok(api_err) => PackageError::ModulusRequest(*api_err),
            Err(other) => {
                error!("EO modulus fetch failed with non-ApiError: {other}");
                PackageError::PackageEoPasswordMissing
            }
        },
        S::Eo(e) => PackageError::PackageEo(e),
    }
}
