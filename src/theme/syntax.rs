use regex::Regex;
use crate::theme::Theme;
use ratatui::style::{Style, Color};
use std::sync::LazyLock;

pub struct SyntaxHighlighter;

struct LanguagePatterns {
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    constants: &'static [&'static str],
    comments: &'static [&'static str],
    strings: &'static [&'static str],
    numbers: &'static str,
    functions: &'static str,
}

static NUMBER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(0x[0-9a-fA-F_]+|\d+\.?\d*(?:[eE][+-]?\d+)?)\b").unwrap()
});

static STRING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""[^"\\]*(?:\\.[^"\\]*)*"|'[^'\\]*(?:\\.[^'\\]*)*'"#).unwrap()
});

impl SyntaxHighlighter {
    fn get_patterns(ext: &str) -> LanguagePatterns {
        match ext {
            "rs" | "rust" => LanguagePatterns {
                keywords: &[
                    "as", "async", "await", "break", "const", "continue", "crate", "dyn",
                    "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
                    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
                    "self", "Self", "static", "struct", "super", "trait", "true", "type",
                    "unsafe", "use", "where", "while", "yield",
                ],
                types: &[
                    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128",
                    "isize", "str", "String", "u8", "u16", "u32", "u64", "u128", "usize",
                    "Vec", "Option", "Result", "Box", "Rc", "Arc", "Cell", "RefCell",
                    "HashMap", "HashSet", "BTreeMap", "BTreeSet", "Iterator", "Fn",
                    "FnOnce", "FnMut", "dyn", "impl",
                ],
                constants: &[
                    "true", "false", "None", "Some", "Ok", "Err",
                ],
                comments: &["//", "/*", "*/", "///", "//!"],
                strings: &["\"", "'"],
                numbers: r"\b(0x[0-9a-fA-F_]+|\d+\.?\d*(?:[eE][+-]?\d+)?)\b",
                functions: r"\b([a-zA-Z_]\w*)\s*\(",
            },
            "py" | "python" => LanguagePatterns {
                keywords: &[
                    "False", "None", "True", "and", "as", "assert", "async", "await",
                    "break", "class", "continue", "def", "del", "elif", "else", "except",
                    "finally", "for", "from", "global", "if", "import", "in", "is",
                    "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try",
                    "while", "with", "yield",
                ],
                types: &[
                    "int", "float", "str", "bool", "list", "dict", "tuple", "set",
                    "frozenset", "bytes", "bytearray", "NoneType", "Any", "Optional",
                    "Callable", "Iterator", "Generator",
                ],
                constants: &["True", "False", "None", "Ellipsis", "NotImplemented"],
                comments: &["#"],
                strings: &["\"", "'", "\"\"\"", "'''"],
                numbers: r"\b(\d+\.?\d*(?:[eE][+-]?\d+)?|0x[0-9a-fA-F]+|0o[0-7]+|0b[01]+)\b",
                functions: r"\b([a-zA-Z_]\w*)\s*\(",
            },
            "js" | "javascript" | "jsx" => LanguagePatterns {
                keywords: &[
                    "async", "await", "break", "case", "catch", "class", "const",
                    "continue", "debugger", "default", "delete", "do", "else", "enum",
                    "export", "extends", "false", "finally", "for", "function", "if",
                    "import", "in", "instanceof", "let", "new", "null", "of", "return",
                    "super", "switch", "this", "throw", "true", "try", "typeof", "var",
                    "void", "while", "with", "yield",
                ],
                types: &[
                    "number", "string", "boolean", "object", "undefined", "symbol",
                    "bigint", "Array", "Map", "Set", "Promise", "Error", "Date",
                    "RegExp", "Function", "Buffer",
                ],
                constants: &["true", "false", "null", "undefined", "Infinity", "NaN"],
                comments: &["//", "/*", "*/"],
                strings: &["\"", "'", "`"],
                numbers: r"\b(\d+\.?\d*(?:[eE][+-]?\d+)?|0x[0-9a-fA-F]+|0o[0-7]+|0b[01]+)\b",
                functions: r"\b([a-zA-Z_$]\w*)\s*\(",
            },
            "ts" | "typescript" | "tsx" => LanguagePatterns {
                keywords: &[
                    "abstract", "as", "async", "await", "break", "case", "catch", "class",
                    "const", "continue", "debugger", "default", "delete", "do", "else",
                    "enum", "export", "extends", "false", "finally", "for", "function",
                    "if", "implements", "import", "in", "instanceof", "interface", "let",
                    "new", "null", "of", "package", "private", "protected", "public",
                    "return", "static", "super", "switch", "this", "throw", "true",
                    "try", "type", "typeof", "var", "void", "while", "with", "yield",
                    "readonly", "declare", "namespace", "module", "keyof", "infer",
                ],
                types: &[
                    "number", "string", "boolean", "undefined", "null", "symbol",
                    "bigint", "object", "any", "unknown", "never", "void",
                    "Array", "Map", "Set", "Promise", "Error", "Date", "RegExp",
                    "Record", "Partial", "Required", "Readonly", "Pick", "Omit",
                    "Exclude", "Extract", "NonNullable", "ReturnType",
                ],
                constants: &["true", "false", "null", "undefined", "Infinity", "NaN"],
                comments: &["//", "/*", "*/"],
                strings: &["\"", "'", "`"],
                numbers: r"\b(\d+\.?\d*(?:[eE][+-]?\d+)?|0x[0-9a-fA-F]+|0o[0-7]+|0b[01]+)\b",
                functions: r"\b([a-zA-Z_$]\w*)\s*\(",
            },
            "toml" => LanguagePatterns {
                keywords: &[
                    "true", "false",
                ],
                types: &[],
                constants: &["true", "false"],
                comments: &["#"],
                strings: &["\"", "'"],
                numbers: r"\b(\d+\.?\d*(?:[eE][+-]?\d+)?)\b",
                functions: r"",
            },
            "json" => LanguagePatterns {
                keywords: &[
                    "true", "false", "null",
                ],
                types: &[],
                constants: &["true", "false", "null"],
                comments: &[],
                strings: &["\""],
                numbers: r"\b(-?\d+\.?\d*(?:[eE][+-]?\d+)?)\b",
                functions: r"",
            },
            "yaml" | "yml" => LanguagePatterns {
                keywords: &[
                    "true", "false", "yes", "no", "on", "off", "null", "~",
                ],
                types: &[],
                constants: &["true", "false", "yes", "no", "null"],
                comments: &["#"],
                strings: &["\"", "'"],
                numbers: r"\b(\d+\.?\d*(?:[eE][+-]?\d+)?)\b",
                functions: r"",
            },
            "sh" | "bash" | "zsh" => LanguagePatterns {
                keywords: &[
                    "if", "then", "else", "elif", "fi", "for", "while", "do", "done",
                    "case", "esac", "in", "function", "return", "exit", "break",
                    "continue", "export", "local", "readonly", "declare", "typeset",
                    "select", "until", "time", "coproc",
                ],
                types: &[],
                constants: &["true", "false", "null"],
                comments: &["#"],
                strings: &["\"", "'"],
                numbers: r"\b(\d+)\b",
                functions: r"\b([a-zA-Z_]\w*)\s*\(",
            },
            "lua" => LanguagePatterns {
                keywords: &[
                    "and", "break", "do", "else", "elseif", "end", "false", "for",
                    "function", "goto", "if", "in", "local", "nil", "not", "or",
                    "repeat", "return", "then", "true", "until", "while",
                ],
                types: &[
                    "number", "string", "boolean", "table", "function", "thread", "userdata",
                ],
                constants: &["true", "false", "nil"],
                comments: &["--", "--[[", "]]"],
                strings: &["\"", "'", "[["],
                numbers: r"\b(\d+\.?\d*(?:[eE][+-]?\d+)?|0x[0-9a-fA-F]+)\b",
                functions: r"\b([a-zA-Z_]\w*)\s*\(",
            },
            "md" | "markdown" => LanguagePatterns {
                keywords: &[],
                types: &[],
                constants: &[],
                comments: &[],
                strings: &[],
                numbers: r"",
                functions: r"",
            },
            _ => LanguagePatterns {
                keywords: &[
                    "if", "else", "for", "while", "do", "switch", "case", "break",
                    "continue", "return", "function", "class", "import", "export",
                    "from", "const", "let", "var", "true", "false", "null", "undefined",
                    "new", "this", "try", "catch", "finally", "throw", "in", "of",
                    "type", "interface", "enum", "module",
                ],
                types: &[
                    "string", "number", "boolean", "object", "array", "function",
                    "void", "any", "never", "unknown",
                ],
                constants: &["true", "false", "null", "undefined"],
                comments: &["//", "/*", "*/", "#"],
                strings: &["\"", "'", "`"],
                numbers: r"\b(\d+\.?\d*(?:[eE][+-]?\d+)?|0x[0-9a-fA-F]+)\b",
                functions: r"\b([a-zA-Z_]\w*)\s*\(",
            },
        }
    }

