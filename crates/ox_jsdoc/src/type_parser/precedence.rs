// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

/// Precedence levels for the Pratt parser.
///
/// Higher numeric value = higher precedence = tighter binding.
/// `PartialOrd` is derived so that `>` comparison works naturally
/// for the Pratt parser's precedence climbing.
///
/// Maps 1:1 to jsdoc-type-pratt-parser's `Precedence` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Precedence {
    /// Lowest precedence — accepts any expression.
    All = 0,
    /// Comma-separated parameter lists.
    ParameterList = 1,
    /// Object literal parsing.
    Object = 2,
    /// Key-value pairs inside objects / function params.
    KeyValue = 3,
    /// Index bracket operations.
    IndexBrackets = 4,
    /// Union types (`A | B`).
    Union = 5,
    /// Intersection types (`A & B`).
    Intersection = 6,
    /// Prefix operators (`?T`, `!T`, `...T`).
    Prefix = 7,
    /// Infix operators (general).
    Infix = 8,
    /// Tuple types (`[A, B]`).
    Tuple = 9,
    /// Symbol types (`Symbol(x)`).
    Symbol = 10,
    /// Optional modifier (`T=`).
    Optional = 11,
    /// Nullable modifier (`?T`, `T?`).
    Nullable = 12,
    /// `keyof` / `typeof` operators.
    KeyOfTypeOf = 13,
    /// Function type parsing.
    Function = 14,
    /// Arrow function (`=>`).
    Arrow = 15,
    /// Array brackets (`T[]`).
    ArrayBrackets = 16,
    /// Generic type parameters (`<T>`).
    Generic = 17,
    /// Name paths (`.`, `~`, `#`).
    NamePath = 18,
    /// Parentheses grouping.
    Parenthesis = 19,
    /// Highest precedence — special types.
    SpecialTypes = 20,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_ordering() {
        assert!(Precedence::SpecialTypes > Precedence::All);
        assert!(Precedence::Generic > Precedence::Union);
        assert!(Precedence::Intersection > Precedence::Union);
        assert!(Precedence::NamePath > Precedence::Generic);
    }

    #[test]
    fn precedence_repr_u8() {
        assert_eq!(Precedence::All as u8, 0);
        assert_eq!(Precedence::SpecialTypes as u8, 20);
    }
}
