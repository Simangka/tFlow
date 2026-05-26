
use lsp_types::*;

pub struct HoverHandler;

impl HoverHandler {
    pub fn process_hover_response(
        result: serde_json::Value,
    ) -> Option<Hover> {
        serde_json::from_value::<Hover>(result).ok()
    }

    pub fn format_hover_content(hover: &Hover) -> String {
        match &hover.contents {
            HoverContents::Scalar(marked) => {
                Self::format_marked_string(marked)
            }
            HoverContents::Array(items) => {
                items.iter()
                    .map(|m| Self::format_marked_string(m))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
            HoverContents::Markup(markup) => {
                markup.value.clone()
            }
        }
    }

    fn format_marked_string(marked: &MarkedString) -> String {
        match marked {
            MarkedString::String(s) => s.clone(),
            MarkedString::LanguageString(ls) => {
                format!("```{}\n{}\n```", ls.language, ls.value)
            }
        }
    }
}
