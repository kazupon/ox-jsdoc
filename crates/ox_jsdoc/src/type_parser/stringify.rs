// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! AST to string reconstruction for JSDoc type expressions.
//!
//! `stringify_type()` converts a `TypeNode` back to its string representation.
//! Used by fixers. Roundtrip guarantee: `parse(stringify(ast)) == ast`.

use super::ast::*;

/// Convert a `TypeNode` AST back to a string.
pub fn stringify_type(node: &TypeNode<'_>) -> String {
    let mut buf = String::new();
    write_node(&mut buf, node);
    buf
}

/// Write a `TypeNode` into the buffer.
fn write_node(buf: &mut String, node: &TypeNode<'_>) {
    match node {
        TypeNode::Name(n) => buf.push_str(n.value),
        TypeNode::Number(n) => buf.push_str(n.value),
        TypeNode::StringValue(n) => buf.push_str(n.value),
        TypeNode::Null(_) => buf.push_str("null"),
        TypeNode::Undefined(_) => buf.push_str("undefined"),
        TypeNode::Any(_) => buf.push('*'),
        TypeNode::Unknown(_) => buf.push('?'),

        TypeNode::Union(n) => write_union(buf, n),
        TypeNode::Intersection(n) => write_intersection(buf, n),
        TypeNode::Generic(n) => write_generic(buf, n),
        TypeNode::Function(n) => write_function(buf, n),
        TypeNode::Object(n) => write_object(buf, n),
        TypeNode::Tuple(n) => write_tuple(buf, n),
        TypeNode::Parenthesis(n) => write_parenthesis(buf, n),

        TypeNode::NamePath(n) => write_name_path(buf, n),
        TypeNode::SpecialNamePath(n) => write_special_name_path(buf, n),

        TypeNode::Nullable(n) => write_nullable(buf, n),
        TypeNode::NotNullable(n) => write_not_nullable(buf, n),
        TypeNode::Optional(n) => write_optional(buf, n),
        TypeNode::Variadic(n) => write_variadic(buf, n),

        TypeNode::Conditional(n) => write_conditional(buf, n),
        TypeNode::Infer(n) => write_infer(buf, n),
        TypeNode::KeyOf(n) => write_keyof(buf, n),
        TypeNode::TypeOf(n) => write_typeof(buf, n),
        TypeNode::Import(n) => write_import(buf, n),
        TypeNode::Predicate(n) => write_predicate(buf, n),
        TypeNode::Asserts(n) => write_asserts(buf, n),
        TypeNode::AssertsPlain(n) => write_asserts_plain(buf, n),
        TypeNode::ReadonlyArray(n) => write_readonly_array(buf, n),
        TypeNode::TemplateLiteral(n) => write_template_literal(buf, n),
        TypeNode::UniqueSymbol(_) => buf.push_str("unique symbol"),

        TypeNode::Symbol(n) => write_symbol(buf, n),

        TypeNode::ObjectField(n) => write_object_field(buf, n),
        TypeNode::JsdocObjectField(n) => write_jsdoc_object_field(buf, n),
        TypeNode::KeyValue(n) => write_key_value(buf, n),
        TypeNode::Property(n) => buf.push_str(n.value),
        TypeNode::IndexSignature(n) => write_index_signature(buf, n),
        TypeNode::MappedType(n) => write_mapped_type(buf, n),
        TypeNode::TypeParameter(n) => write_type_parameter(buf, n),
        TypeNode::CallSignature(n) => write_call_signature(buf, n),
        TypeNode::ConstructorSignature(n) => write_constructor_signature(buf, n),
        TypeNode::MethodSignature(n) => write_method_signature(buf, n),
        TypeNode::IndexedAccessIndex(n) => {
            buf.push('[');
            write_node(buf, &n.right);
            buf.push(']');
        }

        TypeNode::ParameterList(n) => write_comma_separated(buf, &n.elements),
        TypeNode::ReadonlyProperty(n) => {
            buf.push_str("readonly ");
            write_node(buf, &n.element);
        }
    }
}

fn write_union(buf: &mut String, n: &TypeUnion<'_>) {
    for (i, element) in n.elements.iter().enumerate() {
        if i > 0 {
            buf.push_str(" | ");
        }
        write_node(buf, element);
    }
}

