use tflow::config::Config;
use tflow::core::{Buffer, Position, Range};
use tflow::editor::{Cursor, EditOperations};
use tflow::app::AppContext;
use tflow::markdown::MarkdownParser;
use tflow::theme::Theme;
use tflow::theme::syntax::SyntaxHighlighter;

fn temp_path(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    p.push(format!("tflow_crit_{}_{}_{}.txt", name, pid, std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)));
    p
}

#[test]
fn range_lines_handles_reverse_direction() {
    let r = Range::new(Position::new(5, 0), Position::new(2, 0));
    assert!(r.lines() >= 4);
    let fwd = Range::new(Position::new(2, 0), Position::new(5, 0)).lines();
    assert_eq!(fwd, 4);
}

#[test]
fn position_new_checked_rejects_oversize() {
    assert!(Position::new_checked(0, 100_000, 100_000, 100_000).is_some());
    assert!(Position::new_checked(u32::MAX as usize, 0, 100_000, 100_000).is_none());
    assert!(Position::new_checked(0, 0, 0, 0).is_some());
}

#[test]
fn buffer_load_handles_utf8_bom() {
    let path = temp_path("bom");
    std::fs::write(&path, b"\xEF\xBB\xBFhello").unwrap();
    let mut buf = Buffer::from_path(0, path.clone()).unwrap();
    let _ = buf.load();
    assert_eq!(buf.get_text(), "hello");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn buffer_load_handles_empty_file() {
    let path = temp_path("empty");
    std::fs::write(&path, b"").unwrap();
    let mut buf = Buffer::from_path(0, path.clone()).unwrap();
    let _ = buf.load();
    assert!(buf.get_text().is_empty());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn buffer_save_creates_file_with_content() {
    let path = temp_path("save");
    let _ = std::fs::remove_file(&path);
    let mut buf = Buffer::new(0);
    buf.insert_str(Position::zero(), "hi");
    buf.save_as(path.clone()).unwrap();
    assert!(path.exists());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hi");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn cursor_move_left_in_does_not_underflow() {
    let buf = Buffer::new(0);
    let mut c = Cursor::new();
    c.move_left_in(&buf);
    assert_eq!(c.position.column, 0);
    assert_eq!(c.position.line, 0);
    let mut c2 = Cursor::new();
    c2.move_left_in(&buf);
    assert_eq!(c2.position.column, 0);
    assert_eq!(c2.position.line, 0);
}

#[test]
fn join_lines_inserts_single_space() {
    let mut buf = Buffer::new(0);
    buf.insert_str(Position::zero(), "hello\nworld");
    let mut cursor = Cursor::new();
    cursor.position = Position::new(0, 0);
    EditOperations::join_lines(&mut buf, &mut cursor).unwrap();
    assert_eq!(buf.get_text(), "hello world");
}

#[test]
fn toggle_comment_works_on_indented_line() {
    let mut buf = Buffer::new(0);
    buf.insert_str(Position::zero(), "    code");
    let mut cursor = Cursor::new();
    cursor.position = Position::new(0, 0);
    EditOperations::toggle_comment(&mut buf, &mut cursor).unwrap();
    let s = buf.get_text();
    assert!(s.starts_with("//"));
    assert!(s.contains("    code"));
}

#[tokio::test]
async fn open_file_nonexistent_returns_err() {
    let mut ctx = AppContext::new(Config::default());
    let bad = std::path::PathBuf::from("Z:/tflow_nonexistent_path_for_test_zzz/file_does_not_exist.txt");
    let result = ctx.open_file_async(bad).await;
    assert!(result.is_err());
}

#[test]
fn markdown_parser_does_not_panic_on_huge_input() {
    let parser = MarkdownParser::new();
    let huge = "a".repeat(20 * 1024 * 1024);
    let result = std::panic::catch_unwind(|| parser.parse(&huge));
    assert!(result.is_ok());
    let parsed = result.unwrap();
    assert!(parsed.events.len() < huge.len());
}

#[test]
fn markdown_parser_extracts_headings() {
    let parser = MarkdownParser::new();
    let md = "# Title One\n\n## Subtitle\n\nbody\n";
    let headings = parser.parse(md).headings;
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0].text, "Title One");
    assert_eq!(headings[1].text, "Subtitle");
}

#[test]
fn highlight_fence_does_not_emit_heading() {
    let theme = Theme::default_dark();
    let spans = SyntaxHighlighter::highlight_line("```rust", "md", &theme);
    assert!(!spans.is_empty());
    assert_eq!(spans[0].0, "```rust");
    let heading_spans = SyntaxHighlighter::highlight_line("# heading", "md", &theme);
    assert!(!heading_spans.is_empty());
    assert_eq!(heading_spans[0].0, "# heading");
}
