use crate::lexer::token::{Token, TokenKind, Span, Location};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LexerError {
    #[error("unexpected character '{char}' at {pos}")]
    UnexpectedChar { char: char, pos: Location },

    #[error("unterminated string literal at {pos}")]
    UnterminatedString { pos: Location },

    #[error("invalid numeric literal at {pos}")]
    InvalidNumeric { pos: Location },
}

type Result<T> = std::result::Result<T, LexerError>;

pub fn scan(source: &str, filename: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();
    let mut line: usize = 1;
    let mut col: usize = 1;

    while let Some((offset, ch)) = chars.next() {
        let start = Location {
            line,
            col,
            offset,
            filename: filename.to_string(),
        };

        match ch {
            // Whitespace
            ' ' | '\t' => {
                col += 1;
                continue;
            }
            '\n' => {
                line += 1;
                col = 1;
                continue;
            }
            '\r' => {
                col += 1;
                continue;
            }

            // Comments
            '/' if chars.peek().map(|&(_, c)| c) == Some('/') => {
                // Line comment — skip to end of line
                for (_, c) in chars.by_ref() {
                    col += 1;
                    if c == '\n' {
                        line += 1;
                        col = 1;
                        break;
                    }
                }
                continue;
            }
            '/' if chars.peek().map(|&(_, c)| c) == Some('*') => {
                // Block comment
                chars.next(); // consume '*'
                col += 1;
                let mut depth = 1;
                while let Some((_, c)) = chars.next() {
                    col += 1;
                    if c == '\n' {
                        line += 1;
                        col = 1;
                    }
                    if c == '/' && chars.peek().map(|&(_, c)| c) == Some('*') {
                        chars.next();
                        col += 1;
                        depth += 1;
                    }
                    if c == '*' && chars.peek().map(|&(_, c)| c) == Some('/') {
                        chars.next();
                        col += 1;
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                continue;
            }

            // Strings
            '"' => {
                let mut value = String::new();
                let mut escaped = false;
                let _start_col = col;
                col += 1; // opening quote

                loop {
                    match chars.next() {
                        Some((_, '"')) if !escaped => {
                            col += 1;
                            let end = Location {
                                line,
                                col,
                                offset: offset + value.len() + 2,
                                filename: filename.to_string(),
                            };
                            tokens.push(Token {
                                kind: TokenKind::StringLit(value),
                                span: Span { start, end },
                            });
                            break;
                        }
                        Some((_, '\\')) if !escaped => {
                            escaped = true;
                            col += 1;
                        }
                        Some((_, c)) => {
                            if escaped {
                                match c {
                                    'n' => value.push('\n'),
                                    't' => value.push('\t'),
                                    'r' => value.push('\r'),
                                    '\\' => value.push('\\'),
                                    '"' => value.push('"'),
                                    '0' => value.push('\0'),
                                    _ => {
                                        value.push('\\');
                                        value.push(c);
                                    }
                                }
                                escaped = false;
                            } else {
                                value.push(c);
                            }
                            col += 1;
                        }
                        None => {
                            return Err(LexerError::UnterminatedString { pos: start });
                        }
                    }
                }
                continue;
            }

            // Char literals
            '\'' => {
                match chars.next() {
                    Some((_, '\\')) => {
                        // escaped char
                        match chars.next() {
                            Some((_, c)) => {
                                col += 3; // ' \ c '
                                let end = Location {
                                    line, col, offset,
                                    filename: filename.to_string(),
                                };
                                let ch = match c {
                                    'n' => '\n',
                                    't' => '\t',
                                    'r' => '\r',
                                    '\\' => '\\',
                                    '\'' => '\'',
                                    '0' => '\0',
                                    _ => c,
                                };
                                tokens.push(Token {
                                    kind: TokenKind::CharLit(ch),
                                    span: Span { start, end },
                                });
                            }
                            None => return Err(LexerError::UnterminatedString { pos: start }),
                        }
                    }
                    Some((_, c)) => {
                        match chars.next() {
                            Some((_, '\'')) => {
                                col += 3;
                                let end = Location {
                                    line, col, offset,
                                    filename: filename.to_string(),
                                };
                                tokens.push(Token {
                                    kind: TokenKind::CharLit(c),
                                    span: Span { start, end },
                                });
                            }
                            _ => return Err(LexerError::UnterminatedString { pos: start }),
                        }
                    }
                    None => return Err(LexerError::UnterminatedString { pos: start }),
                }
                continue;
            }

            // Numbers
            '0'..='9' => {
                let mut num = String::new();
                num.push(ch);

                // Check for hex, binary, octal
                if ch == '0' {
                    if let Some(&(_, next)) = chars.peek() {
                        match next {
                            'x' | 'X' => {
                                num.push(chars.next().unwrap().1);
                                col += 1;
                                // hex digits
                                while let Some(&(_, c)) = chars.peek() {
                                    if c.is_ascii_hexdigit() || c == '_' {
                                        num.push(chars.next().unwrap().1);
                                        col += 1;
                                    } else {
                                        break;
                                    }
                                }
                                let end = Location {
                                    line, col, offset: offset + num.len(),
                                    filename: filename.to_string(),
                                };
                                let val = u64::from_str_radix(&num.replace('_', "").trim_start_matches("0x").trim_start_matches("0X"), 16)
                                    .map_err(|_| LexerError::InvalidNumeric { pos: start.clone() })?;
                                tokens.push(Token {
                                    kind: TokenKind::NumberLit(val),
                                    span: Span { start, end },
                                });
                                continue;
                            }
                            'b' | 'B' => {
                                num.push(chars.next().unwrap().1);
                                col += 1;
                                while let Some(&(_, c)) = chars.peek() {
                                    if c == '0' || c == '1' || c == '_'  {
                                        num.push(chars.next().unwrap().1);
                                        col += 1;
                                    } else {
                                        break;
                                    }
                                }
                                let end = Location {
                                    line, col, offset: offset + num.len(),
                                    filename: filename.to_string(),
                                };
                                let val = u64::from_str_radix(&num.replace('_', "").trim_start_matches("0b").trim_start_matches("0B"), 2)
                                    .map_err(|_| LexerError::InvalidNumeric { pos: start.clone() })?;
                                tokens.push(Token {
                                    kind: TokenKind::NumberLit(val),
                                    span: Span { start, end },
                                });
                                continue;
                            }
                            _ => {}
                        }
                    }
                }

                // Decimal (including float)
                let mut is_float = false;
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_ascii_digit() || c == '_' {
                        num.push(chars.next().unwrap().1);
                        col += 1;
                    } else if c == '.' {
                        // Check if next char is digit (not .. or .method)
                        let mut peek2 = chars.clone();
                        peek2.next();
                        if let Some((_, c2)) = peek2.peek() {
                            if c2.is_ascii_digit() {
                                is_float = true;
                                num.push(chars.next().unwrap().1);
                                col += 1;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                let end = Location {
                    line, col, offset: offset + num.len(),
                    filename: filename.to_string(),
                };

                if is_float {
                    let val: f64 = num.replace('_', "").parse()
                        .map_err(|_| LexerError::InvalidNumeric { pos: start.clone() })?;
                    tokens.push(Token {
                        kind: TokenKind::FloatLit(val),
                        span: Span { start, end },
                    });
                } else {
                    let val: u64 = num.replace('_', "").parse()
                        .map_err(|_| LexerError::InvalidNumeric { pos: start.clone() })?;
                    tokens.push(Token {
                        kind: TokenKind::NumberLit(val),
                        span: Span { start, end },
                    });
                }
                continue;
            }

            // Identifiers and keywords
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                ident.push(ch);
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        ident.push(chars.next().unwrap().1);
                        col += 1;
                    } else {
                        break;
                    }
                }

                let end = Location {
                    line, col, offset: offset + ident.len(),
                    filename: filename.to_string(),
                };

                let kind = match ident.as_str() {
                    "_" => TokenKind::Underscore,
                    "fn" => TokenKind::Fn,
                    "let" => TokenKind::Let,
                    "mut" => TokenKind::Mut,
                    "const" => TokenKind::Const,
                    "comptime" => TokenKind::Comptime,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "for" => TokenKind::For,
                    "in" => TokenKind::In,
                    "loop" => TokenKind::Loop,
                    "return" => TokenKind::Return,
                    "break" => TokenKind::Break,
                    "continue" => TokenKind::Continue,
                    "struct" => TokenKind::Struct,
                    "enum" => TokenKind::Enum,
                    "impl" => TokenKind::Impl,
                    "trait" => TokenKind::Trait,
                    "match" => TokenKind::Match,
                    "unsafe" => TokenKind::Unsafe,
                    "pin" => TokenKind::Pin,
                    "asm" => TokenKind::Asm,
                    "pub" => TokenKind::Pub,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    "owned" => TokenKind::Owned,
                    "undefined" => TokenKind::Undefined,
                    "as" => TokenKind::As,
                    "vec128" => TokenKind::Vec128,
                    "vec256" => TokenKind::Vec256,
                    "vec512" => TokenKind::Vec512,
                    _ => TokenKind::Ident(ident),
                };

                tokens.push(Token {
                    kind,
                    span: Span { start, end },
                });
                continue;
            }

            // Operators and punctuation
            ':' if chars.peek().map(|&(_, c)| c) == Some(':') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::DoubleColon, span: Span { start, end } });
            }
            '=' if chars.peek().map(|&(_, c)| c) == Some('=') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::EqEq, span: Span { start, end } });
            }
            '!' if chars.peek().map(|&(_, c)| c) == Some('=') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::NeEq, span: Span { start, end } });
            }
            '<' if chars.peek().map(|&(_, c)| c) == Some('=') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Le, span: Span { start, end } });
            }
            '>' if chars.peek().map(|&(_, c)| c) == Some('=') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Ge, span: Span { start, end } });
            }
            '&' if chars.peek().map(|&(_, c)| c) == Some('&') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::AndAnd, span: Span { start, end } });
            }
            '|' if chars.peek().map(|&(_, c)| c) == Some('|') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::OrOr, span: Span { start, end } });
            }
            '+' if chars.peek().map(|&(_, c)| c) == Some('=') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::PlusEq, span: Span { start, end } });
            }
            '-' if chars.peek().map(|&(_, c)| c) == Some('=') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::MinusEq, span: Span { start, end } });
            }
            '-' if chars.peek().map(|&(_, c)| c) == Some('>') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Arrow, span: Span { start, end } });
            }
            '.' if chars.peek().map(|&(_, c)| c) == Some('.') => {
                chars.next();
                col += 1;
                let end = Location { line, col, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::DotDot, span: Span { start, end } });
            }

            // Single-char tokens
            '+' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Plus, span: Span { start, end } });
                col += 1;
            }
            '-' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Minus, span: Span { start, end } });
                col += 1;
            }
            '*' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Star, span: Span { start, end } });
                col += 1;
            }
            '/' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Slash, span: Span { start, end } });
                col += 1;
            }
            '%' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Percent, span: Span { start, end } });
                col += 1;
            }
            '=' if chars.peek().map(|&(_, c)| c) == Some('>') => {
                chars.next();
                let end = Location { line, col: col + 2, offset: offset + 2, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::FatArrow, span: Span { start, end } });
                col += 2;
            }
            '=' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Eq, span: Span { start, end } });
                col += 1;
            }
            '<' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Lt, span: Span { start, end } });
                col += 1;
            }
            '>' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Gt, span: Span { start, end } });
                col += 1;
            }
            '!' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Bang, span: Span { start, end } });
                col += 1;
            }
            '&' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Amp, span: Span { start, end } });
                col += 1;
            }
            '|' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Pipe, span: Span { start, end } });
                col += 1;
            }
            '^' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Caret, span: Span { start, end } });
                col += 1;
            }
            '~' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Tilde, span: Span { start, end } });
                col += 1;
            }
            '.' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Dot, span: Span { start, end } });
                col += 1;
            }
            ',' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Comma, span: Span { start, end } });
                col += 1;
            }
            ';' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Semi, span: Span { start, end } });
                col += 1;
            }
            ':' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::Colon, span: Span { start, end } });
                col += 1;
            }
            '(' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::LParen, span: Span { start, end } });
                col += 1;
            }
            ')' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::RParen, span: Span { start, end } });
                col += 1;
            }
            '{' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::LBrace, span: Span { start, end } });
                col += 1;
            }
            '}' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::RBrace, span: Span { start, end } });
                col += 1;
            }
            '[' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::LBracket, span: Span { start, end } });
                col += 1;
            }
            ']' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::RBracket, span: Span { start, end } });
                col += 1;
            }
            '@' => {
                let end = Location { line, col: col + 1, offset: offset + 1, filename: filename.to_string() };
                tokens.push(Token { kind: TokenKind::At, span: Span { start, end } });
                col += 1;
            }
            '#' => {
                // Preprocessor / attribute — skip to end of line
                for (_, c) in chars.by_ref() {
                    col += 1;
                    if c == '\n' {
                        line += 1;
                        col = 1;
                        break;
                    }
                }
                continue;
            }

            _ => {
                return Err(LexerError::UnexpectedChar { char: ch, pos: start });
            }
        }
    }

    // EOF token
    let eof_loc = Location {
        line, col, offset: source.len(),
        filename: filename.to_string(),
    };
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span { start: eof_loc.clone(), end: eof_loc },
    });

    Ok(tokens)
}