fn write_intersection(buf: &mut String, n: &TypeIntersection<'_>) {
    for (i, element) in n.elements.iter().enumerate() {
        if i > 0 {
            buf.push_str(" & ");
        }
        write_node(buf, element);
    }
}

fn write_generic(buf: &mut String, n: &TypeGeneric<'_>) {
    match n.brackets {
        GenericBrackets::Angle => {
            write_node(buf, &n.left);
            if n.dot {
                buf.push('.');
            }
            buf.push('<');
            write_comma_separated(buf, &n.elements);
            buf.push('>');
        }
        GenericBrackets::Square => {
            write_node(buf, &n.left);
            buf.push_str("[]");
        }
    }
}

fn write_function(buf: &mut String, n: &TypeFunction<'_>) {
    if n.arrow {
        buf.push('(');
        write_comma_separated(buf, &n.parameters);
        buf.push_str(") => ");
        if let Some(ref ret) = n.return_type {
            write_node(buf, ret);
        }
    } else {
        if n.constructor {
            buf.push_str("new ");
        }
        buf.push_str("function");
        if n.parenthesis {
            buf.push('(');
            write_comma_separated(buf, &n.parameters);
            buf.push(')');
            if let Some(ref ret) = n.return_type {
                buf.push_str(": ");
                write_node(buf, ret);
            }
        }
    }
}

fn write_object(buf: &mut String, n: &TypeObject<'_>) {
    buf.push('{');
    let sep = match n.separator {
        Some(ObjectSeparator::Semicolon) | Some(ObjectSeparator::SemicolonAndLinebreak) => "; ",
        _ => ", ",
    };
    for (i, element) in n.elements.iter().enumerate() {
        if i > 0 {
            buf.push_str(sep);
        }
        write_node(buf, element);
    }
    buf.push('}');
}

fn write_tuple(buf: &mut String, n: &TypeTuple<'_>) {
    buf.push('[');
    write_comma_separated(buf, &n.elements);
    buf.push(']');
}

fn write_parenthesis(buf: &mut String, n: &TypeParenthesis<'_>) {
    buf.push('(');
    write_node(buf, &n.element);
    buf.push(')');
}

fn write_name_path(buf: &mut String, n: &TypeNamePath<'_>) {
    write_node(buf, &n.left);
    match n.path_type {
        NamePathType::Property => buf.push('.'),
        NamePathType::Instance => buf.push('#'),
        NamePathType::Inner => buf.push('~'),
        NamePathType::PropertyBrackets => {} // handled by IndexedAccessIndex
    }
    write_node(buf, &n.right);
}

fn write_special_name_path(buf: &mut String, n: &TypeSpecialNamePath<'_>) {
    match n.special_type {
        SpecialPathType::Module => buf.push_str("module:"),
        SpecialPathType::Event => buf.push_str("event:"),
        SpecialPathType::External => buf.push_str("external:"),
    }
    if let Some(q) = n.quote {
        let ch = match q {
            QuoteStyle::Single => '\'',
            QuoteStyle::Double => '"',
        };
        buf.push(ch);
        buf.push_str(n.value);
        buf.push(ch);
    } else {
        buf.push_str(n.value);
    }
}

fn write_nullable(buf: &mut String, n: &TypeNullable<'_>) {
    match n.position {
        ModifierPosition::Prefix => {
            buf.push('?');
            write_node(buf, &n.element);
        }
        ModifierPosition::Suffix => {
            write_node(buf, &n.element);
            buf.push('?');
        }
    }
}

fn write_not_nullable(buf: &mut String, n: &TypeNotNullable<'_>) {
    match n.position {
        ModifierPosition::Prefix => {
            buf.push('!');
            write_node(buf, &n.element);
        }
        ModifierPosition::Suffix => {
            write_node(buf, &n.element);
            buf.push('!');
        }
    }
}

fn write_optional(buf: &mut String, n: &TypeOptional<'_>) {
    match n.position {
        ModifierPosition::Prefix => {
            buf.push('=');
            write_node(buf, &n.element);
        }
        ModifierPosition::Suffix => {
            write_node(buf, &n.element);
            buf.push('=');
        }
    }
}

