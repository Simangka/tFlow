use ratatui::{
    Frame,
    layout::Rect,
    widgets::*,
    style::*,
    text::Line as TextLine,
    text::Span,
};
use pulldown_cmark::{Parser, Event, Tag, TagEnd, HeadingLevel, CodeBlockKind};

pub struct MarkdownView {
    pub rendered: Vec<MarkdownLine>,
    pub scroll_offset: usize,
    pub viewport_height: usize,
}

pub enum MarkdownLine {
    Heading(String, usize, Style),
    Paragraph(Vec<(String, Style)>),
    CodeBlock(String, Style, Style),
    BlockQuote(Vec<MarkdownLine>, Style),
    ListItem(Vec<MarkdownLine>, usize, Style),
    Table(Vec<MarkdownLine>, Vec<Vec<MarkdownLine>>, Vec<ratatui::layout::Alignment>),
    TableHeader(Vec<MarkdownLine>),
    HorizontalRule(Style),
    Checkbox(bool, String, Style),
    Empty,
}

struct MdStyles {
    h1: Style,
    h2: Style,
    h3: Style,
    h4: Style,
    code: Style,
    code_bg: Style,
    quote: Style,
    list: Style,
    link: Style,
    bold: Style,
    italic: Style,
    hr: Style,
    checkbox: Style,
    checked: Style,
    fg: Style,
    border: Style,
    bg: Color,
}

impl Default for MdStyles {
    fn default() -> Self {
        let green = Color::Rgb(0, 255, 0);
        let bright_green = Color::Rgb(100, 255, 100);
        let dark_green = Color::Rgb(0, 100, 0);
        let yellow = Color::Rgb(200, 200, 0);
        let white = Color::Rgb(200, 200, 200);
        let dim_white = Color::Rgb(150, 150, 150);
        let cyan = Color::Rgb(0, 255, 255);
        MdStyles {
            h1: Style::default().fg(bright_green).add_modifier(Modifier::BOLD),
            h2: Style::default().fg(green).add_modifier(Modifier::BOLD),
            h3: Style::default().fg(green),
            h4: Style::default().fg(dim_white),
            code: Style::default().fg(yellow),
            code_bg: Style::default().bg(dark_green),
            quote: Style::default().fg(dim_white),
            list: Style::default().fg(green),
            link: Style::default().fg(cyan).add_modifier(Modifier::UNDERLINED),
            bold: Style::default().fg(white).add_modifier(Modifier::BOLD),
            italic: Style::default().fg(white).add_modifier(Modifier::ITALIC),
            hr: Style::default().fg(dark_green),
            checkbox: Style::default().fg(dim_white),
            checked: Style::default().fg(bright_green),
            fg: Style::default().fg(white),
            border: Style::default().fg(dark_green),
            bg: Color::Rgb(0, 20, 0),
        }
    }
}

enum ParseState {
    Root,
    #[allow(dead_code)]
    InParagraph(Vec<(String, Style)>),
    InBlockQuote(Vec<(String, Style)>),
    InListItem(usize, Vec<MarkdownLine>),
    InCodeBlock(String, Style, Style),
    InTable(Vec<Vec<MarkdownLine>>, Vec<ratatui::layout::Alignment>, Vec<MarkdownLine>),
    InTableRow(Vec<MarkdownLine>),
    InTableHeader(Vec<MarkdownLine>),
    InHeading(usize, Style, Vec<(String, Style)>),
    InImage(String, String, String),
}

fn topmost_state_is_root_or_paragraph(stack: &[ParseState]) -> bool {
    match stack.last() {
        Some(ParseState::Root) | Some(ParseState::InParagraph(_)) => true,
        Some(_) => false,
        None => true,
    }
}

const MAX_RENDERED_LINES: usize = 50_000;

impl MarkdownView {
    pub fn new() -> Self {
        Self {
            rendered: Vec::new(),
            scroll_offset: 0,
            viewport_height: 0,
        }
    }

