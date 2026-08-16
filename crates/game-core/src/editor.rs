#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Comment,
    String,
    Number,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenSpan {
    pub kind: TokenKind,
    pub start: usize, // 字节偏移
    pub end: usize,
}

const KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "if", "else", "for", "while", "loop", "match", "struct", "enum",
    "impl", "trait", "pub", "use", "mod", "const", "static", "return", "move", "ref", "self",
    "Self", "super", "crate", "as", "in", "where", "async", "await", "dyn", "type", "unsafe",
    "extern", "macro_rules", "true", "false",
];

/// 基于 rustc_lexer（rustc 官方词法器，rust-lang/rust 仓库发布）的着色分词。
/// 输出字节偏移片段，可直接切片 &code[start..end]。
/// 支持：注释、原始字符串 r#"..."#、生命周期 'a、全部字面量形式。
pub fn tokenize(code: &str) -> Vec<TokenSpan> {
    use rustc_lexer::{tokenize, LiteralKind, TokenKind as LexKind};
    let mut spans = Vec::new();
    let mut pos = 0;
    for tok in tokenize(code) {
        let kind = match tok.kind {
            LexKind::LineComment | LexKind::BlockComment { .. } => TokenKind::Comment,
            LexKind::Ident => {
                let word = &code[pos..pos + tok.len];
                if KEYWORDS.contains(&word) {
                    TokenKind::Keyword
                } else {
                    TokenKind::Normal
                }
            }
            LexKind::RawIdent | LexKind::Lifetime { .. } => TokenKind::Normal,
            LexKind::Literal { kind, .. } => match kind {
                LiteralKind::Str { .. }
                | LiteralKind::ByteStr { .. }
                | LiteralKind::RawStr { .. }
                | LiteralKind::RawByteStr { .. }
                | LiteralKind::Char { .. }
                | LiteralKind::Byte { .. } => TokenKind::String,
                LiteralKind::Int { .. } | LiteralKind::Float { .. } => TokenKind::Number,
            },
            LexKind::Whitespace
            | LexKind::Semi
            | LexKind::Comma
            | LexKind::Dot
            | LexKind::OpenParen
            | LexKind::CloseParen
            | LexKind::OpenBrace
            | LexKind::CloseBrace
            | LexKind::OpenBracket
            | LexKind::CloseBracket
            | LexKind::At
            | LexKind::Pound
            | LexKind::Tilde
            | LexKind::Question
            | LexKind::Colon
            | LexKind::Dollar
            | LexKind::Eq
            | LexKind::Not
            | LexKind::Lt
            | LexKind::Gt
            | LexKind::Minus
            | LexKind::And
            | LexKind::Or
            | LexKind::Plus
            | LexKind::Star
            | LexKind::Slash
            | LexKind::Caret
            | LexKind::Percent => {
                pos += tok.len;
                continue;
            }
            LexKind::Unknown => TokenKind::Normal,
        };
        spans.push(TokenSpan { kind, start: pos, end: pos + tok.len });
        pos += tok.len;
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(code: &str) -> Vec<TokenKind> {
        tokenize(code).iter().map(|s| s.kind).collect()
    }

    fn slice<'a>(code: &'a str, spans: &[TokenSpan]) -> Vec<&'a str> {
        spans.iter().map(|s| &code[s.start..s.end]).collect()
    }

    #[test]
    fn keyword_and_normal() {
        let code = "let mut x = 5;";
        let spans = tokenize(code);
        let words = slice(code, &spans);
        assert_eq!(words, vec!["let", "mut", "x", "5"]);
        assert_eq!(kinds(code), vec![TokenKind::Keyword, TokenKind::Keyword, TokenKind::Normal, TokenKind::Number]);
    }

    #[test]
    fn line_comment() {
        let code = "// 注释\nlet x = 1;";
        let spans = tokenize(code);
        let words = slice(code, &spans);
        assert_eq!(words[0], "// 注释");
        assert_eq!(spans[0].kind, TokenKind::Comment);
    }

    #[test]
    fn block_comment_multiline() {
        let code = "/* a\nb */ let x = 1;";
        let spans = tokenize(code);
        assert_eq!(spans[0].kind, TokenKind::Comment);
        assert_eq!(&code[spans[0].start..spans[0].end], "/* a\nb */");
    }

    #[test]
    fn string_with_escape() {
        let code = "let s = \"a\\\"b\";";
        let spans = tokenize(code);
        assert!(spans.iter().any(|s| s.kind == TokenKind::String));
        let words = slice(code, &spans);
        assert_eq!(words[2], "\"a\\\"b\"");
    }

    #[test]
    fn char_literal() {
        let code = "let c = 'x';";
        let spans = tokenize(code);
        assert!(spans.iter().any(|s| s.kind == TokenKind::String && &code[s.start..s.end] == "'x'"));
    }

    #[test]
    fn lifetime_is_normal_not_string() {
        let code = "fn f<'a>(x: &'a str) -> &'a str { x }";
        let spans = tokenize(code);
        assert!(!spans.iter().any(|s| s.kind == TokenKind::String));
        assert!(spans.iter().any(|s| s.kind == TokenKind::Normal && &code[s.start..s.end] == "'a"));
    }

    #[test]
    fn raw_string_supported() {
        // 原始字符串内嵌引号不会提前终止
        let code = "let s = r#\"a \\\"b\\\" c\"#;";
        let spans = tokenize(code);
        assert!(spans.iter().any(|s| s.kind == TokenKind::String && &code[s.start..s.end] == "r#\"a \\\"b\\\" c\"#"));
    }

    #[test]
    fn bool_literal_keyword_colored() {
        let code = "let b = true;";
        let spans = tokenize(code);
        assert!(spans.iter().any(|s| s.kind == TokenKind::Keyword && &code[s.start..s.end] == "true"));
    }

    #[test]
    fn byte_offsets_valid_slices() {
        let code = "let 中文 = 1;";
        let spans = tokenize(code);
        for s in &spans {
            let _ = &code[s.start..s.end];
        }
    }
}
