use mail_common::datatypes::CategoryLabel;
use mail_core_common::datatypes::SystemLabel;
use ratatui::prelude::*;
use ratatui::widgets::Tabs;

pub fn category_tabs(categories: &[CategoryLabel]) -> impl Widget {
    let active_idx = categories.iter().position(|c| c.enabled).unwrap_or(0);
    let titles: Vec<Line> = categories.iter().map(category_tab_title).collect();
    Tabs::new(titles)
        .select(active_idx)
        .highlight_style(Style::new().bold().underlined())
        .divider(Span::raw("  "))
        .padding(" ", " ")
}

fn category_tab_title(label: &CategoryLabel) -> Line<'static> {
    let name = category_display_name(label.system_label);
    let text = if label.unread > 0 {
        let count = if label.unread > 999 {
            "999+".to_string()
        } else {
            label.unread.to_string()
        };
        format!("{name} {count}")
    } else {
        name.to_string()
    };
    Line::from(text)
}

fn category_display_name(label: SystemLabel) -> &'static str {
    match label {
        SystemLabel::CategoryDefault => "Primary",
        SystemLabel::CategorySocial => "Social",
        SystemLabel::CategoryPromotions => "Promotions",
        SystemLabel::CategoryNewsletter => "Newsletters",
        SystemLabel::CategoryUpdates => "Updates",
        SystemLabel::CategoryTransactions => "Transactions",
        _ => "Other",
    }
}