    pub fn highlight_line(line: &str, ext: &str, theme: &Theme) -> Vec<(String, Style)> {
        if line.is_empty() {
            return vec![(String::new(), Style::default().fg(theme.fg))];
        }

        let patterns = Self::get_patterns(ext);
        let default_style = Style::default().fg(theme.fg);
        let mut all_ranges: Vec<(usize, usize, Style)> = Vec::new();

        if !patterns.comments.is_empty() {
            let comment_style = Style::default().fg(theme.syntax_comment);
            let mut comment_ranges: Vec<(usize, usize, Style)> = Vec::new();
            for prefix in patterns.comments {
                if prefix == &"/*" || prefix == &"*/" || prefix == &"--[[" || prefix == &"]]" {
                    continue;
                }
                let mut search_start = 0;
                while let Some(pos) = line[search_start..].find(prefix) {
                    let abs_pos = search_start + pos;
                    comment_ranges.push((abs_pos, line.len(), comment_style));
                    search_start = line.len();
                    break;
                }
            }
            all_ranges.extend(comment_ranges);
        }

        if all_ranges.is_empty() {
            let keyword_style = Style::default().fg(theme.syntax_keyword);
            all_ranges.extend(Self::find_keywords(line, patterns.keywords, keyword_style));

            let type_style = Style::default().fg(theme.syntax_type);
            all_ranges.extend(Self::find_keywords(line, patterns.types, type_style));

            let const_style = Style::default().fg(theme.syntax_constant);
            all_ranges.extend(Self::find_keywords(line, patterns.constants, const_style));

            let num_style = Style::default().fg(theme.syntax_number);
            all_ranges.extend(Self::find_pattern(line, &NUMBER_RE, num_style));

            let str_style = Style::default().fg(theme.syntax_string);
            all_ranges.extend(Self::find_pattern(line, &STRING_RE, str_style));

            if !patterns.functions.is_empty() {
                if let Ok(fn_re) = Regex::new(patterns.functions) {
                    let fn_style = Style::default().fg(theme.syntax_function);
                    for cap in fn_re.captures_iter(line) {
                        if let Some(m) = cap.get(1) {
                            all_ranges.push((m.start(), m.end(), fn_style));
                        }
                    }
                }
            }
        }

        let merged = Self::merge_overlapping(all_ranges);
        let mut segments: Vec<(String, Style)> = Vec::new();
        let mut last_end = 0;

        for (start, end, style) in &merged {
            if *start > last_end {
                let text = &line[last_end..*start];
                if !text.is_empty() {
                    segments.push((text.to_string(), default_style));
                }
            }
            let text = &line[*start..*end];
            if !text.is_empty() {
                segments.push((text.to_string(), *style));
            }
            last_end = *end;
        }

        if last_end < line.len() {
            let text = &line[last_end..];
            if !text.is_empty() {
                segments.push((text.to_string(), default_style));
            }
        }

        if segments.is_empty() {
            segments.push((line.to_string(), default_style));
        }

        segments
    }

