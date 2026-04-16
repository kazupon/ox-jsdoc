// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

/// Token kind for JSDoc type expressions.
///
/// Each variant maps to a lexer-recognized symbol, keyword, or literal.
/// The enum is `#[repr(u8)]` to keep `Token` at 12 bytes (Copy-friendly).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TokenKind {
    // --- Punctuation ---
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `|`
    Pipe,
    /// `&`
    Amp,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `;`
    Semicolon,
    /// `,`
    Comma,
    /// `*`
    Star,
    /// `?`
    Question,
    /// `!`
    Bang,
    /// `=`
    Eq,
    /// `:`
    Colon,
    /// `.`
    Dot,
    /// `@`
    At,
    /// `#`
    Hash,
    /// `~`
    Tilde,
    /// `/`
    Slash,

    // --- Multi-character punctuation ---
    /// `=>`
    Arrow,
    /// `...`
    Ellipsis,

    // --- Keywords ---
    /// `null`
    Null,
    /// `undefined`
    Undefined,
    /// `function`
    Function,
    /// `this`
    This,
    /// `new`
    New,
    /// `module`
    Module,
    /// `event`
    Event,
    /// `extends`
    Extends,
    /// `external`
    External,
    /// `typeof`
    Typeof,
    /// `keyof`
    Keyof,
    /// `readonly`
    Readonly,
    /// `import`
    Import,
    /// `infer`
    Infer,
    /// `is`
    Is,
    /// `in`
    In,
    /// `asserts`
    Asserts,
    /// `unique`
    Unique,
    /// `symbol`
    Symbol,

    // --- Literals and identifiers ---
    /// Identifier (e.g. `string`, `MyClass`, `Array`)
    Identifier,
    /// String literal (`"hello"` or `'hello'`)
    StringValue,
    /// Template literal (`` `text${T}` ``)
    TemplateLiteral,
    /// Number literal (`42`, `3.14`, `-1e10`)
    Number,

    // --- Special ---
    /// End of input
    EOF,
}

impl TokenKind {
    /// Returns `true` if this token kind is a keyword that can also be used
    /// as an identifier name (e.g. `keyof` in `keyofFoo` is an identifier).
    #[inline]
    pub fn is_keyword(self) -> bool {
        matches!(
            self,
            Self::Null
                | Self::Undefined
                | Self::Function
                | Self::This
                | Self::New
                | Self::Module
                | Self::Event
                | Self::Extends
                | Self::External
                | Self::Typeof
                | Self::Keyof
                | Self::Readonly
                | Self::Import
                | Self::Infer
                | Self::Is
                | Self::In
                | Self::Asserts
                | Self::Unique
                | Self::Symbol
        )
    }

    /// Returns `true` if this token can appear as a type name in prefix position.
    /// These keywords are also valid identifiers in name contexts.
    #[inline]
    pub fn is_base_name_token(self) -> bool {
        matches!(
            self,
            Self::Module
                | Self::Keyof
                | Self::Event
                | Self::External
                | Self::Readonly
                | Self::Is
                | Self::Typeof
                | Self::In
                | Self::Null
                | Self::Undefined
                | Self::Function
                | Self::Asserts
                | Self::Infer
                | Self::Extends
                | Self::Import
                | Self::Unique
                | Self::Symbol
        )
    }
}

/// A single token produced by the lexer.
///
/// 12 bytes, `Copy`. Designed for register-sized passing.
/// - `start`: absolute byte offset (includes base_offset)
/// - `end`: absolute byte offset (includes base_offset)
/// - `kind`: token kind (1 byte)
/// - 3 bytes padding reserved for future flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// Absolute start byte offset in the source.
    pub start: u32,
    /// Absolute end byte offset in the source.
    pub end: u32,
    /// The kind of this token.
    pub kind: TokenKind,
}

impl Token {
    /// Create a new token.
    #[inline]
    pub fn new(kind: TokenKind, start: u32, end: u32) -> Self {
        Self { start, end, kind }
    }

    /// Create an EOF token at the given offset.
    #[inline]
    pub fn eof(offset: u32) -> Self {
        Self {
            start: offset,
            end: offset,
            kind: TokenKind::EOF,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn token_size_is_12_bytes() {
        assert_eq!(mem::size_of::<Token>(), 12);
    }

    #[test]
    fn token_is_copy() {
        let t = Token::new(TokenKind::Identifier, 0, 5);
        let t2 = t; // Copy
        assert_eq!(t, t2);
    }

    #[test]
    fn eof_token() {
        let t = Token::eof(42);
        assert_eq!(t.kind, TokenKind::EOF);
        assert_eq!(t.start, 42);
        assert_eq!(t.end, 42);
    }
}
