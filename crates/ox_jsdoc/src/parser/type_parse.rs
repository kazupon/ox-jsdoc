// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Type expression parsing methods on `ParserContext`.
//!
//! This file distributes `impl ParserContext` into a separate file for type
//! parsing logic. The Rust compiler treats this identically to having all
//! methods in `context.rs` — the compilation unit is the crate.

use oxc_allocator::{Box as ArenaBox, Vec as ArenaVec};
use oxc_span::Span;

use crate::type_parser::{
    ast::*,
    lexer::Lexer,
    precedence::Precedence,
    token::TokenKind,
};

use super::{
    context::ParserContext,
    diagnostics::{TypeDiagnosticKind, type_diagnostic},
};

impl<'a> ParserContext<'a> {
    /// Parse `{type}` text into a type expression AST.
    ///
    /// Lexer is created on the stack. `allocator` and `diagnostics` use `self`.
    /// Returns `None` if parsing fails (diagnostics are pushed to `self`).
    pub(crate) fn parse_type_expression(
        &mut self,
        type_text: &'a str,
        type_base_offset: u32,
        mode: ParseMode,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let mut lexer = Lexer::new(type_text, type_base_offset, mode.is_loose());
        let mut disallow_conditional = false;
        let result = self.parse_type_pratt(&mut lexer, mode, &mut disallow_conditional, Precedence::All)?;

        // Ensure we consumed everything
        if lexer.current.kind != TokenKind::EOF {
            self.diagnostics.push(type_diagnostic(TypeDiagnosticKind::EarlyEndOfParse));
            return None;
        }

        Some(result)
    }

    // ========================================================================
    // Pratt parser core loop
    // ========================================================================

    /// Core Pratt parser loop.
    ///
    /// 1. Parse prefix (nud) via `match` jump table
    /// 2. While infix precedence > `min_precedence`, parse infix (led)
    fn parse_type_pratt(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        min_precedence: Precedence,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        // Step 1: prefix parse
        let mut left = self.parse_prefix_type(lexer, mode, disallow_conditional)?;

        // Step 2: infix loop
        loop {
            // When inside a conditional extends clause, `?` is the conditional
            // delimiter, not a nullable suffix.
            if *disallow_conditional && lexer.current.kind == TokenKind::Question {
                break;
            }
            let infix_prec = self.cur_infix_precedence(lexer, mode);
            if infix_prec <= min_precedence {
                break;
            }
            left = self.parse_infix_type(lexer, mode, disallow_conditional, left)?;
        }

        Some(left)
    }

    // ========================================================================
    // Prefix parse — match jump table for O(1) dispatch
    // ========================================================================

    /// Parse a prefix (nud) type expression.
    fn parse_prefix_type(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        match lexer.current.kind {
            // Identifier — type name
            TokenKind::Identifier | TokenKind::This | TokenKind::New => {
                self.parse_name(lexer, mode)
            }

            // Keywords that can be names depending on mode
            TokenKind::Keyof if !mode.is_typescript() => self.parse_name(lexer, mode),
            TokenKind::Event | TokenKind::External | TokenKind::In if mode.is_typescript() || mode.is_closure() => {
                self.parse_name(lexer, mode)
            }

            // `null`
            TokenKind::Null => {
                let token = lexer.current;
                lexer.bump();
                Some(ArenaBox::new_in(TypeNode::Null(TypeNull {
                    span: Span::new(token.start, token.end),
                }), self.allocator))
            }

            // `undefined`
            TokenKind::Undefined => {
                let token = lexer.current;
                lexer.bump();
                Some(ArenaBox::new_in(TypeNode::Undefined(TypeUndefined {
                    span: Span::new(token.start, token.end),
                }), self.allocator))
            }

            // `*` — any type
            TokenKind::Star => {
                let token = lexer.current;
                lexer.bump();
                Some(ArenaBox::new_in(TypeNode::Any(TypeAny {
                    span: Span::new(token.start, token.end),
                }), self.allocator))
            }

            // `?` — unknown type (prefix) or nullable
            TokenKind::Question => {
                self.parse_nullable_prefix(lexer, mode, disallow_conditional)
            }

            // `!` — not nullable (prefix)
            TokenKind::Bang => {
                self.parse_not_nullable_prefix(lexer, mode, disallow_conditional)
            }

            // `=` — optional prefix (jsdoc mode)
            TokenKind::Eq if mode.is_jsdoc() || mode.is_closure() => {
                self.parse_optional_prefix(lexer, mode, disallow_conditional)
            }

            // `...` — variadic prefix
            TokenKind::Ellipsis => {
                self.parse_variadic_prefix(lexer, mode, disallow_conditional)
            }

            // `(` — parenthesized type or arrow function
            TokenKind::LParen => {
                self.parse_parenthesis_or_function(lexer, mode, disallow_conditional)
            }

            // `[` — tuple
            TokenKind::LBracket if mode.is_typescript() => {
                self.parse_tuple(lexer, mode, disallow_conditional)
            }

            // `{` — object type
            TokenKind::LBrace => {
                self.parse_object_type(lexer, mode, disallow_conditional)
            }

            // `function` — function type
            TokenKind::Function => {
                self.parse_function_type(lexer, mode, disallow_conditional)
            }

            // `typeof`
            TokenKind::Typeof if mode.is_typescript() || mode.is_closure() => {
                self.parse_typeof(lexer, mode, disallow_conditional)
            }

            // `keyof`
            TokenKind::Keyof if mode.is_typescript() => {
                self.parse_keyof(lexer, mode, disallow_conditional)
            }

            // `readonly` — readonly array
            TokenKind::Readonly if mode.is_typescript() => {
                self.parse_readonly_array(lexer, mode, disallow_conditional)
            }

            // `import` — import type
            TokenKind::Import if mode.is_typescript() => {
                self.parse_import_type(lexer, mode, disallow_conditional)
            }

            // `infer` — infer type
            TokenKind::Infer if mode.is_typescript() => {
                self.parse_infer(lexer, mode)
            }

            // `asserts` — asserts type
            TokenKind::Asserts if mode.is_typescript() => {
                self.parse_asserts(lexer, mode, disallow_conditional)
            }

            // `unique` — unique symbol
            TokenKind::Unique if mode.is_typescript() => {
                self.parse_unique_symbol(lexer)
            }

            // Number literal
            TokenKind::Number => {
                self.parse_number_literal(lexer)
            }

            // String literal
            TokenKind::StringValue => {
                self.parse_string_literal(lexer)
            }

            // Template literal
            TokenKind::TemplateLiteral if mode.is_typescript() => {
                self.parse_template_literal(lexer, mode, disallow_conditional)
            }

            // `module`, `event`, `external` — special name path
            TokenKind::Module => {
                self.parse_special_name_path_or_name(lexer, mode, SpecialPathType::Module)
            }
            TokenKind::Event if mode.is_jsdoc() => {
                self.parse_special_name_path_or_name(lexer, mode, SpecialPathType::Event)
            }
            TokenKind::External if mode.is_jsdoc() => {
                self.parse_special_name_path_or_name(lexer, mode, SpecialPathType::External)
            }

            // Symbol (jsdoc/closure) — handled when Identifier followed by `(`
            TokenKind::Symbol if mode.is_jsdoc() || mode.is_closure() => {
                // `symbol` keyword in jsdoc/closure is a name
                self.parse_name(lexer, mode)
            }

            // Other keywords that can appear as names
            TokenKind::Extends | TokenKind::Is | TokenKind::In
            | TokenKind::Readonly | TokenKind::Event | TokenKind::External => {
                self.parse_name(lexer, mode)
            }

            _ => {
                self.diagnostics.push(type_diagnostic(TypeDiagnosticKind::NoParsletFound));
                None
            }
        }
    }

