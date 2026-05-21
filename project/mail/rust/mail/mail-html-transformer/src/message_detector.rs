//! Proton Mail Message Detector identifies previous messages in HTML content

#[cfg(test)]
#[path = "tests/message_detector.rs"]
mod tests;

use kuchikiki::iter::NodeEdge;
use kuchikiki::{NodeData, NodeRef, Selectors};

use crate::utils::{NodeRefExt, new_element};

const ORIGINAL_MESSAGE: &str = "------- Original Message -------";

// Lowercased so `normalize_from_label` can compare without re-casefolding the
// (potentially long) document text on every check.
const FROM_HEADER_PATTERNS: &[&str] = &[
    "from:",      // English
    "de :",       // French
    "de:",        // Spanish / Portuguese / French (no space)
    "von:",       // German
    "da:",        // Italian
    "van:",       // Dutch
    "od:",        // Polish / Czech / Slovak
    "от:",        // Russian / Bulgarian
    "από:",       // Greek
    "från:",      // Swedish
    "fra:",       // Norwegian / Danish
    "lähettäjä:", // Finnish
    "feladó:",    // Hungarian
    "kimden:",    // Turkish
    "מאת:",       // Hebrew
    "من:",        // Arabic
    "từ:",        // Vietnamese
    "จาก:",       // Thai
    "差出人:",    // Japanese
    "送信者:",    // Japanese (alternative)
    "发件人:",    // Simplified Chinese
    "寄件者:",    // Traditional Chinese
    "보낸 사람:", // Korean
    "보낸사람:",  // Korean (no space)
];

// Microsoft Word hardcodes these two colors when emitting reply dividers.
// The color check is what disambiguates a real quote header from a user's
// own styled callout box that happens to have a 1pt top border.
const OUTLOOK_DIVIDER_COLORS: &[&str] = &["#b5c4df", "#e1e1e1"];

// Windows Mail and Outlook iOS use this specific RGB triplet for the divider.
// As with flavor A's color check, this is what prevents matching arbitrary
// user-styled boxes with a 1px top border. Value matches the Web mail
// implementation (sourced from mailgun/talon's `cut_microsoft_quote`).
const WINDOWS_MAIL_DIVIDER_COLOR: &str = "rgb(229, 229, 229)";
const BLOCKQUOTE_SELECTORS: [&str; 22] = [
    ".protonmail_quote", // Proton Mail
    // Gmail creates both div.gmail_quote and blockquote.gmail_quote. The div
    // version marks text but does not cause indentation, but both should be
    // considered quoted text.
    ".gmail_quote:not(.gmail_quote_container)", // Gmail
    "div.gmail_extra",                          // Gmail
    "div.yahoo_quoted",                         // Yahoo Mail
    "blockquote.iosymail",                      // Yahoo iOS Mail
    ".tutanota_quote",                          // Tutanota Mail
    ".zmail_extra",                             // Zoho
    ".skiff_quote",                             // Skiff Mail
    "blockquote[data-skiff-mail]",              // Skiff Mail
    r"#divRplyFwdMsg",                          // Outlook Mail
    r#"div[id="mail-editor-reference-message-container"]"#, // Outlook
    r#"div[id="3D\"divRplyFwdMsg\""]"#,         // Office365
    "hr[id=replySplit]",
    ".moz-cite-prefix",
    "div[id=isForwardContent]",
    "blockquote[id=isReplyContent]",
    "div[id=mailcontent]",
    "div[id=origbody]",
    "div[id=reply139content]",
    r"blockquote[id=oriMsgHtmlSeperator]",
    r#"blockquote[type="cite"]"#,
    r#"[name="quote"]"#, // gmx
];

static BLOCKQUOTE_SELECTOR: std::sync::LazyLock<Option<Selectors>> =
    std::sync::LazyLock::new(|| {
        let selectors_string = BLOCKQUOTE_SELECTORS
            .map(|v| format!("{v}:not(:empty)"))
            .join(",");

        Selectors::compile(&selectors_string)
            .inspect_err(|()| {
                tracing::warn!("Could not compile selector");
            })
            .ok()
    });

/// This is the result of calling [`locate_blockquote`].
pub struct SplitDoc {
    /// The HTML after the blockquote has been stripped
    pub message: NodeRef,
    /// The HTML of the blockquote if it has found it.
    pub blockquote: Option<NodeRef>,
}

