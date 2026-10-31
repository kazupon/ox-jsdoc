// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Zero-copy lexer for JSDoc type expressions.
//!
//! Verbatim port of `crates/ox_jsdoc/src/type_parser/lexer.rs`.
//! Maintains a 1-token lookahead (`current` + `next`); all token text is
//! borrowed from the source via `start..end` offsets.

use super::token::{Token, TokenKind};

/// Lexer state snapshot for speculative parsing (32 bytes, `Copy`).
#[derive(Debug, Clone, Copy)]
pub struct LexerState {
    /// Byte offset into the type text (relative to type text start).
    pub offset: usize,
    /// Current token.
    pub current: Token,
    /// Lookahead token.
    pub next: Token,
}

/// Two-token lookahead lexer.
pub struct Lexer<'a> {
    source: &'a str,
    offset: usize,
    base_offset: u32,
    loose: bool,
    /// Current token.
    pub current: Token,
    /// Lookahead token.
    pub next: Token,
}

impl<'a> Lexer<'a> {
    /// Create a fresh lexer over `source`. `base_offset` is added to every
    /// emitted token's start/end so spans become absolute.
    #[must_use]
    pub fn new(source: &'a str, base_offset: u32, loose: bool) -> Self {
        let mut lexer = Self {
            source,
            offset: 0,
            base_offset,
            loose,
            current: Token::eof(base_offset),
            next: Token::eof(base_offset),
        };
        lexer.current = lexer.read_token();
        lexer.next = lexer.read_token();
        lexer
    }

    /// Save the current state for speculative parsing.
    #[inline]
    #[must_use]
    pub fn save(&self) -> LexerState {
        LexerState {
            offset: self.offset,
            current: self.current,
            next: self.next,
        }
    }

    /// Restore a previously saved state.
    #[inline]
    pub fn restore(&mut self, state: LexerState) {
        self.offset = state.offset;
        self.current = state.current;
        self.next = state.next;
    }

    /// Advance the lexer: `current = next`, `next = read_token()`.
    #[inline]
    pub fn bump(&mut self) {
        self.current = self.next;
        self.next = self.read_token();
    }

