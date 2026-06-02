use ratatui::{
    Frame,
    layout::Rect,
    widgets::Paragraph,
    text::Line,
    style::{Style, Color},
};
use crate::theme::Theme;
use crate::git::BlameInfo;

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn time_ago(ts: i64) -> String {
    let secs = now_secs() - ts;
    if secs < 60 { "now".into() }
    else if secs < 3600 { format!("{}m", secs / 60) }
    else if secs < 86400 { format!("{}h", secs / 3600) }
    else if secs < 2592000 { format!("{}d", secs / 86400) }
    else if secs < 31536000 { format!("{}mo", secs / 2592000) }
    else { format!("{}y", secs / 31536000) }
}

fn author_short(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() { return "?".to_string(); }
    let parts: Vec<&str> = name.split_whitespace().collect();
    if parts.len() == 1 {
        parts[0].chars().take(6).collect()
    } else {
        format!("{}.{}", parts[0].chars().next().unwrap_or('?'), parts.last().unwrap_or(&"?"))
    }
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    blame_data: &[Option<BlameInfo>],
    visible_start: usize,
    theme: &Theme,
) {
    if blame_data.is_empty() { return; }
    let mut lines = Vec::new();
    let start = visible_start.min(blame_data.len());
    let end = (start + area.height as usize).min(blame_data.len());
    for bline in &blame_data[start..end] {
        let text = match bline {
            Some(info) => {
                let author = author_short(&info.author);
                let ago = time_ago(info.time);
                format!("{:<6} {}", author, ago)
            }
            None => String::new(),
        };
        lines.push(Line::from(text));
    }
    let style = Style::default().fg(Color::Rgb(100, 100, 100)).bg(theme.bg);
    frame.render_widget(Paragraph::new(lines).style(style), area);
}

pub fn compute_width(blame_data: &[Option<BlameInfo>]) -> u16 {
    if blame_data.is_empty() { return 0; }
    let max_author = blame_data.iter()
        .filter_map(|b| b.as_ref())
        .map(|b| author_short(&b.author).len() + time_ago(b.time).len() + 1)
        .max()
        .unwrap_or(10);
    (max_author as u16 + 2).min(14)
}
