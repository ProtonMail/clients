//! Fast HTML-to-plain-text for search indexing only.
//!
//! No DOM parse, no kuchikiki/html2text. Strips tags, drops script/style content,
//! decodes common entities, collapses whitespace. Good enough for indexing;
//! use the full Transformer pipeline for display/sanitization.
//!
//! # Broken / invalid HTML
//!
//! This is a **single-pass tag-stripping** implementation, not a full HTML5 parser.
//! It handles broken or invalid HTML gracefully:
//!
//! - **Unclosed tags** (e.g. `<p>hello`): Content is emitted; the missing `>` is
//!   never found so we scan until end of input. No panic.
//! - **Unclosed comments** (`<!-- foo`): We scan for `-->`; if not found, we consume
//!   to end of input. Content after is not treated as comment.
//! - **Malformed entities** (`&` at end, `&unknown;`): Emit literal `&` or pass through.
//! - **Mismatched nesting** (`<p><div>a</p></div>`): We don't build a tree; we just
//!   skip tag-like regions and emit text. Result may differ from a spec-compliant parser.
//! - **Script/style without close tag**: We scan for `</script>` / `</style>`; if
//!   never found, we consume to end of input (content is dropped).
//!
//! The approach is **resilient**: it never panics, never hangs, and degrades gracefully.
//! Output may include stray `>` or partial text for edge cases, but for search indexing
//! the important words are still extracted. See the `malformed_html_resilience` tests.

/// Max HTML size (bytes) before truncation. Prevents excessive inputs (e.g. huge newsletters)
/// from blocking the indexing worker. 1 MB set for email bodies.
const MAX_HTML_BYTES: usize = 1_048_576;

/// Convert HTML to plain text using a single-pass strip.
/// - Drops `<script>...</script>` and `<style>...</style>` content
/// - Replaces all other tags (and comments) with space
/// - Decodes common HTML entities
/// - Collapses whitespace and trims
#[must_use]
pub fn html_to_text_fast(html: &str) -> String {
    let html = if html.len() > MAX_HTML_BYTES {
        &html[..str::floor_char_boundary(html, MAX_HTML_BYTES)]
    } else {
        html
    };
    let bytes = html.as_bytes();
    let mut out = Vec::with_capacity(html.len() / 2);
    let mut i = 0;
    let mut last_was_space = true;

    while i < bytes.len() {
        if bytes[i] == b'<' {
            i = handle_tag(bytes, i, &mut out, &mut last_was_space);
            continue;
        }

        if bytes[i] == b'&' {
            let (decoded, len) = decode_entity(&bytes[i..]);
            i += len;
            for b in decoded.as_bytes() {
                let is_space = *b == b' ' || *b == b'\t' || *b == b'\n' || *b == b'\r';
                if is_space {
                    if !last_was_space {
                        out.push(b' ');
                        last_was_space = true;
                    }
                } else {
                    out.push(*b);
                    last_was_space = false;
                }
            }
            continue;
        }

        let b = bytes[i];
        let is_space = b == b' ' || b == b'\t' || b == b'\n' || b == b'\r';
        if is_space {
            if !last_was_space {
                out.push(b' ');
                last_was_space = true;
            }
        } else {
            out.push(b);
            last_was_space = false;
        }
        i += 1;
    }

    String::from_utf8_lossy(&out).trim().to_string()
}

fn emit_space_if_needed(out: &mut Vec<u8>, last_was_space: &mut bool) {
    if !*last_was_space {
        out.push(b' ');
        *last_was_space = true;
    }
}

/// Handles script, style, noscript, template, and comments. Returns `Some(new_i)` if matched.
fn try_skip_removal_or_comment(
    bytes: &[u8],
    i: usize,
    rest: &[u8],
    out: &mut Vec<u8>,
    last_was_space: &mut bool,
) -> Option<usize> {
    if rest.len() >= 8 && to_ascii_lower(rest[1]) == b's' {
        if matches_ignore_ascii_case(rest, b"<script") {
            let new_i = skip_until_close_tag(bytes, i, b"script");
            emit_space_if_needed(out, last_was_space);
            return Some(new_i);
        }
        if matches_ignore_ascii_case(rest, b"<style") {
            let new_i = skip_until_close_tag(bytes, i, b"style");
            emit_space_if_needed(out, last_was_space);
            return Some(new_i);
        }
    }
    if rest.len() >= 10
        && to_ascii_lower(rest[1]) == b'n'
        && matches_ignore_ascii_case(rest, b"<noscript")
    {
        let new_i = skip_until_close_tag(bytes, i, b"noscript");
        emit_space_if_needed(out, last_was_space);
        return Some(new_i);
    }
    if rest.len() >= 10
        && to_ascii_lower(rest[1]) == b't'
        && matches_ignore_ascii_case(rest, b"<template")
    {
        let new_i = skip_until_close_tag(bytes, i, b"template");
        emit_space_if_needed(out, last_was_space);
        return Some(new_i);
    }
    if rest.len() >= 4 && rest[1..4] == *b"!--" {
        let mut j = i + 4;
        while j + 3 <= bytes.len() && bytes[j..j + 3] != *b"-->" {
            j += 1;
        }
        let new_i = if j + 3 <= bytes.len() {
            j + 3
        } else {
            bytes.len()
        };
        emit_space_if_needed(out, last_was_space);
        return Some(new_i);
    }
    None
}

