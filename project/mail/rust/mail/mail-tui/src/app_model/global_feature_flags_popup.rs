use crate::app::Command;
use crate::app_model::Popup;
use crate::app_model::feature_flag_variant_fmt::variant_suffix_span;
use crate::messages::Messages;
use crate::widgets::utils::ScrollableState;
use crate::widgets::{ScrollableList, ScrollableListState, TextInput, TextInputState};
use mail_common::MailContext;
use mail_core_common::datatypes::Variant;
use mail_core_common::models::{FeatureFlag, ModelExtension};
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph};
use std::sync::Arc;

struct FlagInfo {
    name: String,
    enabled: bool,
    variant: Option<Variant>,
}

pub struct GlobalFeatureFlagsPopup {
    ctx: Arc<MailContext>,
    flags: Vec<FlagInfo>,
    filtered_flags: Vec<usize>,
    list_state: ScrollableListState,
    filter_input_state: TextInputState,
}

impl GlobalFeatureFlagsPopup {
    fn new(ctx: Arc<MailContext>, flags: Vec<FlagInfo>) -> Self {
        let filtered_flags = (0..flags.len()).collect();
        Self {
            ctx,
            flags,
            filtered_flags,
            list_state: ScrollableListState::new(Some(0)),
            filter_input_state: TextInputState::new().selected(true),
        }
    }

    pub fn open(ctx: Arc<MailContext>) -> Command<Messages> {
        Command::task(async move {
            let tether = ctx.core_context().account_stash().connection();

            let db_flags = match FeatureFlag::all(&tether).await {
                Ok(flags) => flags,
                Err(e) => {
                    return Command::message(Messages::DisplayError(
                        Some("Feature Flags".to_owned()),
                        anyhow::anyhow!("Failed to fetch flags: {}", e),
                    ));
                }
            };

            let flags: Vec<FlagInfo> = db_flags
                .into_iter()
                .map(|flag| FlagInfo {
                    name: flag.name.clone(),
                    enabled: flag.enabled,
                    variant: flag.variant(),
                })
                .collect();

            let popup = Self::new(ctx, flags);
            Command::message(Messages::RaisePopup(Box::new(popup)))
        })
    }

    fn update_filter(&mut self) {
        let filter_text = self.filter_input_state.value();
        if filter_text.is_empty() {
            self.filtered_flags = (0..self.flags.len()).collect();
        } else {
            let filter_lower = filter_text.to_lowercase();
            self.filtered_flags = self
                .flags
                .iter()
                .enumerate()
                .filter(|(_, flag)| flag.name.to_lowercase().contains(&filter_lower))
                .map(|(idx, _)| idx)
                .collect();
        }

        if !self.filtered_flags.is_empty() {
            self.list_state = ScrollableListState::new(Some(0));
        }
    }

    fn refresh(ctx: Arc<MailContext>) -> Command<Messages> {
        Command::batch([
            Command::message(Messages::DismissPopup),
            Command::task(async move {
                if let Err(e) = ctx.core_context().feature_flags().refresh().await {
                    return Command::message(Messages::DisplayError(
                        Some("Feature Flags".to_owned()),
                        anyhow::anyhow!("Failed to refresh: {}", e),
                    ));
                }

                Self::open(ctx)
            }),
        ])
    }
}

impl Popup for GlobalFeatureFlagsPopup {
    fn title(&self) -> Option<String> {
        Some("Feature Flags (Global)".to_string())
    }

    fn handle_event(&mut self, event: Event) -> Command<Messages> {
        let Event::Key(key) = event else {
            return Command::None;
        };

        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => return Command::message(Messages::DismissPopup),
            (KeyCode::Char('r'), m) if m.contains(KeyModifiers::CONTROL) => {
                return Self::refresh(self.ctx.clone());
            }
            (KeyCode::Up | KeyCode::Down, _) => {
                self.list_state.handle_event(key.code);
                return Command::None;
            }
            _ => {}
        }

        self.filter_input_state.handle_event(&event);
        self.update_filter();
        Command::None
    }

    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let [filter_area, list_area, help_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .areas(area);

        frame.render_stateful_widget(
            TextInput::new("").with_max_label_length(0),
            filter_area,
            &mut self.filter_input_state,
        );

        let items: Vec<ListItem> = self
            .filtered_flags
            .iter()
            .map(|&idx| {
                let flag = &self.flags[idx];
                let status = if flag.enabled { "✓" } else { "✗" };
                let mut spans = vec![Span::raw(format!("{status} {}", flag.name))];
                if let Some(suffix) = variant_suffix_span(flag.variant.as_ref()) {
                    spans.push(suffix);
                }
                ListItem::from(Line::from(spans))
            })
            .collect();

        let list = ScrollableList::new(List::new(items).block(Block::default()));
        frame.render_stateful_widget(list, list_area, &mut self.list_state);

        let help = vec![
            Line::from(vec![
                Span::raw("↑↓: Navigate | "),
                Span::raw("Ctrl+r: Refresh | "),
                Span::raw("Esc: Close"),
            ]),
            Line::from(vec![Span::raw("dim text: variant info")]),
        ];
        frame.render_widget(Paragraph::new(help), help_area);
    }

    fn height(&self) -> Constraint {
        Constraint::Percentage(70)
    }

    fn width(&self) -> Constraint {
        Constraint::Percentage(60)
    }
}
