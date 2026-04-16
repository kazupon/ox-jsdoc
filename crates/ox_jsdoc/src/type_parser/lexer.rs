// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use super::token::{Token, TokenKind};

/// Lexer state snapshot for speculative parsing.
///
/// 32 bytes, `Copy`. Saved/restored on the stack with zero heap allocation.
#[derive(Debug, Clone, Copy)]
pub struct LexerState {
    /// Byte offset into the type text (relative to type text start).
    pub offset: usize,
    /// Current token.
    pub current: Token,
    /// Lookahead token.
    pub next: Token,
}

/// Zero-copy lexer for JSDoc type expressions.
///
/// Maintains a 1-token lookahead (`current` + `next`).
/// All token text is borrowed from the source via `start..end` offsets.
pub struct Lexer<'a> {
    /// The type expression source text.
    source: &'a str,
    /// Byte offset of the next character to consume (relative to `source`).
    offset: usize,
    /// Base offset added to all token positions for absolute spans.
    base_offset: u32,
    /// Whether to use loose lexer rules (allows NaN, Infinity, hyphens in identifiers).
    loose: bool,
    /// Current token (already consumed from source).
    pub current: Token,
    /// Lookahead token (next to be consumed).
    pub next: Token,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given type expression text.
    ///
    /// `base_offset` is added to all token positions so spans are absolute
    /// relative to the source file.
    pub fn new(source: &'a str, base_offset: u32, loose: bool) -> Self {
        let mut lexer = Self {
            source,
            offset: 0,
            base_offset,
            loose,
            current: Token::eof(base_offset),
            next: Token::eof(base_offset),
        };
        // Prime the two-token lookahead.
        lexer.current = lexer.read_token();
        lexer.next = lexer.read_token();
        lexer
    }

    /// Save the current lexer state for speculative parsing.
    #[inline]
    pub fn save(&self) -> LexerState {
        LexerState {
            offset: self.offset,
            current: self.current,
            next: self.next,
        }
    }

    /// Restore a previously saved lexer state.
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

    /// Get the text of a token from the source.
    #[inline]
    pub fn token_text(&self, token: Token) -> &'a str {
        let start = (token.start - self.base_offset) as usize;
        let end = (token.end - self.base_offset) as usize;
        &self.source[start..end]
    }

    /// Get the remaining unparsed source text.
    #[inline]
    pub fn remaining(&self) -> &'a str {
        &self.source[self.offset..]
    }

    /// Read the next token from source, advancing `self.offset`.
    fn read_token(&mut self) -> Token {
        self.skip_whitespace();

        if self.offset >= self.source.len() {
            return Token::eof(self.base_offset + self.offset as u32);
        }

        let bytes = self.source.as_bytes();
        let start = self.offset;
        let abs_start = self.base_offset + start as u32;
        let b = bytes[start];

        // Multi-character punctuation first (=> and ...)
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

        // Single-character punctuation
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

        // String literals
        if b == b'"' || b == b'\'' {
            return self.read_string(b, abs_start);
        }

        // Template literals
        if b == b'`' {
            return self.read_template_literal(abs_start);
        }

        // Numbers (including negative)
        if b == b'-' || b.is_ascii_digit() {
            if let Some(token) = self.try_read_number(abs_start) {
                return token;
            }
            // If '-' didn't start a number, fall through to identifier (loose mode)
        }

        // Identifiers and keywords
        if is_ident_start(b) || (self.loose && b == b'-') {
            return self.read_identifier(abs_start);
        }

        // Unknown character — advance to avoid infinite loop
        self.offset += 1;
        Token::new(TokenKind::EOF, abs_start, abs_start + 1)
    }

    /// Skip whitespace characters.
    #[inline]
    fn skip_whitespace(&mut self) {
        let bytes = self.source.as_bytes();
        while self.offset < bytes.len() && bytes[self.offset].is_ascii_whitespace() {
            self.offset += 1;
        }
    }

    /// Peek at the byte at `self.offset + delta` without consuming.
    #[inline]
    fn peek_at(&self, delta: usize) -> Option<u8> {
        self.source.as_bytes().get(self.offset + delta).copied()
    }

    /// Read a quoted string literal (single or double quoted).
    fn read_string(&mut self, quote: u8, abs_start: u32) -> Token {
        self.offset += 1; // skip opening quote
        let bytes = self.source.as_bytes();
        while self.offset < bytes.len() {
            let b = bytes[self.offset];
            if b == b'\\' && self.offset + 1 < bytes.len() {
                self.offset += 2; // skip escaped char
            } else if b == quote {
                self.offset += 1; // skip closing quote
                return Token::new(
                    TokenKind::StringValue,
                    abs_start,
                    self.base_offset + self.offset as u32,
                );
            } else {
                self.offset += 1;
            }
        }
        // Unterminated string — return what we have
        Token::new(
            TokenKind::StringValue,
            abs_start,
            self.base_offset + self.offset as u32,
        )
    }

    /// Read a template literal (backtick-delimited).
    fn read_template_literal(&mut self, abs_start: u32) -> Token {
        self.offset += 1; // skip opening backtick
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
        // Unterminated template literal
        Token::new(
            TokenKind::TemplateLiteral,
            abs_start,
            self.base_offset + self.offset as u32,
        )
    }

    /// Try to read a number literal. Returns `None` if the input doesn't
    /// form a valid number (e.g. a bare `-` followed by a non-digit).
    fn try_read_number(&mut self, abs_start: u32) -> Option<Token> {
        let bytes = self.source.as_bytes();
        let mut pos = self.offset;

        // Optional leading minus
        if pos < bytes.len() && bytes[pos] == b'-' {
            pos += 1;
        }

        // Loose mode: NaN, Infinity
        if self.loose && pos < bytes.len() {
            if bytes[pos..].starts_with(b"NaN")
                && !bytes.get(pos + 3).is_some_and(|&b| is_ident_continue(b))
            {
                self.offset = pos + 3;
                return Some(Token::new(
                    TokenKind::Number,
                    abs_start,
                    self.base_offset + self.offset as u32,
                ));
            }
            if bytes[pos..].starts_with(b"Infinity")
                && !bytes.get(pos + 8).is_some_and(|&b| is_ident_continue(b))
            {
                self.offset = pos + 8;
                return Some(Token::new(
                    TokenKind::Number,
                    abs_start,
                    self.base_offset + self.offset as u32,
                ));
            }
        }

        // Must have at least one digit (or start with `.` followed by digit)
        let has_integer = pos < bytes.len() && bytes[pos].is_ascii_digit();

        if has_integer {
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
        }

        // Decimal point
        if pos < bytes.len() && bytes[pos] == b'.' {
            let next_pos = pos + 1;
            if next_pos < bytes.len() && bytes[next_pos].is_ascii_digit() {
                pos = next_pos + 1;
                while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
            } else if !has_integer {
                return None; // bare `-` or `-` followed by something else
            }
        } else if !has_integer {
            return None;
        }

        // Exponent
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

    /// Read an identifier or keyword token.
    fn read_identifier(&mut self, abs_start: u32) -> Token {
        let bytes = self.source.as_bytes();
        let start = self.offset;

        // In loose mode, hyphens can appear in identifiers
        if self.loose {
            while self.offset < bytes.len()
                && (is_ident_continue(bytes[self.offset]) || bytes[self.offset] == b'-')
            {
                self.offset += 1;
            }
        } else {
            while self.offset < bytes.len() && is_ident_continue(bytes[self.offset]) {
                self.offset += 1;
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
}

/// Returns `true` if `b` can start an identifier (ASCII subset).
#[inline]
fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$'
}

/// Returns `true` if `b` can continue an identifier (ASCII subset).
#[inline]
fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_all(source: &str) -> Vec<Token> {
        let mut lexer = Lexer::new(source, 0, false);
        let mut tokens = Vec::new();
        loop {
            tokens.push(lexer.current);
            if lexer.current.kind == TokenKind::EOF {
                break;
            }
            lexer.bump();
        }
        tokens
    }

    fn lex_all_loose(source: &str) -> Vec<Token> {
        let mut lexer = Lexer::new(source, 0, true);
        let mut tokens = Vec::new();
        loop {
            tokens.push(lexer.current);
            if lexer.current.kind == TokenKind::EOF {
                break;
            }
            lexer.bump();
        }
        tokens
    }

    #[test]
    fn lex_simple_identifier() {
        let tokens = lex_all("string");
        assert_eq!(tokens.len(), 2); // Identifier + EOF
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[0].start, 0);
        assert_eq!(tokens[0].end, 6);
    }

    #[test]
    fn lex_keywords() {
        let tokens = lex_all("null undefined function");
        assert_eq!(tokens[0].kind, TokenKind::Null);
        assert_eq!(tokens[1].kind, TokenKind::Undefined);
        assert_eq!(tokens[2].kind, TokenKind::Function);
    }

    #[test]
    fn lex_keyword_as_identifier_prefix() {
        // "functionBar" should be an Identifier, not Function + Identifier
        let tokens = lex_all("functionBar");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
    }

    #[test]
    fn lex_union_type() {
        let tokens = lex_all("string | number");
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[1].kind, TokenKind::Pipe);
        assert_eq!(tokens[2].kind, TokenKind::Identifier);
    }

    #[test]
    fn lex_generic_type() {
        let tokens = lex_all("Array<string>");
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
        assert_eq!(tokens[1].kind, TokenKind::Lt);
        assert_eq!(tokens[2].kind, TokenKind::Identifier);
        assert_eq!(tokens[3].kind, TokenKind::Gt);
    }

    #[test]
    fn lex_arrow() {
        let tokens = lex_all("(a) => b");
        assert_eq!(tokens[0].kind, TokenKind::LParen);
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
        assert_eq!(tokens[2].kind, TokenKind::RParen);
        assert_eq!(tokens[3].kind, TokenKind::Arrow);
        assert_eq!(tokens[4].kind, TokenKind::Identifier);
    }

    #[test]
    fn lex_ellipsis() {
        let tokens = lex_all("...string");
        assert_eq!(tokens[0].kind, TokenKind::Ellipsis);
        assert_eq!(tokens[1].kind, TokenKind::Identifier);
    }

    #[test]
    fn lex_number_literals() {
        let tokens = lex_all("42 3.14 -1 1e10");
        assert_eq!(tokens[0].kind, TokenKind::Number);
        assert_eq!(tokens[1].kind, TokenKind::Number);
        assert_eq!(tokens[2].kind, TokenKind::Number);
        assert_eq!(tokens[3].kind, TokenKind::Number);
    }

    #[test]
    fn lex_string_literal() {
        let tokens = lex_all("\"hello\"");
        assert_eq!(tokens[0].kind, TokenKind::StringValue);
        assert_eq!(tokens[0].start, 0);
        assert_eq!(tokens[0].end, 7);
    }

    #[test]
    fn lex_template_literal() {
        let tokens = lex_all("`hello`");
        assert_eq!(tokens[0].kind, TokenKind::TemplateLiteral);
    }

    #[test]
    fn lex_base_offset() {
        let lexer = Lexer::new("string", 100, false);
        assert_eq!(lexer.current.start, 100);
        assert_eq!(lexer.current.end, 106);
    }

    #[test]
    fn lex_token_text() {
        let lexer = Lexer::new("Array<string>", 50, false);
        assert_eq!(lexer.token_text(lexer.current), "Array");
    }

    #[test]
    fn lex_save_restore() {
        let mut lexer = Lexer::new("string | number", 0, false);
        let saved = lexer.save();
        assert_eq!(lexer.current.kind, TokenKind::Identifier);
        lexer.bump();
        assert_eq!(lexer.current.kind, TokenKind::Pipe);
        lexer.restore(saved);
        assert_eq!(lexer.current.kind, TokenKind::Identifier);
    }

    #[test]
    fn lex_loose_nan_infinity() {
        let tokens = lex_all_loose("NaN Infinity");
        assert_eq!(tokens[0].kind, TokenKind::Number);
        assert_eq!(tokens[1].kind, TokenKind::Number);
    }

    #[test]
    fn lex_strict_nan_is_identifier() {
        let tokens = lex_all("NaN");
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
    }

    #[test]
    fn lex_loose_hyphen_identifier() {
        let tokens = lex_all_loose("my-type");
        assert_eq!(tokens.len(), 2); // my-type + EOF
        assert_eq!(tokens[0].kind, TokenKind::Identifier);
    }

    #[test]
    fn lex_dot_lt_sequence() {
        // `Array.<string>` — dot followed by `<`
        let tokens = lex_all("Array.<string>");
        assert_eq!(tokens[0].kind, TokenKind::Identifier); // Array
        assert_eq!(tokens[1].kind, TokenKind::Dot); // .
        assert_eq!(tokens[2].kind, TokenKind::Lt); // <
        assert_eq!(tokens[3].kind, TokenKind::Identifier); // string
        assert_eq!(tokens[4].kind, TokenKind::Gt); // >
    }

    #[test]
    fn lex_all_punctuation() {
        let tokens = lex_all("()[]{}|&<>;,*?!=:.@#~/");
        let expected = [
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::Pipe,
            TokenKind::Amp,
            TokenKind::Lt,
            TokenKind::Gt,
            TokenKind::Semicolon,
            TokenKind::Comma,
            TokenKind::Star,
            TokenKind::Question,
            TokenKind::Bang,
            TokenKind::Eq,
            TokenKind::Colon,
            TokenKind::Dot,
            TokenKind::At,
            TokenKind::Hash,
            TokenKind::Tilde,
            TokenKind::Slash,
            TokenKind::EOF,
        ];
        assert_eq!(tokens.len(), expected.len());
        for (i, (token, &exp)) in tokens.iter().zip(expected.iter()).enumerate() {
            assert_eq!(token.kind, exp, "mismatch at position {i}");
        }
    }

    #[test]
    fn lex_empty_input() {
        let tokens = lex_all("");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::EOF);
    }

    #[test]
    fn lex_unique_symbol() {
        let tokens = lex_all("unique symbol");
        assert_eq!(tokens[0].kind, TokenKind::Unique);
        assert_eq!(tokens[1].kind, TokenKind::Symbol);
    }
}
