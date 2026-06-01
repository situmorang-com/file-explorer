use once_cell::sync::Lazy;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

static SYNTAXES: Lazy<SyntaxSet> = Lazy::new(|| {
    // two-face bundles many extras the syntect defaults are missing
    // (TypeScript, TSX, Astro, Vue, modern Rust, etc.).
    two_face::syntax::extra_no_newlines()
});
static THEMES: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

pub struct Span {
    pub text: String,
    pub rgb: (u8, u8, u8),
}

/// Highlight up to `max_lines` lines of `content`, picking a syntax by `extension`.
/// Falls back to plain text if no syntax found. Returns rows of styled spans.
pub fn highlight_lines(content: &str, extension: &str, max_lines: usize) -> Vec<Vec<Span>> {
    let theme = THEMES
        .themes
        .get("base16-mocha.dark")
        .or_else(|| THEMES.themes.get("Solarized (dark)"))
        .unwrap_or_else(|| THEMES.themes.values().next().unwrap());

    let syntax = SYNTAXES
        .find_syntax_by_extension(extension)
        .or_else(|| SYNTAXES.find_syntax_by_first_line(content))
        .unwrap_or_else(|| SYNTAXES.find_syntax_plain_text());

    let mut h = HighlightLines::new(syntax, theme);
    let mut out: Vec<Vec<Span>> = Vec::new();
    for (i, line) in LinesWithEndings::from(content).enumerate() {
        if i >= max_lines {
            break;
        }
        let ranges: Vec<(Style, &str)> = h
            .highlight_line(line, &SYNTAXES)
            .unwrap_or_else(|_| vec![(Style::default(), line)]);
        let mut row: Vec<Span> = Vec::new();
        for (style, text) in ranges {
            let trimmed = text.trim_end_matches('\n');
            if trimmed.is_empty() {
                continue;
            }
            row.push(Span {
                text: trimmed.to_string(),
                rgb: (style.foreground.r, style.foreground.g, style.foreground.b),
            });
        }
        out.push(row);
    }
    out
}

/// Highlight a single line for the content-search snippet.
#[allow(dead_code)]
pub fn highlight_one(line: &str, extension: &str) -> Vec<Span> {
    let rows = highlight_lines(line, extension, 1);
    rows.into_iter().next().unwrap_or_default()
}
