//! The lexer: source text -> tokens, each carrying a 1-based line/column.
//! Whitespace and `#` line comments are insignificant except as separators.

use crate::error::Diagnostic;
use crate::span::Span;
use crate::token::{keyword, StrPart, Token, TokenKind};

pub fn lex(src: &str) -> Result<Vec<Token>, Diagnostic> {
    Lexer::new(src).run()
}

struct Lexer<'a> {
    chars: Vec<char>,
    src: &'a str,
    pos: usize,
    line: u32,
    col: u32,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer {
            chars: src.chars().collect(),
            src,
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn here(&self) -> Span {
        Span::new(self.line, self.col)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn run(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia();
            let start = self.here();
            let c = match self.peek() {
                None => {
                    tokens.push(Token::new(TokenKind::Eof, start));
                    break;
                }
                Some(c) => c,
            };

            if c.is_ascii_digit() {
                tokens.push(self.lex_number(start));
                continue;
            }
            if c == '_' || c.is_alphabetic() {
                tokens.push(self.lex_ident(start));
                continue;
            }
            if c == '"' {
                tokens.push(self.lex_string(start)?);
                continue;
            }
            tokens.push(self.lex_symbol(start)?);
        }
        Ok(tokens)
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('#') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                _ => break,
            }
        }
    }

    fn lex_number(&mut self, start: Span) -> Token {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.bump();
            } else if c == '.' && self.peek2().is_some_and(|d| d.is_ascii_digit()) {
                // a fractional part (but not the `..` range operator)
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        let value: f64 = s.parse().unwrap_or(0.0);
        Token::new(TokenKind::Number(value), start)
    }

    fn lex_ident(&mut self, start: Span) -> Token {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '_' || c.is_alphanumeric() {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        let kind = keyword(&s).unwrap_or(TokenKind::Ident(s));
        Token::new(kind, start)
    }

    fn lex_string(&mut self, start: Span) -> Result<Token, Diagnostic> {
        self.bump(); // opening quote
        let mut parts: Vec<StrPart> = Vec::new();
        let mut lit = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    return Err(Diagnostic::lex(
                        "unterminated glyph literal (missing closing '\"')",
                        start,
                    ));
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\\') => {
                    self.bump();
                    let esc = self.bump();
                    match esc {
                        Some('n') => lit.push('\n'),
                        Some('t') => lit.push('\t'),
                        Some('"') => lit.push('"'),
                        Some('\\') => lit.push('\\'),
                        Some('$') => lit.push('$'),
                        Some(other) => lit.push(other),
                        None => {
                            return Err(Diagnostic::lex("unterminated escape in glyph", start));
                        }
                    }
                }
                Some('$') if self.peek2() == Some('{') => {
                    if !lit.is_empty() {
                        parts.push(StrPart::Lit(std::mem::take(&mut lit)));
                    }
                    self.bump(); // $
                    self.bump(); // {
                    let mut depth = 1;
                    let mut expr = String::new();
                    loop {
                        match self.peek() {
                            None | Some('\n') => {
                                return Err(Diagnostic::lex(
                                    "unterminated interpolation '${...}' in glyph",
                                    start,
                                ));
                            }
                            Some('{') => {
                                depth += 1;
                                expr.push('{');
                                self.bump();
                            }
                            Some('}') => {
                                depth -= 1;
                                self.bump();
                                if depth == 0 {
                                    break;
                                }
                                expr.push('}');
                            }
                            Some(c) => {
                                expr.push(c);
                                self.bump();
                            }
                        }
                    }
                    parts.push(StrPart::Expr(expr));
                }
                Some(c) => {
                    lit.push(c);
                    self.bump();
                }
            }
        }
        if !lit.is_empty() || parts.is_empty() {
            parts.push(StrPart::Lit(lit));
        }
        Ok(Token::new(TokenKind::Str(parts), start))
    }

    fn lex_symbol(&mut self, start: Span) -> Result<Token, Diagnostic> {
        let c = self.bump().unwrap();
        let kind = match c {
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '.' => {
                if self.peek() == Some('.') {
                    self.bump();
                    TokenKind::DotDot
                } else {
                    TokenKind::Dot
                }
            }
            '=' => {
                if self.peek() == Some('=') {
                    self.bump();
                    TokenKind::EqEq
                } else if self.peek() == Some('>') {
                    self.bump();
                    TokenKind::FatArrow
                } else {
                    TokenKind::Eq
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.bump();
                    TokenKind::Le
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.bump();
                    TokenKind::Ge
                } else {
                    TokenKind::Gt
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.bump();
                    TokenKind::Ne
                } else {
                    return Err(Diagnostic::lex("unexpected character '!'", start));
                }
            }
            other => {
                return Err(Diagnostic::lex(
                    format!("unexpected character '{}'", other),
                    start,
                ));
            }
        };
        let _ = self.src;
        Ok(Token::new(kind, start))
    }
}