/// Handles any other tag: extract name, skip to >, emit newline for block else space.
fn handle_other_tag(bytes: &[u8], i: usize, out: &mut Vec<u8>, last_was_space: &mut bool) -> usize {
    let mut tag_start = i + 1;
    if tag_start < bytes.len() && bytes[tag_start] == b'/' {
        tag_start += 1;
    }
    let mut tag_end = tag_start;
    while tag_end < bytes.len()
        && bytes[tag_end] != b' '
        && bytes[tag_end] != b'>'
        && bytes[tag_end] != b'/'
    {
        tag_end += 1;
    }
    let tag_name = &bytes[tag_start..tag_end];

    let mut j = i + 1;
    while j < bytes.len() {
        let b = bytes[j];
        if b == b'>' {
            j += 1;
            break;
        }
        if b == b'"' || b == b'\'' {
            let quote = b;
            j += 1;
            while j < bytes.len() && bytes[j] != quote {
                j += 1;
            }
            if j < bytes.len() {
                j += 1;
            }
        } else {
            j += 1;
        }
    }
    if !*last_was_space {
        if is_block_tag(tag_name) {
            out.push(b'\n');
        } else {
            out.push(b' ');
        }
        *last_was_space = true;
    }
    j
}

fn handle_tag(bytes: &[u8], i: usize, out: &mut Vec<u8>, last_was_space: &mut bool) -> usize {
    let rest = &bytes[i..];
    if let Some(new_i) = try_skip_removal_or_comment(bytes, i, rest, out, last_was_space) {
        return new_i;
    }
    handle_other_tag(bytes, i, out, last_was_space)
}

#[inline]
fn to_ascii_lower(b: u8) -> u8 {
    if b.is_ascii_uppercase() {
        b + (b'a' - b'A')
    } else {
        b
    }
}

fn matches_ignore_ascii_case(rest: &[u8], prefix: &[u8]) -> bool {
    if rest.len() < prefix.len() {
        return false;
    }
    rest[..prefix.len()]
        .iter()
        .zip(prefix.iter())
        .all(|(&a, &b)| to_ascii_lower(a) == to_ascii_lower(b))
}

/// Block-level HTML elements that introduce a line break when stripped.
fn is_block_tag(tag: &[u8]) -> bool {
    if tag.is_empty() {
        return false;
    }
    let eq = |a: &[u8], b: &[u8]| {
        a.len() == b.len()
            && a.iter()
                .zip(b.iter())
                .all(|(&x, &y)| to_ascii_lower(x) == y)
    };
    match tag.len() {
        1 => eq(tag, b"p"),
        2 => {
            eq(tag, b"br")
                || eq(tag, b"hr")
                || eq(tag, b"ul")
                || eq(tag, b"ol")
                || eq(tag, b"li")
                || eq(tag, b"tr")
                || eq(tag, b"td")
                || eq(tag, b"th")
                || eq(tag, b"dl")
                || eq(tag, b"dt")
                || eq(tag, b"dd")
                || (to_ascii_lower(tag[0]) == b'h' && tag[1] >= b'1' && tag[1] <= b'6') // h1-h6
        }
        3 => eq(tag, b"div") || eq(tag, b"pre") || eq(tag, b"nav"),
        5 => eq(tag, b"table") || eq(tag, b"thead") || eq(tag, b"tbody") || eq(tag, b"tfoot"),
        6 => eq(tag, b"header") || eq(tag, b"footer") || eq(tag, b"figure") || eq(tag, b"section"),
        7 => eq(tag, b"article") || eq(tag, b"blockquote"),
        8 => eq(tag, b"figcaption"),
        _ => false,
    }
}