fn write_variadic(buf: &mut String, n: &TypeVariadic<'_>) {
    match n.position {
        Some(VariadicPosition::Prefix) => {
            buf.push_str("...");
            if n.square_brackets {
                buf.push('[');
            }
            if let Some(ref element) = n.element {
                write_node(buf, element);
            }
            if n.square_brackets {
                buf.push(']');
            }
        }
        Some(VariadicPosition::Suffix) => {
            if let Some(ref element) = n.element {
                write_node(buf, element);
            }
            buf.push_str("...");
        }
        None => {
            buf.push_str("...");
        }
    }
}

fn write_conditional(buf: &mut String, n: &TypeConditional<'_>) {
    write_node(buf, &n.checks_type);
    buf.push_str(" extends ");
    write_node(buf, &n.extends_type);
    buf.push_str(" ? ");
    write_node(buf, &n.true_type);
    buf.push_str(" : ");
    write_node(buf, &n.false_type);
}

fn write_infer(buf: &mut String, n: &TypeInfer<'_>) {
    buf.push_str("infer ");
    write_node(buf, &n.element);
}

fn write_keyof(buf: &mut String, n: &TypeKeyOf<'_>) {
    buf.push_str("keyof ");
    write_node(buf, &n.element);
}

fn write_typeof(buf: &mut String, n: &TypeTypeOf<'_>) {
    buf.push_str("typeof ");
    write_node(buf, &n.element);
}

fn write_import(buf: &mut String, n: &TypeImport<'_>) {
    buf.push_str("import(");
    write_node(buf, &n.element);
    buf.push(')');
}

fn write_predicate(buf: &mut String, n: &TypePredicate<'_>) {
    write_node(buf, &n.left);
    buf.push_str(" is ");
    write_node(buf, &n.right);
}

fn write_asserts(buf: &mut String, n: &TypeAsserts<'_>) {
    buf.push_str("asserts ");
    write_node(buf, &n.left);
    buf.push_str(" is ");
    write_node(buf, &n.right);
}

fn write_asserts_plain(buf: &mut String, n: &TypeAssertsPlain<'_>) {
    buf.push_str("asserts ");
    write_node(buf, &n.element);
}

fn write_readonly_array(buf: &mut String, n: &TypeReadonlyArray<'_>) {
    buf.push_str("readonly ");
    write_node(buf, &n.element);
}

fn write_template_literal(buf: &mut String, n: &TypeTemplateLiteral<'_>) {
    // For now, output the stored literal text (which includes backticks from the lexer)
    for (i, literal) in n.literals.iter().enumerate() {
        buf.push_str(literal);
        if i < n.interpolations.len() {
            buf.push_str("${");
            write_node(buf, &n.interpolations[i]);
            buf.push('}');
        }
    }
}

fn write_symbol(buf: &mut String, n: &TypeSymbol<'_>) {
    buf.push_str(n.value);
    buf.push('(');
    if let Some(ref element) = n.element {
        write_node(buf, element);
    }
    buf.push(')');
}

fn write_object_field(buf: &mut String, n: &TypeObjectField<'_>) {
    if n.readonly {
        buf.push_str("readonly ");
    }
    write_node(buf, &n.key);
    if n.optional {
        buf.push('?');
    }
    if let Some(ref right) = n.right {
        buf.push_str(": ");
        write_node(buf, right);
    }
}

fn write_jsdoc_object_field(buf: &mut String, n: &TypeJsdocObjectField<'_>) {
    write_node(buf, &n.left);
    buf.push_str(": ");
    write_node(buf, &n.right);
}

fn write_key_value(buf: &mut String, n: &TypeKeyValue<'_>) {
    if n.variadic {
        buf.push_str("...");
    }
    buf.push_str(n.key);
    if n.optional {
        buf.push('?');
    }
    if let Some(ref right) = n.right {
        buf.push_str(": ");
        write_node(buf, right);
    }
}

fn write_index_signature(buf: &mut String, n: &TypeIndexSignature<'_>) {
    buf.push('[');
    buf.push_str(n.key);
    buf.push_str(": ");
    write_node(buf, &n.right);
    buf.push(']');
}