    pub fn highlight_text(text: &str, ext: &str, theme: &Theme) -> Vec<Vec<(String, Style)>> {
        text.lines()
            .map(|line| Self::highlight_line(line, ext, theme))
            .collect()
    }

    fn find_keywords(text: &str, keywords: &[&str], style: Style) -> Vec<(usize, usize, Style)> {
        let mut result = Vec::new();
        for kw in keywords {
            if kw.is_empty() {
                continue;
            }
            let mut search_start = 0;
            while let Some(pos) = text[search_start..].find(kw) {
                let abs_pos = search_start + pos;
                let end = abs_pos + kw.len();
                if Self::is_word_boundary(text, abs_pos, end) {
                    result.push((abs_pos, end, style));
                }
                search_start = end;
                if search_start >= text.len() {
                    break;
                }
            }
        }
        result
    }

    fn find_pattern(text: &str, re: &Regex, style: Style) -> Vec<(usize, usize, Style)> {
        let mut result = Vec::new();
        for cap in re.find_iter(text) {
            result.push((cap.start(), cap.end(), style));
        }
        result
    }

    fn is_word_boundary(text: &str, start: usize, end: usize) -> bool {
        let before = if start == 0 {
            true
        } else {
            let c = text.as_bytes()[start - 1] as char;
            !c.is_alphanumeric() && c != '_'
        };
        let after = if end >= text.len() {
            true
        } else {
            let c = text.as_bytes()[end] as char;
            !c.is_alphanumeric() && c != '_'
        };
        before && after
    }

    fn merge_overlapping(mut ranges: Vec<(usize, usize, Style)>) -> Vec<(usize, usize, Style)> {
        if ranges.is_empty() {
            return ranges;
        }
        ranges.sort_by_key(|(s, _, _)| *s);
        let mut merged: Vec<(usize, usize, Style)> = Vec::new();
        for (start, end, style) in ranges {
            if let Some(last) = merged.last_mut() {
                if start <= last.1 {
                    if end > last.1 {
                        last.1 = end;
                    }
                    continue;
                }
            }
            merged.push((start, end, style));
        }
        merged
    }
}
