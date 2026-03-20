use crate::Dimension;
use core_telemetry::TelemetryEvent;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, strum::Display)]
#[strum(serialize_all = "snake_case")]
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

impl Dimension for UpsellEntryPoint {
    const NAME: &str = "upsell_entry_point";
}

#[derive(Debug, Clone, Copy, strum::Display)]
pub enum DaysSinceAccountCreation {
    #[strum(serialize = "0")]
    Zero,
    #[strum(serialize = "01-03")]
    OneThroughThree,
    #[strum(serialize = "04-10")]
    FourThroughTen,
    #[strum(serialize = "11-30")]
    ElevenThroughThirty,
    #[strum(serialize = "31-60")]
    ThirtyOneThroughSixty,
    #[strum(serialize = "61-120")]
    SixtyOneThroughHundredTwenty,
    #[strum(serialize = ">120")]
    MoreThanHundredTwenty,
    #[strum(serialize = "n/a")]
    NotApplicable,
}

impl Dimension for DaysSinceAccountCreation {
    const NAME: &str = "days_since_account_creation";
}

#[derive(Debug, Clone, Copy, strum::Display)]
pub enum UpsellModalVariant {
    #[strum(serialize = "Comparison.Plus")]
    ComparisonPlus,
    #[strum(serialize = "Comparison.Unlimited")]
    ComparisonUnlimited,
}

impl Dimension for UpsellModalVariant {
    const NAME: &str = "upsell_modal_variant";
}

#[derive(Debug, Clone)]
pub struct PlanBeforeUpgrade(String);

impl PlanBeforeUpgrade {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl Dimension for PlanBeforeUpgrade {
    const NAME: &str = "plan_before_upgrade";
}

impl std::fmt::Display for PlanBeforeUpgrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct SelectedPlan(String);

impl SelectedPlan {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl Dimension for SelectedPlan {
    const NAME: &str = "selected_plan";
}

impl std::fmt::Display for SelectedPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct SelectedCycle(String);

impl SelectedCycle {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl Dimension for SelectedCycle {
    const NAME: &str = "selected_cycle";
}

impl std::fmt::Display for SelectedCycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, strum::Display)]
pub enum UpsellIsPromotional {
    #[strum(serialize = "true")]
    Yes,
    #[strum(serialize = "false")]
    No,
}

impl Dimension for UpsellIsPromotional {
    const NAME: &str = "upsell_is_promotional";
}

impl From<bool> for UpsellIsPromotional {
    fn from(val: bool) -> Self {
        if val { Self::Yes } else { Self::No }
    }
}

pub struct UpsellEvents;

impl UpsellEvents {
    const MEASUREMENT_GROUP: &str = "mail.any.upsell";

    #[must_use]
    pub fn upsell_button_tapped(
        upsell_entry_point: UpsellEntryPoint,
        plan_before_upgrade: PlanBeforeUpgrade,
        days_since_account_creation: DaysSinceAccountCreation,
        upsell_modal_variant: UpsellModalVariant,
    ) -> TelemetryEvent {
        Self::build_event(
            "upsell_button_tapped",
            HashMap::from([
                upsell_entry_point.to_dimension(),
                plan_before_upgrade.to_dimension(),
                days_since_account_creation.to_dimension(),
                upsell_modal_variant.to_dimension(),
            ]),
        )
    }

    #[must_use]
    pub fn drive_spotlight_mailbox_button_tapped(
        plan_before_upgrade: PlanBeforeUpgrade,
        days_since_account_creation: DaysSinceAccountCreation,
    ) -> TelemetryEvent {
        Self::build_event(
            "drive_spotlight_mailbox_button_tapped",
            HashMap::from([
                plan_before_upgrade.to_dimension(),
                days_since_account_creation.to_dimension(),
            ]),
        )
    }

    #[must_use]
    pub fn drive_spotlight_cta_button_tapped(
        plan_before_upgrade: PlanBeforeUpgrade,
        days_since_account_creation: DaysSinceAccountCreation,
    ) -> TelemetryEvent {
        Self::build_event(
            "drive_spotlight_cta_button_tapped",
            HashMap::from([
                plan_before_upgrade.to_dimension(),
                days_since_account_creation.to_dimension(),
            ]),
        )
    }

    #[must_use]
    pub fn upgrade_attempt(
        upsell_entry_point: UpsellEntryPoint,
        plan_before_upgrade: PlanBeforeUpgrade,
        days_since_account_creation: DaysSinceAccountCreation,
        upsell_modal_variant: UpsellModalVariant,
        selected_plan: SelectedPlan,
        selected_cycle: SelectedCycle,
        upsell_is_promotional: UpsellIsPromotional,
    ) -> TelemetryEvent {
        Self::build_event(
            "upgrade_attempt",
            Self::upgrade_dimensions(
                upsell_entry_point,
                plan_before_upgrade,
                days_since_account_creation,
                upsell_modal_variant,
                selected_plan,
                selected_cycle,
                upsell_is_promotional,
            ),
        )
    }

