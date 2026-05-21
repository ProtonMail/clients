use super::*;

mod message_detector_test_messages;

use html5ever::tendril::TendrilSink;
use insta::assert_snapshot;

fn strip_blockquote_strings(input: &str) -> (String, String) {
    let document = kuchikiki::parse_html().one(input);
    let SplitDoc {
        message,
        blockquote,
    } = strip_blockquote(document);

    let blockquote = blockquote.map_or_else(String::new, |e| e.to_string());

    (message.to_string(), blockquote)
}

#[test]
fn detect_blockquote_or_signature() {
    let input = include_str!("./html/blockquote_or_signature.html");
    let (before, after) = strip_blockquote_strings(input);

    assert!(!before.contains("On Tuesday"));
    assert!(after.contains("On Tuesday"));
}

#[test]
fn should_take_the_last_element_containing_text_in_case_of_sibling_blockquotes() {
    let input = r#"Email content
<div class="protonmail_quote">
    blockquote1
</div>
<div class="protonmail_quote">
    blockquote2
</div>"#;

    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("Email content"));
    assert!(before.contains("blockquote1"));
    assert!(!before.contains("blockquote2"));
    assert!(after.contains("blockquote2"));
    assert!(!after.contains("blockquote1"));
}

#[test]
fn should_take_the_last_element_containing_an_image_in_cas_of_sibling_blockquotes() {
    let input = r#"Email content
<div class="protonmail_quote">
    blockquote1
</div>
<div class="protonmail_quote">
    <span class="proton-image-anchor" />
</div>"#;

    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("Email content"));
    assert!(before.contains("blockquote1"));
    assert!(!before.contains("proton-image-anchor"));
    assert!(after.contains("proton-image-anchor"));
    assert!(!after.contains("blockquote1"));
}

#[test]
fn should_pick_inner_gmail_quote_not_outer_gmail_quote_container_wrapper() {
    // Gmail's "forward" composer wraps the entire body in
    // `.gmail_quote.gmail_quote_container` (user note + forwarded message),
    // with the actual forwarded message in an inner `.gmail_quote`. Only
    // the inner one is a real quote.
    let input = r#"
        <div dir="ltr">
            <div class="gmail_quote gmail_quote_container">
                <div>Hi Andy, here's the message I wanted to forward.</div>
                <div class="gmail_quote">
                    <div class="gmail_attr">---------- Forwarded message ---------<br>From: Someone</div>
                    <p>Original forwarded body</p>
                </div>
            </div>
        </div>"#;

    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("Hi Andy, here"));
    assert!(!before.contains("Forwarded message"));
    assert!(!before.contains("Original forwarded body"));
    assert!(after.contains("Forwarded message"));
    assert!(after.contains("Original forwarded body"));
}

#[test]
fn should_display_nothing_in_blockquote_when_there_is_text_after_blockquotes() {
    let input = r#"Email content
<div class="protonmail_quote">
    blockquote1
</div>
text after blockquote"#;

    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("Email content"));
    assert!(before.contains("blockquote1"));
    assert!(before.contains("text after blockquote"));
    assert!(after.is_empty());
}

#[test]
fn should_display_nothing_in_blockquote_when_there_is_an_image_after_blockquotes() {
    let input = r#"Email content
<div class="protonmail_quote">
    blockquote1
</div>
<span class="proton-image-anchor" />"#;

    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("Email content"));
    assert!(before.contains("blockquote1"));
    assert!(before.contains("proton-image-anchor"));
    assert!(after.is_empty());
}

#[test]
fn should_find_blockquote_in_mail() {
    let mut failed = vec![];
    for (name, mail) in message_detector_test_messages::DEFAULT {
        let (_, after) = strip_blockquote_strings(mail);
        if after.is_empty() {
            failed.push(name);
        }
    }

    assert!(
        failed.is_empty(),
        "finding blockquote failed for messages {failed:#?}"
    );
}

#[test]
fn should_display_nothing_in_blockquote_when_it_is_not_last_important_element() {
    let original = include_str!("./html/interleaved.html");
    let (sanitized, blockquote) = strip_blockquote_strings(original);

    assert_snapshot!(sanitized);
    assert_snapshot!(blockquote);
}

#[test]
fn detect_outlook_word_blue_divider() {
    let input = include_str!("./html/supported/outlook_structureless/word_blue.html");
    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("my reply to your email"));
    assert!(!before.contains("Previous message body"));
    assert!(after.contains("From:"));
    assert!(after.contains("Previous message body"));
}

#[test]
fn detect_outlook_word_grey_divider() {
    let input = include_str!("./html/supported/outlook_structureless/word_grey.html");
    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("my reply to your email"));
    assert!(!before.contains("Previous message body"));
    assert!(after.contains("From:"));
    assert!(after.contains("Previous message body"));
}

#[test]
fn detect_windows_mail_rgb_divider() {
    let input = include_str!("./html/supported/outlook_structureless/windows_mail.html");
    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("My reply text"));
    assert!(!before.contains("Previous message body"));
    assert!(after.contains("From:"));
    assert!(after.contains("Previous message body"));
}

#[test]
fn detect_windows_mail_logical_property_divider() {
    let input = include_str!("./html/supported/outlook_structureless/windows_mail_logical.html");
    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("My reply text"));
    assert!(!before.contains("Previous message body"));
    assert!(after.contains("From:"));
    assert!(after.contains("Previous message body"));
}

// Regression guard for the NBSP-normalization rule in `normalize_from_label`.
// French orthography forces a NBSP before the colon — drop that rewrite and
// every French Outlook reply silently stops being detected.
#[test]
fn detect_outlook_french_nbsp() {
    let input = include_str!("./html/supported/outlook_structureless/french_nbsp.html");
    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("voici ma réponse"));
    assert!(!before.contains("Corps du message précédent"));
    assert!(after.contains("De"));
    assert!(after.contains("Corps du message précédent"));
}

