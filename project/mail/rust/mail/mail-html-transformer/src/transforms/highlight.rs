//! Search-keyword highlighting pass.
//!
//! Wraps occurrences of the search query terms in the visible text of the message with
//! `<mark class="proton-search-highlight">` elements so the client renders them highlighted.
//! The companion stylesheet is injected by
//! [`crate::transforms::styles::inject_search_highlight_css`].
//!
//! This is term-based, not position-based: the search index is plain text and its offsets do not
//! map to the rendered (and transformed) HTML, so we re-find the terms here. Because we operate on
//! the parsed DOM, matches inside element attributes, `<script>` and `<style>` are skipped
//! structurally — only text nodes of visible elements are considered.

use std::collections::BTreeSet;

use kuchikiki::iter::NodeEdge;
use kuchikiki::{NodeData, NodeRef};
use regex::{Regex, RegexBuilder};

use crate::utils::new_element;

/// Upper bound on the number of distinct terms we highlight. Bounds the size of the alternation
/// regex so an oversized query can't drive unbounded compile cost; when the cap is hit the longest
/// (most specific) terms are kept.
const MAX_TERMS: usize = 16;
/// Terms shorter than this are dropped: single characters match far too much to be useful.
const MIN_TERM_LENGTH: usize = 2;

/// Boolean search operators that are part of the query syntax, not literal terms to highlight.
const BOOLEAN_OPERATORS: [&str; 3] = ["and", "or", "not"];

const MARK_TAG: &str = "mark";
const MARK_CLASS: &str = "proton-search-highlight";

/// Wraps occurrences of the sanitized [`query`] terms in the visible text of [`document`] with
/// `<mark>` elements.
///
/// Returns `true` if at least one occurrence was highlighted, so the caller can decide whether the
/// highlight stylesheet needs to be injected.
pub fn highlight_search_terms(document: &NodeRef, query: &str) -> bool {
    let terms = sanitize_terms(query);
    let Some(regex) = build_regex(&terms) else {
        return false;
    };

    // Safe to mutate while iterating this lazy traversal: `insert_before` only adds siblings ahead
    // of the already-visited node, and detaching is deferred until after the walk completes (same
    // reasoning as `transforms::insert_links`).
    let text_nodes = document
        .traverse_inclusive()
        .filter_map(|edge| match edge {
            NodeEdge::Start(node) => Some(node),
            NodeEdge::End(_) => None,
        })
        .filter(|node| !is_opaque_text_container(node))
        .flat_map(|node| node.children())
        .filter(|child| matches!(child.data(), NodeData::Text(_)));

    let mut detach_me = Vec::new();
    for text_node in text_nodes {
        let NodeData::Text(text) = text_node.data() else {
            continue;
        };
        let Some(replacement) = wrap_matches(&regex, &text.borrow()) else {
            continue;
        };
        for node in replacement {
            text_node.insert_before(node);
        }
        detach_me.push(text_node);
    }

    let highlighted = !detach_me.is_empty();
    for node in detach_me {
        node.detach();
    }
    highlighted
}

/// `<script>` and `<style>` hold raw text that must never be highlighted (it isn't visible content
/// and altering it would corrupt the element).
fn is_opaque_text_container(node: &NodeRef) -> bool {
    let NodeData::Element(data) = node.data() else {
        return false;
    };
    let name = &*data.name.local;
    name == "script" || name == "style"
}

/// Splits the raw query into a deduplicated set of literal terms to highlight, capped at
/// [`MAX_TERMS`].
///
/// Mirrors the previous Android implementation: boolean operators are dropped, terms shorter than
/// [`MIN_TERM_LENGTH`] are dropped, and terms are deduplicated case-insensitively. Terms are
/// lowercased — the alternation regex is case-insensitive so the original casing is irrelevant, and
/// the wrapped `<mark>` carries the matched text from the document rather than the term itself.
fn sanitize_terms(query: &str) -> BTreeSet<String> {
    let unique: BTreeSet<String> = query
        .split_whitespace()
        .filter(|token| token.chars().count() >= MIN_TERM_LENGTH)
        .map(str::to_lowercase)
        .filter(|token| !BOOLEAN_OPERATORS.contains(&token.as_str()))
        .collect();

    if unique.len() <= MAX_TERMS {
        return unique;
    }

    // Over the cap: keep the longest terms — they are the most specific. Sorting a set iterated in
    // its (deterministic) order keeps the choice stable when several terms share a length.
    let mut by_length: Vec<String> = unique.into_iter().collect();
    by_length.sort_by_key(|term| std::cmp::Reverse(term.chars().count()));
    by_length.truncate(MAX_TERMS);
    by_length.into_iter().collect()
}