    // ========================================================================
    // Infix precedence lookup — match jump table
    // ========================================================================

    /// Determine the infix precedence of the current token.
    #[inline]
    fn cur_infix_precedence(&self, lexer: &Lexer<'a>, mode: ParseMode) -> Precedence {
        match lexer.current.kind {
            TokenKind::Pipe => Precedence::Union,
            TokenKind::Amp if mode.is_typescript() => Precedence::Intersection,
            TokenKind::Question => Precedence::Nullable,
            TokenKind::Eq => Precedence::Optional,
            TokenKind::LBracket => Precedence::ArrayBrackets,
            TokenKind::Lt => Precedence::Generic,
            TokenKind::Dot if lexer.next.kind == TokenKind::Lt => Precedence::Generic,
            TokenKind::Dot | TokenKind::Hash | TokenKind::Tilde => Precedence::NamePath,
            TokenKind::Arrow => Precedence::Arrow,
            TokenKind::Is if mode.is_typescript() => Precedence::Infix,
            TokenKind::Extends if mode.is_typescript() => Precedence::Infix,
            TokenKind::Ellipsis if mode.is_jsdoc() => Precedence::Infix, // postfix variadic
            TokenKind::LParen if (mode.is_jsdoc() || mode.is_closure()) => Precedence::Symbol, // symbol(x)
            _ => Precedence::All, // no infix
        }
    }

    // ========================================================================
    // Infix parse — match jump table
    // ========================================================================

    /// Parse an infix (led) type expression.
    fn parse_infix_type(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        match lexer.current.kind {
            TokenKind::Pipe => self.parse_union(lexer, mode, disallow_conditional, left),
            TokenKind::Amp if mode.is_typescript() => self.parse_intersection(lexer, mode, disallow_conditional, left),
            TokenKind::Lt => self.parse_generic(lexer, mode, disallow_conditional, left),
            TokenKind::Dot if lexer.next.kind == TokenKind::Lt => self.parse_generic(lexer, mode, disallow_conditional, left),
            TokenKind::LBracket => self.parse_array_brackets_or_indexed(lexer, mode, disallow_conditional, left),
            TokenKind::Dot | TokenKind::Hash | TokenKind::Tilde => self.parse_name_path(lexer, mode, left),
            TokenKind::Question => self.parse_nullable_suffix(lexer, left),
            TokenKind::Eq => self.parse_optional_suffix(lexer, left),
            TokenKind::Arrow => self.parse_arrow_function(lexer, mode, disallow_conditional, left),
            TokenKind::Is if mode.is_typescript() => self.parse_predicate(lexer, mode, disallow_conditional, left),
            TokenKind::Extends if mode.is_typescript() => self.parse_conditional(lexer, mode, disallow_conditional, left),
            TokenKind::Ellipsis if mode.is_jsdoc() => self.parse_variadic_suffix(lexer, left),
            TokenKind::LParen if (mode.is_jsdoc() || mode.is_closure()) => self.parse_symbol(lexer, mode, disallow_conditional, left),
            _ => Some(left), // no infix match — return left as is
        }
    }

    // ========================================================================
    // Speculative parsing (checkpoint)
    // ========================================================================

    /// Try a parse function speculatively. On failure, restore lexer and diagnostics.
    fn try_parse_type<F, T>(
        &mut self,
        lexer: &mut Lexer<'a>,
        f: F,
    ) -> Option<T>
    where
        F: FnOnce(&mut Self, &mut Lexer<'a>) -> Option<T>,
    {
        let saved_lexer = lexer.save();
        let saved_diag_len = self.diagnostics.len();
        let result = f(self, lexer);
        if result.is_none() {
            lexer.restore(saved_lexer);
            self.diagnostics.truncate(saved_diag_len);
        }
        result
    }

    // ========================================================================
    // Helper methods
    // ========================================================================

    /// Consume the current token if it matches the expected kind.
    /// Returns `true` if consumed.
    #[inline]
    fn eat(&self, lexer: &mut Lexer<'a>, kind: TokenKind) -> bool {
        if lexer.current.kind == kind {
            lexer.bump();
            true
        } else {
            false
        }
    }

    /// Expect and consume the current token. Push diagnostic on mismatch.
    #[inline]
    fn expect(&mut self, lexer: &mut Lexer<'a>, kind: TokenKind) -> bool {
        if lexer.current.kind == kind {
            lexer.bump();
            true
        } else {
            self.diagnostics.push(type_diagnostic(TypeDiagnosticKind::ExpectedToken));
            false
        }
    }

    // ========================================================================
    // Prefix parse implementations
    // ========================================================================

    /// Parse a name (identifier or keyword-as-name).
    fn parse_name(
        &mut self,
        lexer: &mut Lexer<'a>,
        _mode: ParseMode,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let token = lexer.current;
        let text = lexer.token_text(token);
        lexer.bump();
        Some(ArenaBox::new_in(TypeNode::Name(TypeName {
            span: Span::new(token.start, token.end),
            value: text,
        }), self.allocator))
    }

    /// Parse `?T` (nullable prefix) or `?` (unknown type).
    fn parse_nullable_prefix(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `?`

        // Standalone `?` = unknown type (when followed by nothing parseable as prefix)
        match lexer.current.kind {
            TokenKind::EOF | TokenKind::Pipe | TokenKind::Comma | TokenKind::RParen
            | TokenKind::RBracket | TokenKind::RBrace | TokenKind::Gt | TokenKind::Eq => {
                return Some(ArenaBox::new_in(TypeNode::Unknown(TypeUnknown {
                    span: Span::new(start, start + 1),
                }), self.allocator));
            }
            _ => {}
        }

        let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::Nullable)?;
        let end = self.node_end(&element);
        Some(ArenaBox::new_in(TypeNode::Nullable(TypeNullable {
            span: Span::new(start, end),
            element,
            position: ModifierPosition::Prefix,
        }), self.allocator))
    }

