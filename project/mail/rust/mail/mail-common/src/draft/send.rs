mod adapter;

use crate::datatypes::{LocalMessageId, MimeType};
use crate::draft::recipients::ValidationState;
use crate::draft::{CancelScheduleSendError, PackageError, SendError};
use crate::models::{Attachment, DraftMetadata, Message};
use crate::{AppError, MailContextError, MailContextResult, MailUserContext};
use anyhow::anyhow;
use chrono::{DateTime, Datelike, Days, Local, LocalResult, NaiveTime};
use mail_action_queue::observers::ActionAwaiter;
use mail_action_queue::queue::{BroadcastMessage, Queue, QueuedError};
use mail_api::services::proton::ProtonMail;
use mail_api::services::proton::request_data::Package;
use mail_core_api::consts::Mail;
use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::{PrivateEmail, PrivateEmailRef};
use mail_core_api::session::Session;
use mail_core_common::models::{ModelExtension, User};
use mail_core_common::services::NetworkMonitorService;
use mail_core_common::services::crypto_key_service::mail_core_key_manager::error::{
    KeyHandlingError, LoadingError,
};
use mail_core_common::services::crypto_key_service::mail_core_key_manager::{
    PublicAddressKeyApiFetchPolicy, PublicAddressKeyContactFetchPolicy,
};
use mail_crypto_inbox::keys::{ComposerPreference, SendPreferences};
use mail_stash::UserDb;
use mail_stash::orm::Model;
use mail_stash::stash::Tether;
use proton_crypto_account::keys::{CryptoMailSettings, UnlockedAddressKeys};
use proton_crypto_account::proton_crypto::crypto::PGPProviderSync;
use secrecy::SecretString;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, instrument};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailType {
    Draft,
    Direct,
}

/// Encrypt e-mail with password composer input (EO)
pub struct EoData {
    pub password: SecretString,
    pub password_hint: Option<String>,
}

#[instrument(skip_all)]
pub async fn load_prefs<P>(
    context: &MailUserContext,
    pgp: &P,
    tether: &Tether,
    recipient_emails: &[PrivateEmail],
    crypto_mail_settings: CryptoMailSettings,
    composer_preference: ComposerPreference,
) -> MailContextResult<HashMap<PrivateEmail, SendPreferences<P::PublicKey>>>
where
    P: PGPProviderSync,
{
    let mut send_preferences = HashMap::with_capacity(recipient_emails.len());

    for recipient in recipient_emails {
        let send_preference = context
            .recipient_send_preferences(
                pgp,
                tether,
                PrivateEmailRef::new(recipient.as_clear_text_str()),
                crypto_mail_settings,
                composer_preference,
                PublicAddressKeyApiFetchPolicy::RequireSync,
                PublicAddressKeyContactFetchPolicy::RequireSync,
            )
            .await
            .map_err(|err| {
                error!(
                    "Failed to load send preferences for recipient {}: {}",
                    recipient, err
                );

                if let MailContextError::KeySelection(KeyHandlingError::Loading(
                    LoadingError::Api(err),
                )) = &err
                {
                    match ValidationState::from(err) {
                        ValidationState::InvalidEmail => {
                            return SendError::SendMessage(PackageError::RecipientEmailInvalid(
                                recipient.clone(),
                            ))
                            .into();
                        }
                        ValidationState::DoesNotExist => {
                            return SendError::SendMessage(
                                PackageError::ProtonRecipientDoesNotExist(recipient.clone()),
                            )
                            .into();
                        }
                        ValidationState::Unknown => {
                            return SendError::SendMessage(PackageError::RecipientEmailInvalid(
                                recipient.clone(),
                            ))
                            .into();
                        }
                        _ => {}
                    }
                }
                err
            })?;

        debug!("{} recipient preferences: {}", recipient, send_preference);

        send_preferences.insert(recipient.clone(), send_preference);
    }

    Ok(send_preferences)
}

#[allow(clippy::too_many_arguments)]
#[instrument(skip_all)]
pub async fn build_packages<P>(
    context: &MailUserContext,
    ty: MailType,
    pgp: &P,
    address_keys: &UnlockedAddressKeys<P>,
    send_preferences: HashMap<PrivateEmail, SendPreferences<P::PublicKey>>,
    mime_type: MimeType,
    stored_message_body: &str,
    attachments: &[Attachment],
    eo_data: Option<EoData>,
    tether: &mut Tether,
) -> Result<Vec<Package>, PackageError>
where
    P: PGPProviderSync,
{
    let loaded_attachments =
        adapter::hydrate_attachments::<P>(context, tether, attachments, &send_preferences).await?;

    let eo_container = eo_data.map(|eo_data| mail_package_builder::EoContainer {
        eo_data: adapter::to_shared_eo_data(eo_data),
        eo_modulus_provider: context,
    });

    mail_package_builder::build_packages(
        pgp,
        ty.into(),
        address_keys,
        &send_preferences,
        adapter::to_shared_body(mime_type, stored_message_body),
        loaded_attachments,
        eo_container,
    )
    .await
    .map_err(adapter::translate_package_error)
}

