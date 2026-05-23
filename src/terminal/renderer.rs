use ratatui::text::{Span, Line};
use ratatui::style::{Style, Color, Modifier};
use crate::theme::Theme;

pub fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&n) = chars.peek() {
                    if n == 'm' { chars.next(); break; }
                    if n.is_ascii_alphabetic() && n != 'm' {
                        chars.next();
                        break;
                    }
                    chars.next();
                }
            } else if *chars.peek().unwrap_or(&'\0') == ']' {
                chars.next();
                while let Some(&n) = chars.peek() {
                    if n == '\x07' || n == '\x1b' {
                        if n == '\x1b' { break; }
                        chars.next();
                        break;
                    }
                    chars.next();
                }
            }
        } else if c == '\x07' || c == '\x00' {
            // skip bell and null
        } else {
            result.push(c);
        }
    }
    result
}

pub fn parse_ansi_spans(text: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_style = Style::default().fg(theme.fg).bg(theme.bg);
    let mut chars = text.chars().peekable();
    let mut buf = String::new();

    macro_rules! flush {
        () => {
            if !buf.is_empty() {
                let content: String = std::mem::take(&mut buf);
                spans.push(Span::styled(content, current_style));
            }
        };
    }

    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            flush!();
            chars.next();
            let mut params = String::new();
            while let Some(&n) = chars.peek() {
                if n == 'm' { chars.next(); break; }
                params.push(n);
                chars.next();
            }
            current_style = apply_sgr(&params, current_style, theme);
        } else if c == '\x07' || c == '\x00' {
            // skip
        } else {
            buf.push(c);
        }
    }
    flush!();

    if spans.is_empty() {
        let cleaned = strip_ansi(text);
        spans.push(Span::styled(cleaned, Style::default().fg(theme.fg)));
    }

    spans
}

fn apply_sgr(params: &str, mut style: Style, theme: &Theme) -> Style {
    if params.is_empty() || params == "0" {
        return Style::default().fg(theme.fg).bg(theme.bg);
    }

    for param in params.split(';') {
        match param {
            "0" | "" => style = Style::default().fg(theme.fg).bg(theme.bg),
            "1" => style = style.add_modifier(Modifier::BOLD),
            "3" => style = style.add_modifier(Modifier::ITALIC),
            "4" => style = style.add_modifier(Modifier::UNDERLINED),
            "7" => style = style.add_modifier(Modifier::REVERSED),
            "22" => style = style.remove_modifier(Modifier::BOLD),
            "23" => style = style.remove_modifier(Modifier::ITALIC),
            "24" => style = style.remove_modifier(Modifier::UNDERLINED),
            "27" => style = style.remove_modifier(Modifier::REVERSED),
            "30" | "90" => style = style.fg(Color::Black),
            "31" | "91" => style = style.fg(Color::Red),
            "32" | "92" => style = style.fg(Color::Green),
            "33" | "93" => style = style.fg(Color::Yellow),
            "34" | "94" => style = style.fg(Color::Blue),
            "35" | "95" => style = style.fg(Color::Magenta),
            "36" | "96" => style = style.fg(Color::Cyan),
            "37" | "97" => style = style.fg(Color::White),
            "39" => style = style.fg(theme.fg),
            "40" | "100" => style = style.bg(Color::Black),
            "41" | "101" => style = style.bg(Color::Red),
            "42" | "102" => style = style.bg(Color::Green),
            "43" | "103" => style = style.bg(Color::Yellow),
            "44" | "104" => style = style.bg(Color::Blue),
            "45" | "105" => style = style.bg(Color::Magenta),
            "46" | "106" => style = style.bg(Color::Cyan),
            "47" | "107" => style = style.bg(Color::White),
            "49" => style = style.bg(theme.bg),
            // 256-color: 38;5;N or 48;5;N
            p if p.starts_with("38;5;") || p.starts_with("48;5;") => {
                // Handled by the split logic below for 256-color
            }
            // truecolor: 38;2;R;G;B or 48;2;R;G;B
            p if p.starts_with("38;2;") || p.starts_with("48;2;") => {
                // Handled below
            }
            _ => {}
        }
    }

    // Handle 256-color and truecolor (the params may contain them as separate args)
    // This is a simplification; full parsing would need to look at semicolons differently
    if params.contains("38;5;") || params.contains("48;5;") {
        let parts: Vec<&str> = params.split(';').collect();
        for i in 0..parts.len() {
            if parts[i] == "38" && i + 2 < parts.len() && parts[i+1] == "5" {
                if let Ok(c) = parts[i+2].parse::<u8>() {
                    style = style.fg(Color::Indexed(c));
                }
            }
            if parts[i] == "48" && i + 2 < parts.len() && parts[i+1] == "5" {
                if let Ok(c) = parts[i+2].parse::<u8>() {
                    style = style.bg(Color::Indexed(c));
                }
            }
        }
    }
    if params.contains("38;2;") || params.contains("48;2;") {
        let parts: Vec<&str> = params.split(';').collect();
        for i in 0..parts.len() {
            if parts[i] == "38" && i + 4 < parts.len() && parts[i+1] == "2" {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    parts[i+2].parse::<u8>(),
                    parts[i+3].parse::<u8>(),
                    parts[i+4].parse::<u8>(),
                ) {
                    style = style.fg(Color::Rgb(r, g, b));
                }
            }
            if parts[i] == "48" && i + 4 < parts.len() && parts[i+1] == "2" {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    parts[i+2].parse::<u8>(),
                    parts[i+3].parse::<u8>(),
                    parts[i+4].parse::<u8>(),
                ) {
                    style = style.bg(Color::Rgb(r, g, b));
                }
            }
        }
    }

    style
}

pub fn render_terminal_lines(lines: &[String], theme: &Theme, width: usize) -> Vec<Line<'static>> {
    lines.iter().map(|line| {
        let cleaned = strip_ansi(line);
        let truncated: String = cleaned.chars().take(width).collect();
        Line::from(Span::styled(truncated, Style::default().fg(theme.fg)))
    }).collect()
}
