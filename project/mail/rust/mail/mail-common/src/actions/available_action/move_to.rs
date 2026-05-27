use crate::actions::{
    CustomFolderDestination, InboxDestination, MoveDestination, SystemFolderDestination,
};
use crate::datatypes::MovableSystemFolder;
use crate::datatypes::labels::{color_to_display, hierarchy};
use crate::{AppError, CategoryView};
use mail_core_common::datatypes::{LabelType, SystemLabel};
use mail_core_common::models::Label;
use mail_stash::orm::Model;
use mail_stash::stash::Tether;

pub(crate) struct MoveTo<'a> {
    from: &'a Label,
    grouping: Grouping,
}

enum Grouping {
    Conversation,
    Message,
}

impl<'a> MoveTo<'a> {
    pub(crate) fn for_conversation(from: &'a Label) -> Self {
        Self {
            from,
            grouping: Grouping::Conversation,
        }
    }

    pub(crate) fn for_message(from: &'a Label) -> Self {
        Self {
            from,
            grouping: Grouping::Message,
        }
    }

    pub(crate) async fn build(self, tether: &Tether) -> Result<Vec<MoveDestination>, AppError> {
        let from_id = self.from.local_id;
        let moving_from_sent = matches!(self.grouping, Grouping::Message)
            && matches!(
                SystemLabel::from_opt_rid(self.from.remote_id.as_ref()),
                Some(SystemLabel::AllSent | SystemLabel::Sent)
            );
        let all_system = Label::find_by_kind(LabelType::System, tether).await?;
        let all_custom_folders = Label::find_by_kind(LabelType::Folder, tether).await?;
        let inbox_id = SystemLabel::Inbox.local_id(tether).await?;

        let mut destinations = Vec::new();

        for label in all_system.iter() {
            if label.local_id == from_id && from_id != inbox_id {
                continue;
            }
            if moving_from_sent
                && matches!(
                    SystemLabel::from_opt_rid(label.remote_id.as_ref()),
                    Some(SystemLabel::Inbox | SystemLabel::Spam)
                )
            {
                continue;
            }
            let destination =
                label_to_destination(label, tether)
                    .await?
                    .filter(|dest| match dest {
                        MoveDestination::Inbox(inbox) => {
                            !(from_id == inbox_id && inbox.categories.is_empty())
                        }
                        _ => true,
                    });

            if let Some(destination) = destination {
                destinations.push(destination);
            }
        }

        for label in all_custom_folders.iter() {
            if let Some(destination) = label_to_destination(label, tether).await? {
                destinations.push(destination);
            }
        }

        let destinations = resolve_custom_folder_colors(destinations, tether).await?;
        Ok(build_folder_structure(destinations).collect())
    }
}

async fn label_to_destination(
    label: &Label,
    tether: &Tether,
) -> Result<Option<MoveDestination>, AppError> {
    let action = match label.label_type {
        LabelType::System
            if matches!(
                SystemLabel::from_opt_rid(label.remote_id.as_ref()),
                Some(SystemLabel::Inbox),
            ) =>
        {
            let inbox_id = label.id();
            Some(MoveDestination::Inbox(InboxDestination {
                local_id: inbox_id,
                name: MovableSystemFolder::Inbox,
                categories: SystemFolderDestination::from_categories(
                    CategoryView::load(inbox_id, tether)
                        .await?
                        .into_labels(tether)
                        .await?,
                ),
            }))
        }

        LabelType::System
            if !SystemLabel::from_opt_rid(label.remote_id.as_ref())
                .is_some_and(|sl| sl.is_category()) =>
        {
            SystemFolderDestination::from_label(label).map(MoveDestination::SystemFolder)
        }

        LabelType::Folder => {
            CustomFolderDestination::from_label(label).map(MoveDestination::CustomFolder)
        }
        _ => None,
    };

    Ok(action)
}

async fn resolve_custom_folder_colors(
    actions: Vec<MoveDestination>,
    tether: &Tether,
) -> Result<Vec<MoveDestination>, AppError> {
    use futures::stream::{self, StreamExt, TryStreamExt};

    stream::iter(actions)
        .then(|action| async move {
            match action {
                MoveDestination::CustomFolder(mut action) => {
                    let label = Label::load(action.local_id, tether).await?.unwrap();
                    action.color = color_to_display(&label, tether).await?;
                    Ok::<_, AppError>(MoveDestination::CustomFolder(action))
                }
                other => Ok(other),
            }
        })
        .try_collect()
        .await
}

fn build_folder_structure(
    actions: impl IntoIterator<Item = MoveDestination>,
) -> impl Iterator<Item = MoveDestination> {
    let actions = actions.into_iter();
    let system_size = SystemLabel::movable_folders().len();
    let (custom_size, _) = actions.size_hint();
    let (inbox, system_folders, custom_folders) = actions.fold(
        (
            None::<InboxDestination>,
            Vec::with_capacity(system_size),
            Vec::with_capacity(custom_size),
        ),
        |(mut inbox, mut system, mut custom), action| {
            match action {
                MoveDestination::Inbox(action) => inbox = Some(action),
                MoveDestination::SystemFolder(action) => system.push(action),
                MoveDestination::CustomFolder(action) => custom.push(action),
            }
            (inbox, system, custom)
        },
    );

    let custom_folders = hierarchy::custom_folder_hierarchy(&custom_folders)
        .into_iter()
        .map(MoveDestination::CustomFolder);

    inbox
        .into_iter()
        .map(MoveDestination::Inbox)
        .chain(
            system_folders
                .into_iter()
                .map(MoveDestination::SystemFolder),
        )
        .chain(custom_folders)
}