#[derive(Debug, thiserror::Error)]
#[error("An invalid date time was generated")]
pub struct ScheduleSendOptionsDateTimeError;

pub struct ScheduleSendOptions<Tz: chrono::TimeZone> {
    /// Timestamp for the next day at 8:00
    pub time_tomorrow: DateTime<Tz>,
    /// Timestamp for the next Monday at 8:00
    pub time_next_monday: DateTime<Tz>,
    /// Indicates whether the custom date time picker is available, paying users only.
    pub is_custom_datetime_available: bool,
}

impl ScheduleSendOptions<Local> {
    pub fn new(user: &User) -> Result<Self, ScheduleSendOptionsDateTimeError> {
        let now = Local::now();
        Ok(Self {
            time_tomorrow: Self::calculate_tomorrow(now)?,
            time_next_monday: Self::calculate_next_monday(now)?,
            is_custom_datetime_available: user.has_paid_mail_plan(),
        })
    }
}

impl<Tz: chrono::TimeZone> ScheduleSendOptions<Tz> {
    fn calculate_tomorrow(
        now: DateTime<Tz>,
    ) -> Result<DateTime<Tz>, ScheduleSendOptionsDateTimeError> {
        Self::calculate_next(now, 1)
    }
    fn calculate_next_monday(
        now: DateTime<Tz>,
    ) -> Result<DateTime<Tz>, ScheduleSendOptionsDateTimeError> {
        let days = 7 - now.weekday().num_days_from_monday();
        Self::calculate_next(now, days as u64)
    }

    pub(super) fn calculate_next(
        now: DateTime<Tz>,
        days: u64,
    ) -> Result<DateTime<Tz>, ScheduleSendOptionsDateTimeError> {
        let Some(tomorrow) = now.checked_add_days(Days::new(days)) else {
            error!("Failed to calculate next date");
            return Err(ScheduleSendOptionsDateTimeError);
        };

        match tomorrow.with_time(NaiveTime::from_hms_opt(8, 0, 0).expect("Should never fail")) {
            LocalResult::Single(v) => Ok(v),
            LocalResult::Ambiguous(v1, _) => Ok(v1),
            LocalResult::None => {
                error!("Failed to calculate date time at 08:00");
                Err(ScheduleSendOptionsDateTimeError)
            }
        }
    }
}

