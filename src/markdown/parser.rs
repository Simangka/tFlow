use std::sync::OnceLock;

use pulldown_cmark::{Parser, Event, Tag, TagEnd, HeadingLevel, CodeBlockKind, CowStr};

use super::prepare_md;

#[derive(Debug, Clone)]
pub struct MarkdownEvent {
    pub event: Event<'static>,
    pub start_offset: usize,
    pub end_offset: usize,
}

#[derive(Debug, Clone)]
pub struct MarkdownHeading {
    pub level: HeadingLevel,
    pub text: String,
    pub line: usize,
    pub offset: usize,
}

#[derive(Debug, Clone)]
pub struct ParsedMarkdown {
    pub events: Vec<MarkdownEvent>,
    pub headings: Vec<MarkdownHeading>,
}

pub struct MarkdownParser;

impl MarkdownParser {
    pub fn new() -> Self {
        MarkdownParser
    }

    pub fn parse(&self, text: &str) -> ParsedMarkdown {
        let prepared = prepare_md(text);
        let text: &str = &prepared;
        let mut events = Vec::new();
        let mut headings = Vec::new();
        let mut current_heading_text = String::new();
        let mut current_heading_level = HeadingLevel::H1;
        let mut in_heading = false;
        let mut heading_start_offset = 0;

        let mut opts = pulldown_cmark::Options::empty();
        opts.insert(pulldown_cmark::Options::ENABLE_TABLES);
        opts.insert(pulldown_cmark::Options::ENABLE_FOOTNOTES);
        opts.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(text, opts);

        for (event, range) in parser.into_offset_iter() {
            let start_offset = range.start;
            let end_offset = range.end;

            match &event {
                Event::Start(tag) => {
                    if let Tag::Heading { level, .. } = tag {
                        in_heading = true;
                        current_heading_level = *level;
                        current_heading_text.clear();
                        heading_start_offset = start_offset;
                    }
                }
                Event::End(tag_end) => {
                    if let TagEnd::Heading(_) = tag_end {
                        if in_heading {
                            let line = text[..start_offset].matches('\n').count();
                            headings.push(MarkdownHeading {
                                level: current_heading_level,
                                text: current_heading_text.clone(),
                                line,
                                offset: heading_start_offset,
                            });
                            in_heading = false;
                        }
                    }
                }
                Event::Text(t) => {
                    if in_heading {
                        current_heading_text.push_str(&t);
                    }
                }
                _ => {}
            }

            let owned_event = match event {
                Event::Start(tag) => Event::Start(tag_to_owned(tag)),
                Event::End(tag_end) => Event::End(tag_end_to_owned(tag_end)),
                Event::Text(t) => Event::Text(CowStr::Boxed(t.to_string().into())),
                Event::Code(t) => Event::Code(CowStr::Boxed(t.to_string().into())),
                Event::Html(t) => Event::Html(CowStr::Boxed(t.to_string().into())),
                Event::InlineHtml(t) => Event::InlineHtml(CowStr::Boxed(t.to_string().into())),
                Event::FootnoteReference(t) => Event::FootnoteReference(CowStr::Boxed(t.to_string().into())),
                Event::SoftBreak => Event::SoftBreak,
                Event::HardBreak => Event::HardBreak,
                Event::Rule => Event::Rule,
                Event::TaskListMarker(v) => Event::TaskListMarker(v),
                Event::InlineMath(t) => Event::InlineMath(CowStr::Boxed(t.to_string().into())),
                Event::DisplayMath(t) => Event::DisplayMath(CowStr::Boxed(t.to_string().into())),
            };

            events.push(MarkdownEvent {
                event: owned_event,
                start_offset,
                end_offset,
            });
        }

        ParsedMarkdown { events, headings }
    }

    pub fn extract_headings(text: &str) -> Vec<MarkdownHeading> {
        let prepared = prepare_md(text);
        let text: &str = &prepared;
        let parser = MarkdownParser::new();
        let parsed = parser.parse(text);
        parsed.headings
    }