    #[must_use]
    pub fn upgrade_error(
        upsell_entry_point: UpsellEntryPoint,
        plan_before_upgrade: PlanBeforeUpgrade,
        days_since_account_creation: DaysSinceAccountCreation,
        upsell_modal_variant: UpsellModalVariant,
        selected_plan: SelectedPlan,
        selected_cycle: SelectedCycle,
        upsell_is_promotional: UpsellIsPromotional,
    ) -> TelemetryEvent {
        Self::build_event(
            "upgrade_error",
            Self::upgrade_dimensions(
                upsell_entry_point,
                plan_before_upgrade,
                days_since_account_creation,
                upsell_modal_variant,
                selected_plan,
                selected_cycle,
                upsell_is_promotional,
            ),
        )
    }

    #[must_use]
    pub fn upgrade_cancelled_by_user(
        upsell_entry_point: UpsellEntryPoint,
        plan_before_upgrade: PlanBeforeUpgrade,
        days_since_account_creation: DaysSinceAccountCreation,
        upsell_modal_variant: UpsellModalVariant,
        selected_plan: SelectedPlan,
        selected_cycle: SelectedCycle,
        upsell_is_promotional: UpsellIsPromotional,
    ) -> TelemetryEvent {
        Self::build_event(
            "upgrade_cancelled_by_user",
            Self::upgrade_dimensions(
                upsell_entry_point,
                plan_before_upgrade,
                days_since_account_creation,
                upsell_modal_variant,
                selected_plan,
                selected_cycle,
                upsell_is_promotional,
            ),
        )
    }

    #[must_use]
    pub fn upgrade_success(
        upsell_entry_point: UpsellEntryPoint,
        plan_before_upgrade: PlanBeforeUpgrade,
        days_since_account_creation: DaysSinceAccountCreation,
        upsell_modal_variant: UpsellModalVariant,
        selected_plan: SelectedPlan,
        selected_cycle: SelectedCycle,
        upsell_is_promotional: UpsellIsPromotional,
    ) -> TelemetryEvent {
        Self::build_event(
            "upgrade_success",
            Self::upgrade_dimensions(
                upsell_entry_point,
                plan_before_upgrade,
                days_since_account_creation,
                upsell_modal_variant,
                selected_plan,
                selected_cycle,
                upsell_is_promotional,
            ),
        )
    }

    fn upgrade_dimensions(
        upsell_entry_point: UpsellEntryPoint,
        plan_before_upgrade: PlanBeforeUpgrade,
        days_since_account_creation: DaysSinceAccountCreation,
        upsell_modal_variant: UpsellModalVariant,
        selected_plan: SelectedPlan,
        selected_cycle: SelectedCycle,
        upsell_is_promotional: UpsellIsPromotional,
    ) -> HashMap<String, String> {
        HashMap::from([
            upsell_entry_point.to_dimension(),
            plan_before_upgrade.to_dimension(),
            days_since_account_creation.to_dimension(),
            upsell_modal_variant.to_dimension(),
            selected_plan.to_dimension(),
            selected_cycle.to_dimension(),
            upsell_is_promotional.to_dimension(),
        ])
    }

