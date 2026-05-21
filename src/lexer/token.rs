use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub line: usize,
    pub col: usize,
    pub offset: usize,
    pub filename: String,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.filename, self.line, self.col)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start: Location,
    pub end: Location,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    NumberLit(u64),
    FloatLit(f64),
    StringLit(String),
    CharLit(char),

    // Identifiers
    Ident(String),

    // Keywords
    Fn, Let, Mut, Const, Comptime,
    If, Else, While, For, In, Loop,
    Return, Break, Continue,
    Struct, Enum, Impl,
    Match,
    Unsafe, Pin, Asm,
    Pub, Owned, Undefined, As,
    True, False,
    Vec128, Vec256, Vec512,

    // Operators
    Plus, Minus, Star, Slash, Percent,
    Eq, EqEq, NeEq,
    Lt, Gt, Le, Ge,
    AndAnd, OrOr,
    Amp, Pipe, Caret, Tilde,
    Underscore,
    Bang,
    PlusEq, MinusEq,
    Arrow, FatArrow,

    // Punctuation
    Dot, DotDot, DoubleColon,
    Comma, Semi, Colon,
    LParen, RParen,
    LBrace, RBrace,
    LBracket, RBracket,
    At,

    // Special
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::NumberLit(n) => write!(f, "{}", n),
            TokenKind::FloatLit(n) => write!(f, "{}", n),
            TokenKind::StringLit(s) => write!(f, "\"{}\"", s),
            TokenKind::CharLit(c) => write!(f, "'{}'", c),
            TokenKind::Ident(s) => write!(f, "{}", s),
            TokenKind::Eof => write!(f, "EOF"),
            _ => write!(f, "{:?}", self),
        }
    }
}
