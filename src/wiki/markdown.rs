//! Common syntax and source ranges used by analysis and document edits.
use super::{frontmatter::parse_front_matter, links::split_wiki_link_parts, LinkKind};
use pulldown_cmark::{BrokenLink, Event, LinkType, Options, Parser, Tag};
use std::ops::Range;

#[derive(Clone, Debug)]
pub(crate) struct SyntaxLink {
    pub(crate) kind: LinkKind,
    pub(crate) target: String,
    pub(crate) span: Range<usize>,
    pub(crate) destination: Range<usize>,
    pub(crate) inline: bool,
    pub(crate) title: String,
}
#[derive(Clone, Debug)]
pub(crate) struct BrokenReference {
    pub(crate) span: Range<usize>,
    pub(crate) label: String,
}
#[derive(Clone, Debug)]
pub(crate) struct LocalResource {
    pub(crate) target: String,
    pub(crate) span: Range<usize>,
    pub(crate) code_checked: bool,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct Markdown {
    pub(crate) links: Vec<SyntaxLink>,
    pub(crate) verbatim: Vec<Range<usize>>,
    pub(crate) image_destinations: Vec<Range<usize>>,
    pub(crate) broken_references: Vec<BrokenReference>,
    pub(crate) local_resources: Vec<LocalResource>,
    pub(crate) first_paragraph: bool,
    pub(crate) first_offset: usize,
}
impl Markdown {
    pub(crate) fn parse(content: &str) -> Self {
        let fm = parse_front_matter(content);
        let mut parsed = Self::default();
        if fm.present {
            parsed.segment(&content[..fm.body_offset], 0, false);
        }
        parsed.segment(&content[fm.body_offset..], fm.body_offset, true);
        parsed.links.sort_by_key(|link| link.span.start);
        parsed
    }
    fn segment(&mut self, text: &str, offset: usize, body: bool) {
        let options = Options::ENABLE_WIKILINKS
            | Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS;
        let mut broken_references = Vec::new();
        let mut callback = |link: BrokenLink<'_>| {
            if body && matches!(link.link_type, LinkType::Reference | LinkType::Collapsed) {
                broken_references.push(BrokenReference {
                    span: shift(link.span, offset),
                    label: link.reference.to_string(),
                });
            }
            None
        };
        let parser = Parser::new_with_broken_link_callback(text, options, Some(&mut callback));
        let mut events = parser.into_offset_iter();
        if body {
            self.first_offset = offset;
        }
        let mut first = !body;
        let mut images = 0;
        while let Some((event, span)) = events.next() {
            if !first {
                let comment = matches!(event, Event::Html(_) | Event::Start(Tag::HtmlBlock))
                    && comments_only(&text[span.clone()]);
                if !comment && !matches!(event, Event::End(_)) {
                    self.first_paragraph = matches!(event, Event::Start(Tag::Paragraph));
                    self.first_offset = offset + span.start;
                    first = true;
                }
            }
            match event {
                Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock) => {
                    self.verbatim.push(shift(span, offset))
                }
                Event::Start(Tag::Image {
                    link_type,
                    dest_url,
                    id,
                    ..
                }) => {
                    images += 1;
                    if body && !matches!(link_type, LinkType::WikiLink { .. }) {
                        self.local_resources.push(LocalResource {
                            target: dest_url.to_string(),
                            span: shift(span.clone(), offset),
                            code_checked: false,
                        });
                    }
                    if let Some(range) = destination(
                        text,
                        &span,
                        link_type,
                        events
                            .reference_definitions()
                            .get(&id)
                            .map(|def| def.span.clone()),
                    ) {
                        self.image_destinations.push(shift(range, offset));
                    }
                }
                Event::End(pulldown_cmark::TagEnd::Image) => images -= 1,
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    id,
                    title,
                }) if images == 0 => {
                    let kind = if matches!(link_type, LinkType::WikiLink { .. }) {
                        LinkKind::Wiki
                    } else {
                        LinkKind::Markdown
                    };
                    if body && kind == LinkKind::Markdown && link_type != LinkType::Email {
                        self.local_resources.push(LocalResource {
                            target: dest_url.to_string(),
                            span: shift(span.clone(), offset),
                            code_checked: true,
                        });
                    }
                    let Some(range) = destination(
                        text,
                        &span,
                        link_type,
                        events
                            .reference_definitions()
                            .get(&id)
                            .map(|def| def.span.clone()),
                    ) else {
                        continue;
                    };
                    let target = if kind == LinkKind::Wiki {
                        text[range.clone()].trim().to_string()
                    } else {
                        dest_url.to_string()
                    };
                    self.links.push(SyntaxLink {
                        kind,
                        target,
                        span: shift(span, offset),
                        destination: shift(range, offset),
                        inline: link_type == LinkType::Inline,
                        title: title.to_string(),
                    });
                }
                _ => {}
            }
        }
        drop(events);
        self.broken_references.extend(broken_references);
    }
}
fn shift(range: Range<usize>, offset: usize) -> Range<usize> {
    range.start + offset..range.end + offset
}
fn comments_only(mut text: &str) -> bool {
    while let Some(rest) = text.trim_start().strip_prefix("<!--") {
        let Some(end) = rest.find("-->") else {
            return true;
        };
        text = &rest[end + 3..];
    }
    text.trim().is_empty()
}
fn destination(
    text: &str,
    span: &Range<usize>,
    kind: LinkType,
    definition: Option<Range<usize>>,
) -> Option<Range<usize>> {
    if matches!(kind, LinkType::WikiLink { .. }) {
        let inner = &text[span.start + 2..span.end - 2];
        let (target, ..) = split_wiki_link_parts(inner);
        return Some(span.start + 2..span.start + 2 + target.len());
    }
    if kind == LinkType::Inline {
        let raw = &text[span.clone()];
        let label = label_end(raw)?;
        return target_range(text, span.start + label + 2);
    }
    let range = definition?;
    let label = label_end(&text[range.clone()])?;
    target_range(text, range.start + label + 2)
}
/// Closing bracket of a label, ignoring escaped brackets and inline code.
pub(crate) fn label_end(raw: &str) -> Option<usize> {
    let masked = crate::text::mask_inline_code(raw);
    let bytes = masked.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'[' => depth += 1,
            b']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}
fn target_range(text: &str, mut start: usize) -> Option<Range<usize>> {
    let bytes = text.as_bytes();
    while bytes.get(start).is_some_and(u8::is_ascii_whitespace) {
        start += 1;
    }
    if bytes.get(start) == Some(&b'<') {
        start += 1;
        let mut end = start;
        while end < bytes.len() {
            if bytes[end] == b'\\' {
                end += 2;
                continue;
            }
            if bytes[end] == b'>' {
                return Some(start..end);
            }
            end += 1;
        }
        return None;
    }
    let mut end = start;
    let mut depth = 0usize;
    while end < bytes.len() {
        match bytes[end] {
            b'\\' => {
                end += 2;
                continue;
            }
            b'(' => depth += 1,
            b')' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            c if c.is_ascii_whitespace() => break,
            _ => {}
        }
        end += 1;
    }
    Some(start..end)
}

pub(crate) fn local_target(target: &str) -> bool {
    let path = target.split('#').next().unwrap_or("");
    !target.is_empty()
        && !target.starts_with(['#'])
        && !target.starts_with("//")
        && !path.split('/').next().unwrap_or("").contains(':')
}

pub(crate) fn replace_ranges(content: &str, mut edits: Vec<(Range<usize>, String)>) -> String {
    edits.sort_by_key(|(range, _)| (range.start, range.end));
    edits.dedup_by(|a, b| a.0 == b.0);
    let mut out = String::new();
    let mut end = 0;
    for (range, value) in edits {
        assert!(range.start >= end, "overlapping document edits");
        out.push_str(&content[end..range.start]);
        out.push_str(&value);
        end = range.end;
    }
    out.push_str(&content[end..]);
    out
}

pub(crate) fn write_destination(content: &str, range: &Range<usize>, value: &str) -> String {
    if range.start > 0 && content.as_bytes()[range.start - 1] == b'<' {
        return value.to_string();
    }
    if value.contains([' ', '\t', '\n', '(', ')']) {
        format!("<{value}>")
    } else {
        value.to_string()
    }
}