/// Attempt to cancel a schedule send.
///
/// Contrary to all other methods that operate on the server, this method does not queue any actions
/// as we need to guarantee that the cancel request reaches the servers at the time it is performed.
///
/// Failing to do so can lead to cancellation request running after the message has already been
/// sent.
///
/// On completion returns the original scheduled time of the message.
#[instrument(
    level = "debug",
    skip(
        tether,
        queue,
        session,
        wait_on_completion_duration,
        network_monitor_service
    )
)]
pub async fn cancel_schedule_send(
    message_id: LocalMessageId,
    tether: &mut Tether,
    queue: &Queue<UserDb>,
    session: &Session,
    wait_on_completion_duration: Duration,
    network_monitor_service: &NetworkMonitorService,
) -> Result<DateTime<Local>, MailContextError> {
    info!("Cancelling schedule sent message");
    // Validate if the message is actually scheduled for sending
    let message = Message::find_by_id(message_id, tether)
        .await?
        .ok_or(CancelScheduleSendError::MessageNotFound(message_id))?;

    if !message.is_scheduled_for_send() {
        return Err(CancelScheduleSendError::MessageIsNotScheduled(message_id).into());
    }

    // Pre-check we are offline before proceeding to avoid long stalls during api requests.
    if network_monitor_service.check_now().await.is_offline() {
        return Err(MailContextError::Api(ApiServiceError::NetworkError(
            "Offline".into(),
        )));
    }

    let original_dt: DateTime<Local> = message
        .time
        .to_date_time()
        .ok_or(MailContextError::Other(anyhow!("Invalid timestamp")))?;

    // If we have metadata for this message it means we created the message and
    // there may still be a queued send request. If we do not have any metadata, it either means
    // the message was scheduled on the server already or by another session.
    let message = if let Some(metadata) =
        DraftMetadata::find_by_message_id(message_id, tether).await?
    {
        debug!("Found metadata, message was sent by us.");
        if let Some(send_action_id) = metadata.send_action_id {
            match queue.cancel(send_action_id).await {
                Ok(_) => {
                    // action was cancelled and state reverted.
                    info!("Message {message_id} schedule send cancelled successfully");
                    return Ok(original_dt);
                }
                Err(QueuedError::ActionNotFound(_)) => {
                    // action already executed, proceed to next stage. Before that we need to
                    // reload the message to check whether it actually succeeded or not.
                    debug!("Action no longer exist, either it succeeded or failed");
                }
                Err(QueuedError::ActionInExecution(id)) => {
                    debug!(
                        "Action is being executed ({id}), waiting at most {wait_on_completion_duration:?} until finished"
                    );
                    // Action is currently being executed, wait for it to finish.
                    let mut waiter = ActionAwaiter::new(queue);

                    let Ok(message) =
                        tokio::time::timeout(wait_on_completion_duration, waiter.wait(id))
                            .await
                            .map_err(|_| CancelScheduleSendError::TimedOut)?
                    else {
                        return Err(MailContextError::Other(anyhow!("Connection to queue lost")));
                    };
                    // If the action did not complete it means this message was not scheduled.
                    if !matches!(message, BroadcastMessage::Success(_, _)) {
                        debug!("Action did not complete successfully");
                        return Err(
                            CancelScheduleSendError::MessageIsNotScheduled(message_id).into()
                        );
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        // reload the message in case something changed
        let message = Message::find_by_id(message_id, tether)
            .await?
            .ok_or(CancelScheduleSendError::MessageNotFound(message_id))?;
        if !message.is_scheduled_for_send() {
            return Err(CancelScheduleSendError::MessageIsNotScheduled(message_id).into());
        }
        message
    } else {
        message
    };

    let original_dt: DateTime<Local> = message
        .time
        .to_date_time()
        .ok_or(MailContextError::Other(anyhow!("Invalid timestamp")))?;
    debug!("Cancelling send on the server");
    let remote_id = message
        .remote_id
        .clone()
        .ok_or(AppError::MessageHasNoRemoteId(message_id))?;

    // Invoke server call
    let response = match session.cancel_send(remote_id).await.inspect_err(|e| {
        error!("Failed to cancel send on server: {e:?}");
    }) {
        Ok(response) => response,
        Err(err) => {
            if let Some(api_error) = err.to_proton_error()
                && api_error.code == Mail::MessageAlreadySent as u32
            {
                return Err(CancelScheduleSendError::AlreadySent(message_id).into());
            }
            return Err(err.into());
        }
    };

    // Put message back into drafts
    let mut updated_message = Message::from_api_metadata(response.message, tether).await?;

    tether
        .write_tx(async |tx| updated_message.save(tx).await)
        .await?;

    Ok(original_dt)
}

#[cfg(test)]
mod tests {
    // All test need to run with utc timezone, but the real code should use local timezone.
    use super::*;
    use chrono::Utc;
    use test_case::test_case;

    #[test]
    fn calculate_tomorrow() {
        let now: DateTime<Utc> = DateTime::parse_from_rfc2822("Mon, 12 May 2025 09:30:00 GMT")
            .unwrap()
            .into();
        let expected: DateTime<Utc> = DateTime::parse_from_rfc2822("Tue, 13 May 2025 08:00:00 GMT")
            .unwrap()
            .into();

        let output = ScheduleSendOptions::calculate_tomorrow(now).unwrap();
        assert_eq!(output, expected);
    }

    #[test_case("Mon, 12 May 2025 09:30:00 GMT", "Mon, 19 May 2025 08:00:00 GMT"; "monday to monday" )]
    #[test_case("Wed, 14 May 2025 12:30:00 GMT", "Mon, 19 May 2025 08:00:00 GMT"; "wednesday to monday" )]
    #[test_case("Sun, 18 May 2025 23:30:00 GMT", "Mon, 19 May 2025 08:00:00 GMT"; "Sunday to monday" )]
    #[test_case("Sun, 30 Mar 2025 23:30:00 GMT", "Mon, 31 Mar 2025 08:00:00 GMT"; "Daylight Savings" )]
    fn calculate_next_monday(input: &str, expected: &str) {
        let now: DateTime<Utc> = DateTime::parse_from_rfc2822(input).unwrap().into();
        let expected: DateTime<Utc> = DateTime::parse_from_rfc2822(expected).unwrap().into();

        let output: DateTime<Utc> = ScheduleSendOptions::calculate_next_monday(now).unwrap();
        assert_eq!(output, expected);
    }

    #[test]
    fn send_options_custom_option_available_only_for_paid_users() {
        let options = ScheduleSendOptions::new(&User::default()).unwrap();
        assert!(!options.is_custom_datetime_available);
        let options = ScheduleSendOptions::new(&User::default().with_paid_mail_plan()).unwrap();
        assert!(options.is_custom_datetime_available)
    }
}
