//! Tokens. Mundane keywords are plain (`define let var while if else return`);
//! evocative vocabulary names the intelligence/human boundary (`oracle summon
//! divine enact fallback speak listen`). Type names `spark`/`glyph` arrive as
//! identifiers and are resolved in type position.

use crate::span::Span;

/// A segment of a `glyph` literal: literal text, or an interpolated expression
/// captured as raw source to be parsed later (`${ ... }`).
#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Lit(String),
    Expr(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    // Literals
    Number(f64),
    Str(Vec<StrPart>),
    True,
    False,
    Ident(String),

    // Mundane keywords (plain register)
    Define,
    Let,
    Var,
    While,
    If,
    Else,
    Return,

    // Evocative keywords (intelligence / human boundary)
    Oracle,
    Summon,
    Divine,
    Enact,
    Fallback,
    Speak,
    Listen,

    // Type / clause keywords
    Type,
    OneOf,
    In,
    From,
    Using,
    With,
    Confidence,

    // Capability / effect keywords
    Requires,
    Grant,

    // Governed-memory keywords
    Memory,
    Within,

    // Bounded-familiar keywords
    Familiar,
    Permits,

    // Boolean / logical operators
    And,
    Or,
    Not,

    // Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    DotDot,
    FatArrow,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Lt,
    Le,
    Gt,
    Ge,
    EqEq,
    Ne,
    Eq,

    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }
}

/// Map an identifier to its keyword token, if it is one.
pub fn keyword(ident: &str) -> Option<TokenKind> {
    Some(match ident {
        "define" => TokenKind::Define,
        "let" => TokenKind::Let,
        "var" => TokenKind::Var,
        "while" => TokenKind::While,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "return" => TokenKind::Return,
        "speak" => TokenKind::Speak,
        "listen" => TokenKind::Listen,
        "oracle" => TokenKind::Oracle,
        "summon" => TokenKind::Summon,
        "divine" => TokenKind::Divine,
        "enact" => TokenKind::Enact,
        "fallback" => TokenKind::Fallback,
        "type" => TokenKind::Type,
        "one_of" => TokenKind::OneOf,
        "in" => TokenKind::In,
        "from" => TokenKind::From,
        "using" => TokenKind::Using,
        "with" => TokenKind::With,
        "confidence" => TokenKind::Confidence,
        "requires" => TokenKind::Requires,
        "grant" => TokenKind::Grant,
        "memory" => TokenKind::Memory,
        "within" => TokenKind::Within,
        "familiar" => TokenKind::Familiar,
        "permits" => TokenKind::Permits,
        "and" => TokenKind::And,
        "or" => TokenKind::Or,
        "not" => TokenKind::Not,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        _ => return None,
    })
}
