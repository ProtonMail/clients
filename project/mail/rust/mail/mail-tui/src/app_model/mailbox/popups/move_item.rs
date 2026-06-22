use crate::app::Command;
use crate::app_model::mailbox::{ConversationMessage, Items, MessageMessage};
use crate::messages::Messages;
use crate::widgets::category_tabs::category_display_name;
use crate::widgets::utils::ScrollableState;
use crate::widgets::{ScrollableList, ScrollableListState};
use mail_common::actions::{CustomFolderDestination, MoveDestination};
use mail_common::datatypes::{MovableCategoryFolder, MovableSystemFolder};
use mail_common::models::Conversation;
use mail_common::{MailContextResult, MailUserContext};
use mail_core_common::datatypes::{LocalLabelId, SystemLabel};
use mail_core_common::models::Label;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::widgets::{List, ListItem};

pub struct MoveItemPopup {
    rows: Vec<MoveRow>,
    list_state: ScrollableListState,
    item: Items,
}

enum MoveRow {
    Category {
        id: LocalLabelId,
        name: MovableCategoryFolder,
    },
    System {
        id: LocalLabelId,
        name: MovableSystemFolder,
    },
    Custom {
        id: LocalLabelId,
        name: String,
        depth: u8,
    },
}

impl MoveRow {
    fn label_id(&self) -> LocalLabelId {
        match self {
            MoveRow::Category { id, .. }
            | MoveRow::System { id, .. }
            | MoveRow::Custom { id, .. } => *id,
        }
    }

    fn display(&self) -> String {
        match self {
            MoveRow::Category { name, .. } => {
                category_display_name(SystemLabel::from(*name)).to_owned()
            }
            MoveRow::System { name, .. } => system_folder_display_name(*name).to_owned(),
            MoveRow::Custom { name, depth, .. } => {
                format!("{}{name}", "  ".repeat(*depth as usize))
            }
        }
    }
}

fn system_folder_display_name(folder: MovableSystemFolder) -> &'static str {
    match folder {
        MovableSystemFolder::Inbox => "Inbox",
        MovableSystemFolder::Trash => "Trash",
        MovableSystemFolder::Spam => "Spam",
        MovableSystemFolder::Archive => "Archive",
    }
}

fn push_destination(rows: &mut Vec<MoveRow>, destination: MoveDestination) {
    match destination {
        // An Inbox carrying categories means the move targets a specific category, so
        // the categories replace the plain Inbox row. Without categories it stays a
        // single Inbox destination.
        MoveDestination::Inbox(inbox) if inbox.categories.is_empty() => {
            rows.push(MoveRow::System {
                id: inbox.local_id,
                name: inbox.name,
            })
        }
        MoveDestination::Inbox(inbox) => {
            rows.extend(
                inbox
                    .categories
                    .into_iter()
                    .map(|category| MoveRow::Category {
                        id: category.local_id,
                        name: category.name,
                    }),
            );
        }
        MoveDestination::SystemFolder(folder) => rows.push(MoveRow::System {
            id: folder.local_id,
            name: folder.name,
        }),
        MoveDestination::CustomFolder(folder) => push_custom_folder(rows, folder, 0),
    }
}

fn push_custom_folder(rows: &mut Vec<MoveRow>, folder: CustomFolderDestination, depth: u8) {
    rows.push(MoveRow::Custom {
        id: folder.local_id,
        name: folder.name,
        depth,
    });
    for child in folder.children {
        push_custom_folder(rows, child, depth + 1);
    }
}

impl MoveItemPopup {
    pub async fn new(ctx: &MailUserContext, item: Items, view: Label) -> MailContextResult<Self> {
        let destinations = match item.clone() {
            Items::Conversation(ids) => {
                Conversation::available_move_to_destinations(view, ids, ctx).await?
            }
            Items::Message(ids) => {
                mail_common::models::Message::available_move_to_destinations(view, ids, ctx).await?
            }
        };

        let mut rows = Vec::new();
        for destination in destinations {
            push_destination(&mut rows, destination);
        }

        Ok(Self {
            rows,
            item,
            list_state: ScrollableListState::new(Some(0)),
        })
    }

    fn selected_label_id(&self) -> Option<LocalLabelId> {
        let index = self.list_state.selected()?;
        self.rows.get(index).map(MoveRow::label_id)
    }
}

impl crate::app_model::Popup for MoveItemPopup {
    fn title(&self) -> Option<String> {
        Some("Select Folder to Move to".to_owned())
    }

    fn handle_event(&mut self, event: Event) -> Command<Messages> {
        let Event::Key(key) = event else {
            return Command::None;
        };
        if self.list_state.handle_event(key.code) {
            return Command::None;
        }

        match key.code {
            KeyCode::Enter => self
                .selected_label_id()
                .map(|id| match self.item.clone() {
                    Items::Conversation(item_id) => Command::batch([
                        Command::message(Messages::DismissPopup),
                        Command::message(ConversationMessage::MoveTo(item_id, id)),
                    ]),
                    Items::Message(item_id) => Command::batch([
                        Command::message(Messages::DismissPopup),
                        Command::message(MessageMessage::MoveTo(item_id, id)),
                    ]),
                })
                .unwrap_or_default(),
            _ => Command::None,
        }
    }

    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let list = self
            .rows
            .iter()
            .map(|row| ListItem::from(row.display()))
            .collect::<List<'_>>();
        let list = ScrollableList::new(list);
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
}
