//! Tokens. Mundane keywords are un-themed (`fn let var while if else print`,
//! per §7); occult vocabulary is reserved for the genuinely new (`oracle summon
//! divine enact fallback`). Type names `spark`/`glyph` arrive as identifiers and
//! are resolved in type position.

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

    // Mundane keywords (un-themed)
    Fn,
    Let,
    Var,
    While,
    If,
    Else,
    Print,
    Return,

    // Occult keywords (genuinely new)
    Oracle,
    Summon,
    Divine,
    Enact,
    Fallback,

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
        "fn" => TokenKind::Fn,
        "let" => TokenKind::Let,
        "var" => TokenKind::Var,
        "while" => TokenKind::While,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "print" => TokenKind::Print,
        "return" => TokenKind::Return,
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
        "and" => TokenKind::And,
        "or" => TokenKind::Or,
        "not" => TokenKind::Not,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        _ => return None,
    })
}