    pub fn extract_tables(text: &str) -> Vec<Vec<Vec<String>>> {
        let prepared = prepare_md(text);
        let text: &str = &prepared;
        let mut tables = Vec::new();
        let mut current_table: Vec<Vec<String>> = Vec::new();
        let mut in_table = false;

        for line in text.lines() {
            if line.trim_start().starts_with('|') {
                let cells = split_table_row(line);
                if !in_table {
                    in_table = true;
                    current_table = Vec::new();
                }
                if !cells.is_empty() && cells.iter().any(|c| !c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')) {
                    current_table.push(cells);
                }
            } else if in_table {
                if !current_table.is_empty() {
                    tables.push(current_table.clone());
                }
                in_table = false;
                current_table = Vec::new();
            }
        }

        if in_table && !current_table.is_empty() {
            tables.push(current_table);
        }

        tables
    }

    pub fn extract_code_blocks(text: &str) -> Vec<(Option<String>, String)> {
        let prepared = prepare_md(text);
        let text: &str = &prepared;
        let mut blocks = Vec::new();
        let mut in_block = false;
        let mut lang = None;
        let mut code = String::new();

        for line in text.lines() {
            if line.starts_with("```") {
                if in_block {
                    blocks.push((lang.take(), code.clone()));
                    code.clear();
                    in_block = false;
                    lang = None;
                } else {
                    in_block = true;
                    let rest = line[3..].trim();
                    if !rest.is_empty() {
                        lang = Some(rest.to_string());
                    }
                }
            } else if in_block {
                code.push_str(line);
                code.push('\n');
            }
        }

        if in_block && !code.is_empty() {
            blocks.push((lang, code));
        }

        blocks
    }

    pub fn extract_checkboxes(text: &str) -> Vec<(usize, bool, String)> {
        let prepared = prepare_md(text);
        let text: &str = &prepared;
        let mut checkboxes = Vec::new();
        let re = checkbox_regex();

        for (line_idx, line) in text.lines().enumerate() {
            if let Some(caps) = re.captures(line) {
                let checked = match caps.get(1).map(|m| m.as_str()) {
                    Some("x" | "X") => true,
                    _ => false,
                };
                let label = caps.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
                checkboxes.push((line_idx, checked, label));
            }
        }

        checkboxes
    }
}

fn checkbox_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^\s*[-*+]\s+\[([ xX])\]\s+(.*)").expect("invalid regex"))
}

fn split_table_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    if chars.peek() == Some(&'|') {
        chars.next();
    }
    loop {
        let Some(c) = chars.next() else { break };
        if c == '\\' {
            if chars.peek() == Some(&'|') {
                current.push('|');
                chars.next();
            } else {
                current.push('\\');
            }
        } else if c == '|' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                cells.push(trimmed);
            }
            current.clear();
        } else {
            current.push(c);
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        cells.push(trimmed);
    }
    cells
}

