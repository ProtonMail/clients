use crate::error::PackageError;
use crate::types::{LoadedAttachment, SendType};
use mail_api::services::proton::common::AttachmentId;
use mail_api::services::proton::prelude as api;
use std::collections::HashMap;

/// Builder for [`api::PackageAttachmentEntries`]. Indexing differs by send type:
///
/// - For `SendType::Draft`, the message and attachments already exist on the
///   server, so the entries map is keyed by remote attachment id. An attachment
///   without a remote id is a programmer error and reported as
///   `AttachmentHasNoRemoteId`.
///
/// - For `SendType::Direct`, the message and attachments are created in the
///   same API call, so entries are inserted by position. The server assigns
///   remote ids in the response. An attachment that already has a remote id
///   under `Direct` is a programmer error and reported as
///   `AttachmentAlreadyHasRemoteId`.
#[derive(Clone, Debug, PartialEq)]
pub enum PackageAttachmentEntries<T> {
    Draft(HashMap<AttachmentId, T>),
    Direct(Vec<T>),
}

impl<T> PackageAttachmentEntries<T> {
    pub fn new(ty: SendType) -> Self {
        match ty {
            SendType::Draft => Self::Draft(HashMap::new()),
            SendType::Direct => Self::Direct(Vec::new()),
        }
    }

    pub fn insert(
        &mut self,
        position: usize,
        att: &LoadedAttachment,
        entry: T,
    ) -> Result<(), PackageError> {
        match (self, att.remote_id.as_ref()) {
            (Self::Draft(_), None) => Err(PackageError::AttachmentHasNoRemoteId(position)),

            (Self::Draft(entries), Some(id)) => {
                entries.insert(id.clone(), entry);
                Ok(())
            }

            (Self::Direct(entries), None) => {
                entries.push(entry);
                Ok(())
            }

            (Self::Direct(_), Some(_)) => Err(PackageError::AttachmentAlreadyHasRemoteId(position)),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Draft(entries) => entries.is_empty(),
            Self::Direct(entries) => entries.is_empty(),
        }
    }
}

impl<T> From<PackageAttachmentEntries<T>> for api::PackageAttachmentEntries<T> {
    fn from(this: PackageAttachmentEntries<T>) -> Self {
        match this {
            PackageAttachmentEntries::Draft(entries) => Self::Draft(
                entries
                    .into_iter()
                    .map(|(id, entry)| (id.to_string(), entry))
                    .collect(),
            ),

            PackageAttachmentEntries::Direct(entries) => Self::Direct(entries),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AttachmentDisposition;

    fn att(remote_id: Option<&str>) -> LoadedAttachment {
        LoadedAttachment {
            filename: String::new(),
            mime_type: String::new(),
            data: Vec::new(),
            disposition: AttachmentDisposition::Attachment,
            content_id: None,
            local_id: String::new(),
            remote_id: remote_id.map(AttachmentId::from),
            key_packets: None,
            signature: None,
            enc_signature: None,
        }
    }

    #[track_caller]
    fn assert_eq(lhs: Result<(), PackageError>, rhs: Result<(), PackageError>) {
        assert_eq!(
            lhs.map_err(|err| err.to_string()),
            rhs.map_err(|err| err.to_string()),
        );
    }

    #[test]
    fn draft() {
        let mut target = PackageAttachmentEntries::new(SendType::Draft);

        assert_eq(
            Ok(()),
            target.insert(0, &att(Some("d5nBB4MV")), "some payload"),
        );
        assert_eq(
            Err(PackageError::AttachmentHasNoRemoteId(1)),
            target.insert(1, &att(None), "some payload"),
        );
    }

    #[test]
    fn direct() {
        let mut target = PackageAttachmentEntries::new(SendType::Direct);

        assert_eq(Ok(()), target.insert(0, &att(None), "some payload"));
        assert_eq(
            Err(PackageError::AttachmentAlreadyHasRemoteId(1)),
            target.insert(1, &att(Some("d5nBB4MV")), "some payload"),
        );
    }
}
