use crate::mail::MailUserSession;
use mail_common::MailUserContext;
use mail_core_common::datatypes::UnixTimestamp;
use mail_core_common::services::TelemetryService;
use mail_telemetry::{
    DaysSinceAccountCreation, PlanBeforeUpgrade, SelectedCycle, SelectedPlan, TelemetryEvent,
    UpsellEntryPoint as TelemetryUpsellEntryPoint, UpsellEvents, UpsellExperimentVariant,
    UpsellIsPromotional, UpsellModalVariant as TelemetryUpsellModalVariant,
};
use mail_uniffi_runtime::async_runtime;
use std::sync::Arc;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum UpsellTelemetryError {
    #[error("Invalid account creation time: {0}")]
    InvalidTimestamp(String),
    #[error("Future account creation time: {0}")]
    FutureDate(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Async runtime error: {0}")]
    Task(#[from] tokio::task::JoinError),
}

fn calculate_days_from_timestamps(
    create_time: UnixTimestamp,
    current_time: UnixTimestamp,
) -> Result<DaysSinceAccountCreation, UpsellTelemetryError> {
    let create_dt = create_time.to_date_time_utc().ok_or_else(|| {
        UpsellTelemetryError::InvalidTimestamp(format!(
            "Invalid timestamp: {}",
            create_time.as_u64()
        ))
    })?;

    let current_dt = current_time.to_date_time_utc().ok_or_else(|| {
        UpsellTelemetryError::InvalidTimestamp("Invalid current time".to_string())
    })?;

    let duration = current_dt.signed_duration_since(create_dt);
    let days = duration.num_days();

    if days < 0 {
        return Err(UpsellTelemetryError::FutureDate(format!(
            "Account creation time {create_dt} is in the future"
        )));
    }

    Ok(match days {
        0 => DaysSinceAccountCreation::Zero,
        1..=3 => DaysSinceAccountCreation::OneThroughThree,
        4..=10 => DaysSinceAccountCreation::FourThroughTen,
        11..=30 => DaysSinceAccountCreation::ElevenThroughThirty,
        31..=60 => DaysSinceAccountCreation::ThirtyOneThroughSixty,
        61..=120 => DaysSinceAccountCreation::SixtyOneThroughHundredTwenty,
        _ => DaysSinceAccountCreation::MoreThanHundredTwenty,
    })
}

#[tracing::instrument(skip_all)]
pub async fn calculate_days_since_account_creation(
    mail_user_context: &MailUserContext,
    current_time: UnixTimestamp,
) -> Result<DaysSinceAccountCreation, UpsellTelemetryError> {
    let user = mail_user_context
        .user()
        .await
        .map_err(|e| UpsellTelemetryError::Database(e.to_string()))?;

    calculate_days_from_timestamps(user.create_time, current_time)
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum UpsellEntryPoint {
    AutoDeleteMessages,
    ContactGroups,
    DollarPromo,
    FoldersCreation,
    LabelsCreation,
    MailboxTopBar,
    MailboxTopBarPromo,
    NavbarUpsell,
    MobileSignatureEdit,
    PostOnboarding,
    ScheduleSend,
    Snooze,
}

impl From<UpsellEntryPoint> for TelemetryUpsellEntryPoint {
    fn from(val: UpsellEntryPoint) -> Self {
        match val {
            UpsellEntryPoint::AutoDeleteMessages => Self::AutoDeleteMessages,
            UpsellEntryPoint::ContactGroups => Self::ContactGroups,
            UpsellEntryPoint::DollarPromo => Self::DollarPromo,
            UpsellEntryPoint::FoldersCreation => Self::FoldersCreation,
            UpsellEntryPoint::LabelsCreation => Self::LabelsCreation,
            UpsellEntryPoint::MailboxTopBar => Self::MailboxTopBar,
            UpsellEntryPoint::MailboxTopBarPromo => Self::MailboxTopBarPromo,
            UpsellEntryPoint::NavbarUpsell => Self::NavbarUpsell,
            UpsellEntryPoint::MobileSignatureEdit => Self::MobileSignatureEdit,
            UpsellEntryPoint::PostOnboarding => Self::PostOnboarding,
            UpsellEntryPoint::ScheduleSend => Self::ScheduleSend,
            UpsellEntryPoint::Snooze => Self::Snooze,
        }
    }
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum UpsellModalVariant {
    ComparisonPlus,
    ComparisonUnlimited,
}

impl From<UpsellModalVariant> for TelemetryUpsellModalVariant {
    fn from(val: UpsellModalVariant) -> Self {
        match val {
            UpsellModalVariant::ComparisonPlus => Self::ComparisonPlus,
            UpsellModalVariant::ComparisonUnlimited => Self::ComparisonUnlimited,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct GeneralDimensions {
    pub upsell_entry_point: UpsellEntryPoint,
    pub plan_before_upgrade: String,
    pub modal_variant: UpsellModalVariant,
    /// Used by analysts to identify the active experiment cohort (variant name).
    pub upsell_experiment_flag: UpsellExperimentFlag,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct UpsellExperimentFlag {
    pub flag_name: String,
}

#[uniffi_export]
pub fn upsell_experiment_flag_for_ios() -> UpsellExperimentFlag {
    UpsellExperimentFlag {
        flag_name: mail_common::FF_UPSELL_EXPERIMENT.to_string(),
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PlanSpecificDimensions {
    pub selected_plan: String,
    pub selected_cycle: String,
    pub upsell_is_promotional: bool,
}

async fn record_telemetry_event(user_context: &MailUserContext, event: TelemetryEvent) {
    if let Some(telemetry) = user_context
        .user_context()
        .get_service_opt::<TelemetryService>()
    {
        if let Err(err) = telemetry.record_event(event).await {
            error!("Failed to record telemetry event: {err:?}");
        }
    } else {
        error!("TelemetryService not available");
    }
}

async fn resolve_experiment_variant(
    user_context: &MailUserContext,
    flag: &UpsellExperimentFlag,
) -> UpsellExperimentVariant {
    match user_context
        .user_context()
        .feature_flags()
        .get_feature_flag_variant(&flag.flag_name)
        .await
    {
        Ok(Some(variant)) if variant.enabled => UpsellExperimentVariant(variant.name),
        _ => UpsellExperimentVariant("none".to_string()),
    }
}

#[uniffi_export]
#[tracing::instrument(skip_all)]
pub async fn record_upsell_button_tapped(
    user_session: Arc<MailUserSession>,
    general: GeneralDimensions,
) {
    if let Err(err) = async_runtime()
        .spawn(async move {
            let user_context = match user_session.ctx() {
                Ok(ctx) => ctx,
                Err(err) => {
                    error!("Failed to get user context: {err:?}");
                    return;
                }
            };

            let days_since_account_creation = match calculate_days_since_account_creation(
                user_context.as_ref(),
                UnixTimestamp::now(),
            )
            .await
            {
                Ok(days) => days,
                Err(err) => {
                    error!("Failed to calculate days since account creation: {err:?}");
                    return;
                }
            };

            let experiment_variant =
                resolve_experiment_variant(user_context.as_ref(), &general.upsell_experiment_flag)
                    .await;

            let event = UpsellEvents::upsell_button_tapped(
                general.upsell_entry_point.into(),
                PlanBeforeUpgrade::new(general.plan_before_upgrade),
                days_since_account_creation,
                general.modal_variant.into(),
                experiment_variant,
            );

            record_telemetry_event(user_context.as_ref(), event).await;
        })
        .await
    {
        error!("Failed to spawn uniffi async task: {err}");
    }
}

#[uniffi_export]
#[tracing::instrument(skip_all)]
pub async fn record_drive_spotlight_mailbox_button_tapped(
    user_session: Arc<MailUserSession>,
    plan_before_upgrade: String,
) {
    if let Err(err) = async_runtime()
        .spawn(async move {
            let user_context = match user_session.ctx() {
                Ok(ctx) => ctx,
                Err(err) => {
                    error!("Failed to get user context: {err:?}");
                    return;
                }
            };

            let days_since_account_creation = match calculate_days_since_account_creation(
                user_context.as_ref(),
                UnixTimestamp::now(),
            )
            .await
            {
                Ok(days) => days,
                Err(err) => {
                    error!("Failed to calculate days since account creation: {err:?}");
                    return;
                }
            };

            let event = UpsellEvents::drive_spotlight_mailbox_button_tapped(
                PlanBeforeUpgrade::new(plan_before_upgrade),
                days_since_account_creation,
            );

            record_telemetry_event(user_context.as_ref(), event).await;
        })
        .await
    {
        error!("Failed to spawn uniffi async task: {err}");
    }
}

#[uniffi_export]
#[tracing::instrument(skip_all)]
pub async fn record_drive_spotlight_cta_button_tapped(
    user_session: Arc<MailUserSession>,
    plan_before_upgrade: String,
) {
    if let Err(err) = async_runtime()
        .spawn(async move {
            let user_context = match user_session.ctx() {
                Ok(ctx) => ctx,
                Err(err) => {
                    error!("Failed to get user context: {err:?}");
                    return;
                }
            };

            let days_since_account_creation = match calculate_days_since_account_creation(
                user_context.as_ref(),
                UnixTimestamp::now(),
            )
            .await
            {
                Ok(days) => days,
                Err(err) => {
                    error!("Failed to calculate days since account creation: {err:?}");
                    return;
                }
            };

            let event = UpsellEvents::drive_spotlight_cta_button_tapped(
                PlanBeforeUpgrade::new(plan_before_upgrade),
                days_since_account_creation,
            );

            record_telemetry_event(user_context.as_ref(), event).await;
        })
        .await
    {
        error!("Failed to spawn uniffi async task: {err}");
    }
}

#[uniffi_export]
#[tracing::instrument(skip_all)]
pub async fn record_upgrade_attempt(
    user_session: Arc<MailUserSession>,
    general: GeneralDimensions,
    plan_specific: PlanSpecificDimensions,
) {
    record_upgrade_event(
        user_session,
        general,
        plan_specific,
        UpsellEvents::upgrade_attempt,
    )
    .await;
}

#[uniffi_export]
#[tracing::instrument(skip_all)]
pub async fn record_upgrade_error(
    user_session: Arc<MailUserSession>,
    general: GeneralDimensions,
    plan_specific: PlanSpecificDimensions,
) {
    record_upgrade_event(
        user_session,
        general,
        plan_specific,
        UpsellEvents::upgrade_error,
    )
    .await;
}

#[uniffi_export]
#[tracing::instrument(skip_all)]
pub async fn record_upgrade_cancelled_by_user(
    user_session: Arc<MailUserSession>,
    general: GeneralDimensions,
    plan_specific: PlanSpecificDimensions,
) {
    record_upgrade_event(
        user_session,
        general,
        plan_specific,
        UpsellEvents::upgrade_cancelled_by_user,
    )
    .await;
}

#[uniffi_export]
#[tracing::instrument(skip_all)]
pub async fn record_upgrade_success(
    user_session: Arc<MailUserSession>,
    general: GeneralDimensions,
    plan_specific: PlanSpecificDimensions,
) {
    record_upgrade_event(
        user_session,
        general,
        plan_specific,
        UpsellEvents::upgrade_success,
    )
    .await;
}

async fn record_upgrade_event(
    user_session: Arc<MailUserSession>,
    general: GeneralDimensions,
    plan_specific: PlanSpecificDimensions,
    build_event: fn(
        TelemetryUpsellEntryPoint,
        PlanBeforeUpgrade,
        DaysSinceAccountCreation,
        TelemetryUpsellModalVariant,
        SelectedPlan,
        SelectedCycle,
        UpsellIsPromotional,
        UpsellExperimentVariant,
    ) -> TelemetryEvent,
) {
    if let Err(err) = async_runtime()
        .spawn(async move {
            let user_context = match user_session.ctx() {
                Ok(ctx) => ctx,
                Err(err) => {
                    error!("Failed to get user context: {err:?}");
                    return;
                }
            };

            let days_since_account_creation = match calculate_days_since_account_creation(
                user_context.as_ref(),
                UnixTimestamp::now(),
            )
            .await
            {
                Ok(days) => days,
                Err(err) => {
                    error!("Failed to calculate days since account creation: {err:?}");
                    return;
                }
            };

            let experiment_variant =
                resolve_experiment_variant(user_context.as_ref(), &general.upsell_experiment_flag)
                    .await;

            let event = build_event(
                general.upsell_entry_point.into(),
                PlanBeforeUpgrade::new(general.plan_before_upgrade),
                days_since_account_creation,
                general.modal_variant.into(),
                SelectedPlan::new(plan_specific.selected_plan),
                SelectedCycle::new(plan_specific.selected_cycle),
                plan_specific.upsell_is_promotional.into(),
                experiment_variant,
            );

            record_telemetry_event(user_context.as_ref(), event).await;
        })
        .await
    {
        error!("Failed to spawn uniffi async task: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};

    #[test]
    fn test_calculate_days_from_timestamps_valid_cases() {
        let account_created = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let create_time = UnixTimestamp::from(account_created);

        let same_day = account_created;
        let result =
            calculate_days_from_timestamps(create_time, UnixTimestamp::from(same_day)).unwrap();
        assert!(matches!(result, DaysSinceAccountCreation::Zero));

        let one_day_later = account_created + Duration::days(1);
        let result =
            calculate_days_from_timestamps(create_time, UnixTimestamp::from(one_day_later))
                .unwrap();
        assert!(matches!(result, DaysSinceAccountCreation::OneThroughThree));

        let three_days_later = account_created + Duration::days(3);
        let result =
            calculate_days_from_timestamps(create_time, UnixTimestamp::from(three_days_later))
                .unwrap();
        assert!(matches!(result, DaysSinceAccountCreation::OneThroughThree));

        let four_days_later = account_created + Duration::days(4);
        let result =
            calculate_days_from_timestamps(create_time, UnixTimestamp::from(four_days_later))
                .unwrap();
        assert!(matches!(result, DaysSinceAccountCreation::FourThroughTen));

        let ten_days_later = account_created + Duration::days(10);
        let result =
            calculate_days_from_timestamps(create_time, UnixTimestamp::from(ten_days_later))
                .unwrap();
        assert!(matches!(result, DaysSinceAccountCreation::FourThroughTen));

        let thirty_days_later = account_created + Duration::days(30);
        let result =
            calculate_days_from_timestamps(create_time, UnixTimestamp::from(thirty_days_later))
                .unwrap();
        assert!(matches!(
            result,
            DaysSinceAccountCreation::ElevenThroughThirty
        ));

        let sixty_days_later = account_created + Duration::days(60);
        let result =
            calculate_days_from_timestamps(create_time, UnixTimestamp::from(sixty_days_later))
                .unwrap();
        assert!(matches!(
            result,
            DaysSinceAccountCreation::ThirtyOneThroughSixty
        ));

        let sixty_one_days_later = account_created + Duration::days(61);
        let result =
            calculate_days_from_timestamps(create_time, UnixTimestamp::from(sixty_one_days_later))
                .unwrap();
        assert!(matches!(
            result,
            DaysSinceAccountCreation::SixtyOneThroughHundredTwenty
        ));

        let hundred_twenty_one_days_later = account_created + Duration::days(121);
        let result = calculate_days_from_timestamps(
            create_time,
            UnixTimestamp::from(hundred_twenty_one_days_later),
        )
        .unwrap();
        assert!(matches!(
            result,
            DaysSinceAccountCreation::MoreThanHundredTwenty
        ));
    }

    #[test]
    fn test_calculate_days_from_timestamps_future_date_error() {
        let current_time = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let future_create_time = current_time + Duration::days(1);

        let result = calculate_days_from_timestamps(
            UnixTimestamp::from(future_create_time),
            UnixTimestamp::from(current_time),
        );
        assert!(matches!(result, Err(UpsellTelemetryError::FutureDate(_))));
    }

    #[test]
    fn test_calculate_days_from_timestamps_invalid_timestamp() {
        let valid_time = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let invalid_timestamp = UnixTimestamp::from(u64::MAX);

        let result =
            calculate_days_from_timestamps(invalid_timestamp, UnixTimestamp::from(valid_time));
        assert!(matches!(
            result,
            Err(UpsellTelemetryError::InvalidTimestamp(_))
        ));

        let result =
            calculate_days_from_timestamps(UnixTimestamp::from(valid_time), invalid_timestamp);
        assert!(matches!(
            result,
            Err(UpsellTelemetryError::InvalidTimestamp(_))
        ));
    }
}