/// Builds a single case-insensitive alternation regex from the literal terms.
///
/// One compiled regex is reused across every text node. `regex` is a linear-time DFA engine with
/// no backtracking, so a flat alternation of escaped terms is safe regardless of body size. The
/// term count is bounded by [`MAX_TERMS`], so the compiled pattern stays small.
fn build_regex(terms: &BTreeSet<String>) -> Option<Regex> {
    if terms.is_empty() {
        return None;
    }

    let mut pattern = String::with_capacity(terms.iter().map(|t| t.len() + 8).sum());
    pattern.push_str("(?:");
    for (index, term) in terms.iter().enumerate() {
        if index > 0 {
            pattern.push('|');
        }
        pattern.push_str(&regex::escape(term));
    }
    pattern.push(')');

    RegexBuilder::new(&pattern)
        .case_insensitive(true)
        .build()
        .ok()
}

/// Splits [`text`] into the sibling nodes that replace the original text node: plain text nodes for
/// the unmatched spans and `<mark>` elements wrapping each match. Returns `None` (no allocation)
/// when nothing matches.
///
/// Nodes are built directly rather than via an HTML string: text-node content is escaped by the
/// serializer on its own, so no manual escaping or reparsing is needed.
fn wrap_matches(regex: &Regex, text: &str) -> Option<Vec<NodeRef>> {
    let mut matches = regex.find_iter(text).peekable();
    matches.peek()?;

    let mut nodes = Vec::new();
    let mut last_end = 0;

    for matched in matches {
        if matched.start() > last_end {
            nodes.push(NodeRef::new_text(&text[last_end..matched.start()]));
        }
        let mark = new_element(MARK_TAG, [("class", MARK_CLASS)]);
        mark.append(NodeRef::new_text(matched.as_str()));
        nodes.push(mark);
        last_end = matched.end();
    }

    if last_end < text.len() {
        nodes.push(NodeRef::new_text(&text[last_end..]));
    }

    Some(nodes)
}

#[cfg(test)]
mod tests {
    use html5ever::tendril::TendrilSink;

    use super::*;

    fn highlight(html: &str, query: &str) -> (String, bool) {
        let document = kuchikiki::parse_html().one(html);
        let highlighted = highlight_search_terms(&document, query);
        (document.to_string(), highlighted)
    }