/// Try to remove the blockquote and returns if it had it.
#[must_use]
pub fn strip_blockquote(message: NodeRef) -> SplitDoc {
    let blockquote = strip_blockquote_inner(&message);
    SplitDoc {
        message,
        blockquote,
    }
}

fn traverse_blockquotes(message: NodeRef, selectors: &Selectors) -> impl Iterator<Item = NodeRef> {
    let mut node = Some(message);

    std::iter::from_fn(move || {
        loop {
            let current = node.take()?;
            if matches_node_ref(current.clone(), selectors) {
                // Its a match, we dont want to go deeper, so we take next node instead
                node = current.following_nodes().next();
                return Some(current);
            } else if let Some(child) = current.first_child() {
                node = Some(child);
            } else {
                // Node was childless, we need to process next nodes
                node = current.following_nodes().next();
            }
        }
    })
}

fn matches_node_ref(node_ref: NodeRef, selectors: &Selectors) -> bool {
    let Some(element_ref) = node_ref.into_element_ref() else {
        return false;
    };
    selectors.matches(&element_ref)
}

fn strip_blockquote_inner(message: &NodeRef) -> Option<NodeRef> {
    let selectors = BLOCKQUOTE_SELECTOR.as_ref()?;

    let blockquote = traverse_blockquotes(message.clone(), selectors)
        // We first focus on last blockquote, because the next step iterates on following nodes.
        // And only last blockquote can possibly have NO following nodes ;)
        .last()
        .and_then(move |last_blockquote| {
            if last_blockquote.following_nodes().any(|following_node| {
                // If there is any text content - its not our blockquote
                if !following_node.text_contents().trim().is_empty() {
                    return true;
                }
                // And if there is any image - then again its not our blockquote
                // At this point we already replaced images with an anchor, but we want to keep them
                following_node.select_first(".proton-image-anchor").is_ok()
            }) {
                return None;
            }
            Some(last_blockquote)
        });

    // First let's find an element with a well known selector
    // such as an element with class `protonmail_quote
    if let Some(blockquote) = blockquote {
        blockquote.detach();
        return Some(blockquote);
    }

    // We haven't found such a thing, let's see if we find the string
    // ------- Original Message ------- in a text node and get its parent.
    let text_quote = message
        .traverse_inclusive()
        .filter_map(|node| match node {
            NodeEdge::Start(node_ref) => Some(node_ref),
            NodeEdge::End(_) => None,
        })
        .filter_map(|node_ref| match node_ref.data() {
            NodeData::Text(text) if text.borrow().trim() == ORIGINAL_MESSAGE => {
                node_ref
                    .parent() // Get tag
                    .and_then(|x| x.parent()) // Get partent
            }
            _ => None,
        })
        .last();

    if let Some(text_quote) = text_quote {
        text_quote.detach();
        return Some(text_quote);
    }

    // Outlook desktop, Windows Mail and Outlook iOS sometimes emit replies with
    // no structural marker at all — no class, no id, no wrapping blockquote —
    // so the previous passes have nothing to latch onto. Fall back to a
    // shape-based heuristic (divider style + localized "From:" header).
    if let Some(quote) = strip_outlook_structureless_quote(message) {
        return Some(quote);
    }

    None
}

fn normalize_from_label(s: &str) -> String {
    // French typography puts a NBSP before the colon ("De\u{00A0}:") per
    // orthography rules — without the NBSP→space rewrite our "De :" pattern
    // would silently miss every French Outlook reply.
    s.trim_start().replace('\u{00A0}', " ").to_lowercase()
}

fn contains_from_header(node: &NodeRef) -> bool {
    let text = node.text_contents();
    let normalized = normalize_from_label(&text);
    FROM_HEADER_PATTERNS
        .iter()
        .any(|pattern| normalized.starts_with(pattern))
}

fn parse_inline_style(style: &str) -> Vec<(String, String)> {
    style
        .split(';')
        .filter_map(|decl| {
            let (k, v) = decl.split_once(':')?;
            Some((k.trim().to_lowercase(), v.trim().to_lowercase()))
        })
        .collect()
}

fn style_get<'a>(decls: &'a [(String, String)], key: &str) -> Option<&'a str> {
    decls
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn style_has(decls: &[(String, String)], key: &str) -> bool {
    decls.iter().any(|(k, _)| k == key)
}