fn write_mapped_type(buf: &mut String, n: &TypeMappedType<'_>) {
    buf.push('[');
    buf.push_str(n.key);
    buf.push_str(" in ");
    write_node(buf, &n.right);
    buf.push(']');
}

fn write_type_parameter(buf: &mut String, n: &TypeTypeParameter<'_>) {
    write_node(buf, &n.name);
    if let Some(ref constraint) = n.constraint {
        buf.push_str(" extends ");
        write_node(buf, constraint);
    }
    if let Some(ref default) = n.default_value {
        buf.push_str(" = ");
        write_node(buf, default);
    }
}

fn write_call_signature(buf: &mut String, n: &TypeCallSignature<'_>) {
    if !n.type_parameters.is_empty() {
        buf.push('<');
        write_comma_separated(buf, &n.type_parameters);
        buf.push('>');
    }
    buf.push('(');
    write_comma_separated(buf, &n.parameters);
    buf.push_str("): ");
    write_node(buf, &n.return_type);
}

fn write_constructor_signature(buf: &mut String, n: &TypeConstructorSignature<'_>) {
    buf.push_str("new ");
    if !n.type_parameters.is_empty() {
        buf.push('<');
        write_comma_separated(buf, &n.type_parameters);
        buf.push('>');
    }
    buf.push('(');
    write_comma_separated(buf, &n.parameters);
    buf.push_str("): ");
    write_node(buf, &n.return_type);
}

fn write_method_signature(buf: &mut String, n: &TypeMethodSignature<'_>) {
    buf.push_str(n.name);
    if !n.type_parameters.is_empty() {
        buf.push('<');
        write_comma_separated(buf, &n.type_parameters);
        buf.push('>');
    }
    buf.push('(');
    write_comma_separated(buf, &n.parameters);
    buf.push_str("): ");
    write_node(buf, &n.return_type);
}