    pub fn render_markdown(&mut self, markdown_text: &str) {
        self.rendered.clear();
        let parser = Parser::new(markdown_text);

        let s = MdStyles::default();

        let mut state_stack: Vec<ParseState> = vec![ParseState::Root];
        let mut current_text: Vec<(String, Style)> = Vec::new();
        let mut current_style_stack: Vec<Style> = vec![s.fg];
        let mut list_item_index: usize = 0;

        for event in parser {
            if self.rendered.len() >= MAX_RENDERED_LINES {
                break;
            }
            match event {
                Event::Start(tag) => {
                    match tag {
                        Tag::Paragraph => {
                            current_style_stack.push(s.fg);
                        }
                        Tag::Heading { level, .. } => {
                            let style = match level {
                                HeadingLevel::H1 => s.h1,
                                HeadingLevel::H2 => s.h2,
                                HeadingLevel::H3 => s.h3,
                                HeadingLevel::H4 | _ => s.h4,
                            };
                            let size = match level {
                                HeadingLevel::H1 => 1,
                                HeadingLevel::H2 => 2,
                                HeadingLevel::H3 => 3,
                                HeadingLevel::H4 | _ => 4,
                            };
                            state_stack.push(ParseState::InHeading(size, style, Vec::new()));
                        }
                        Tag::BlockQuote(_) => {
                            state_stack.push(ParseState::InBlockQuote(Vec::new()));
                        }
                        Tag::CodeBlock(kind) => {
                            let _lang = match &kind {
                                CodeBlockKind::Fenced(lang) => lang.to_string(),
                                CodeBlockKind::Indented => String::new(),
                            };
                            state_stack.push(ParseState::InCodeBlock(
                                String::new(),
                                s.code,
                                s.code_bg,
                            ));
                        }
                        Tag::List(start) => {
                            list_item_index = start.unwrap_or(1) as usize;
                        }
                        Tag::Item => {
                            state_stack.push(ParseState::InListItem(list_item_index, Vec::new()));
                        }
                        Tag::Table(alignments) => {
                            let al = alignments.iter().map(|a| match a {
                                pulldown_cmark::Alignment::Left => ratatui::layout::Alignment::Left,
                                pulldown_cmark::Alignment::Right => ratatui::layout::Alignment::Right,
                                pulldown_cmark::Alignment::Center => ratatui::layout::Alignment::Center,
                                pulldown_cmark::Alignment::None => ratatui::layout::Alignment::Left,
                            }).collect();
                            state_stack.push(ParseState::InTable(Vec::new(), al, Vec::new()));
                        }
                        Tag::TableHead => {
                            state_stack.push(ParseState::InTableHeader(Vec::new()));
                        }
                        Tag::TableRow => {
                            state_stack.push(ParseState::InTableRow(Vec::new()));
                        }
                        Tag::TableCell => {
                            current_text.clear();
                        }
                        Tag::Emphasis => {
                            current_style_stack.push(s.italic);
                        }
                        Tag::Strong => {
                            current_style_stack.push(s.bold);
                        }
                        Tag::Strikethrough => {
                            current_style_stack.push(
                                Style::default().add_modifier(Modifier::CROSSED_OUT),
                            );
                        }
                        Tag::Link { dest_url, title, .. } => {
                            current_style_stack.push(s.link);
                            if topmost_state_is_root_or_paragraph(&state_stack) {
                                let label = if !title.is_empty() {
                                    title.to_string()
                                } else {
                                    dest_url.to_string()
                                };
                                current_text.push((format!("[{}]({})", label, dest_url), s.link));
                            }
                        }
                        Tag::Image { dest_url, title, .. } => {
                            state_stack.push(ParseState::InImage(
                                dest_url.to_string(),
                                title.to_string(),
                                String::new(),
                            ));
                        }
                        _ => {}
                    }
                }
                Event::End(tag_end) => {
                    match tag_end {
                        TagEnd::Paragraph => {
                            if !current_text.is_empty() {
                                let text = std::mem::take(&mut current_text);
                                self.rendered.push(MarkdownLine::Paragraph(text));
                            }
                            current_style_stack.pop();
                        }
                        TagEnd::Heading(_level) => {
                            if let Some(ParseState::InHeading(size, style, ref mut segments)) = state_stack.last_mut() {
                                if !segments.is_empty() || !current_text.is_empty() {
                                    segments.extend(std::mem::take(&mut current_text));
                                }
                                let mut text = String::new();
                                for (s_text, _) in segments.iter() {
                                    text.push_str(s_text);
                                }
                                self.rendered.push(MarkdownLine::Heading(text, *size, *style));
                            }
                            state_stack.pop();
                        }
                        TagEnd::BlockQuote(_) => {
                            if let Some(ParseState::InBlockQuote(ref mut segments)) = state_stack.last_mut() {
                                if !current_text.is_empty() {
                                    segments.extend(std::mem::take(&mut current_text));
                                }
                                let mut children = Vec::new();
                                if !segments.is_empty() {
                                    children.push(MarkdownLine::Paragraph(segments.clone()));
                                }
                                state_stack.pop();
                                self.rendered.push(MarkdownLine::BlockQuote(children, s.quote));
                            } else {
                                while let Some(ParseState::InBlockQuote(_)) = state_stack.last() {
                                    state_stack.pop();
                                }
                                self.rendered.push(MarkdownLine::BlockQuote(Vec::new(), s.quote));
                            }
                        }
                        TagEnd::CodeBlock => {
                            if let Some(ParseState::InCodeBlock(code, cs, cbs)) = state_stack.pop() {
                                self.rendered.push(MarkdownLine::CodeBlock(code, cs, cbs));
                            }
                        }
                        TagEnd::List(_) => {
                            list_item_index = 0;
                        }
                        TagEnd::Item => {
                            let pop_now = matches!(state_stack.last(), Some(ParseState::InListItem(_, _)));
                            if pop_now {
                                if let Some(ParseState::InListItem(idx, ref mut children)) = state_stack.last_mut() {
                                    if !current_text.is_empty() {
                                        let text = std::mem::take(&mut current_text);
                                        children.push(MarkdownLine::Paragraph(text));
                                    }
                                    let mut children_vec = Vec::new();
                                    std::mem::swap(&mut children_vec, children);
                                    self.rendered.push(MarkdownLine::ListItem(children_vec, *idx, s.list));
                                }
                                state_stack.pop();
                            }
                        }
                        TagEnd::Table => {
                            if let Some(ParseState::InTable(rows, aligns, header)) = state_stack.pop() {
                                if header.is_empty() && rows.is_empty() {
                                } else {
                                    self.rendered.push(MarkdownLine::Table(header, rows, aligns));
                                }
                            }
                        }
                        TagEnd::TableHead => {
                            if let Some(ParseState::InTableHeader(ref mut cells)) = state_stack.last_mut() {
                                if !current_text.is_empty() {
                                    let text = std::mem::take(&mut current_text);
                                    cells.push(MarkdownLine::Paragraph(text));
                                }
                                let header_lines: Vec<MarkdownLine> = cells.drain(..).collect();
                                if let Some(ParseState::InTable(_, _, ref mut hdr)) = state_stack.iter_mut().rev().find_map(|s| {
                                    if let ParseState::InTable(_, _, _) = s {
                                        Some(s)
                                    } else {
                                        None
                                    }
                                }) {
                                    *hdr = header_lines;
                                }
                            }
                            state_stack.pop();
                        }
                        TagEnd::TableRow => {
                            if let Some(ParseState::InTableRow(ref mut cells)) = state_stack.last_mut() {
                                if !current_text.is_empty() {
                                    let text = std::mem::take(&mut current_text);
                                    cells.push(MarkdownLine::Paragraph(text));
                                }
                                let mut row_cells = Vec::new();
                                std::mem::swap(&mut row_cells, cells);
                                if let Some(ParseState::InTable(ref mut rows, _, _)) = state_stack.last_mut() {
                                    rows.push(row_cells);
                                }
                            }
                            state_stack.pop();
                        }
                        TagEnd::TableCell => {
                            if !current_text.is_empty() {
                                let text = std::mem::take(&mut current_text);
                                let para = MarkdownLine::Paragraph(text);
                                for state in state_stack.iter_mut().rev() {
                                    match state {
                                        ParseState::InTableRow(ref mut cells) => {
                                            cells.push(para);
                                            break;
                                        }
                                        ParseState::InTableHeader(ref mut cells) => {
                                            cells.push(para);
                                            break;
                                        }
                                        _ => continue,
                                    }
                                }
                            }
                        }
                        TagEnd::Emphasis => {
                            current_style_stack.pop();
                        }
                        TagEnd::Strong => {
                            current_style_stack.pop();
                        }
                        TagEnd::Strikethrough => {
                            current_style_stack.pop();
                        }
                        TagEnd::Link => {
                            current_style_stack.pop();
                        }
                        TagEnd::Image => {
                            if let Some(ParseState::InImage(dest_url, title, mut alt)) = state_stack.pop() {
                                if !current_text.is_empty() {
                                    let text = std::mem::take(&mut current_text);
                                    for (seg, _) in text {
                                        alt.push_str(&seg);
                                    }
                                }
                                let label = if !alt.is_empty() {
                                    alt
                                } else if !title.is_empty() {
                                    title
                                } else {
                                    dest_url
                                };
                                self.rendered.push(MarkdownLine::Paragraph(vec![(
                                    format!("[image: {}]", label),
                                    Style::default().fg(Color::Rgb(150, 150, 150)).add_modifier(Modifier::ITALIC),
                                )]));
                            }
                        }
                        _ => {}
                    }
                }
                Event::Text(text) => {
                    let style = current_style_stack.last().copied().unwrap_or(s.fg);
                    let mut handled = false;
                    for state in state_stack.iter_mut().rev() {
                        match state {
                            ParseState::InHeading(_, _, ref mut segments) => {
                                segments.push((text.to_string(), style));
                                handled = true;
                                break;
                            }
                            ParseState::InCodeBlock(ref mut code, _, _) => {
                                code.push_str(&text);
                                handled = true;
                                break;
                            }
                            ParseState::InParagraph(ref mut segs) => {
                                segs.push((text.to_string(), style));
                                handled = true;
                                break;
                            }
                            ParseState::InImage(_, _, ref mut alt) => {
                                alt.push_str(&text);
                                handled = true;
                                break;
                            }
                            _ => {
                                continue;
                            }
                        }
                    }
                    if !handled {
                        current_text.push((text.to_string(), style));
                    }
                }
                Event::Code(text) => {
                    let style = Style::default().fg(Color::Rgb(200, 200, 0)).bg(Color::Rgb(0, 100, 0));
                    match state_stack.last_mut() {
                        Some(ParseState::InHeading(_, _, ref mut segments)) => {
                            segments.push((format!("`{}`", text), style));
                        }
                        _ => {
                            current_text.push((format!("`{}`", text), style));
                        }
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    current_text.push((" ".to_string(), s.fg));
                }
                Event::Rule => {
                    self.rendered.push(MarkdownLine::HorizontalRule(s.hr));
                }
                Event::TaskListMarker(checked) => {
                    current_text.push((
                        if checked { "[x] ".to_string() } else { "[ ] ".to_string() },
                        if checked { s.checked } else { s.checkbox },
                    ));
                }
                _ => {}
            }
        }

        if !current_text.is_empty() {
            self.rendered.push(MarkdownLine::Paragraph(current_text));
        }
    }

    pub fn render_view(&self, frame: &mut Frame, area: Rect) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let s = MdStyles::default();

        let block = Block::default()
            .title(" Preview ")
            .borders(Borders::ALL)
            .border_style(s.border)
            .style(Style::default().bg(s.bg));

        let inner = block.inner(area);

        let mut lines: Vec<TextLine<'static>> = Vec::new();
        let viewport_end = self.scroll_offset + inner.height as usize;

        for i in self.scroll_offset..viewport_end.min(self.rendered.len()) {
            let md_line = &self.rendered[i];
            match md_line {
                MarkdownLine::Heading(text, level, style) => {
                    let prefix = "#".repeat(*level);
                    let display = format!("{} {}", prefix, text);
                    let line = TextLine::from(vec![Span::styled(display, *style)]);
                    lines.push(line);
                }
                MarkdownLine::Paragraph(segments) => {
                    let spans: Vec<Span> = segments
                        .iter()
                        .map(|(text, style)| Span::styled(text.clone(), *style))
                        .collect();
                    lines.push(TextLine::from(spans));
                }
                MarkdownLine::CodeBlock(code, text_style, bg_style) => {
                    for line_text in code.lines() {
                        let span = Span::styled(
                            format!(" {}", line_text),
                            text_style.add_modifier(Modifier::DIM),
                        );
                        lines.push(TextLine::from(vec![span]).style(*bg_style));
                    }
                }
                MarkdownLine::BlockQuote(children, style) => {
                    for child in children {
                        if let MarkdownLine::Paragraph(segments) = child {
                            let spans: Vec<Span> = std::iter::once(
                                Span::styled("| ", *style),
                            )
                            .chain(
                                segments
                                    .iter()
                                    .map(|(text, s_text)| Span::styled(text.clone(), *s_text)),
                            )
                            .collect();
                            lines.push(TextLine::from(spans));
                        }
                    }
                }
                MarkdownLine::ListItem(children, idx, style) => {
                    let marker = format!("{}. ", idx);
                    for child in children {
                        if let MarkdownLine::Paragraph(segments) = child {
                            let spans: Vec<Span> = std::iter::once(
                                Span::styled(marker.clone(), *style),
                            )
                            .chain(
                                segments
                                    .iter()
                                    .map(|(text, s_text)| Span::styled(text.clone(), *s_text)),
                            )
                            .collect();
                            lines.push(TextLine::from(spans));
                        } else if let MarkdownLine::Checkbox(checked, text, _) = child {
                            let check = if *checked { "[x]" } else { "[ ]" };
                            let span = Span::styled(
                                format!("{} {} {}", marker, check, text),
                                *style,
                            );
                            lines.push(TextLine::from(vec![span]));
                        }
                    }
                }
                MarkdownLine::Table(header, rows, _alignments) => {
                    let render_row = |row: &[MarkdownLine], row_spans: &mut Vec<Span>| {
                        for cell in row {
                            if let MarkdownLine::Paragraph(segments) = cell {
                                for (text, s_text) in segments {
                                    row_spans.push(Span::styled(
                                        format!(" {} ", text),
                                        *s_text,
                                    ));
                                }
                                row_spans.push(Span::styled(
                                    "|",
                                    Style::default().fg(Color::Rgb(0, 100, 0)),
                                ));
                            }
                        }
                    };
                    let mut header_spans: Vec<Span> = Vec::new();
                    render_row(header, &mut header_spans);
                    if !header_spans.is_empty() {
                        lines.push(TextLine::from(header_spans));
                    }
                    for row in rows {
                        let mut row_spans: Vec<Span> = Vec::new();
                        render_row(row, &mut row_spans);
                        if !row_spans.is_empty() {
                            lines.push(TextLine::from(row_spans));
                        }
                    }
                }
                MarkdownLine::TableHeader(cells) => {
                    let mut header_spans: Vec<Span> = Vec::new();
                    for cell in cells {
                        if let MarkdownLine::Paragraph(segments) = cell {
                            for (text, s_text) in segments {
                                header_spans.push(Span::styled(
                                    format!(" {} ", text),
                                    s_text.patch(Modifier::BOLD),
                                ));
                            }
                            header_spans.push(Span::styled(
                                "|",
                                Style::default().fg(Color::Rgb(0, 100, 0)),
                            ));
                        }
                    }
                    if !header_spans.is_empty() {
                        lines.push(TextLine::from(header_spans));
                    }
                }
                MarkdownLine::HorizontalRule(style) => {
                    let rule = "~".repeat(inner.width.saturating_sub(2) as usize);
                    lines.push(TextLine::from(vec![Span::styled(rule, *style)]));
                }
                MarkdownLine::Checkbox(checked, text, _style) => {
                    let check = if *checked { "[x]" } else { "[ ]" };
                    let span = Span::styled(
                        format!("{} {}", check, text),
                        if *checked {
                            Style::default().fg(Color::Rgb(100, 255, 100))
                        } else {
                            Style::default().fg(Color::Rgb(150, 150, 150))
                        },
                    );
                    lines.push(TextLine::from(vec![span]));
                }
                MarkdownLine::Empty => {
                    lines.push(TextLine::from(vec![Span::styled(" ", Style::default())]));
                }
            }
        }

        let empty_count = (inner.height as usize).saturating_sub(lines.len());
        for _ in 0..empty_count {
            lines.push(TextLine::from(vec![Span::styled(" ", Style::default())]));
        }

        let paragraph = Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(s.bg));
        frame.render_widget(paragraph, area);
    }

    pub fn scroll_down(&mut self) {
        let max_scroll = self.rendered.len().saturating_sub(self.viewport_height);
        self.scroll_offset = (self.scroll_offset + 1).min(max_scroll);
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.rendered.len().saturating_sub(self.viewport_height);
    }
}
