// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! AST to string reconstruction for JSDoc type expressions.
//!
//! `stringify_type_data()` converts a [`TypeNodeData`] back to its string
//! representation. Mirrors `crates/ox_jsdoc/src/type_parser/stringify.rs`
//! but operates on the heap-allocated [`TypeNodeData`] tree produced by the
//! binary parser instead of the arena-allocated `TypeNode` tree.
//!
//! Roundtrip guarantee: `parse(stringify(ast)) == ast`.

use super::type_data::*;

/// Convert a [`TypeNodeData`] AST back to a string.
#[must_use]
pub fn stringify_type_data(node: &TypeNodeData<'_>) -> String {
    let mut buf = String::new();
    write_node(&mut buf, node);
    buf
}

/// Write a [`TypeNodeData`] into the buffer.
fn write_node(buf: &mut String, node: &TypeNodeData<'_>) {
    match node {
        TypeNodeData::Name(n) => buf.push_str(n.value),
        TypeNodeData::Number(n) => buf.push_str(n.value),
        TypeNodeData::StringValue(n) => buf.push_str(n.value),
        TypeNodeData::Null(_) => buf.push_str("null"),
        TypeNodeData::Undefined(_) => buf.push_str("undefined"),
        TypeNodeData::Any(_) => buf.push('*'),
        TypeNodeData::Unknown(_) => buf.push('?'),

        TypeNodeData::Union(n) => write_union(buf, n),
        TypeNodeData::Intersection(n) => write_intersection(buf, n),
        TypeNodeData::Generic(n) => write_generic(buf, n),
        TypeNodeData::Function(n) => write_function(buf, n),
        TypeNodeData::Object(n) => write_object(buf, n),
        TypeNodeData::Tuple(n) => write_tuple(buf, n),
        TypeNodeData::Parenthesis(n) => write_parenthesis(buf, n),

        TypeNodeData::NamePath(n) => write_name_path(buf, n),
        TypeNodeData::SpecialNamePath(n) => write_special_name_path(buf, n),

        TypeNodeData::Nullable(n) => write_nullable(buf, n),
        TypeNodeData::NotNullable(n) => write_not_nullable(buf, n),
        TypeNodeData::Optional(n) => write_optional(buf, n),
        TypeNodeData::Variadic(n) => write_variadic(buf, n),

        TypeNodeData::Conditional(n) => write_conditional(buf, n),
        TypeNodeData::Infer(n) => write_infer(buf, n),
        TypeNodeData::KeyOf(n) => write_keyof(buf, n),
        TypeNodeData::TypeOf(n) => write_typeof(buf, n),
        TypeNodeData::Import(n) => write_import(buf, n),
        TypeNodeData::Predicate(n) => write_predicate(buf, n),
        TypeNodeData::Asserts(n) => write_asserts(buf, n),
        TypeNodeData::AssertsPlain(n) => write_asserts_plain(buf, n),
        TypeNodeData::ReadonlyArray(n) => write_readonly_array(buf, n),
        TypeNodeData::TemplateLiteral(n) => write_template_literal(buf, n),
        TypeNodeData::UniqueSymbol(_) => buf.push_str("unique symbol"),

        TypeNodeData::Symbol(n) => write_symbol(buf, n),

        TypeNodeData::ObjectField(n) => write_object_field(buf, n),
        TypeNodeData::JsdocObjectField(n) => write_jsdoc_object_field(buf, n),
        TypeNodeData::KeyValue(n) => write_key_value(buf, n),
        TypeNodeData::Property(n) => buf.push_str(n.value),
        TypeNodeData::IndexSignature(n) => write_index_signature(buf, n),
        TypeNodeData::MappedType(n) => write_mapped_type(buf, n),
        TypeNodeData::TypeParameter(n) => write_type_parameter(buf, n),
        TypeNodeData::CallSignature(n) => write_call_signature(buf, n),
        TypeNodeData::ConstructorSignature(n) => write_constructor_signature(buf, n),
        TypeNodeData::MethodSignature(n) => write_method_signature(buf, n),
        TypeNodeData::IndexedAccessIndex(n) => {
            buf.push('[');
            write_node(buf, &n.right);
            buf.push(']');
        }

        TypeNodeData::ParameterList(n) => write_comma_separated(buf, &n.elements),
        TypeNodeData::ReadonlyProperty(n) => {
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
fn write_comma_separated(buf: &mut String, elements: &[Box<TypeNodeData<'_>>]) {
    for (i, element) in elements.iter().enumerate() {
        if i > 0 {
            buf.push_str(", ");
        }
        write_node(buf, element);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ParseOptions, context::ParserContext, type_data::ParseMode};
    use super::*;
    use oxc_allocator::Allocator;

    fn roundtrip(source: &str, mode: ParseMode) -> String {
        let allocator = Allocator::default();
        let mut ctx = ParserContext::new(&allocator, source, 0, ParseOptions::default());
        let node = ctx
            .parse_type_expression(source, 0, mode)
            .unwrap_or_else(|| panic!("failed to parse: {source}"));
        stringify_type_data(&node)
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