/// Find next `</tagname>` (case-insensitive) and return index past `>`.
fn skip_until_close_tag(bytes: &[u8], start: usize, tag: &[u8]) -> usize {
    let mut i = start;
    while i + 2 + tag.len() < bytes.len() {
        if bytes[i] == b'<' && bytes[i + 1] == b'/' {
            let slice = &bytes[i + 2..];
            if slice.len() >= tag.len() {
                let mut eq = true;
                for (a, &b) in slice.iter().zip(tag.iter()) {
                    if to_ascii_lower(*a) != to_ascii_lower(b) {
                        eq = false;
                        break;
                    }
                }
                if eq {
                    let after_tag = i + 2 + tag.len();
                    if after_tag < bytes.len()
                        && (bytes[after_tag] == b'>' || bytes[after_tag] == b' ')
                    {
                        let gt = bytes[after_tag..].iter().position(|&c| c == b'>');
                        if let Some(pos) = gt {
                            return after_tag + pos + 1;
                        }
                    }
                }
            }
        }
        i += 1;
    }
    bytes.len()
}

/// Decode one entity starting at `bytes`. Returns (decoded char as string, byte length consumed).
fn decode_entity(bytes: &[u8]) -> (String, usize) {
    if bytes.is_empty() || bytes[0] != b'&' {
        return (String::new(), 0);
    }
    let mut i = 1;
    while i < bytes.len() && bytes[i] != b';' && bytes[i] != b'&' {
        i += 1;
    }
    let end = i;
    if end >= bytes.len() || bytes[end] != b';' {
        return ("&".to_string(), 1);
    }
    let entity = &bytes[1..end];
    let len = end + 1;

    match entity {
        b"amp" => ("&".to_string(), len),
        b"lt" => ("<".to_string(), len),
        b"gt" => (">".to_string(), len),
        b"quot" => ("\"".to_string(), len),
        b"apos" | b"#39" => ("'".to_string(), len),
        b"nbsp" => (" ".to_string(), len),
        _ if entity.len() >= 2 && entity[0] == b'#' => {
            let num = if entity.len() >= 2 && (entity[1] == b'x' || entity[1] == b'X') {
                let hex_str = std::str::from_utf8(&entity[2..]).unwrap_or_default();
                u32::from_str_radix(hex_str, 16).ok()
            } else {
                let dec_str = std::str::from_utf8(&entity[1..]).unwrap_or_default();
                dec_str.parse::<u32>().ok()
            };
            match num.and_then(char::from_u32) {
                Some(c) => (c.to_string(), len),
                None => ("\u{FFFD}".to_string(), len),
            }
        }
        _ => ("&".to_string(), 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("<p>Hello</p>", "Hello" ; "simple_paragraph")]
    #[test_case("<p>Hello <b>world</b></p>", "Hello world" ; "nested_tags")]
    #[test_case("<div><span>text</span></div>", "text" ; "div_span")]
    #[test_case("<h1>Title</h1><p>Body</p>", "Title\nBody" ; "multiple_blocks")]
    #[test_case("plain text", "plain text" ; "no_tags")]
    #[test_case("", "" ; "empty")]
    fn strip_tags(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    #[test_case("<p>Hello</p>", "Hello" ; "lowercase")]
    #[test_case("<P>Hello</P>", "Hello" ; "uppercase")]
    #[test_case("<Script>alert(1)</Script>", "" ; "script_mixed_case")]
    #[test_case("<STYLE>.x{}</STYLE>", "" ; "style_uppercase")]
    fn tag_case_insensitive(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    #[test_case("Before <script>alert(1)</script> after", "Before after" ; "script_inline")]
    #[test_case("Before <style>.x{}</style> after", "Before after" ; "style_inline")]
    #[test_case("<script>drop</script>text", "text" ; "script_then_text")]
    #[test_case("a<script>X</script>b<style>Y</style>c", "a b c" ; "script_and_style")]
    #[test_case("<script type=\"text/javascript\">alert('x')</script>", "" ; "script_with_attrs")]
    #[test_case("<style type=\"text/css\">.cls{color:red}</style>", "" ; "style_with_attrs")]
    fn strip_script_style(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    #[test_case("a&amp;b", "a&b" ; "amp")]
    #[test_case("&lt;tag&gt;", "<tag>" ; "lt_gt")]
    #[test_case("&quot;quoted&quot;", "\"quoted\"" ; "quot")]
    #[test_case("&#39;apos&#39;", "'apos'" ; "apos_numeric")]
    #[test_case("&apos;x&apos;", "'x'" ; "apos_named")]
    #[test_case("&nbsp;space", "space" ; "nbsp")]
    #[test_case("&#65;&#x42;", "AB" ; "numeric_decimal_hex")]
    #[test_case("&#x00E9;", "é" ; "unicode_hex")]
    #[test_case("&#233;", "é" ; "unicode_decimal")]
    fn decode_entities(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    #[test_case("&#xD800;", "\u{FFFD}" ; "surrogate_high")]
    #[test_case("&#xDFFF;", "\u{FFFD}" ; "surrogate_low")]
    #[test_case("&unknown;", "&unknown;" ; "unknown_entity")]
    #[test_case("a&b", "a&b" ; "amp_no_semicolon")]
    fn entity_edge_cases(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    #[test_case("  one   two\n\tthree  ", "one two three" ; "mixed_whitespace")]
    #[test_case("\n\nleading\n\n", "leading" ; "leading_trailing_newlines")]
    #[test_case("<p>  spaced  </p>", "spaced" ; "whitespace_in_tags")]
    fn collapse_whitespace(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    //Whitespace correctness around removals — single space between words, no extra spaces
    #[test_case("word1<script>drop</script>word2", "word1 word2" ; "removal_between_words")]
    #[test_case("word1 <script>drop</script> word2", "word1 word2" ; "removal_with_existing_spaces")]
    #[test_case("word1<script>drop</script><style>drop</style>word2", "word1 word2" ; "consecutive_removals_between")]
    #[test_case("<script>drop</script>word", "word" ; "removal_at_start")]
    #[test_case("word<script>drop</script>", "word" ; "removal_at_end")]
    #[test_case("<script>drop</script><style>drop</style>word", "word" ; "consecutive_removals_at_start")]
    #[test_case("a<b>c", "a c" ; "tag_between_words")]
    #[test_case("a<p></p>b", "a\nb" ; "empty_block_between")]
    #[test_case("a<!-- -->b", "a b" ; "comment_between")]
    #[test_case("a<!-- --><!-- -->b", "a b" ; "consecutive_comments_between")]
    #[test_case("  <p>x</p>  ", "x" ; "leading_trailing_around_tag")]
    #[test_case("a\n<script>drop</script>\nb", "a b" ; "newlines_around_removal")]
    #[test_case("a</p><p>b", "a\nb" ; "adjacent_block_tags")]
    #[test_case("a<b></b>c", "a c" ; "empty_inline_between")]
    fn whitespace_around_removals(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    //Block vs inline spacing — block elements (p, div, h1–h6) get newline; inline (span, b) get space
    #[test_case("<p>a</p><p>b</p>", "a\nb" ; "block_paragraphs")]
    #[test_case("<div>a</div><div>b</div>", "a\nb" ; "block_divs")]
    #[test_case("<span>a</span><span>b</span>", "a b" ; "inline_spans")]
    #[test_case("<h1>Title</h1><p>Body</p>", "Title\nBody" ; "block_heading_paragraph")]
    #[test_case("<p>a</p><span>b</span><p>c</p>", "a\nb c" ; "block_inline_block")]
    #[test_case("a<br>b", "a\nb" ; "br_newline")]
    fn block_vs_inline_spacing(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    // Hidden content beyond script/style — noscript, template dropped like script/style
    #[test_case("before<noscript>fallback</noscript>after", "before after" ; "noscript_dropped")]
    #[test_case("before<template><p>x</p></template>after", "before after" ; "template_dropped")]
    #[test_case("a<noscript>JS off</noscript>b<template>tpl</template>c", "a b c" ; "noscript_and_template")]
    fn hidden_content_dropped(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    #[test_case("<!-- comment -->", "" ; "comment_only")]
    #[test_case("a<!-- drop -->b", "a b" ; "comment_between")]
    #[test_case("<!-- multi\nline\ncomment -->", "" ; "multiline_comment")]
    fn strip_comments(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    #[test_case("<div class=\"foo\">Hello</div>", "Hello" ; "tag_with_attrs")]
    #[test_case("<a href=\"http://x.com\">link</a>", "link" ; "anchor")]
    #[test_case("<img src=\"x.png\" alt=\"pic\">", "" ; "self_closing")]
    #[test_case("<br/>", "" ; "br_self_closing")]
    #[test_case("<p>unclosed", "unclosed" ; "unclosed_tag")]
    fn tags_with_attributes(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    // Malformed HTML resilience — no panic, no hang, graceful degradation
    #[test_case("<", "" ; "lone_open_angle")]
    #[test_case("<>", "" ; "empty_tag")]
    #[test_case("a<", "a" ; "open_angle_at_end")]
    #[test_case("a>b", "a>b" ; "stray_gt_in_text")]
    #[test_case("<p>hello", "hello" ; "unclosed_tag_with_content")]
    #[test_case("<p", "" ; "tag_no_close_angle")]
    #[test_case("a<!--", "a" ; "unclosed_comment")]
    #[test_case("a<!-- foo", "a" ; "unclosed_comment_with_content")]
    #[test_case("&", "&" ; "amp_at_end")]
    #[test_case("<script>no close", "" ; "script_no_close_tag")]
    #[test_case("<p><div>a</p></div>", "a" ; "mismatched_nesting")]
    #[test_case("a&lt;b", "a<b" ; "lt_entity")]
    fn malformed_html_resilience(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    // Attribute parsing edge cases — must not treat > inside quoted attrs as tag end
    #[test_case("<p attr=\"value\">x</p>", "x" ; "double_quoted_attr")]
    #[test_case("<p attr='value'>x</p>", "x" ; "single_quoted_attr")]
    #[test_case("<p attr=val>x</p>", "x" ; "unquoted_attr")]
    #[test_case("<p a=\"b\" c='d'>x</p>", "x" ; "mixed_quotes")]
    #[test_case("<p title=\"x>y\">z</p>", "z" ; "gt_inside_double_quotes")]
    #[test_case("<p title='x>y'>z</p>", "z" ; "gt_inside_single_quotes")]
    #[test_case("<p empty=\"\">x</p>", "x" ; "empty_attr_value")]
    fn attribute_parsing_edge_cases(html: &str, expected: &str) {
        assert_eq!(html_to_text_fast(html), expected);
    }

    // Transformer test emails — run against real HTML fixtures
    #[test]
    fn transformer_test_emails_no_panic() {
        let fixtures = [
            include_str!("../tests/htmls/empty.html"),
            include_str!("../tests/htmls/strip_bad.html"),
            include_str!("../tests/htmls/acceptable.html"),
            include_str!("../tests/htmls/styled.html"),
            include_str!("../tests/htmls/email_privacy_tester.html"),
            include_str!("../tests/htmls/strip_uri_elements.html"),
            include_str!("../tests/htmls/nested.html"),
        ];
        for (name, html) in [
            "empty",
            "strip_bad",
            "acceptable",
            "styled",
            "email_privacy_tester",
            "strip_uri_elements",
            "nested",
        ]
        .iter()
        .zip(fixtures.iter())
        {
            let out = html_to_text_fast(html);
            assert!(
                !out.contains('<') || out.contains("&lt;"),
                "{name}: output should not contain raw < (except as entity)"
            );
        }
    }

    #[test]
    fn transformer_strip_bad_extracts_visible_text() {
        let html = include_str!("../tests/htmls/strip_bad.html");
        let out = html_to_text_fast(html);
        assert!(out.contains("foo"));
        assert!(out.contains("bar"));
        assert!(out.contains("baz"));
        assert!(!out.contains("alert"));
        assert!(!out.contains("Hello, world"));
    }

    #[test]
    fn transformer_smoke_extracts_visible_text() {
        let html = include_str!("../tests/htmls/styled.html");
        let out = html_to_text_fast(html);
        assert!(
            out.contains("Example") || out.contains("style") || !out.is_empty(),
            "should extract some visible text"
        );
    }

    #[test]
    fn transformer_newsletter_no_panic() {
        let html = include_str!("tests/html/newsletter.html");
        let _out = html_to_text_fast(html);
    }

    #[test]
    fn transformer_amos_http_no_panic() {
        let html = include_str!("../benches/amos_http.html");
        let out = html_to_text_fast(html);
        assert!(
            !out.is_empty() || html.len() < 100,
            "large HTML should yield some text"
        );
    }

    #[test]
    fn size_guard_truncates_oversized_input() {
        let prefix = "<p>Start</p>";
        let after_limit = "<p>End</p>";
        let padding = "x".repeat(MAX_HTML_BYTES);
        let html = format!("{prefix}{padding}{after_limit}");
        assert!(html.len() > MAX_HTML_BYTES);
        let out = html_to_text_fast(&html);
        assert!(
            !out.contains("End"),
            "content after limit should not appear"
        );
    }
}
