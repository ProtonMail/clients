use crate::app::Command;
use crate::app_model::mailbox::MailboxModel;
use crate::app_model::{AppState, AppStateHandler};
use crate::messages::Messages;
use crate::widgets::{TextInput, TextInputState};
use anyhow::anyhow;
use base64::prelude::*;
use mail_common::MailContext;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::layout::Flex;
use ratatui::prelude::*;
use std::sync::Arc;

pub enum Message {
    Submit,
    ToggleInput,
}

enum ActiveInput {
    Username,
    Selector,
    Key,
}

pub struct ForkSelectModel {
    username_input_state: TextInputState,
    selector_input_state: TextInputState,
    key_input_state: TextInputState,
    active_input: ActiveInput,
}

impl ForkSelectModel {
    pub fn new() -> Self {
        Self {
            username_input_state: TextInputState::new().selected(true),
            selector_input_state: TextInputState::new(),
            key_input_state: TextInputState::new().secret(true),
            active_input: ActiveInput::Username,
        }
    }

    fn active_text_input_state(&self) -> &TextInputState {
        match self.active_input {
            ActiveInput::Username => &self.username_input_state,
            ActiveInput::Selector => &self.selector_input_state,
            ActiveInput::Key => &self.key_input_state,
        }
    }

    fn active_text_input_state_mut(&mut self) -> &mut TextInputState {
        match self.active_input {
            ActiveInput::Username => &mut self.username_input_state,
            ActiveInput::Selector => &mut self.selector_input_state,
            ActiveInput::Key => &mut self.key_input_state,
        }
    }
}

impl AppStateHandler for ForkSelectModel {
    fn handle_event(&mut self, event: Event) -> Command<Messages> {
        let Event::Key(k) = event else {
            return Command::None;
        };
        match k.code {
            KeyCode::Esc => Command::none(),
            KeyCode::Enter => Command::message(Message::Submit),
            KeyCode::Tab => Command::message(Message::ToggleInput),
            _ => {
                self.active_text_input_state_mut().handle_event(&event);
                Command::none()
            }
        }
    }

    fn update(&mut self, ctx: &Arc<MailContext>, message: Messages) -> Command<Messages> {
        let Messages::ForkSelect(message) = message else {
            return Command::None;
        };

        match message {
            Message::ToggleInput => {
                self.active_text_input_state_mut().set_selected(false);
                self.active_input = match self.active_input {
                    ActiveInput::Username => ActiveInput::Selector,
                    ActiveInput::Selector => ActiveInput::Key,
                    ActiveInput::Key => ActiveInput::Username,
                };
                self.active_text_input_state_mut().set_selected(true);
                Command::None
            }
            Message::Submit => {
                let username = self.username_input_state.value().trim().to_owned();
                let selector = self.selector_input_state.value().trim().to_owned();
                let key_b64 = self.key_input_state.value().trim().to_owned();

                if username.is_empty() || selector.is_empty() || key_b64.is_empty() {
                    return Command::message(Messages::DisplayError(
                        None,
                        anyhow!("Username, selector and key can not be empty"),
                    ));
                }

                let payload_key = match BASE64_STANDARD.decode(key_b64.as_bytes()) {
                    Ok(key) => key,
                    Err(e) => {
                        return Command::message(Messages::DisplayError(
                            None,
                            anyhow!("Invalid base64 key: {e}"),
                        ));
                    }
                };

                let ctx = Arc::clone(ctx);
                Command::batch([
                    Command::message(Messages::DisplayBackgroundProgress(
                        "Redeeming forked session ...".to_owned(),
                    )),
                    Command::task(async move {
                        let cmd = match ctx
                            .user_context_from_fork(username, selector, payload_key)
                            .await
                        {
                            Ok(user_ctx) => match MailboxModel::new(user_ctx).await {
                                Ok(model) => {
                                    Command::message(Messages::SwitchAppState(model.into()))
                                }
                                Err(e) => Command::message(e),
                            },
                            Err(e) => Command::message(Messages::from(e)),
                        };
                        Command::batch([Command::message(Messages::DismissBackgroundProgress), cmd])
                    }),
                ])
            }
        }
    }

    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.inner(Margin {
            horizontal: 10,
            vertical: 2,
        });

        let [_, username_area, selector_area, key_area, _] = Layout::default()
            .direction(Direction::Vertical)
            .flex(Flex::Center)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Fill(1),
            ])
            .areas(area);

        let max_label_size: u16 = 11;
        frame.render_stateful_widget(
            TextInput::new("Username:").with_max_label_length(max_label_size),
            username_area,
            &mut self.username_input_state,
        );

        frame.render_stateful_widget(
            TextInput::new("Selector:").with_max_label_length(max_label_size),
            selector_area,
            &mut self.selector_input_state,
        );

        frame.render_stateful_widget(
            TextInput::new("Key (b64):").with_max_label_length(max_label_size),
            key_area,
            &mut self.key_input_state,
        );

        let (x, y) = self.active_text_input_state().frame_cursor();
        frame.set_cursor_position(Position { x, y });
    }

    fn view_top_bar(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Line::from(vec![
                Span::styled("Enter: ", Style::new().bold()),
                Span::raw("Submit"),
                Span::styled(" Tab: ", Style::new().bold()),
                Span::raw("Switch Input"),
            ]),
            area,
        );
    }

    fn view_status_bar(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Text::from("Fork Selector"), area);
    }

    fn help_options(&self) -> Vec<(&'static str, &'static str)> {
        vec![("enter", "Submit"), ("tab", "Switch Input")]
    }
}

impl From<ForkSelectModel> for AppState {
    fn from(value: ForkSelectModel) -> Self {
        Self::ForkSelect(value)
    }
}

impl From<Message> for Messages {
    fn from(value: Message) -> Self {
        Self::ForkSelect(value)
    }
}
