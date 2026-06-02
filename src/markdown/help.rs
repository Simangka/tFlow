use ratatui::style::{Style, Modifier};
use crate::theme::Theme;
use super::renderer::MarkdownRenderLine;

pub struct HelpScreen;

impl HelpScreen {
    pub fn render(theme: &Theme) -> Vec<MarkdownRenderLine> {
        let dim = Style::default().fg(theme.comment).bg(theme.bg);
        let head = Style::default().fg(theme.heading1).bg(theme.bg).add_modifier(Modifier::BOLD);
        let sub = Style::default().fg(theme.heading2).bg(theme.bg).add_modifier(Modifier::BOLD);
        let key = Style::default().fg(theme.keyword).bg(theme.bg).add_modifier(Modifier::BOLD);
        let val = Style::default().fg(theme.fg).bg(theme.bg);
        let accent = Style::default().fg(theme.string).bg(theme.bg);

        let mut lines = Vec::new();

        // Mountain landscape
        let mountain = vec![
            "                                 ",
            "        .     .  .               ",
            "         .   .  . .    .         ",
            "     .   . . .. . .. ..  .       ",
            "      .  . .   .. . .  .. .      ",
            "  _-^^-.__ _-^^-.__ _-^^-.__     ",
            " /__^____\\/__^____\\/__^____\\    ",
            "  |_| |_|  |_| |_|  |_| |_|     ",
        ];
        for l in &mountain {
            lines.push(MarkdownRenderLine {
                content: l.to_string(),
                style: dim,
                indent: 0, is_heading: false, heading_level: 0,
            });
        }

        lines.push(MarkdownRenderLine { content: "".into(), style: dim, indent: 0, is_heading: false, heading_level: 0 });

        lines.push(MarkdownRenderLine {
            content: "  T F L O W".to_string(),
            style: head,
            indent: 0, is_heading: false, heading_level: 0,
        });
        lines.push(MarkdownRenderLine {
            content: "  terminal text editor  v0.1.0".to_string(),
            style: accent,
            indent: 0, is_heading: false, heading_level: 0,
        });

        lines.push(MarkdownRenderLine { content: "".into(), style: dim, indent: 0, is_heading: false, heading_level: 0 });
        lines.push(MarkdownRenderLine { content: " ───────────────────────────────────── ".into(), style: dim, indent: 0, is_heading: false, heading_level: 0 });

        // ── NAVIGATION ──
        lines.push(MarkdownRenderLine::section(" NAVIGATION ", sub));
        lines.push(MarkdownRenderLine::keybind("← → ↑ ↓", "Move cursor", key, val));
        lines.push(MarkdownRenderLine::keybind("Ctrl+←/→", "Move by word", key, val));
        lines.push(MarkdownRenderLine::keybind("Home / End", "Start / end of line", key, val));
        lines.push(MarkdownRenderLine::keybind("PgUp / PgDn", "Page up / down", key, val));
        lines.push(MarkdownRenderLine::keybind("gg / G", "Start / end of file", key, val));
        lines.push(MarkdownRenderLine::keybind(":42", "Go to line 42", key, val));

        lines.push(MarkdownRenderLine::section(" EDITING ", sub));
        lines.push(MarkdownRenderLine::keybind("i", "Insert mode", key, val));
        lines.push(MarkdownRenderLine::keybind("Esc", "Normal mode", key, val));
        lines.push(MarkdownRenderLine::keybind("u / Ctrl+r", "Undo / Redo", key, val));
        lines.push(MarkdownRenderLine::keybind("dd", "Delete line", key, val));
        lines.push(MarkdownRenderLine::keybind("yy / p", "Copy line / Paste", key, val));
        lines.push(MarkdownRenderLine::keybind(">> / <<", "Indent / Unindent", key, val));
        lines.push(MarkdownRenderLine::keybind("J", "Join lines", key, val));

        lines.push(MarkdownRenderLine::section(" FILES ", sub));
        lines.push(MarkdownRenderLine::keybind(":w", "Save file", key, val));
        lines.push(MarkdownRenderLine::keybind(":q / :q!", "Quit / Force quit", key, val));
        lines.push(MarkdownRenderLine::keybind(":e <path>", "Open file", key, val));
        lines.push(MarkdownRenderLine::keybind(":new", "New buffer", key, val));
        lines.push(MarkdownRenderLine::keybind(":bn / :bp", "Next / prev buffer", key, val));

        lines.push(MarkdownRenderLine::section(" SEARCH ", sub));
        lines.push(MarkdownRenderLine::keybind("/<query>", "Search forward", key, val));
        lines.push(MarkdownRenderLine::keybind("n / N", "Next / prev match", key, val));

        lines.push(MarkdownRenderLine::section(" PREVIEW PANEL ", sub));
        lines.push(MarkdownRenderLine::keybind("Ctrl+K / F11", "Toggle this panel", key, val));
        lines.push(MarkdownRenderLine::keybind("Alt+M", "Switch plain/markdown mode", key, val));

        lines.push(MarkdownRenderLine::section(" COMMAND PALETTE ", sub));
        lines.push(MarkdownRenderLine::keybind("Ctrl+P", "Open palette", key, val));
        lines.push(MarkdownRenderLine::section(" FILE TREE ", sub));
        lines.push(MarkdownRenderLine::keybind("Ctrl+T / F1", "Toggle file tree", key, val));
        lines.push(MarkdownRenderLine::keybind("↑↓ / j k", "Navigate files", key, val));
        lines.push(MarkdownRenderLine::keybind("Enter", "Open file / expand dir", key, val));
        lines.push(MarkdownRenderLine::keybind("← → / h l", "Collapse / expand", key, val));
        lines.push(MarkdownRenderLine::keybind("Esc / Tab", "Back to editor", key, val));

        lines.push(MarkdownRenderLine::blank());
        lines.push(MarkdownRenderLine {
            content: "  Press Ctrl+K or F11 to close this panel  ".to_string(),
            style: dim,
            indent: 0, is_heading: false, heading_level: 0,
        });

        lines
    }
}

impl MarkdownRenderLine {
    fn blank() -> Self {
        MarkdownRenderLine {
            content: String::new(),
            style: Style::default(),
            indent: 0, is_heading: false, heading_level: 0,
        }
    }

    fn section(title: &str, style: Style) -> Self {
        let width: usize = 38;
        let side = width.saturating_sub(title.chars().count()) / 2;
        let bar = "\u{2500}".repeat(side);
        let line = format!("{}{}{}", bar, title, bar);
        MarkdownRenderLine {
            content: line,
            style,
            indent: 0, is_heading: false, heading_level: 0,
        }
    }

    fn keybind(k: &str, desc: &str, _key_style: Style, val_style: Style) -> Self {
        MarkdownRenderLine {
            content: format!("  {:<14}  {}", k, desc),
            style: val_style,
            indent: 0, is_heading: false, heading_level: 0,
        }
    }
}
