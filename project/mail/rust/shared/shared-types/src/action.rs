use mail_api_event_types::Action as ApiAction;
use mail_proton_ids::ProtonIdMarker;

/// The action taken on a resource during an event.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Action {
    Delete = 0,
    Create = 1,
    Update = 2,
    UpdateFlags = 3,
}

impl Action {
    pub async fn log_entry<T: ProtonIdMarker>(
        self,
        id: &T,
        local_id: impl AsyncFnOnce(&T) -> Option<u64>,
    ) {
        let action_str = match self {
            Action::Delete => "Deleting",
            Action::Create => "Creating",
            Action::Update => "Updating",
            Action::UpdateFlags => "Updating (flags)",
        };

        if self != Action::Create
            && let Some(local_id) = local_id(id).await
        {
            tracing::info!("{action_str} {id:?} -> {local_id}");
        } else {
            tracing::info!("{action_str} {id:?}");
        }
    }
}

impl From<ApiAction> for Action {
    fn from(value: ApiAction) -> Self {
        match value {
            ApiAction::Delete => Self::Delete,
            ApiAction::Create => Self::Create,
            ApiAction::Update => Self::Update,
            ApiAction::UpdateFlags => Self::UpdateFlags,
        }
    }
}