    #[test]
    fn wraps_a_single_term() {
        let (out, highlighted) = highlight("<p>hello world</p>", "world");
        assert!(highlighted);
        assert!(
            out.contains(r#"<mark class="proton-search-highlight">world</mark>"#),
            "{out}"
        );
        assert!(out.contains("hello "), "{out}");
    }

    #[test]
    fn matching_is_case_insensitive() {
        let (out, highlighted) = highlight("<p>Hello WORLD</p>", "hello world");
        assert!(highlighted);
        assert!(out.contains(r#"<mark class="proton-search-highlight">Hello</mark>"#));
        assert!(out.contains(r#"<mark class="proton-search-highlight">WORLD</mark>"#));
    }

    #[test]
    fn wraps_multiple_distinct_terms() {
        let (out, _) = highlight("<p>alpha beta gamma</p>", "alpha gamma");
        assert!(out.contains(r#"<mark class="proton-search-highlight">alpha</mark>"#));
        assert!(out.contains(r#"<mark class="proton-search-highlight">gamma</mark>"#));
        assert!(!out.contains(r#"<mark class="proton-search-highlight">beta</mark>"#));
    }

    #[test]
    fn preserves_order_of_replacement_nodes() {
        // `wrap_matches` now emits a sequence of sibling nodes (text + `<mark>`) that the caller
        // inserts before the original text node. Assert the unmatched gap and both marks keep their
        // original left-to-right order, with no nodes dropped or reordered.
        let (out, _) = highlight("<p>aa bb</p>", "aa bb");
        assert!(
            out.contains(concat!(
                r#"<mark class="proton-search-highlight">aa</mark>"#,
                " ",
                r#"<mark class="proton-search-highlight">bb</mark>"#,
            )),
            "{out}"
        );
    }

    #[test]
    fn does_not_match_returns_false() {
        let (out, highlighted) = highlight("<p>hello world</p>", "absent");
        assert!(!highlighted);
        assert!(!out.contains("proton-search-highlight"), "{out}");
    }

    #[test]
    fn skips_script_contents() {
        let (out, highlighted) = highlight("<p>token</p><script>var token = 1;</script>", "token");
        assert!(highlighted);
        // The visible paragraph is highlighted...
        assert!(out.contains(r#"<mark class="proton-search-highlight">token</mark>"#));
        // ...but the script body is untouched.
        assert!(out.contains("var token = 1;"), "{out}");
    }

    #[test]
    fn skips_style_contents() {
        let (out, highlighted) =
            highlight("<style>.token { color: red; }</style><p>token</p>", "token");
        assert!(highlighted);
        assert!(out.contains(".token { color: red; }"), "{out}");
    }

    #[test]
    fn does_not_touch_attributes() {
        let (out, highlighted) = highlight(r#"<a href="https://token.example">link</a>"#, "token");
        assert!(!highlighted);
        assert!(out.contains(r#"href="https://token.example""#), "{out}");
    }

    #[test]
    fn escapes_surrounding_markup_characters() {
        let (out, _) = highlight("<p>a &lt; token &amp; b</p>", "token");
        assert!(out.contains(r#"<mark class="proton-search-highlight">token</mark>"#));
        // The literal < and & characters survive the reparse as entities.
        assert!(out.contains("&lt;"), "{out}");
        assert!(out.contains("&amp;"), "{out}");
    }

    #[test]
    fn escapes_special_characters_inside_the_match() {
        // The matched text is placed into a text node directly; the serializer must re-encode the
        // markup-significant characters it contains, so a match can never inject raw `<`/`>`/`&`.
        // Input entities decode to the text node `a<b&c>d`, which the term matches in full.
        let (out, highlighted) = highlight("<p>a&lt;b&amp;c&gt;d</p>", "a<b&c>d");
        assert!(highlighted);
        assert!(
            out.contains(r#"<mark class="proton-search-highlight">a&lt;b&amp;c&gt;d</mark>"#),
            "{out}"
        );
    }

    #[test]
    fn empty_query_is_a_no_op() {
        let (out, highlighted) = highlight("<p>hello</p>", "");
        assert!(!highlighted);
        assert!(!out.contains("proton-search-highlight"));
    }

    /// Collects the terms into a sorted `Vec` so the assertions read in a stable order (a
    /// `BTreeSet` already iterates sorted, this just makes the literals easy to compare).
    fn terms(query: &str) -> Vec<String> {
        sanitize_terms(query).into_iter().collect()
    }

    #[test]
    fn drops_short_terms_and_boolean_operators() {
        assert_eq!(terms("a or to"), vec!["to"]);
        assert!(terms("AND not OR").is_empty());
    }

    #[test]
    fn deduplicates_terms_case_insensitively() {
        assert_eq!(terms("foo Foo FOO bar"), vec!["bar", "foo"]);
    }

    #[test]
    fn caps_at_the_longest_terms() {
        // Distinct terms of increasing length: "xx", "xxx", … so exactly MAX_TERMS survive.
        let query = (0..20)
            .map(|i| "x".repeat(i + 2))
            .collect::<Vec<_>>()
            .join(" ");
        let terms = sanitize_terms(&query);
        assert_eq!(terms.len(), MAX_TERMS);
        // The longest term is retained, the shortest dropped.
        assert!(terms.contains(&"x".repeat(21)));
        assert!(!terms.contains("xx"));
    }

    #[test]
    fn transformer_injects_css_only_when_something_matched() {
        use crate::Transformer;

        let mut matched = Transformer::new("<html><head></head><body><p>find me</p></body></html>");
        matched.highlight_search_terms("find");
        let matched = matched.to_string();
        assert!(
            matched.contains(r#"<mark class="proton-search-highlight">find</mark>"#),
            "{matched}"
        );
        // The stylesheet (not just the class on the mark) must be present.
        assert!(matched.contains("background-color"), "{matched}");

        let mut unmatched =
            Transformer::new("<html><head></head><body><p>nothing here</p></body></html>");
        unmatched.highlight_search_terms("absent");
        let unmatched = unmatched.to_string();
        // No match → neither marks nor the stylesheet are injected.
        assert!(
            !unmatched.contains("proton-search-highlight"),
            "{unmatched}"
        );
        assert!(!unmatched.contains("background-color"), "{unmatched}");
    }
}