// Two divider shapes coexist because the upstream clients disagree about CSS
// conventions: Word/Outlook-desktop folds everything into shorthand, while
// Windows Mail and Outlook iOS spell out longhand top-border properties (with
// some versions using logical `border-block-start-*` instead of physical
// `border-top-*`).
fn is_outlook_quote_divider_style(style: &str) -> bool {
    let decls = parse_inline_style(style);

    if let (Some(border), Some(border_top)) =
        (style_get(&decls, "border"), style_get(&decls, "border-top"))
    {
        let is_solid_1pt = border == "none"
            && border_top.contains("solid")
            && (border_top.contains("1.0pt") || border_top.contains("1pt"));
        let has_accepted_color = OUTLOOK_DIVIDER_COLORS
            .iter()
            .any(|c| border_top.contains(c));
        if is_solid_1pt && has_accepted_color {
            return true;
        }
    }

    let top_style = style_get(&decls, "border-top-style")
        .or_else(|| style_get(&decls, "border-block-start-style"));
    let top_width = style_get(&decls, "border-top-width")
        .or_else(|| style_get(&decls, "border-block-start-width"));
    let top_color = style_get(&decls, "border-top-color")
        .or_else(|| style_get(&decls, "border-block-start-color"));
    let has_top_padding =
        style_has(&decls, "padding-top") || style_has(&decls, "padding-block-start");
    let no_other_side_styles = !style_has(&decls, "border-right-style")
        && !style_has(&decls, "border-bottom-style")
        && !style_has(&decls, "border-left-style")
        && !style_has(&decls, "border-inline-end-style")
        && !style_has(&decls, "border-inline-start-style")
        && !style_has(&decls, "border-block-end-style");

    top_style == Some("solid")
        && top_width.is_some_and(|w| w.contains("1px") || w.contains("1pt"))
        && top_color.is_some_and(|c| c.contains(WINDOWS_MAIL_DIVIDER_COLOR))
        && has_top_padding
        && no_other_side_styles
}

fn is_outlook_quote_divider(node: &NodeRef) -> bool {
    let Some(element_ref) = node.clone().into_element_ref() else {
        return false;
    };
    if &*element_ref.name.local != "div" {
        return false;
    }
    let style = {
        let attrs = element_ref.attributes.borrow();
        attrs.get("style").map(str::to_owned)
    };
    let Some(style) = style else {
        return false;
    };
    is_outlook_quote_divider_style(&style) && contains_from_header(node)
}

fn strip_outlook_structureless_quote(message: &NodeRef) -> Option<NodeRef> {
    // Take the LAST candidate because nothing rules out quote-of-quote chains:
    // every divider but the bottom-most one will fail the "no real content
    // follows" guard below, so only the last one can ever bound the message
    // tail correctly. Same invariant as the selector-based pass above.
    let divider = message
        .traverse_inclusive()
        .filter_map(|edge| match edge {
            NodeEdge::Start(n) => Some(n),
            NodeEdge::End(_) => None,
        })
        .filter(is_outlook_quote_divider)
        .last()?;

    // Outlook does not wrap the quoted body in the divider div; the body lives
    // as peer paragraphs after it in the same parent. We have to gather the
    // divider AND every following sibling, otherwise the quote leaks back into
    // the user's reply.
    let mut captured = vec![divider.clone()];
    let mut sibling = divider.next_sibling();
    while let Some(s) = sibling {
        let next = s.next_sibling();
        captured.push(s);
        sibling = next;
    }

    // If real content sits beyond the captured group (e.g. at uncle level),
    // this divider isn't actually the boundary of the original message — it's
    // a mid-document artifact, and stripping would truncate the user's reply.
    // Bail rather than guess. Same logic as the selector branch.
    let last = captured.last().cloned()?;
    let any_real_following_content = last.following_nodes().any(|following| {
        if !following.text_contents().trim().is_empty() {
            return true;
        }
        following.select_first(".proton-image-anchor").is_ok()
    });
    if any_real_following_content {
        return None;
    }

    let wrapper = new_element::<&str, &str>("div", std::iter::empty());
    for node in captured {
        node.detach();
        wrapper.append(node);
    }
    Some(wrapper)
}