    /// Borrow the source text slice covered by `token`.
    #[inline]
    #[must_use]
    pub fn token_text(&self, token: Token) -> &'a str {
        let start = (token.start - self.base_offset) as usize;
        let end = (token.end - self.base_offset) as usize;
        &self.source[start..end]
    }

    /// Remaining unparsed source text.
    #[inline]
    #[must_use]
    pub fn remaining(&self) -> &'a str {
        &self.source[self.offset..]
    }

    fn read_token(&mut self) -> Token {
        self.skip_whitespace();
        if self.offset >= self.source.len() {
            return Token::eof(self.base_offset + self.offset as u32);
        }
        let bytes = self.source.as_bytes();
        let start = self.offset;
        let abs_start = self.base_offset + start as u32;
        let b = bytes[start];

        match b {
            b'=' if self.peek_at(1) == Some(b'>') => {
                self.offset += 2;
                return Token::new(TokenKind::Arrow, abs_start, abs_start + 2);
            }
            b'.' if self.peek_at(1) == Some(b'.') && self.peek_at(2) == Some(b'.') => {
                self.offset += 3;
                return Token::new(TokenKind::Ellipsis, abs_start, abs_start + 3);
            }
            _ => {}
        }

        let single = match b {
            b'(' => Some(TokenKind::LParen),
            b')' => Some(TokenKind::RParen),
            b'[' => Some(TokenKind::LBracket),
            b']' => Some(TokenKind::RBracket),
            b'{' => Some(TokenKind::LBrace),
            b'}' => Some(TokenKind::RBrace),
            b'|' => Some(TokenKind::Pipe),
            b'&' => Some(TokenKind::Amp),
            b'<' => Some(TokenKind::Lt),
            b'>' => Some(TokenKind::Gt),
            b';' => Some(TokenKind::Semicolon),
            b',' => Some(TokenKind::Comma),
            b'*' => Some(TokenKind::Star),
            b'?' => Some(TokenKind::Question),
            b'!' => Some(TokenKind::Bang),
            b'=' => Some(TokenKind::Eq),
            b':' => Some(TokenKind::Colon),
            b'.' => Some(TokenKind::Dot),
            b'@' => Some(TokenKind::At),
            b'#' => Some(TokenKind::Hash),
            b'~' => Some(TokenKind::Tilde),
            b'/' => Some(TokenKind::Slash),
            _ => None,
        };
        if let Some(kind) = single {
            self.offset += 1;
            return Token::new(kind, abs_start, abs_start + 1);
        }

        if b == b'"' || b == b'\'' {
            return self.read_string(b, abs_start);
        }
        if b == b'`' {
            return self.read_template_literal(abs_start);
        }
        if b == b'-' || b.is_ascii_digit() {
            if let Some(token) = self.try_read_number(abs_start) {
                return token;
            }
        }
        if is_ident_start(b) || (self.loose && b == b'-') {
            return self.read_identifier(abs_start);
        }
        if !b.is_ascii() {
            return self.read_non_ascii(abs_start);
        }

        self.offset += 1;
        Token::new(TokenKind::Unknown, abs_start, abs_start + 1)
    }

    /// Read a token that starts with a non-ASCII character: an identifier if
    /// the character is `ID_Start`, otherwise an [`TokenKind::Unknown`] token
    /// for the character. Kept out of line so that the ASCII path stays small.
    #[cold]
    #[inline(never)]
    fn read_non_ascii(&mut self, abs_start: u32) -> Token {
        let ch = self.char_at(self.offset);
        if unicode_id_start::is_id_start_unicode(ch) {
            return self.read_identifier(abs_start);
        }
        self.offset += ch.len_utf8();
        Token::new(
            TokenKind::Unknown,
            abs_start,
            self.base_offset + self.offset as u32,
        )
    }

    /// Skip whitespace as `\s` does in JavaScript (what jsdoc-type-pratt-parser
    /// skips), including the non-ASCII spaces such as NBSP.
    #[inline]
    fn skip_whitespace(&mut self) {
        let bytes = self.source.as_bytes();
        while let Some(&b) = bytes.get(self.offset) {
            if is_ascii_whitespace(b) {
                self.offset += 1;
            } else if b.is_ascii() {
                break;
            } else {
                let len = self.non_ascii_whitespace_len();
                if len == 0 {
                    break;
                }
                self.offset += len;
            }
        }
    }

    /// The byte length of the character at the current offset if it is
    /// non-ASCII whitespace, or 0.
    #[cold]
    #[inline(never)]
    fn non_ascii_whitespace_len(&self) -> usize {
        let ch = self.char_at(self.offset);
        if is_non_ascii_whitespace(ch) {
            ch.len_utf8()
        } else {
            0
        }
    }

    #[inline]
    fn peek_at(&self, delta: usize) -> Option<u8> {
        self.source.as_bytes().get(self.offset + delta).copied()
    }

    /// The character that starts at byte `offset`, which must be a character
    /// boundary.
    #[inline]
    fn char_at(&self, offset: usize) -> char {
        self.source[offset..].chars().next().unwrap_or('\0')
    }

    /// Whether the character at byte `offset` can continue an identifier.
    #[inline]
    fn continues_identifier_at(&self, offset: usize) -> bool {
        match self.source.as_bytes().get(offset) {
            Some(&b) if b.is_ascii() => is_ident_continue(b),
            Some(_) => is_non_ascii_ident_continue(self.char_at(offset)),
            None => false,
        }
    }

    /// Read a quoted string. A string that is not closed becomes an
    /// [`TokenKind::Unknown`] token up to the end of the text.
    fn read_string(&mut self, quote: u8, abs_start: u32) -> Token {
        self.offset += 1;
        let bytes = self.source.as_bytes();
        while self.offset < bytes.len() {
            let b = bytes[self.offset];
            if b == b'\\' && self.offset + 1 < bytes.len() {
                self.offset += 2;
            } else if b == quote {
                self.offset += 1;
                return Token::new(
                    TokenKind::StringValue,
                    abs_start,
                    self.base_offset + self.offset as u32,
                );
            } else {
                self.offset += 1;
            }
        }
        self.offset = bytes.len();
        Token::new(
            TokenKind::Unknown,
            abs_start,
            self.base_offset + self.offset as u32,
        )
    }

    /// Read a template literal. One that is not closed becomes an
    /// [`TokenKind::Unknown`] token up to the end of the text.
    fn read_template_literal(&mut self, abs_start: u32) -> Token {
        self.offset += 1;
        let bytes = self.source.as_bytes();
        while self.offset < bytes.len() {
            let b = bytes[self.offset];
            if b == b'\\' && self.offset + 1 < bytes.len() {
                self.offset += 2;
            } else if b == b'`' {
                self.offset += 1;
                return Token::new(
                    TokenKind::TemplateLiteral,
                    abs_start,
                    self.base_offset + self.offset as u32,
                );
            } else {
                self.offset += 1;
            }
        }
        self.offset = bytes.len();
        Token::new(
            TokenKind::Unknown,
            abs_start,
            self.base_offset + self.offset as u32,
        )
    }

    fn try_read_number(&mut self, abs_start: u32) -> Option<Token> {
        let bytes = self.source.as_bytes();
        let mut pos = self.offset;
        if pos < bytes.len() && bytes[pos] == b'-' {
            pos += 1;
        }
        if self.loose && pos < bytes.len() {
            if bytes[pos..].starts_with(b"NaN") && !self.continues_identifier_at(pos + 3) {
                self.offset = pos + 3;
                return Some(Token::new(
                    TokenKind::Number,
                    abs_start,
                    self.base_offset + self.offset as u32,
                ));
            }
            if bytes[pos..].starts_with(b"Infinity") && !self.continues_identifier_at(pos + 8) {
                self.offset = pos + 8;
                return Some(Token::new(
                    TokenKind::Number,
                    abs_start,
                    self.base_offset + self.offset as u32,
                ));
            }
        }
        let has_integer = pos < bytes.len() && bytes[pos].is_ascii_digit();
        if has_integer {
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
            // As in jsdoc-type-pratt-parser, digits right before an `n` are
            // a BigInt (`1n`, `-1n`). `1.5n` stays a number and a name.
            if bytes.get(pos) == Some(&b'n') {
                self.offset = pos + 1;
                return Some(Token::new(
                    TokenKind::BigInt,
                    abs_start,
                    self.base_offset + self.offset as u32,
                ));
            }
        }
        if pos < bytes.len() && bytes[pos] == b'.' {
            let next_pos = pos + 1;
            if next_pos < bytes.len() && bytes[next_pos].is_ascii_digit() {
                pos = next_pos + 1;
                while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
            } else if !has_integer {
                return None;
            }
        } else if !has_integer {
            return None;
        }
        if pos < bytes.len() && (bytes[pos] == b'e' || bytes[pos] == b'E') {
            let mut exp_pos = pos + 1;
            if exp_pos < bytes.len() && (bytes[exp_pos] == b'+' || bytes[exp_pos] == b'-') {
                exp_pos += 1;
            }
            if exp_pos < bytes.len() && bytes[exp_pos].is_ascii_digit() {
                pos = exp_pos + 1;
                while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
            }
        }
        if pos == self.offset || (pos == self.offset + 1 && bytes[self.offset] == b'-') {
            return None;
        }
        self.offset = pos;
        Some(Token::new(
            TokenKind::Number,
            abs_start,
            self.base_offset + pos as u32,
        ))
    }

    /// Read an identifier: ASCII is checked byte by byte, other characters
    /// by the Unicode `ID_Continue` property (plus ZWNJ and ZWJ), as
    /// jsdoc-type-pratt-parser does. The loose lexer also takes `-`.
    fn read_identifier(&mut self, abs_start: u32) -> Token {
        let bytes = self.source.as_bytes();
        let start = self.offset;
        let loose = self.loose;
        while let Some(&b) = bytes.get(self.offset) {
            if is_ident_continue(b) || (loose && b == b'-') {
                self.offset += 1;
            } else if b.is_ascii() {
                break;
            } else {
                let len = self.non_ascii_ident_continue_len();
                if len == 0 {
                    break;
                }
                self.offset += len;
            }
        }
        let text = &self.source[start..self.offset];
        let abs_end = self.base_offset + self.offset as u32;
        let kind = match text {
            "null" => TokenKind::Null,
            "undefined" => TokenKind::Undefined,
            "function" => TokenKind::Function,
            "this" => TokenKind::This,
            "new" => TokenKind::New,
            "module" => TokenKind::Module,
            "event" => TokenKind::Event,
            "extends" => TokenKind::Extends,
            "external" => TokenKind::External,
            "typeof" => TokenKind::Typeof,
            "keyof" => TokenKind::Keyof,
            "readonly" => TokenKind::Readonly,
            "import" => TokenKind::Import,
            "infer" => TokenKind::Infer,
            "is" => TokenKind::Is,
            "in" => TokenKind::In,
            "asserts" => TokenKind::Asserts,
            "unique" => TokenKind::Unique,
            "symbol" => TokenKind::Symbol,
            "NaN" | "Infinity" if self.loose => TokenKind::Number,
            _ => TokenKind::Identifier,
        };
        Token::new(kind, abs_start, abs_end)
    }

    /// The byte length of the character at the current offset if it is a
    /// non-ASCII character that continues an identifier, or 0.
    #[cold]
    #[inline(never)]
    fn non_ascii_ident_continue_len(&self) -> usize {
        let ch = self.char_at(self.offset);
        if is_non_ascii_ident_continue(ch) {
            ch.len_utf8()
        } else {
            0
        }
    }
}

#[inline]
fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$'
}

#[inline]
fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// The ASCII characters that `\s` matches in JavaScript: tab, line feed,
/// vertical tab, form feed, carriage return and space.
#[inline]
fn is_ascii_whitespace(b: u8) -> bool {
    matches!(b, b'\t'..=b'\r' | b' ')
}

/// `ID_Continue`, ZWNJ (U+200C) or ZWJ (U+200D), for a non-ASCII character.
#[inline]
fn is_non_ascii_ident_continue(ch: char) -> bool {
    unicode_id_start::is_id_continue_unicode(ch) || ch == '\u{200C}' || ch == '\u{200D}'
}

/// The non-ASCII characters that `\s` matches in JavaScript.
#[inline]
fn is_non_ascii_whitespace(ch: char) -> bool {
    matches!(ch, '\u{2000}'..='\u{200A}')
        || matches!(
            ch,
            '\u{00A0}'
                | '\u{1680}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
        )
}