fn tag_to_owned(tag: Tag) -> Tag<'static> {
    match tag {
        Tag::Paragraph => Tag::Paragraph,
        Tag::Heading { level, id, classes, attrs } => {
            let owned_attrs: Vec<(CowStr<'static>, Option<CowStr<'static>>)> = attrs
                .into_iter()
                .map(|(k, v)| {
                    let k: CowStr<'static> = CowStr::Boxed(k.to_string().into());
                    let v: Option<CowStr<'static>> = v.map(|s| CowStr::Boxed(s.to_string().into()));
                    (k, v)
                })
                .collect();
            Tag::Heading {
                level,
                id: id.map(|s| CowStr::Boxed(s.to_string().into())),
                classes: classes.into_iter().map(|s| CowStr::Boxed(s.to_string().into())).collect(),
                attrs: owned_attrs,
            }
        }
        Tag::BlockQuote(kind) => Tag::BlockQuote(kind),
        Tag::CodeBlock(kind) => {
            let kind = match kind {
                CodeBlockKind::Indented => CodeBlockKind::Indented,
                CodeBlockKind::Fenced(lang) => CodeBlockKind::Fenced(CowStr::Boxed(lang.to_string().into())),
            };
            Tag::CodeBlock(kind)
        }
        Tag::HtmlBlock => {
            tracing::warn!("markdown: unknown tag {:?}", Tag::HtmlBlock);
            Tag::Paragraph
        }
        Tag::List(ord) => Tag::List(ord),
        Tag::Item => Tag::Item,
        Tag::FootnoteDefinition(name) => Tag::FootnoteDefinition(CowStr::Boxed(name.to_string().into())),
        Tag::DefinitionList => {
            tracing::warn!("markdown: unknown tag {:?}", Tag::DefinitionList);
            Tag::Paragraph
        }
        Tag::DefinitionListTitle => {
            tracing::warn!("markdown: unknown tag {:?}", Tag::DefinitionListTitle);
            Tag::Paragraph
        }
        Tag::DefinitionListDefinition => {
            tracing::warn!("markdown: unknown tag {:?}", Tag::DefinitionListDefinition);
            Tag::Paragraph
        }
        Tag::Table(alignments) => Tag::Table(alignments),
        Tag::TableHead => {
            tracing::warn!("markdown: unknown tag {:?}", Tag::TableHead);
            Tag::Paragraph
        }
        Tag::TableRow => Tag::TableRow,
        Tag::TableCell => Tag::TableCell,
        Tag::Emphasis => Tag::Emphasis,
        Tag::Strong => Tag::Strong,
        Tag::Strikethrough => Tag::Strikethrough,
        Tag::Link { link_type, dest_url, title, id } => Tag::Link {
            link_type,
            dest_url: CowStr::Boxed(dest_url.to_string().into()),
            title: CowStr::Boxed(title.to_string().into()),
            id: CowStr::Boxed(id.to_string().into()),
        },
        Tag::Image { link_type, dest_url, title, id } => Tag::Image {
            link_type,
            dest_url: CowStr::Boxed(dest_url.to_string().into()),
            title: CowStr::Boxed(title.to_string().into()),
            id: CowStr::Boxed(id.to_string().into()),
        },
        Tag::MetadataBlock(kind) => Tag::MetadataBlock(kind),
        #[allow(unreachable_patterns)]
        _ => {
            tracing::warn!("markdown: unknown tag {:?}", tag);
            Tag::Paragraph
        }
    }
}

fn tag_end_to_owned(tag_end: TagEnd) -> TagEnd {
    match tag_end {
        TagEnd::Paragraph => TagEnd::Paragraph,
        TagEnd::Heading(level) => TagEnd::Heading(level),
        TagEnd::BlockQuote(kind) => TagEnd::BlockQuote(kind),
        TagEnd::CodeBlock => TagEnd::CodeBlock,
        TagEnd::HtmlBlock => {
            tracing::warn!("markdown: unknown tag end {:?}", TagEnd::HtmlBlock);
            TagEnd::Paragraph
        }
        TagEnd::List(ord) => TagEnd::List(ord),
        TagEnd::Item => TagEnd::Item,
        TagEnd::FootnoteDefinition => TagEnd::FootnoteDefinition,
        TagEnd::DefinitionList => {
            tracing::warn!("markdown: unknown tag end {:?}", TagEnd::DefinitionList);
            TagEnd::Paragraph
        }
        TagEnd::DefinitionListTitle => {
            tracing::warn!("markdown: unknown tag end {:?}", TagEnd::DefinitionListTitle);
            TagEnd::Paragraph
        }
        TagEnd::DefinitionListDefinition => {
            tracing::warn!("markdown: unknown tag end {:?}", TagEnd::DefinitionListDefinition);
            TagEnd::Paragraph
        }
        TagEnd::Table => TagEnd::Table,
        TagEnd::TableHead => {
            tracing::warn!("markdown: unknown tag end {:?}", TagEnd::TableHead);
            TagEnd::Paragraph
        }
        TagEnd::TableRow => TagEnd::TableRow,
        TagEnd::TableCell => TagEnd::TableCell,
        TagEnd::Emphasis => TagEnd::Emphasis,
        TagEnd::Strong => TagEnd::Strong,
        TagEnd::Strikethrough => TagEnd::Strikethrough,
        TagEnd::Link => TagEnd::Link,
        TagEnd::Image => TagEnd::Image,
        TagEnd::MetadataBlock(kind) => TagEnd::MetadataBlock(kind),
        #[allow(unreachable_patterns)]
        _ => {
            tracing::warn!("markdown: unknown tag end {:?}", tag_end);
            TagEnd::Paragraph
        }
    }
}