#[test]
fn outlook_quote_captures_following_siblings() {
    let input = include_str!("./html/supported/outlook_structureless/multi_sibling.html");
    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("My short reply"));
    assert!(!before.contains("Previous body line 1"));
    assert!(!before.contains("Previous body line 2"));
    assert!(!before.contains("Previous body line 3"));
    assert!(after.contains("Previous body line 1"));
    assert!(after.contains("Previous body line 2"));
    assert!(after.contains("Previous body line 3"));
}

// Style-only match isn't enough: users style their own divs with the exact
// same Word shorthand for decorative section breaks. The localized "From:"
// label is what promotes a divider to a quote boundary.
#[test]
fn styled_divider_without_from_header_is_kept() {
    let input = include_str!("./html/supported/outlook_structureless/decorative_divider.html");
    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("Just a styled divider"));
    assert!(before.contains("More of my own content"));
    assert!(after.is_empty());
}

// The inverse of the case above: people legitimately discuss "From:" headers
// in body prose. The divider style is what proves the label is structural.
#[test]
fn from_text_without_divider_style_is_kept() {
    let input = include_str!("./html/supported/outlook_structureless/from_text_in_body.html");
    let (before, _after) = strip_blockquote_strings(input);

    assert!(before.contains("From: discussion of mail headers"));
    assert!(before.contains("how the From: header gets parsed"));
}

// Symmetric to flavor A's color guard: Windows Mail / Outlook iOS only emit
// rgb(229, 229, 229) for the reply divider. Without the value check we'd match
// arbitrary 1px-top-border boxes that happen to contain "From:" prose.
#[test]
fn windows_mail_wrong_color_is_kept() {
    let input = r#"<html><body><div>
        <p>Reply</p>
        <div style="padding-top: 5px; border-top-color: red; border-top-width: 1px; border-top-style: solid;">
            <p><b>From:</b> someone discussing red-bordered boxes.</p>
        </div>
        <p>More of my own content.</p>
    </div></body></html>"#;
    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("Reply"));
    assert!(before.contains("red-bordered"));
    assert!(before.contains("More of my own content"));
    assert!(after.is_empty());
}

// Guards the color+width constraints on flavor A: thick borders and arbitrary
// colors must be rejected because Word only ever emits #B5C4DF / #E1E1E1 at
// 1pt for the reply divider. Loosening either check would catch decorative
// callout boxes that happen to contain "From:" in the body.
#[test]
fn unrelated_border_top_is_kept() {
    let input = r#"<html><body><div>
        <p>Note to self</p>
        <div style="border:none;border-top:solid #00FF00 4.0pt;padding:3.0pt 0cm 0cm 0cm">
            <p>From: my future self with a styled callout, not a quote.</p>
        </div>
        <p>More notes.</p>
    </div></body></html>"#;
    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("Note to self"));
    assert!(before.contains("More notes"));
    assert!(after.is_empty());
}

#[test]
fn detect_outlook_locales() {
    let cases: &[(&str, &str)] = &[
        ("English", "From:"),
        ("French", "De\u{00A0}:"),
        ("French (no space)", "De:"),
        ("Spanish/Portuguese", "De:"),
        ("German", "Von:"),
        ("Italian", "Da:"),
        ("Dutch", "Van:"),
        ("Polish/Czech/Slovak", "Od:"),
        ("Russian/Bulgarian", "От:"),
        ("Greek", "Από:"),
        ("Swedish", "Från:"),
        ("Norwegian/Danish", "Fra:"),
        ("Finnish", "Lähettäjä:"),
        ("Hungarian", "Feladó:"),
        ("Turkish", "Kimden:"),
        ("Hebrew", "מאת:"),
        ("Arabic", "من:"),
        ("Vietnamese", "Từ:"),
        ("Thai", "จาก:"),
        ("Japanese", "差出人:"),
        ("Japanese (alt)", "送信者:"),
        ("Simplified Chinese", "发件人:"),
        ("Traditional Chinese", "寄件者:"),
        ("Korean", "보낸 사람:"),
        ("Korean (no space)", "보낸사람:"),
    ];

    let mut failed = vec![];
    for (locale, label) in cases {
        let html = format!(
            r#"<html><body><div>
                <p>my reply</p>
                <div style="border:none;border-top:solid #E1E1E1 1.0pt;padding:3.0pt 0cm 0cm 0cm">
                    <p><b>{label}</b> sender@example.com</p>
                </div>
                <p>quoted body for {locale}</p>
            </div></body></html>"#
        );
        let (before, after) = strip_blockquote_strings(&html);
        let expected = format!("quoted body for {locale}");
        if before.contains(&expected) || !after.contains(&expected) {
            failed.push(*locale);
        }
    }
    assert!(
        failed.is_empty(),
        "structureless-quote detection failed for locales: {failed:#?}"
    );
}

// Real-world Windows Mail fixture that this detector exists to handle:
// Russian "От:" inside a longhand `border-block-start-*` divider. Before the
// structureless-quote pass, the entire historical thread leaked into the
// user-visible reply because none of the selector rules matched.
#[test]
fn windows_mail_fixture_now_detects_cyrillic_from() {
    let input = include_str!("./html/supported/windows_mail.html");
    let (before, after) = strip_blockquote_strings(input);

    assert!(before.contains("Hi. I am fine"));
    assert!(!before.contains("Hello! How are you?"));
    assert!(after.contains("От:"));
    assert!(after.contains("Hello! How are you?"));
}