    fn build_event(event_name: &str, dimensions: HashMap<String, String>) -> TelemetryEvent {
        TelemetryEvent {
            id: Uuid::new_v4().to_string(),
            measurement_group: Self::MEASUREMENT_GROUP.to_string(),
            event: event_name.to_string(),
            values: HashMap::new(),
            dimensions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsell_button_tapped_event() {
        let event = UpsellEvents::upsell_button_tapped(
            UpsellEntryPoint::MailboxTopBar,
            PlanBeforeUpgrade::new("free"),
            DaysSinceAccountCreation::FourThroughTen,
            UpsellModalVariant::ComparisonPlus,
        );

        assert_eq!(event.measurement_group, "mail.any.upsell");
        assert_eq!(event.event, "upsell_button_tapped");
        assert!(event.values.is_empty());
        assert_eq!(event.dimensions["upsell_entry_point"], "mailbox_top_bar");
        assert_eq!(event.dimensions["plan_before_upgrade"], "free");
        assert_eq!(event.dimensions["days_since_account_creation"], "04-10");
        assert_eq!(event.dimensions["upsell_modal_variant"], "Comparison.Plus");
        assert_eq!(event.dimensions.len(), 4);
    }

    #[test]
    fn drive_spotlight_mailbox_button_tapped_event() {
        let event = UpsellEvents::drive_spotlight_mailbox_button_tapped(
            PlanBeforeUpgrade::new("free"),
            DaysSinceAccountCreation::MoreThanHundredTwenty,
        );

        assert_eq!(event.event, "drive_spotlight_mailbox_button_tapped");
        assert!(event.values.is_empty());
        assert_eq!(event.dimensions["plan_before_upgrade"], "free");
        assert_eq!(event.dimensions["days_since_account_creation"], ">120");
        assert_eq!(event.dimensions.len(), 2);
    }

    #[test]
    fn drive_spotlight_cta_button_tapped_event() {
        let event = UpsellEvents::drive_spotlight_cta_button_tapped(
            PlanBeforeUpgrade::new("plus"),
            DaysSinceAccountCreation::Zero,
        );

        assert_eq!(event.event, "drive_spotlight_cta_button_tapped");
        assert!(event.values.is_empty());
        assert_eq!(event.dimensions["days_since_account_creation"], "0");
        assert_eq!(event.dimensions.len(), 2);
    }

    #[test]
    fn upgrade_attempt_event() {
        let event = UpsellEvents::upgrade_attempt(
            UpsellEntryPoint::NavbarUpsell,
            PlanBeforeUpgrade::new("free"),
            DaysSinceAccountCreation::Zero,
            UpsellModalVariant::ComparisonPlus,
            SelectedPlan::new("plus"),
            SelectedCycle::new("12"),
            UpsellIsPromotional::Yes,
        );

        assert_eq!(event.measurement_group, "mail.any.upsell");
        assert_eq!(event.event, "upgrade_attempt");
        assert!(event.values.is_empty());
        assert_eq!(event.dimensions["upsell_entry_point"], "navbar_upsell");
        assert_eq!(event.dimensions["plan_before_upgrade"], "free");
        assert_eq!(event.dimensions["days_since_account_creation"], "0");
        assert_eq!(event.dimensions["upsell_modal_variant"], "Comparison.Plus");
        assert_eq!(event.dimensions["selected_plan"], "plus");
        assert_eq!(event.dimensions["selected_cycle"], "12");
        assert_eq!(event.dimensions["upsell_is_promotional"], "true");
        assert_eq!(event.dimensions.len(), 7);
    }

    #[test]
    fn upgrade_error_event() {
        let event = UpsellEvents::upgrade_error(
            UpsellEntryPoint::ScheduleSend,
            PlanBeforeUpgrade::new("free"),
            DaysSinceAccountCreation::ElevenThroughThirty,
            UpsellModalVariant::ComparisonPlus,
            SelectedPlan::new("bundle"),
            SelectedCycle::new("24"),
            UpsellIsPromotional::No,
        );

        assert_eq!(event.event, "upgrade_error");
        assert_eq!(event.dimensions["upsell_is_promotional"], "false");
        assert_eq!(event.dimensions.len(), 7);
    }

    #[test]
    fn upgrade_cancelled_by_user_event() {
        let event = UpsellEvents::upgrade_cancelled_by_user(
            UpsellEntryPoint::Snooze,
            PlanBeforeUpgrade::new("free"),
            DaysSinceAccountCreation::ThirtyOneThroughSixty,
            UpsellModalVariant::ComparisonPlus,
            SelectedPlan::new("unlimited"),
            SelectedCycle::new("1"),
            UpsellIsPromotional::No,
        );

        assert_eq!(event.event, "upgrade_cancelled_by_user");
        assert_eq!(event.dimensions.len(), 7);
    }

    #[test]
    fn upgrade_success_event() {
        let event = UpsellEvents::upgrade_success(
            UpsellEntryPoint::PostOnboarding,
            PlanBeforeUpgrade::new("free"),
            DaysSinceAccountCreation::OneThroughThree,
            UpsellModalVariant::ComparisonPlus,
            SelectedPlan::new("plus"),
            SelectedCycle::new("12"),
            UpsellIsPromotional::Yes,
        );

        assert_eq!(event.event, "upgrade_success");
        assert_eq!(event.dimensions["days_since_account_creation"], "01-03");
        assert_eq!(event.dimensions.len(), 7);
    }

    #[test]
    fn not_applicable_days() {
        let event = UpsellEvents::upsell_button_tapped(
            UpsellEntryPoint::DollarPromo,
            PlanBeforeUpgrade::new("free"),
            DaysSinceAccountCreation::NotApplicable,
            UpsellModalVariant::ComparisonPlus,
        );

        assert_eq!(event.dimensions["days_since_account_creation"], "n/a");
    }

    #[test]
    fn all_entry_points_serialize() {
        let entry_points = [
            (UpsellEntryPoint::AutoDeleteMessages, "auto_delete_messages"),
            (UpsellEntryPoint::ContactGroups, "contact_groups"),
            (UpsellEntryPoint::DollarPromo, "dollar_promo"),
            (UpsellEntryPoint::FoldersCreation, "folders_creation"),
            (UpsellEntryPoint::LabelsCreation, "labels_creation"),
            (UpsellEntryPoint::MailboxTopBar, "mailbox_top_bar"),
            (
                UpsellEntryPoint::MailboxTopBarPromo,
                "mailbox_top_bar_promo",
            ),
            (UpsellEntryPoint::NavbarUpsell, "navbar_upsell"),
            (
                UpsellEntryPoint::MobileSignatureEdit,
                "mobile_signature_edit",
            ),
            (UpsellEntryPoint::PostOnboarding, "post_onboarding"),
            (UpsellEntryPoint::ScheduleSend, "schedule_send"),
            (UpsellEntryPoint::Snooze, "snooze"),
        ];

        for (variant, expected) in entry_points {
            assert_eq!(variant.to_string(), expected);
        }
    }
}