/// Write comma-separated elements.
fn write_comma_separated(buf: &mut String, elements: &[oxc_allocator::Box<'_, TypeNode<'_>>]) {
    for (i, element) in elements.iter().enumerate() {
        if i > 0 {
            buf.push_str(", ");
        }
        write_node(buf, element);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParseOptions, ParserContext};
    use crate::type_parser::ast::ParseMode;
    use oxc_allocator::Allocator;

    fn roundtrip(source: &str, mode: ParseMode) -> String {
        let allocator = Allocator::default();
        let mut ctx = ParserContext::new(&allocator, "/** */", 0, ParseOptions::default());
        let node = ctx
            .parse_type_expression(source, 0, mode)
            .expect(&format!("failed to parse: {source}"));
        stringify_type(&node)
    }

    #[test]
    fn stringify_simple_name() {
        assert_eq!(roundtrip("string", ParseMode::Jsdoc), "string");
    }

    #[test]
    fn stringify_union() {
        assert_eq!(
            roundtrip("string | number", ParseMode::Typescript),
            "string | number"
        );
    }

    #[test]
    fn stringify_intersection() {
        assert_eq!(roundtrip("A & B", ParseMode::Typescript), "A & B");
    }

    #[test]
    fn stringify_generic_angle() {
        assert_eq!(
            roundtrip("Array<string>", ParseMode::Typescript),
            "Array<string>"
        );
    }

    #[test]
    fn stringify_generic_dot_angle() {
        assert_eq!(
            roundtrip("Array.<string>", ParseMode::Jsdoc),
            "Array.<string>"
        );
    }

    #[test]
    fn stringify_array_brackets() {
        assert_eq!(roundtrip("string[]", ParseMode::Typescript), "string[]");
    }

    #[test]
    fn stringify_nullable_prefix() {
        assert_eq!(roundtrip("?string", ParseMode::Jsdoc), "?string");
    }

    #[test]
    fn stringify_nullable_suffix() {
        assert_eq!(roundtrip("string?", ParseMode::Jsdoc), "string?");
    }

    #[test]
    fn stringify_not_nullable() {
        assert_eq!(roundtrip("!string", ParseMode::Jsdoc), "!string");
    }

    #[test]
    fn stringify_optional_suffix() {
        assert_eq!(roundtrip("string=", ParseMode::Jsdoc), "string=");
    }

    #[test]
    fn stringify_variadic_prefix() {
        assert_eq!(roundtrip("...string", ParseMode::Jsdoc), "...string");
    }

    #[test]
    fn stringify_any() {
        assert_eq!(roundtrip("*", ParseMode::Jsdoc), "*");
    }

    #[test]
    fn stringify_unknown() {
        assert_eq!(roundtrip("?", ParseMode::Jsdoc), "?");
    }

    #[test]
    fn stringify_null() {
        assert_eq!(roundtrip("null", ParseMode::Typescript), "null");
    }

    #[test]
    fn stringify_undefined() {
        assert_eq!(roundtrip("undefined", ParseMode::Typescript), "undefined");
    }

    #[test]
    fn stringify_parenthesis() {
        assert_eq!(
            roundtrip("(string | number)", ParseMode::Typescript),
            "(string | number)"
        );
    }

    #[test]
    fn stringify_function_closure_style() {
        assert_eq!(
            roundtrip("function(string): number", ParseMode::Jsdoc),
            "function(string): number"
        );
    }

    #[test]
    fn stringify_bare_function() {
        assert_eq!(roundtrip("function", ParseMode::Jsdoc), "function");
    }

    #[test]
    fn stringify_typeof() {
        assert_eq!(
            roundtrip("typeof myVar", ParseMode::Typescript),
            "typeof myVar"
        );
    }

    #[test]
    fn stringify_keyof() {
        assert_eq!(
            roundtrip("keyof MyType", ParseMode::Typescript),
            "keyof MyType"
        );
    }

    #[test]
    fn stringify_conditional() {
        assert_eq!(
            roundtrip("T extends U ? X : Y", ParseMode::Typescript),
            "T extends U ? X : Y"
        );
    }

    #[test]
    fn stringify_infer() {
        assert_eq!(roundtrip("infer T", ParseMode::Typescript), "infer T");
    }

    #[test]
    fn stringify_predicate() {
        assert_eq!(
            roundtrip("x is string", ParseMode::Typescript),
            "x is string"
        );
    }

    #[test]
    fn stringify_asserts() {
        assert_eq!(
            roundtrip("asserts x is string", ParseMode::Typescript),
            "asserts x is string"
        );
    }

    #[test]
    fn stringify_asserts_plain() {
        assert_eq!(roundtrip("asserts x", ParseMode::Typescript), "asserts x");
    }

    #[test]
    fn stringify_readonly_array() {
        assert_eq!(
            roundtrip("readonly string[]", ParseMode::Typescript),
            "readonly string[]"
        );
    }

    #[test]
    fn stringify_unique_symbol() {
        assert_eq!(
            roundtrip("unique symbol", ParseMode::Typescript),
            "unique symbol"
        );
    }

    #[test]
    fn stringify_name_path_dot() {
        assert_eq!(roundtrip("Foo.Bar", ParseMode::Jsdoc), "Foo.Bar");
    }

    #[test]
    fn stringify_name_path_hash() {
        assert_eq!(
            roundtrip("MyClass#method", ParseMode::Jsdoc),
            "MyClass#method"
        );
    }

    #[test]
    fn stringify_name_path_tilde() {
        assert_eq!(
            roundtrip("MyModule~inner", ParseMode::Jsdoc),
            "MyModule~inner"
        );
    }

    #[test]
    fn stringify_number_literal() {
        assert_eq!(roundtrip("42", ParseMode::Typescript), "42");
    }

    #[test]
    fn stringify_tuple() {
        assert_eq!(
            roundtrip("[string, number]", ParseMode::Typescript),
            "[string, number]"
        );
    }

    #[test]
    fn stringify_object() {
        assert_eq!(
            roundtrip("{key: string}", ParseMode::Typescript),
            "{key: string}"
        );
    }

    #[test]
    fn stringify_import() {
        assert_eq!(
            roundtrip("import('./module')", ParseMode::Typescript),
            "import('./module')"
        );
    }

    #[test]
    fn stringify_generic_multi_params() {
        assert_eq!(
            roundtrip("Map<string, number>", ParseMode::Typescript),
            "Map<string, number>"
        );
    }
}