    /// Parse `!T` (not-nullable prefix).
    fn parse_not_nullable_prefix(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `!`
        let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::Prefix)?;
        let end = self.node_end(&element);
        Some(ArenaBox::new_in(TypeNode::NotNullable(TypeNotNullable {
            span: Span::new(start, end),
            element,
            position: ModifierPosition::Prefix,
        }), self.allocator))
    }

    /// Parse `=T` (optional prefix).
    fn parse_optional_prefix(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `=`
        let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::Optional)?;
        let end = self.node_end(&element);
        Some(ArenaBox::new_in(TypeNode::Optional(TypeOptional {
            span: Span::new(start, end),
            element,
            position: ModifierPosition::Prefix,
        }), self.allocator))
    }

    /// Parse `...T` (variadic prefix).
    fn parse_variadic_prefix(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `...`

        // Bare `...` with no following type
        if matches!(lexer.current.kind, TokenKind::EOF | TokenKind::Comma | TokenKind::RParen | TokenKind::RBracket) {
            return Some(ArenaBox::new_in(TypeNode::Variadic(TypeVariadic {
                span: Span::new(start, start + 3),
                element: None,
                position: None,
                square_brackets: false,
            }), self.allocator));
        }

        // `...[T]` — variadic with enclosing brackets (jsdoc mode)
        if mode.is_jsdoc() && lexer.current.kind == TokenKind::LBracket {
            lexer.bump(); // consume `[`
            let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
            if !self.expect(lexer, TokenKind::RBracket) {
                return None;
            }
            let end = lexer.current.start; // position after `]`
            return Some(ArenaBox::new_in(TypeNode::Variadic(TypeVariadic {
                span: Span::new(start, end),
                element: Some(element),
                position: Some(VariadicPosition::Prefix),
                square_brackets: true,
            }), self.allocator));
        }

        let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::Prefix)?;
        let end = self.node_end(&element);
        Some(ArenaBox::new_in(TypeNode::Variadic(TypeVariadic {
            span: Span::new(start, end),
            element: Some(element),
            position: Some(VariadicPosition::Prefix),
            square_brackets: false,
        }), self.allocator))
    }

    /// Parse `(T)` parenthesized type, or `(a: T) => U` arrow function.
    fn parse_parenthesis_or_function(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;

        // Try arrow function first: `(params) =>`
        if mode.is_typescript() {
            if let Some(result) = self.try_parse_type(lexer, |this, lex| {
                this.try_parse_arrow_function_prefix(lex, mode, disallow_conditional, start)
            }) {
                return Some(result);
            }
        }

        // Parenthesized type: `(T)`
        lexer.bump(); // consume `(`
        let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
        if !self.expect(lexer, TokenKind::RParen) {
            return None;
        }
        let end = lexer.current.start;
        Some(ArenaBox::new_in(TypeNode::Parenthesis(TypeParenthesis {
            span: Span::new(start, end),
            element,
        }), self.allocator))
    }

    /// Try to parse an arrow function prefix: `(a: T, b: U) => ReturnType`.
    fn try_parse_arrow_function_prefix(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        start: u32,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        lexer.bump(); // consume `(`

        let mut parameters = ArenaVec::new_in(self.allocator);

        // Parse parameter list
        if lexer.current.kind != TokenKind::RParen {
            loop {
                let param = self.parse_key_value_or_type(lexer, mode, disallow_conditional)?;
                parameters.push(param);
                if !self.eat(lexer, TokenKind::Comma) {
                    break;
                }
            }
        }

        if !self.eat(lexer, TokenKind::RParen) {
            return None;
        }

        // Must have `=>`
        if lexer.current.kind != TokenKind::Arrow {
            return None;
        }
        lexer.bump(); // consume `=>`

        let return_type = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
        let end = self.node_end(&return_type);

        Some(ArenaBox::new_in(TypeNode::Function(TypeFunction {
            span: Span::new(start, end),
            parameters,
            return_type: Some(return_type),
            type_parameters: ArenaVec::new_in(self.allocator),
            constructor: false,
            arrow: true,
            parenthesis: true,
        }), self.allocator))
    }

    /// Parse a key-value pair or plain type (for function params).
    fn parse_key_value_or_type(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        // Try key: value pattern
        if (lexer.current.kind == TokenKind::Identifier
            || lexer.current.kind == TokenKind::This
            || lexer.current.kind == TokenKind::New
            || lexer.current.kind.is_keyword())
            && lexer.next.kind == TokenKind::Colon
        {
            return self.try_parse_type(lexer, |this, lex| {
                this.parse_key_value(lex, mode, disallow_conditional)
            }).or_else(|| self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::ParameterList));
        }

        // Variadic parameter: `...T`
        if lexer.current.kind == TokenKind::Ellipsis {
            let start = lexer.current.start;
            lexer.bump();

            // Check for `...name: Type` pattern
            if (lexer.current.kind == TokenKind::Identifier || lexer.current.kind.is_keyword())
                && lexer.next.kind == TokenKind::Colon
            {
                let key_token = lexer.current;
                let key = lexer.token_text(key_token);
                lexer.bump(); // consume key
                lexer.bump(); // consume `:`
                let right = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::ParameterList)?;
                let end = self.node_end(&right);
                return Some(ArenaBox::new_in(TypeNode::KeyValue(TypeKeyValue {
                    span: Span::new(start, end),
                    key,
                    right: Some(right),
                    optional: false,
                    variadic: true,
                }), self.allocator));
            }

            let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::Prefix)?;
            let end = self.node_end(&element);
            return Some(ArenaBox::new_in(TypeNode::Variadic(TypeVariadic {
                span: Span::new(start, end),
                element: Some(element),
                position: Some(VariadicPosition::Prefix),
                square_brackets: false,
            }), self.allocator));
        }

        self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::ParameterList)
    }

    /// Parse a key-value pair: `name: Type`, `name?: Type`.
    fn parse_key_value(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        let key_token = lexer.current;
        let key = lexer.token_text(key_token);
        lexer.bump(); // consume key

        // Optional `?` before `:`
        let optional = self.eat(lexer, TokenKind::Question);

        if !self.eat(lexer, TokenKind::Colon) {
            return None;
        }

        let right = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::KeyValue)?;
        let end = self.node_end(&right);

        Some(ArenaBox::new_in(TypeNode::KeyValue(TypeKeyValue {
            span: Span::new(start, end),
            key,
            right: Some(right),
            optional,
            variadic: false,
        }), self.allocator))
    }

    /// Parse `[A, B, C]` tuple type.
    fn parse_tuple(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `[`

        let mut elements = ArenaVec::new_in(self.allocator);

        if lexer.current.kind != TokenKind::RBracket {
            loop {
                let element = self.parse_key_value_or_type(lexer, mode, disallow_conditional)?;
                elements.push(element);
                if !self.eat(lexer, TokenKind::Comma) {
                    break;
                }
            }
        }

        if !self.expect(lexer, TokenKind::RBracket) {
            self.diagnostics.push(type_diagnostic(TypeDiagnosticKind::UnclosedTuple));
            return None;
        }

        let end = lexer.current.start;
        Some(ArenaBox::new_in(TypeNode::Tuple(TypeTuple {
            span: Span::new(start, end),
            elements,
        }), self.allocator))
    }

    /// Parse `{key: Type, ...}` object type.
    fn parse_object_type(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `{`

        let mut elements = ArenaVec::new_in(self.allocator);
        let mut separator = None;

        if lexer.current.kind != TokenKind::RBrace {
            loop {
                let field = self.parse_object_field(lexer, mode, disallow_conditional)?;
                elements.push(field);

                if self.eat(lexer, TokenKind::Comma) {
                    separator = Some(ObjectSeparator::Comma);
                } else if self.eat(lexer, TokenKind::Semicolon) {
                    separator = Some(ObjectSeparator::Semicolon);
                } else {
                    break;
                }
            }
        }

        if !self.expect(lexer, TokenKind::RBrace) {
            self.diagnostics.push(type_diagnostic(TypeDiagnosticKind::UnclosedObject));
            return None;
        }

        let end = lexer.current.start;
        Some(ArenaBox::new_in(TypeNode::Object(TypeObject {
            span: Span::new(start, end),
            elements,
            separator,
        }), self.allocator))
    }

    /// Parse a single object field.
    fn parse_object_field(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;

        // `readonly` prefix
        let readonly = lexer.current.kind == TokenKind::Readonly
            && lexer.next.kind != TokenKind::Colon
            && lexer.next.kind != TokenKind::Comma
            && lexer.next.kind != TokenKind::Semicolon
            && lexer.next.kind != TokenKind::RBrace;
        if readonly {
            lexer.bump(); // consume `readonly`
        }

        // Index signature: `[key: Type]: ValueType`
        if lexer.current.kind == TokenKind::LBracket {
            return self.parse_index_signature_or_mapped(lexer, mode, disallow_conditional, start, readonly);
        }

        // Method signature, computed property, etc. for typescript
        // For now handle basic `key: Type` and `key?: Type`
        let quote = match lexer.current.kind {
            TokenKind::StringValue => {
                let q = if lexer.token_text(lexer.current).starts_with('"') {
                    Some(QuoteStyle::Double)
                } else {
                    Some(QuoteStyle::Single)
                };
                q
            }
            _ => None,
        };

        // In jsdoc mode with allowKeyTypes, the key can be a full type expression
        if mode.is_jsdoc() && !matches!(lexer.next.kind, TokenKind::Colon | TokenKind::Question) {
            let left = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::KeyValue)?;
            if self.eat(lexer, TokenKind::Colon) {
                let right = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::KeyValue)?;
                let end = self.node_end(&right);
                return Some(ArenaBox::new_in(TypeNode::JsdocObjectField(TypeJsdocObjectField {
                    span: Span::new(start, end),
                    left,
                    right,
                }), self.allocator));
            }
            // No colon — just a type as a field
            let end = self.node_end(&left);
            return Some(ArenaBox::new_in(TypeNode::JsdocObjectField(TypeJsdocObjectField {
                span: Span::new(start, end),
                left: ArenaBox::new_in(TypeNode::Name(TypeName {
                    span: Span::new(start, start),
                    value: "",
                }), self.allocator),
                right: left,
            }), self.allocator));
        }

        // Regular field: parse key name
        let key_token = lexer.current;
        let key_text = lexer.token_text(key_token);
        lexer.bump();

        let key = ArenaBox::new_in(TypeNode::Name(TypeName {
            span: Span::new(key_token.start, key_token.end),
            value: key_text,
        }), self.allocator);

        let optional = self.eat(lexer, TokenKind::Question);

        let right = if self.eat(lexer, TokenKind::Colon) {
            Some(self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::KeyValue)?)
        } else {
            None
        };

        let end = right.as_ref().map_or(key_token.end, |r| self.node_end(r));

        Some(ArenaBox::new_in(TypeNode::ObjectField(TypeObjectField {
            span: Span::new(start, end),
            key,
            right,
            optional,
            readonly,
            quote,
        }), self.allocator))
    }

    /// Parse index signature `[key: Type]: ValueType` or mapped type `[K in keyof T]: V`.
    fn parse_index_signature_or_mapped(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        start: u32,
        _readonly: bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        lexer.bump(); // consume `[`

        let key_token = lexer.current;
        let key = lexer.token_text(key_token);
        lexer.bump();

        // Mapped type: `[K in keyof T]`
        if lexer.current.kind == TokenKind::In {
            lexer.bump(); // consume `in`
            let _right = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
            self.expect(lexer, TokenKind::RBracket);
            self.expect(lexer, TokenKind::Colon);
            let value = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::KeyValue)?;
            let end = self.node_end(&value);
            return Some(ArenaBox::new_in(TypeNode::MappedType(TypeMappedType {
                span: Span::new(start, end),
                key,
                right: value,
            }), self.allocator));
        }

        // Index signature: `[key: Type]: ValueType`
        self.expect(lexer, TokenKind::Colon);
        let _index_type = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
        self.expect(lexer, TokenKind::RBracket);
        self.expect(lexer, TokenKind::Colon);
        let value = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::KeyValue)?;
        let end = self.node_end(&value);

        Some(ArenaBox::new_in(TypeNode::IndexSignature(TypeIndexSignature {
            span: Span::new(start, end),
            key,
            right: value,
        }), self.allocator))
    }

    /// Parse `function(a, b): ReturnType` (jsdoc/closure) or `function` bare (jsdoc).
    fn parse_function_type(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `function`

        // Bare `function` without parentheses (jsdoc mode)
        if lexer.current.kind != TokenKind::LParen {
            if mode.is_jsdoc() {
                return Some(ArenaBox::new_in(TypeNode::Function(TypeFunction {
                    span: Span::new(start, start + 8),
                    parameters: ArenaVec::new_in(self.allocator),
                    return_type: None,
                    type_parameters: ArenaVec::new_in(self.allocator),
                    constructor: false,
                    arrow: false,
                    parenthesis: false,
                }), self.allocator));
            }
            // Name `function` as identifier in other modes
            return Some(ArenaBox::new_in(TypeNode::Name(TypeName {
                span: Span::new(start, start + 8),
                value: "function",
            }), self.allocator));
        }

        lexer.bump(); // consume `(`

        let mut parameters = ArenaVec::new_in(self.allocator);
        let mut constructor = false;

        if lexer.current.kind != TokenKind::RParen {
            // Check for `new:` parameter
            if lexer.current.kind == TokenKind::New && lexer.next.kind == TokenKind::Colon {
                constructor = true;
            }

            loop {
                let param = self.parse_key_value_or_type(lexer, mode, disallow_conditional)?;
                parameters.push(param);
                if !self.eat(lexer, TokenKind::Comma) {
                    break;
                }
            }
        }

        self.expect(lexer, TokenKind::RParen);

        // Return type after `:`
        let return_type = if self.eat(lexer, TokenKind::Colon) {
            Some(self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?)
        } else {
            None
        };

        let end = return_type.as_ref().map_or(lexer.current.start, |r| self.node_end(r));

        Some(ArenaBox::new_in(TypeNode::Function(TypeFunction {
            span: Span::new(start, end),
            parameters,
            return_type,
            type_parameters: ArenaVec::new_in(self.allocator),
            constructor,
            arrow: false,
            parenthesis: true,
        }), self.allocator))
    }

    /// Parse `typeof X`.
    fn parse_typeof(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `typeof`
        let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::KeyOfTypeOf)?;
        let end = self.node_end(&element);
        Some(ArenaBox::new_in(TypeNode::TypeOf(TypeTypeOf {
            span: Span::new(start, end),
            element,
        }), self.allocator))
    }

    /// Parse `keyof T`.
    fn parse_keyof(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `keyof`
        let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::KeyOfTypeOf)?;
        let end = self.node_end(&element);
        Some(ArenaBox::new_in(TypeNode::KeyOf(TypeKeyOf {
            span: Span::new(start, end),
            element,
        }), self.allocator))
    }

    /// Parse `readonly T[]`.
    fn parse_readonly_array(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `readonly`
        let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::ArrayBrackets)?;
        let end = self.node_end(&element);
        Some(ArenaBox::new_in(TypeNode::ReadonlyArray(TypeReadonlyArray {
            span: Span::new(start, end),
            element,
        }), self.allocator))
    }

    /// Parse `import('module')`.
    fn parse_import_type(
        &mut self,
        lexer: &mut Lexer<'a>,
        _mode: ParseMode,
        _disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `import`
        self.expect(lexer, TokenKind::LParen);
        let element_token = lexer.current;
        if element_token.kind != TokenKind::StringValue {
            self.diagnostics.push(type_diagnostic(TypeDiagnosticKind::ExpectedToken));
            return None;
        }
        let text = lexer.token_text(element_token);
        let element = ArenaBox::new_in(TypeNode::StringValue(TypeStringValue {
            span: Span::new(element_token.start, element_token.end),
            value: text,
            quote: if text.starts_with('"') { QuoteStyle::Double } else { QuoteStyle::Single },
        }), self.allocator);
        lexer.bump();
        self.expect(lexer, TokenKind::RParen);
        let end = lexer.current.start;

        Some(ArenaBox::new_in(TypeNode::Import(TypeImport {
            span: Span::new(start, end),
            element,
        }), self.allocator))
    }

    /// Parse `infer T`.
    fn parse_infer(
        &mut self,
        lexer: &mut Lexer<'a>,
        _mode: ParseMode,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `infer`
        let name_token = lexer.current;
        let name_text = lexer.token_text(name_token);
        lexer.bump();
        let element = ArenaBox::new_in(TypeNode::Name(TypeName {
            span: Span::new(name_token.start, name_token.end),
            value: name_text,
        }), self.allocator);
        let end = name_token.end;
        Some(ArenaBox::new_in(TypeNode::Infer(TypeInfer {
            span: Span::new(start, end),
            element,
        }), self.allocator))
    }

    /// Parse `asserts x is T` or `asserts x`.
    fn parse_asserts(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `asserts`

        let name_token = lexer.current;
        let name_text = lexer.token_text(name_token);
        lexer.bump();
        let left = ArenaBox::new_in(TypeNode::Name(TypeName {
            span: Span::new(name_token.start, name_token.end),
            value: name_text,
        }), self.allocator);

        if lexer.current.kind == TokenKind::Is {
            lexer.bump(); // consume `is`
            let right = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
            let end = self.node_end(&right);
            Some(ArenaBox::new_in(TypeNode::Asserts(TypeAsserts {
                span: Span::new(start, end),
                left,
                right,
            }), self.allocator))
        } else {
            let end = name_token.end;
            Some(ArenaBox::new_in(TypeNode::AssertsPlain(TypeAssertsPlain {
                span: Span::new(start, end),
                element: left,
            }), self.allocator))
        }
    }

    /// Parse `unique symbol`.
    fn parse_unique_symbol(
        &mut self,
        lexer: &mut Lexer<'a>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;
        lexer.bump(); // consume `unique`
        if lexer.current.kind == TokenKind::Symbol {
            let end = lexer.current.end;
            lexer.bump(); // consume `symbol`
            Some(ArenaBox::new_in(TypeNode::UniqueSymbol(TypeUniqueSymbol {
                span: Span::new(start, end),
            }), self.allocator))
        } else {
            // Just the identifier `unique`
            Some(ArenaBox::new_in(TypeNode::Name(TypeName {
                span: Span::new(start, start + 6),
                value: "unique",
            }), self.allocator))
        }
    }

    /// Parse number literal.
    fn parse_number_literal(
        &mut self,
        lexer: &mut Lexer<'a>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let token = lexer.current;
        let text = lexer.token_text(token);
        lexer.bump();
        Some(ArenaBox::new_in(TypeNode::Number(TypeNumber {
            span: Span::new(token.start, token.end),
            value: text,
        }), self.allocator))
    }

    /// Parse string literal.
    fn parse_string_literal(
        &mut self,
        lexer: &mut Lexer<'a>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let token = lexer.current;
        let text = lexer.token_text(token);
        let quote = if text.starts_with('"') { QuoteStyle::Double } else { QuoteStyle::Single };
        lexer.bump();
        Some(ArenaBox::new_in(TypeNode::StringValue(TypeStringValue {
            span: Span::new(token.start, token.end),
            value: text,
            quote,
        }), self.allocator))
    }

    /// Parse template literal.
    fn parse_template_literal(
        &mut self,
        lexer: &mut Lexer<'a>,
        _mode: ParseMode,
        _disallow_conditional: &mut bool,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let token = lexer.current;
        let text = lexer.token_text(token);
        lexer.bump();

        // For now, store the whole template as a single literal with no interpolations.
        // Full interpolation parsing will be added in Phase 4.
        let mut literals = ArenaVec::new_in(self.allocator);
        literals.push(text);

        Some(ArenaBox::new_in(TypeNode::TemplateLiteral(TypeTemplateLiteral {
            span: Span::new(token.start, token.end),
            literals,
            interpolations: ArenaVec::new_in(self.allocator),
        }), self.allocator))
    }

    /// Parse `module:x`, `event:x`, `external:x` or fall back to name.
    fn parse_special_name_path_or_name(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        special_type: SpecialPathType,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = lexer.current.start;

        // Check if followed by `:` — if so, it's a special name path
        if lexer.next.kind == TokenKind::Colon {
            lexer.bump(); // consume keyword
            lexer.bump(); // consume `:`

            // Value can be a quoted string or identifier path
            let quote = match lexer.current.kind {
                TokenKind::StringValue => {
                    let text = lexer.token_text(lexer.current);
                    if text.starts_with('"') { Some(QuoteStyle::Double) } else { Some(QuoteStyle::Single) }
                }
                _ => None,
            };

            // Read the rest as a path (consume identifier-like tokens and separators)
            let value_start = lexer.current.start;
            let mut value_end = lexer.current.end;
            lexer.bump();

            // Continue consuming path segments
            while matches!(lexer.current.kind, TokenKind::Dot | TokenKind::Slash | TokenKind::Identifier)
                || lexer.current.kind.is_keyword()
            {
                value_end = lexer.current.end;
                lexer.bump();
            }

            let value = self.get_type_source_text(lexer, value_start, value_end);

            return Some(ArenaBox::new_in(TypeNode::SpecialNamePath(TypeSpecialNamePath {
                span: Span::new(start, value_end),
                value,
                special_type,
                quote,
            }), self.allocator));
        }

        // Not followed by `:` — treat as a regular name
        self.parse_name(lexer, mode)
    }

    /// Get source text between two absolute offsets from the type source.
    fn get_type_source_text(
        &self,
        lexer: &Lexer<'a>,
        abs_start: u32,
        abs_end: u32,
    ) -> &'a str {
        let token = crate::type_parser::token::Token::new(
            crate::type_parser::token::TokenKind::Identifier,
            abs_start,
            abs_end,
        );
        lexer.token_text(token)
    }

    // ========================================================================
    // Infix parse implementations
    // ========================================================================

    /// Parse union: `A | B | C`.
    fn parse_union(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);
        lexer.bump(); // consume `|`

        let mut elements = ArenaVec::with_capacity_in(4, self.allocator);
        elements.push(left);

        loop {
            let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::Union)?;
            elements.push(element);
            if !self.eat(lexer, TokenKind::Pipe) {
                break;
            }
        }

        let end = self.node_end(elements.last().unwrap());
        Some(ArenaBox::new_in(TypeNode::Union(TypeUnion {
            span: Span::new(start, end),
            elements,
        }), self.allocator))
    }

    /// Parse intersection: `A & B & C`.
    fn parse_intersection(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);
        lexer.bump(); // consume `&`

        let mut elements = ArenaVec::with_capacity_in(4, self.allocator);
        elements.push(left);

        loop {
            let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::Intersection)?;
            elements.push(element);
            if !self.eat(lexer, TokenKind::Amp) {
                break;
            }
        }

        let end = self.node_end(elements.last().unwrap());
        Some(ArenaBox::new_in(TypeNode::Intersection(TypeIntersection {
            span: Span::new(start, end),
            elements,
        }), self.allocator))
    }

    /// Parse generic: `Array<T>`, `Array.<T>`.
    fn parse_generic(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);
        let dot = self.eat(lexer, TokenKind::Dot);
        lexer.bump(); // consume `<`

        let mut elements = ArenaVec::with_capacity_in(4, self.allocator);

        loop {
            let element = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::ParameterList)?;
            elements.push(element);
            if !self.eat(lexer, TokenKind::Comma) {
                break;
            }
        }

        if !self.expect(lexer, TokenKind::Gt) {
            self.diagnostics.push(type_diagnostic(TypeDiagnosticKind::UnclosedGeneric));
            return None;
        }

        let end = lexer.current.start;
        Some(ArenaBox::new_in(TypeNode::Generic(TypeGeneric {
            span: Span::new(start, end),
            left,
            elements,
            brackets: GenericBrackets::Angle,
            dot,
        }), self.allocator))
    }

    /// Parse array brackets `T[]` or indexed access `T[K]`.
    fn parse_array_brackets_or_indexed(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);
        lexer.bump(); // consume `[`

        // Empty brackets `T[]` = Array shorthand
        if lexer.current.kind == TokenKind::RBracket {
            lexer.bump(); // consume `]`
            let end = lexer.current.start;
            let elements = ArenaVec::new_in(self.allocator);
            return Some(ArenaBox::new_in(TypeNode::Generic(TypeGeneric {
                span: Span::new(start, end),
                left,
                elements,
                brackets: GenericBrackets::Square,
                dot: false,
            }), self.allocator));
        }

        // Indexed access `T[K]`
        if mode.is_typescript() {
            let index = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
            self.expect(lexer, TokenKind::RBracket);
            let end = lexer.current.start;
            let right = ArenaBox::new_in(TypeNode::IndexedAccessIndex(TypeIndexedAccessIndex {
                span: Span::new(start, end),
                right: index,
            }), self.allocator);
            return Some(ArenaBox::new_in(TypeNode::NamePath(TypeNamePath {
                span: Span::new(start, end),
                left,
                right,
                path_type: NamePathType::PropertyBrackets,
            }), self.allocator));
        }

        // For non-typescript, just close and make array
        self.expect(lexer, TokenKind::RBracket);
        let end = lexer.current.start;
        Some(ArenaBox::new_in(TypeNode::Generic(TypeGeneric {
            span: Span::new(start, end),
            left,
            elements: ArenaVec::new_in(self.allocator),
            brackets: GenericBrackets::Square,
            dot: false,
        }), self.allocator))
    }

    /// Parse name path: `A.B`, `A#B`, `A~B`.
    fn parse_name_path(
        &mut self,
        lexer: &mut Lexer<'a>,
        _mode: ParseMode,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);

        let path_type = match lexer.current.kind {
            TokenKind::Dot => NamePathType::Property,
            TokenKind::Hash => NamePathType::Instance,
            TokenKind::Tilde => NamePathType::Inner,
            _ => return Some(left),
        };
        lexer.bump(); // consume `.`, `#`, or `~`

        // Parse right side — property name or quoted string
        let right_token = lexer.current;
        let right_text = lexer.token_text(right_token);

        let quote = match right_token.kind {
            TokenKind::StringValue => {
                if right_text.starts_with('"') { Some(QuoteStyle::Double) } else { Some(QuoteStyle::Single) }
            }
            _ => None,
        };

        lexer.bump();

        let right = ArenaBox::new_in(TypeNode::Property(TypeProperty {
            span: Span::new(right_token.start, right_token.end),
            value: right_text,
            quote,
        }), self.allocator);

        let end = right_token.end;
        Some(ArenaBox::new_in(TypeNode::NamePath(TypeNamePath {
            span: Span::new(start, end),
            left,
            right,
            path_type,
        }), self.allocator))
    }

    /// Parse nullable suffix: `T?`.
    fn parse_nullable_suffix(
        &mut self,
        lexer: &mut Lexer<'a>,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);
        let end = lexer.current.end;
        lexer.bump(); // consume `?`
        Some(ArenaBox::new_in(TypeNode::Nullable(TypeNullable {
            span: Span::new(start, end),
            element: left,
            position: ModifierPosition::Suffix,
        }), self.allocator))
    }

    /// Parse optional suffix: `T=`.
    fn parse_optional_suffix(
        &mut self,
        lexer: &mut Lexer<'a>,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);
        let end = lexer.current.end;
        lexer.bump(); // consume `=`
        Some(ArenaBox::new_in(TypeNode::Optional(TypeOptional {
            span: Span::new(start, end),
            element: left,
            position: ModifierPosition::Suffix,
        }), self.allocator))
    }

    /// Parse arrow function: `left => ReturnType` (where left came from parenthesized params).
    fn parse_arrow_function(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);
        lexer.bump(); // consume `=>`
        let return_type = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
        let end = self.node_end(&return_type);

        // Convert the left side (should be Parenthesis or ParameterList) to parameters
        let parameters = ArenaVec::new_in(self.allocator);

        Some(ArenaBox::new_in(TypeNode::Function(TypeFunction {
            span: Span::new(start, end),
            parameters,
            return_type: Some(return_type),
            type_parameters: ArenaVec::new_in(self.allocator),
            constructor: false,
            arrow: true,
            parenthesis: true,
        }), self.allocator))
    }

    /// Parse predicate: `x is T`.
    fn parse_predicate(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);
        lexer.bump(); // consume `is`
        let right = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
        let end = self.node_end(&right);
        Some(ArenaBox::new_in(TypeNode::Predicate(TypePredicate {
            span: Span::new(start, end),
            left,
            right,
        }), self.allocator))
    }

    /// Parse conditional: `A extends B ? C : D`.
    fn parse_conditional(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        if *disallow_conditional {
            return Some(left);
        }

        let start = self.node_start(&left);
        lexer.bump(); // consume `extends`

        // Parse extends type with conditional disabled to prevent nested
        let mut nested_disallow = true;
        let extends_type = self.parse_type_pratt(lexer, mode, &mut nested_disallow, Precedence::All)?;

        self.expect(lexer, TokenKind::Question);
        let true_type = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
        self.expect(lexer, TokenKind::Colon);
        let false_type = self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?;
        let end = self.node_end(&false_type);

        Some(ArenaBox::new_in(TypeNode::Conditional(TypeConditional {
            span: Span::new(start, end),
            checks_type: left,
            extends_type,
            true_type,
            false_type,
        }), self.allocator))
    }

    /// Parse variadic suffix: `T...` (jsdoc mode).
    fn parse_variadic_suffix(
        &mut self,
        lexer: &mut Lexer<'a>,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);
        let end = lexer.current.end;
        lexer.bump(); // consume `...`
        Some(ArenaBox::new_in(TypeNode::Variadic(TypeVariadic {
            span: Span::new(start, end),
            element: Some(left),
            position: Some(VariadicPosition::Suffix),
            square_brackets: false,
        }), self.allocator))
    }

    /// Parse symbol: `Name(arg)` (jsdoc/closure mode).
    fn parse_symbol(
        &mut self,
        lexer: &mut Lexer<'a>,
        mode: ParseMode,
        disallow_conditional: &mut bool,
        left: ArenaBox<'a, TypeNode<'a>>,
    ) -> Option<ArenaBox<'a, TypeNode<'a>>> {
        let start = self.node_start(&left);

        // Get the name from the left node
        let value = match left.as_ref() {
            TypeNode::Name(name) => name.value,
            _ => "",
        };

        lexer.bump(); // consume `(`

        let element = if lexer.current.kind != TokenKind::RParen {
            Some(self.parse_type_pratt(lexer, mode, disallow_conditional, Precedence::All)?)
        } else {
            None
        };

        self.expect(lexer, TokenKind::RParen);
        let end = lexer.current.start;

        Some(ArenaBox::new_in(TypeNode::Symbol(TypeSymbol {
            span: Span::new(start, end),
            value,
            element,
        }), self.allocator))
    }

    // ========================================================================
    // Node span helpers
    // ========================================================================

    /// Get the start offset of a node.
    #[inline]
    fn node_start(&self, node: &TypeNode<'a>) -> u32 {
        match node {
            TypeNode::Name(n) => n.span.start,
            TypeNode::Number(n) => n.span.start,
            TypeNode::StringValue(n) => n.span.start,
            TypeNode::Null(n) => n.span.start,
            TypeNode::Undefined(n) => n.span.start,
            TypeNode::Any(n) => n.span.start,
            TypeNode::Unknown(n) => n.span.start,
            TypeNode::Union(n) => n.span.start,
            TypeNode::Intersection(n) => n.span.start,
            TypeNode::Generic(n) => n.span.start,
            TypeNode::Function(n) => n.span.start,
            TypeNode::Object(n) => n.span.start,
            TypeNode::Tuple(n) => n.span.start,
            TypeNode::Parenthesis(n) => n.span.start,
            TypeNode::NamePath(n) => n.span.start,
            TypeNode::SpecialNamePath(n) => n.span.start,
            TypeNode::Nullable(n) => n.span.start,
            TypeNode::NotNullable(n) => n.span.start,
            TypeNode::Optional(n) => n.span.start,
            TypeNode::Variadic(n) => n.span.start,
            TypeNode::Conditional(n) => n.span.start,
            TypeNode::Infer(n) => n.span.start,
            TypeNode::KeyOf(n) => n.span.start,
            TypeNode::TypeOf(n) => n.span.start,
            TypeNode::Import(n) => n.span.start,
            TypeNode::Predicate(n) => n.span.start,
            TypeNode::Asserts(n) => n.span.start,
            TypeNode::AssertsPlain(n) => n.span.start,
            TypeNode::ReadonlyArray(n) => n.span.start,
            TypeNode::TemplateLiteral(n) => n.span.start,
            TypeNode::UniqueSymbol(n) => n.span.start,
            TypeNode::Symbol(n) => n.span.start,
            TypeNode::ObjectField(n) => n.span.start,
            TypeNode::JsdocObjectField(n) => n.span.start,
            TypeNode::KeyValue(n) => n.span.start,
            TypeNode::Property(n) => n.span.start,
            TypeNode::IndexSignature(n) => n.span.start,
            TypeNode::MappedType(n) => n.span.start,
            TypeNode::TypeParameter(n) => n.span.start,
            TypeNode::CallSignature(n) => n.span.start,
            TypeNode::ConstructorSignature(n) => n.span.start,
            TypeNode::MethodSignature(n) => n.span.start,
            TypeNode::IndexedAccessIndex(n) => n.span.start,
            TypeNode::ParameterList(n) => n.span.start,
            TypeNode::ReadonlyProperty(n) => n.span.start,
        }
    }

    /// Get the end offset of a node.
    #[inline]
    fn node_end(&self, node: &TypeNode<'a>) -> u32 {
        match node {
            TypeNode::Name(n) => n.span.end,
            TypeNode::Number(n) => n.span.end,
            TypeNode::StringValue(n) => n.span.end,
            TypeNode::Null(n) => n.span.end,
            TypeNode::Undefined(n) => n.span.end,
            TypeNode::Any(n) => n.span.end,
            TypeNode::Unknown(n) => n.span.end,
            TypeNode::Union(n) => n.span.end,
            TypeNode::Intersection(n) => n.span.end,
            TypeNode::Generic(n) => n.span.end,
            TypeNode::Function(n) => n.span.end,
            TypeNode::Object(n) => n.span.end,
            TypeNode::Tuple(n) => n.span.end,
            TypeNode::Parenthesis(n) => n.span.end,
            TypeNode::NamePath(n) => n.span.end,
            TypeNode::SpecialNamePath(n) => n.span.end,
            TypeNode::Nullable(n) => n.span.end,
            TypeNode::NotNullable(n) => n.span.end,
            TypeNode::Optional(n) => n.span.end,
            TypeNode::Variadic(n) => n.span.end,
            TypeNode::Conditional(n) => n.span.end,
            TypeNode::Infer(n) => n.span.end,
            TypeNode::KeyOf(n) => n.span.end,
            TypeNode::TypeOf(n) => n.span.end,
            TypeNode::Import(n) => n.span.end,
            TypeNode::Predicate(n) => n.span.end,
            TypeNode::Asserts(n) => n.span.end,
            TypeNode::AssertsPlain(n) => n.span.end,
            TypeNode::ReadonlyArray(n) => n.span.end,
            TypeNode::TemplateLiteral(n) => n.span.end,
            TypeNode::UniqueSymbol(n) => n.span.end,
            TypeNode::Symbol(n) => n.span.end,
            TypeNode::ObjectField(n) => n.span.end,
            TypeNode::JsdocObjectField(n) => n.span.end,
            TypeNode::KeyValue(n) => n.span.end,
            TypeNode::Property(n) => n.span.end,
            TypeNode::IndexSignature(n) => n.span.end,
            TypeNode::MappedType(n) => n.span.end,
            TypeNode::TypeParameter(n) => n.span.end,
            TypeNode::CallSignature(n) => n.span.end,
            TypeNode::ConstructorSignature(n) => n.span.end,
            TypeNode::MethodSignature(n) => n.span.end,
            TypeNode::IndexedAccessIndex(n) => n.span.end,
            TypeNode::ParameterList(n) => n.span.end,
            TypeNode::ReadonlyProperty(n) => n.span.end,
        }
    }
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;
    use crate::type_parser::ast::ParseMode;
    use super::*;

    fn parse_type(source: &str, mode: ParseMode) -> (Allocator, Option<String>) {
        let allocator = Allocator::default();
        let result = {
            let mut ctx = ParserContext::new(
                &allocator,
                "/** */", // dummy source
                0,
                crate::parser::ParseOptions::default(),
            );
            let node = ctx.parse_type_expression(source, 0, mode);
            node.map(|n| format!("{:?}", n.as_ref()))
        };
        (allocator, result)
    }

    #[test]
    fn parse_simple_name() {
        let (_, result) = parse_type("string", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Name"));
        assert!(s.contains("string"));
    }

    #[test]
    fn parse_union_type() {
        let (_, result) = parse_type("string | number", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Union"));
    }

    #[test]
    fn parse_intersection_type() {
        let (_, result) = parse_type("A & B", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Intersection"));
    }

    #[test]
    fn parse_generic_type() {
        let (_, result) = parse_type("Array<string>", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Generic"));
    }

    #[test]
    fn parse_nullable_prefix() {
        let (_, result) = parse_type("?string", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Nullable"));
        assert!(s.contains("Prefix"));
    }

    #[test]
    fn parse_nullable_suffix() {
        let (_, result) = parse_type("string?", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Nullable"));
        assert!(s.contains("Suffix"));
    }

    #[test]
    fn parse_not_nullable() {
        let (_, result) = parse_type("!string", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("NotNullable"));
    }

    #[test]
    fn parse_any_type() {
        let (_, result) = parse_type("*", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Any"));
    }

    #[test]
    fn parse_unknown_type() {
        let (_, result) = parse_type("?", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Unknown"));
    }

    #[test]
    fn parse_null_type() {
        let (_, result) = parse_type("null", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Null"));
    }

    #[test]
    fn parse_undefined_type() {
        let (_, result) = parse_type("undefined", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Undefined"));
    }

    #[test]
    fn parse_variadic_prefix() {
        let (_, result) = parse_type("...string", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Variadic"));
    }

    #[test]
    fn parse_optional_suffix() {
        let (_, result) = parse_type("string=", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Optional"));
        assert!(s.contains("Suffix"));
    }

    #[test]
    fn parse_parenthesized_type() {
        let (_, result) = parse_type("(string | number)", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Parenthesis"));
    }

    #[test]
    fn parse_function_closure_style() {
        let (_, result) = parse_type("function(string): number", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Function"));
    }

    #[test]
    fn parse_bare_function() {
        let (_, result) = parse_type("function", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Function"));
    }

    #[test]
    fn parse_typeof() {
        let (_, result) = parse_type("typeof myVar", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("TypeOf"));
    }

    #[test]
    fn parse_keyof() {
        let (_, result) = parse_type("keyof MyType", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("KeyOf"));
    }

    #[test]
    fn parse_array_brackets() {
        let (_, result) = parse_type("string[]", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Generic"));
        assert!(s.contains("Square"));
    }

    #[test]
    fn parse_number_literal() {
        let (_, result) = parse_type("42", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Number"));
    }

    #[test]
    fn parse_string_literal() {
        let (_, result) = parse_type("\"hello\"", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("StringValue"));
    }

    #[test]
    fn parse_conditional_type() {
        let (_, result) = parse_type("T extends U ? X : Y", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Conditional"));
    }

    #[test]
    fn parse_infer_type() {
        let (_, result) = parse_type("infer T", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Infer"));
    }

    #[test]
    fn parse_import_type() {
        let (_, result) = parse_type("import('./module')", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Import"));
    }

    #[test]
    fn parse_unique_symbol() {
        let (_, result) = parse_type("unique symbol", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("UniqueSymbol"));
    }

    #[test]
    fn parse_object_type() {
        let (_, result) = parse_type("{key: string}", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Object"));
    }

    #[test]
    fn parse_dot_notation_generic() {
        let (_, result) = parse_type("Array.<string>", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Generic"));
    }

    #[test]
    fn parse_name_path_dot() {
        let (_, result) = parse_type("Foo.Bar", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("NamePath"));
    }

    #[test]
    fn parse_name_path_hash() {
        let (_, result) = parse_type("MyClass#method", ParseMode::Jsdoc);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("NamePath"));
        assert!(s.contains("Instance"));
    }

    #[test]
    fn parse_asserts_is() {
        let (_, result) = parse_type("asserts x is string", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Asserts"));
    }

    #[test]
    fn parse_asserts_plain() {
        let (_, result) = parse_type("asserts x", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("AssertsPlain"));
    }

    #[test]
    fn parse_predicate() {
        let (_, result) = parse_type("x is string", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Predicate"));
    }

    #[test]
    fn parse_readonly_array() {
        let (_, result) = parse_type("readonly string[]", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("ReadonlyArray"));
    }

    #[test]
    fn parse_tuple_type() {
        let (_, result) = parse_type("[string, number]", ParseMode::Typescript);
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("Tuple"));
    }
}
