//! Account-related ID newtypes.

pub use mail_proton_ids::{PrivateEmail, ProtonIdMarker, declare_proton_id};

// Core IDs
declare_proton_id! { pub UserId }
declare_proton_id! { pub AddressId }
declare_proton_id! { pub SessionId }
declare_proton_id! { pub SaltId }
declare_proton_id! { pub IncomingDefaultId }

impl UserId {
    #[must_use]
    pub fn short_id(&self) -> String {
        self.0[..10].to_string()
    }
}

// Payment IDs
declare_proton_id! { pub PlanId }
declare_proton_id! { pub ProductId }
declare_proton_id! { pub CustomerId }
declare_proton_id! { pub BundleId }
declare_proton_id! { pub PackageNameId }
declare_proton_id! { pub TransactionId }
declare_proton_id! { pub OrderId }
declare_proton_id! { pub PaymentMethodId }
declare_proton_id! { pub SubscriptionId }
