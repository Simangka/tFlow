use std::borrow::Cow;

pub mod parser;
pub mod renderer;
pub mod plain;
pub mod help;

pub use parser::MarkdownParser;
pub use renderer::MarkdownRenderer;
pub use plain::PlainTextRenderer;
pub use help::HelpScreen;

pub(crate) const MAX_MD_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn prepare_md(text: &str) -> Cow<'_, str> {
    let trimmed = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let needs_norm = trimmed.contains('\r');
    let needs_cap = trimmed.len() > MAX_MD_BYTES;

    if !needs_norm && !needs_cap {
        return Cow::Borrowed(trimmed);
    }

    let mut s: String = if needs_norm {
        trimmed.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        trimmed.to_string()
    };
    if s.len() > MAX_MD_BYTES {
        s.truncate(MAX_MD_BYTES);
    }
    Cow::Owned(s)
}
