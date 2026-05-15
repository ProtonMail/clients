use mail_core_common::datatypes::Variant;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use std::fmt::Write;

const PAYLOAD_VALUE_MAX: usize = 40;

pub fn variant_suffix_span(variant: Option<&Variant>) -> Option<Span<'static>> {
    let v = variant?;
    let state = if v.enabled { "on" } else { "off" };
    let mut text = format!("  variant={}[{state}]", v.name);
    if let Some(payload) = &v.payload {
        let _ = write!(
            text,
            " payload={:?}:\"{}\"",
            payload.ty,
            truncate(&payload.value, PAYLOAD_VALUE_MAX)
        );
    }
    Some(Span::styled(text, Style::default().fg(Color::DarkGray)))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}
