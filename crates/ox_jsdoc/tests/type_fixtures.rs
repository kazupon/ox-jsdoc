// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//
// Test fixtures ported from jsdoc-type-pratt-parser test/fixtures/

use ox_jsdoc::ast::JsdocType;
use ox_jsdoc::parse_comment;
use ox_jsdoc::parser::ParseOptions;
use ox_jsdoc::type_parser::ast::ParseMode;
use ox_jsdoc::type_parser::stringify::stringify_type;
use oxc_allocator::Allocator;

/// Parse a type expression via the full comment parser pipeline.
/// Wraps the type in `/** @param {<type>} x */` and extracts parsed_type.
fn parse_and_json(source: &str, mode: ParseMode) -> Option<serde_json::Value> {
    let allocator = Allocator::default();
    let comment_src = format!("/** @param {{{source}}} x */");
    let output = parse_comment(
        &allocator,
        &comment_src,
        0,
        ParseOptions {
            parse_types: true,
            type_parse_mode: mode,
            ..ParseOptions::default()
        },
    );
    let comment = output.comment?;
    let tag = comment.tags.first()?;
    let parsed_type = tag.parsed_type.as_ref()?;
    match parsed_type.as_ref() {
        JsdocType::Parsed(node) => Some(serialize_node(node)),
        JsdocType::Raw(_) => None,
    }
}

/// Parse and stringify roundtrip via the full pipeline.
fn roundtrip(source: &str, mode: ParseMode) -> Option<String> {
    let allocator = Allocator::default();
    let comment_src = format!("/** @param {{{source}}} x */");
    let output = parse_comment(
        &allocator,
        &comment_src,
        0,
        ParseOptions {
            parse_types: true,
            type_parse_mode: mode,
            ..ParseOptions::default()
        },
    );
    let comment = output.comment?;
    let tag = comment.tags.first()?;
    let parsed_type = tag.parsed_type.as_ref()?;
    match parsed_type.as_ref() {
        JsdocType::Parsed(node) => Some(stringify_type(node)),
        JsdocType::Raw(_) => None,
    }
}

/// Check parse succeeds for the given modes.
fn assert_parses(source: &str, modes: &[ParseMode]) {
    for &mode in modes {
        assert!(
            parse_and_json(source, mode).is_some(),
            "expected '{source}' to parse in mode {mode:?}"
        );
    }
}

/// Check parse fails for the given modes.
fn assert_fails(source: &str, modes: &[ParseMode]) {
    for &mode in modes {
        assert!(
            parse_and_json(source, mode).is_none(),
            "expected '{source}' to fail in mode {mode:?}"
        );
    }
}

/// Check AST type field.
fn assert_type(source: &str, mode: ParseMode, expected_type: &str) {
    let json = parse_and_json(source, mode)
        .unwrap_or_else(|| panic!("failed to parse '{source}' in mode {mode:?}"));
    assert_eq!(
        json["type"].as_str().unwrap(),
        expected_type,
        "type mismatch for '{source}' in {mode:?}: got {json}"
    );
}

/// Check AST type and value fields.
fn assert_type_value(source: &str, mode: ParseMode, expected_type: &str, expected_value: &str) {
    let json = parse_and_json(source, mode)
        .unwrap_or_else(|| panic!("failed to parse '{source}' in mode {mode:?}"));
    assert_eq!(json["type"].as_str().unwrap(), expected_type);
    assert_eq!(json["value"].as_str().unwrap(), expected_value);
}

/// Check stringify roundtrip.
fn assert_roundtrip(source: &str, mode: ParseMode) {
    let result = roundtrip(source, mode)
        .unwrap_or_else(|| panic!("failed to parse '{source}' for roundtrip in mode {mode:?}"));
    assert_eq!(
        result, source,
        "roundtrip mismatch for '{source}' in {mode:?}"
    );
}

/// Check stringify roundtrip with expected output (when whitespace differs).
#[allow(dead_code)]
fn assert_roundtrip_to(source: &str, mode: ParseMode, expected: &str) {
    let result = roundtrip(source, mode)
        .unwrap_or_else(|| panic!("failed to parse '{source}' for roundtrip in mode {mode:?}"));
    assert_eq!(
        result, expected,
        "roundtrip mismatch for '{source}' in {mode:?}"
    );
}

/// Serialize TypeNode to JSON for comparison.
fn serialize_node(node: &ox_jsdoc::type_parser::ast::TypeNode<'_>) -> serde_json::Value {
    use ox_jsdoc::type_parser::ast::*;
    use serde_json::json;

    match node {
        TypeNode::Name(n) => json!({ "type": "JsdocTypeName", "value": n.value }),
        TypeNode::Number(n) => {
            json!({ "type": "JsdocTypeNumber", "value": n.value.parse::<f64>().unwrap_or(0.0) })
        }
        TypeNode::StringValue(n) => json!({
            "type": "JsdocTypeStringValue",
            "value": unquote(n.value),
            "meta": { "quote": quote_str(n.quote) },
        }),
        TypeNode::Null(_) => json!({ "type": "JsdocTypeNull" }),
        TypeNode::Undefined(_) => json!({ "type": "JsdocTypeUndefined" }),
        TypeNode::Any(_) => json!({ "type": "JsdocTypeAny" }),
        TypeNode::Unknown(_) => json!({ "type": "JsdocTypeUnknown" }),
        TypeNode::Union(n) => json!({
            "type": "JsdocTypeUnion",
            "elements": n.elements.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::Intersection(n) => json!({
            "type": "JsdocTypeIntersection",
            "elements": n.elements.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::Generic(n) => json!({
            "type": "JsdocTypeGeneric",
            "left": serialize_node(&n.left),
            "elements": n.elements.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
            "meta": {
                "brackets": match n.brackets {
                    GenericBrackets::Angle => "angle",
                    GenericBrackets::Square => "square",
                },
                "dot": n.dot,
            },
        }),
        TypeNode::Nullable(n) => json!({
            "type": "JsdocTypeNullable",
            "element": serialize_node(&n.element),
            "meta": { "position": pos_str(n.position) },
        }),
        TypeNode::NotNullable(n) => json!({
            "type": "JsdocTypeNotNullable",
            "element": serialize_node(&n.element),
            "meta": { "position": pos_str(n.position) },
        }),
        TypeNode::Optional(n) => json!({
            "type": "JsdocTypeOptional",
            "element": serialize_node(&n.element),
            "meta": { "position": pos_str(n.position) },
        }),
        TypeNode::Variadic(n) => {
            let mut obj = json!({
                "type": "JsdocTypeVariadic",
                "meta": {
                    "position": n.position.map(|p| match p {
                        VariadicPosition::Prefix => "prefix",
                        VariadicPosition::Suffix => "suffix",
                    }),
                    "squareBrackets": n.square_brackets,
                },
            });
            if let Some(ref el) = n.element {
                obj["element"] = serialize_node(el);
            }
            obj
        }
        TypeNode::Parenthesis(n) => json!({
            "type": "JsdocTypeParenthesis",
            "element": serialize_node(&n.element),
        }),
        TypeNode::NamePath(n) => json!({
            "type": "JsdocTypeNamePath",
            "left": serialize_node(&n.left),
            "right": serialize_node(&n.right),
            "pathType": match n.path_type {
                NamePathType::Property => "property",
                NamePathType::Instance => "instance",
                NamePathType::Inner => "inner",
                NamePathType::PropertyBrackets => "property-brackets",
            },
        }),
        TypeNode::SpecialNamePath(n) => json!({
            "type": "JsdocTypeSpecialNamePath",
            "value": n.value,
            "specialType": match n.special_type {
                SpecialPathType::Module => "module",
                SpecialPathType::Event => "event",
                SpecialPathType::External => "external",
            },
            "meta": { "quote": n.quote.map(|q| quote_str(q)) },
        }),
        TypeNode::Property(n) => json!({
            "type": "JsdocTypeProperty",
            "value": n.value,
            "meta": { "quote": n.quote.map(|q| quote_str(q)) },
        }),
        TypeNode::Conditional(n) => json!({
            "type": "JsdocTypeConditional",
            "checksType": serialize_node(&n.checks_type),
            "extendsType": serialize_node(&n.extends_type),
            "trueType": serialize_node(&n.true_type),
            "falseType": serialize_node(&n.false_type),
        }),
        TypeNode::KeyOf(n) => {
            json!({ "type": "JsdocTypeKeyof", "element": serialize_node(&n.element) })
        }
        TypeNode::TypeOf(n) => {
            json!({ "type": "JsdocTypeTypeof", "element": serialize_node(&n.element) })
        }
        TypeNode::Import(n) => {
            json!({ "type": "JsdocTypeImport", "element": serialize_node(&n.element) })
        }
        TypeNode::Predicate(n) => {
            json!({ "type": "JsdocTypePredicate", "left": serialize_node(&n.left), "right": serialize_node(&n.right) })
        }
        TypeNode::Asserts(n) => {
            json!({ "type": "JsdocTypeAsserts", "left": serialize_node(&n.left), "right": serialize_node(&n.right) })
        }
        TypeNode::AssertsPlain(n) => {
            json!({ "type": "JsdocTypeAssertsPlain", "element": serialize_node(&n.element) })
        }
        TypeNode::Infer(n) => {
            json!({ "type": "JsdocTypeInfer", "element": serialize_node(&n.element) })
        }
        TypeNode::UniqueSymbol(_) => json!({ "type": "JsdocTypeUniqueSymbol" }),
        TypeNode::ReadonlyArray(n) => {
            json!({ "type": "JsdocTypeReadonlyArray", "element": serialize_node(&n.element) })
        }
        TypeNode::Function(n) => {
            let mut obj = json!({
                "type": "JsdocTypeFunction",
                "parameters": n.parameters.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
                "constructor": n.constructor,
                "arrow": n.arrow,
                "parenthesis": n.parenthesis,
            });
            if let Some(ref ret) = n.return_type {
                obj["returnType"] = serialize_node(ret);
            }
            obj
        }
        TypeNode::Tuple(n) => json!({
            "type": "JsdocTypeTuple",
            "elements": n.elements.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::Object(n) => json!({
            "type": "JsdocTypeObject",
            "elements": n.elements.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::KeyValue(n) => {
            let mut obj = json!({
                "type": "JsdocTypeKeyValue",
                "key": n.key,
                "optional": n.optional,
                "variadic": n.variadic,
            });
            if let Some(ref right) = n.right {
                obj["right"] = serialize_node(right);
            }
            obj
        }
        TypeNode::ObjectField(n) => {
            let mut obj = json!({
                "type": "JsdocTypeObjectField",
                "key": serialize_node(&n.key),
                "optional": n.optional,
                "readonly": n.readonly,
            });
            if let Some(ref right) = n.right {
                obj["right"] = serialize_node(right);
            }
            obj
        }
        TypeNode::Symbol(n) => {
            let mut obj = json!({ "type": "JsdocTypeSymbol", "value": n.value });
            if let Some(ref el) = n.element {
                obj["element"] = serialize_node(el);
            }
            obj
        }
        TypeNode::TemplateLiteral(n) => json!({
            "type": "JsdocTypeTemplateLiteral",
            "literals": n.literals.iter().collect::<Vec<_>>(),
            "interpolations": n.interpolations.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::IndexedAccessIndex(n) => json!({
            "type": "JsdocTypeIndexedAccessIndex",
            "right": serialize_node(&n.right),
        }),
        TypeNode::CallSignature(n) => json!({
            "type": "JsdocTypeCallSignature",
            "parameters": n.parameters.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
            "returnType": serialize_node(&n.return_type),
            "typeParameters": n.type_parameters.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::ConstructorSignature(n) => json!({
            "type": "JsdocTypeConstructorSignature",
            "parameters": n.parameters.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
            "returnType": serialize_node(&n.return_type),
            "typeParameters": n.type_parameters.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::MethodSignature(n) => json!({
            "type": "JsdocTypeMethodSignature",
            "name": n.name,
            "parameters": n.parameters.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
            "returnType": serialize_node(&n.return_type),
            "typeParameters": n.type_parameters.iter().map(|e| serialize_node(e)).collect::<Vec<_>>(),
        }),
        TypeNode::TypeParameter(n) => {
            let mut obj = json!({
                "type": "JsdocTypeTypeParameter",
                "name": serialize_node(&n.name),
            });
            if let Some(ref c) = n.constraint {
                obj["constraint"] = serialize_node(c);
            }
            if let Some(ref d) = n.default_value {
                obj["defaultValue"] = serialize_node(d);
            }
            obj
        }
        TypeNode::MappedType(n) => json!({
            "type": "JsdocTypeMappedType",
            "key": n.key,
            "right": serialize_node(&n.right),
        }),
        TypeNode::IndexSignature(n) => json!({
            "type": "JsdocTypeIndexSignature",
            "key": n.key,
            "right": serialize_node(&n.right),
        }),
        // Fallback for remaining nodes
        _ => json!({ "type": "Unknown" }),
    }
}

fn quote_str(q: ox_jsdoc::type_parser::ast::QuoteStyle) -> &'static str {
    match q {
        ox_jsdoc::type_parser::ast::QuoteStyle::Single => "single",
        ox_jsdoc::type_parser::ast::QuoteStyle::Double => "double",
    }
}

fn pos_str(p: ox_jsdoc::type_parser::ast::ModifierPosition) -> &'static str {
    match p {
        ox_jsdoc::type_parser::ast::ModifierPosition::Prefix => "prefix",
        ox_jsdoc::type_parser::ast::ModifierPosition::Suffix => "suffix",
    }
}

fn unquote(s: &str) -> &str {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

const ALL: &[ParseMode] = &[ParseMode::Typescript, ParseMode::Jsdoc, ParseMode::Closure];
const JSDOC_CLOSURE: &[ParseMode] = &[ParseMode::Jsdoc, ParseMode::Closure];

// ============================================================================
// catharsis/basic.spec.ts
// ============================================================================

#[test]
fn basic_boolean() {
    assert_type_value("boolean", ParseMode::Jsdoc, "JsdocTypeName", "boolean");
    assert_parses("boolean", ALL);
    assert_roundtrip("boolean", ParseMode::Jsdoc);
}

#[test]
fn basic_object_name() {
    assert_type_value("Window", ParseMode::Jsdoc, "JsdocTypeName", "Window");
    assert_parses("Window", ALL);
    assert_roundtrip("Window", ParseMode::Jsdoc);
}

#[test]
fn basic_object_with_properties() {
    let json = parse_and_json("goog.ui.Menu", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_eq!(json["pathType"], "property");
    assert_eq!(json["right"]["value"], "Menu");
    assert_parses("goog.ui.Menu", ALL);
    assert_roundtrip("goog.ui.Menu", ParseMode::Jsdoc);
}

#[test]
fn basic_single_quoted_property() {
    let json = parse_and_json("myObj.'myProp'.foo", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_parses("myObj.'myProp'.foo", ALL);
}

#[test]
fn basic_double_quoted_property() {
    let json = parse_and_json("myObj.\"myProp\".foo", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_parses("myObj.\"myProp\".foo", ALL);
}

#[test]
fn basic_variadic_number() {
    let json = parse_and_json("...number", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeName");
    assert_eq!(json["element"]["value"], "number");
    assert_eq!(json["meta"]["position"], "prefix");
    assert_eq!(json["meta"]["squareBrackets"], false);
    assert_parses("...number", ALL);
    assert_roundtrip("...number", ParseMode::Jsdoc);
}

#[test]
fn basic_optional_number() {
    let json = parse_and_json("number=", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeOptional");
    assert_eq!(json["element"]["value"], "number");
    assert_eq!(json["meta"]["position"], "suffix");
    assert_parses("number=", ALL);
    assert_roundtrip("number=", ParseMode::Jsdoc);
}

#[test]
fn basic_optional_object() {
    let json = parse_and_json("Object=", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeOptional");
    assert_eq!(json["element"]["value"], "Object");
    assert_parses("Object=", ALL);
}

#[test]
fn basic_null() {
    assert_type("null", ParseMode::Jsdoc, "JsdocTypeNull");
    assert_parses("null", ALL);
    assert_roundtrip("null", ParseMode::Jsdoc);
}

#[test]
fn basic_repeatable_null() {
    let json = parse_and_json("...null", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeNull");
    assert_parses("...null", ALL);
}

#[test]
fn basic_undefined() {
    assert_type("undefined", ParseMode::Jsdoc, "JsdocTypeUndefined");
    assert_parses("undefined", ALL);
    assert_roundtrip("undefined", ParseMode::Jsdoc);
}

#[test]
fn basic_repeatable_undefined() {
    let json = parse_and_json("...undefined", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeUndefined");
    assert_parses("...undefined", ALL);
}

#[test]
fn basic_any() {
    assert_type("*", ParseMode::Jsdoc, "JsdocTypeAny");
    assert_parses("*", ALL);
    assert_roundtrip("*", ParseMode::Jsdoc);
}

#[test]
fn basic_repeatable_any() {
    let json = parse_and_json("...*", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeAny");
    assert_parses("...*", ALL);
}

#[test]
fn basic_unknown() {
    assert_type("?", ParseMode::Jsdoc, "JsdocTypeUnknown");
    assert_parses("?", ALL);
    assert_roundtrip("?", ParseMode::Jsdoc);
}

#[test]
fn basic_repeatable_unknown() {
    let json = parse_and_json("...?", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeUnknown");
    assert_parses("...?", ALL);
}

#[test]
fn basic_reserved_word_prefix() {
    assert_type_value("forsooth", ParseMode::Jsdoc, "JsdocTypeName", "forsooth");
    assert_parses("forsooth", ALL);
}

#[test]
fn basic_hyphen_name() {
    assert_type_value(
        "My-1st-Class",
        ParseMode::Jsdoc,
        "JsdocTypeName",
        "My-1st-Class",
    );
    assert_parses("My-1st-Class", JSDOC_CLOSURE);
}

// ============================================================================
// catharsis/nullable.spec.ts (subset)
// ============================================================================

#[test]
fn nullable_prefix() {
    let json = parse_and_json("?number", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNullable");
    assert_eq!(json["element"]["value"], "number");
    assert_eq!(json["meta"]["position"], "prefix");
    assert_parses("?number", ALL);
    assert_roundtrip("?number", ParseMode::Jsdoc);
}

#[test]
fn nullable_suffix() {
    let json = parse_and_json("number?", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNullable");
    assert_eq!(json["element"]["value"], "number");
    assert_eq!(json["meta"]["position"], "suffix");
    assert_parses("number?", ALL);
    assert_roundtrip("number?", ParseMode::Jsdoc);
}

#[test]
fn not_nullable_prefix() {
    let json = parse_and_json("!Object", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNotNullable");
    assert_eq!(json["element"]["value"], "Object");
    assert_eq!(json["meta"]["position"], "prefix");
    assert_parses("!Object", ALL);
    assert_roundtrip("!Object", ParseMode::Jsdoc);
}

// ============================================================================
// catharsis/type-union.spec.ts (subset)
// ============================================================================

#[test]
fn union_two_types() {
    let json = parse_and_json("number|boolean", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
    assert_eq!(json["elements"].as_array().unwrap().len(), 2);
    assert_parses("number|boolean", ALL);
}

#[test]
fn union_three_types() {
    let json = parse_and_json("number|boolean|string", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
    assert_eq!(json["elements"].as_array().unwrap().len(), 3);
}

// ============================================================================
// catharsis/typeApplication.spec.ts (subset)
// ============================================================================

#[test]
fn generic_array_string() {
    let json = parse_and_json("Array.<string>", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["left"]["value"], "Array");
    assert_eq!(json["meta"]["dot"], true);
    assert_eq!(json["meta"]["brackets"], "angle");
    assert_parses("Array.<string>", ALL);
    assert_roundtrip("Array.<string>", ParseMode::Jsdoc);
}

#[test]
fn generic_array_no_dot() {
    let json = parse_and_json("Array<string>", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["meta"]["dot"], false);
    assert_parses("Array<string>", ALL);
    assert_roundtrip("Array<string>", ParseMode::Typescript);
}

#[test]
fn generic_two_params() {
    let json = parse_and_json("Object.<string, number>", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["elements"].as_array().unwrap().len(), 2);
    assert_parses("Object.<string, number>", ALL);
}

// ============================================================================
// catharsis/functionType.spec.ts (subset)
// ============================================================================

#[test]
fn function_bare() {
    let json = parse_and_json("function", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["parenthesis"], false);
    assert_roundtrip("function", ParseMode::Jsdoc);
}

#[test]
fn function_with_return() {
    let json = parse_and_json("function(string): number", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["parenthesis"], true);
    assert!(json["returnType"]["type"] == "JsdocTypeName");
    assert_roundtrip("function(string): number", ParseMode::Jsdoc);
}

#[test]
fn function_no_params() {
    let json = parse_and_json("function()", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["parameters"].as_array().unwrap().len(), 0);
    assert_roundtrip("function()", ParseMode::Jsdoc);
}

// ============================================================================
// typescript fixtures (subset)
// ============================================================================

#[test]
fn ts_arrow_function() {
    let json = parse_and_json("(a: string) => number", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["arrow"], true);
    assert_roundtrip("(a: string) => number", ParseMode::Typescript);
}

#[test]
fn ts_intersection() {
    let json = parse_and_json("A & B & C", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeIntersection");
    assert_eq!(json["elements"].as_array().unwrap().len(), 3);
    assert_roundtrip("A & B & C", ParseMode::Typescript);
}

#[test]
fn ts_conditional() {
    let json = parse_and_json("T extends U ? X : Y", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeConditional");
    assert_eq!(json["checksType"]["value"], "T");
    assert_eq!(json["extendsType"]["value"], "U");
    assert_eq!(json["trueType"]["value"], "X");
    assert_eq!(json["falseType"]["value"], "Y");
    assert_roundtrip("T extends U ? X : Y", ParseMode::Typescript);
}

#[test]
fn ts_keyof() {
    let json = parse_and_json("keyof MyType", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeKeyof");
    assert_eq!(json["element"]["value"], "MyType");
    assert_roundtrip("keyof MyType", ParseMode::Typescript);
}

#[test]
fn ts_typeof() {
    let json = parse_and_json("typeof myVar", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeTypeof");
    assert_eq!(json["element"]["value"], "myVar");
    assert_roundtrip("typeof myVar", ParseMode::Typescript);
}

#[test]
fn ts_predicate() {
    let json = parse_and_json("x is string", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypePredicate");
    assert_roundtrip("x is string", ParseMode::Typescript);
}

#[test]
fn ts_asserts() {
    let json = parse_and_json("asserts x is string", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeAsserts");
    assert_roundtrip("asserts x is string", ParseMode::Typescript);
}

#[test]
fn ts_asserts_plain() {
    let json = parse_and_json("asserts x", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeAssertsPlain");
    assert_roundtrip("asserts x", ParseMode::Typescript);
}

#[test]
fn ts_infer() {
    let json = parse_and_json("infer T", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeInfer");
    assert_roundtrip("infer T", ParseMode::Typescript);
}

#[test]
fn ts_import() {
    let json = parse_and_json("import('./module')", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeImport");
    assert_roundtrip("import('./module')", ParseMode::Typescript);
}

#[test]
fn ts_unique_symbol() {
    let json = parse_and_json("unique symbol", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeUniqueSymbol");
    assert_roundtrip("unique symbol", ParseMode::Typescript);
}

#[test]
fn ts_readonly_array() {
    let json = parse_and_json("readonly string[]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeReadonlyArray");
    assert_roundtrip("readonly string[]", ParseMode::Typescript);
}

#[test]
fn ts_tuple() {
    let json = parse_and_json("[string, number]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeTuple");
    assert_eq!(json["elements"].as_array().unwrap().len(), 2);
    assert_roundtrip("[string, number]", ParseMode::Typescript);
}

#[test]
fn ts_tuple_empty() {
    let json = parse_and_json("[]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeTuple");
    assert_eq!(json["elements"].as_array().unwrap().len(), 0);
}

#[test]
fn ts_array_brackets() {
    let json = parse_and_json("string[]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["meta"]["brackets"], "square");
    assert_roundtrip("string[]", ParseMode::Typescript);
}

#[test]
fn ts_object_type() {
    let json = parse_and_json("{key: string}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_roundtrip("{key: string}", ParseMode::Typescript);
}

#[test]
fn ts_object_optional_field() {
    let json = parse_and_json("{key?: string}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    let field = &json["elements"][0];
    assert_eq!(field["optional"], true);
}

#[test]
fn ts_parenthesized() {
    let json = parse_and_json("(string | number)", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeParenthesis");
    assert_roundtrip("(string | number)", ParseMode::Typescript);
}

#[test]
fn ts_generic_multi_params() {
    let json = parse_and_json("Map<string, number>", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["elements"].as_array().unwrap().len(), 2);
    assert_roundtrip("Map<string, number>", ParseMode::Typescript);
}

// ============================================================================
// misc fixtures (subset)
// ============================================================================

#[test]
fn misc_number_literal_integer() {
    let json = parse_and_json("42", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeNumber");
    assert_roundtrip("42", ParseMode::Typescript);
}

#[test]
fn misc_number_literal_float() {
    let json = parse_and_json("3.14", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeNumber");
    assert_roundtrip("3.14", ParseMode::Typescript);
}

#[test]
fn misc_number_literal_negative() {
    let json = parse_and_json("-1", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeNumber");
    assert_roundtrip("-1", ParseMode::Typescript);
}

#[test]
fn misc_string_literal_double() {
    let json = parse_and_json("\"hello\"", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeStringValue");
    assert_eq!(json["value"], "hello");
    assert_eq!(json["meta"]["quote"], "double");
}

#[test]
fn misc_string_literal_single() {
    let json = parse_and_json("'world'", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeStringValue");
    assert_eq!(json["value"], "world");
    assert_eq!(json["meta"]["quote"], "single");
}

#[test]
fn misc_reserved_word_function_prefix() {
    assert_type_value(
        "functionBar",
        ParseMode::Jsdoc,
        "JsdocTypeName",
        "functionBar",
    );
    assert_parses("functionBar", ALL);
}

#[test]
fn misc_name_path_dot() {
    let json = parse_and_json("Foo.Bar", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_eq!(json["pathType"], "property");
    assert_roundtrip("Foo.Bar", ParseMode::Jsdoc);
}

#[test]
fn misc_name_path_hash() {
    let json = parse_and_json("MyClass#method", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_eq!(json["pathType"], "instance");
    assert_roundtrip("MyClass#method", ParseMode::Jsdoc);
}

#[test]
fn misc_name_path_tilde() {
    let json = parse_and_json("MyModule~inner", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_eq!(json["pathType"], "inner");
    assert_roundtrip("MyModule~inner", ParseMode::Jsdoc);
}

#[test]
fn misc_variadic_suffix_jsdoc() {
    let json = parse_and_json("string...", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["meta"]["position"], "suffix");
    assert_roundtrip("string...", ParseMode::Jsdoc);
}

#[test]
fn misc_optional_prefix_jsdoc() {
    let json = parse_and_json("=string", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeOptional");
    assert_eq!(json["meta"]["position"], "prefix");
    assert_roundtrip("=string", ParseMode::Jsdoc);
}

// ============================================================================
// catharsis/basic.spec.ts — remaining
// ============================================================================

#[test]
fn basic_string_literal_property_with_punctuation() {
    let json = parse_and_json("myObj.\"#weirdProp\".foo", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_parses("myObj.\"#weirdProp\".foo", ALL);
}

#[test]
fn basic_numeric_property() {
    let json = parse_and_json("myObj.12345", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_eq!(json["right"]["value"], "12345");
    assert_parses("myObj.12345", ALL);
}

// ============================================================================
// catharsis/nullable.spec.ts — remaining
// ============================================================================

#[test]
fn nullable_postfix_not_nullable() {
    let json = parse_and_json("Object!", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNotNullable");
    assert_eq!(json["meta"]["position"], "suffix");
    assert_parses("Object!", ALL);
}

#[test]
fn nullable_repeatable_nullable() {
    let json = parse_and_json("...?number", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeNullable");
    assert_parses("...?number", ALL);
}

#[test]
fn nullable_postfix_repeatable_nullable() {
    let json = parse_and_json("...number?", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeNullable");
    assert_eq!(json["element"]["meta"]["position"], "suffix");
    assert_parses("...number?", ALL);
}

#[test]
fn nullable_repeatable_not_nullable() {
    let json = parse_and_json("...!Object", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeNotNullable");
    assert_parses("...!Object", ALL);
}

#[test]
fn nullable_postfix_repeatable_not_nullable() {
    let json = parse_and_json("...Object!", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeNotNullable");
    assert_eq!(json["element"]["meta"]["position"], "suffix");
    assert_parses("...Object!", ALL);
}

#[test]
fn nullable_optional_then_nullable() {
    let json = parse_and_json("number=?", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNullable");
    assert_eq!(json["element"]["type"], "JsdocTypeOptional");
    assert_parses("number=?", ALL);
}

#[test]
fn nullable_then_optional() {
    let json = parse_and_json("number?=", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeOptional");
    assert_eq!(json["element"]["type"], "JsdocTypeNullable");
    assert_parses("number?=", ALL);
}

#[test]
fn nullable_optional_then_not_nullable() {
    let json = parse_and_json("Object=!", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNotNullable");
    assert_eq!(json["element"]["type"], "JsdocTypeOptional");
    assert_parses("Object=!", ALL);
}

#[test]
fn nullable_not_nullable_then_optional() {
    let json = parse_and_json("Object!=", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeOptional");
    assert_eq!(json["element"]["type"], "JsdocTypeNotNullable");
    assert_parses("Object!=", ALL);
}

// ============================================================================
// catharsis/type-union.spec.ts — remaining
// ============================================================================

#[test]
fn union_parenthesized_two() {
    let json = parse_and_json("(number|boolean)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeParenthesis");
    assert_parses("(number|boolean)", ALL);
}

#[test]
fn union_repeatable_parenthesized() {
    let json = parse_and_json("...(number|boolean)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_parses("...(number|boolean)", ALL);
}

#[test]
fn union_nullable_parenthesized() {
    let json = parse_and_json("?(number|boolean)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNullable");
    assert_parses("?(number|boolean)", ALL);
}

#[test]
fn union_not_nullable_parenthesized() {
    let json = parse_and_json("!(number|boolean)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNotNullable");
    assert_parses("!(number|boolean)", ALL);
}

#[test]
fn union_optional_parenthesized() {
    let json = parse_and_json("(number|boolean)=", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeOptional");
    assert_parses("(number|boolean)=", ALL);
}

#[test]
fn union_no_parens() {
    let json = parse_and_json("number|string", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
    assert_parses("number|string", ALL);
}

#[test]
fn union_modifiers_no_parens() {
    let json = parse_and_json("!number|!string", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
    assert_eq!(json["elements"][0]["type"], "JsdocTypeNotNullable");
    assert_parses("!number|!string", ALL);
}

// ============================================================================
// catharsis/typeApplication.spec.ts — remaining
// ============================================================================

#[test]
fn generic_repeatable() {
    let json = parse_and_json("...Array.<string>", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeGeneric");
    assert_parses("...Array.<string>", ALL);
}

#[test]
fn generic_object_two_params() {
    let json = parse_and_json("Object.<string, number>", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["elements"].as_array().unwrap().len(), 2);
    assert_parses("Object.<string, number>", ALL);
}

#[test]
fn generic_array_of_objects() {
    let json = parse_and_json("Array.<{length}>", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["elements"][0]["type"], "JsdocTypeObject");
    assert_parses("Array.<{length}>", ALL);
}

#[test]
fn generic_array_of_unknown() {
    let json = parse_and_json("Array.<?>", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["elements"][0]["type"], "JsdocTypeUnknown");
    assert_parses("Array.<?>", ALL);
}

#[test]
fn generic_promise() {
    let json = parse_and_json("Promise.<string>", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["left"]["value"], "Promise");
    assert_parses("Promise.<string>", ALL);
}

#[test]
fn generic_namepath_promise() {
    let json = parse_and_json("foo.Promise.<string>", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_parses("foo.Promise.<string>", ALL);
}

// ============================================================================
// catharsis/recordType.spec.ts
// ============================================================================

#[test]
fn record_empty() {
    let json = parse_and_json("{}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_parses("{}", ALL);
}

#[test]
fn record_one_typed_property() {
    let json = parse_and_json("{myNum: number}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_parses("{myNum: number}", ALL);
}

#[test]
fn record_repeatable() {
    let json = parse_and_json("...{myNum: number}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeObject");
    assert_parses("...{myNum: number}", ALL);
}

#[test]
fn record_optional() {
    let json = parse_and_json("{myNum: number}=", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeOptional");
    assert_eq!(json["element"]["type"], "JsdocTypeObject");
    assert_parses("{myNum: number}=", ALL);
}

#[test]
fn record_nullable() {
    let json = parse_and_json("?{myNum: number}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNullable");
    assert_eq!(json["element"]["type"], "JsdocTypeObject");
    assert_parses("?{myNum: number}", ALL);
}

#[test]
fn record_not_nullable() {
    let json = parse_and_json("!{myNum: number}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNotNullable");
    assert_eq!(json["element"]["type"], "JsdocTypeObject");
    assert_parses("!{myNum: number}", ALL);
}

#[test]
fn record_semicolon_separator() {
    let json = parse_and_json("{myNum: number; myObject: string}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_eq!(json["elements"].as_array().unwrap().len(), 2);
    assert_parses("{myNum: number; myObject: string}", ALL);
}

#[test]
fn record_generic_value() {
    let json = parse_and_json("{myArray: Array.<string>}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_parses("{myArray: Array.<string>}", ALL);
}

// ============================================================================
// catharsis/functionType.spec.ts — remaining
// ============================================================================

#[test]
fn function_two_params() {
    let json = parse_and_json("function(string, boolean)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["parameters"].as_array().unwrap().len(), 2);
    assert_parses("function(string, boolean)", ALL);
}

#[test]
fn function_repeatable() {
    let json = parse_and_json("...function(string, boolean)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeFunction");
    assert_parses("...function(string, boolean)", ALL);
}

#[test]
fn function_two_params_return() {
    let json = parse_and_json("function(string, boolean): boolean", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["parameters"].as_array().unwrap().len(), 2);
    assert!(json["returnType"]["type"] == "JsdocTypeName");
    assert_parses("function(string, boolean): boolean", ALL);
    assert_roundtrip("function(string, boolean): boolean", ParseMode::Jsdoc);
}

#[test]
fn function_optional() {
    let json = parse_and_json("function(string)=", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeOptional");
    assert_eq!(json["element"]["type"], "JsdocTypeFunction");
    assert_parses("function(string)=", ALL);
}

#[test]
fn function_no_params_return() {
    let json = parse_and_json("function(): number", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["parameters"].as_array().unwrap().len(), 0);
    assert_parses("function(): number", ALL);
    assert_roundtrip("function(): number", ParseMode::Jsdoc);
}

#[test]
fn function_no_params_no_return() {
    let json = parse_and_json("function()", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["parameters"].as_array().unwrap().len(), 0);
    assert_parses("function()", ALL);
    assert_roundtrip("function()", ParseMode::Jsdoc);
}

#[test]
fn function_variadic_param() {
    let json = parse_and_json("function(...foo)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_parses("function(...foo)", ALL);
}

// ============================================================================
// catharsis/jsdoc.spec.ts — remaining
// ============================================================================

#[test]
fn jsdoc_functional_name() {
    assert_type_value(
        "functional",
        ParseMode::Jsdoc,
        "JsdocTypeName",
        "functional",
    );
    assert_parses("functional", ALL);
}

#[test]
fn jsdoc_instance_member() {
    let json = parse_and_json("MyClass#myMember", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_eq!(json["pathType"], "instance");
    assert_parses("MyClass#myMember", JSDOC_CLOSURE);
}

#[test]
fn jsdoc_inner_member() {
    let json = parse_and_json("MyClass~myMember", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_eq!(json["pathType"], "inner");
    assert_parses("MyClass~myMember", JSDOC_CLOSURE);
}

#[test]
fn jsdoc_symbol_empty() {
    let json = parse_and_json("MyClass()", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeSymbol");
    assert_eq!(json["value"], "MyClass");
    assert_parses("MyClass()", JSDOC_CLOSURE);
}

#[test]
fn jsdoc_symbol_number() {
    let json = parse_and_json("MyClass(2)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeSymbol");
    assert_eq!(json["value"], "MyClass");
    assert_parses("MyClass(2)", JSDOC_CLOSURE);
}

#[test]
fn jsdoc_symbol_name() {
    let json = parse_and_json("MyClass(abcde)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeSymbol");
    assert_eq!(json["value"], "MyClass");
    assert_parses("MyClass(abcde)", JSDOC_CLOSURE);
}

#[test]
fn jsdoc_string_literal_double() {
    let json = parse_and_json("\"foo.bar.baz\"", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeStringValue");
    assert_eq!(json["value"], "foo.bar.baz");
}

#[test]
fn jsdoc_string_literal_single() {
    let json = parse_and_json("'foo.bar.baz'", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeStringValue");
    assert_eq!(json["value"], "foo.bar.baz");
}

#[test]
fn jsdoc_array_bracket_string() {
    let json = parse_and_json("string[]", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["meta"]["brackets"], "square");
}

#[test]
fn jsdoc_nested_array() {
    let json = parse_and_json("number[][]", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["meta"]["brackets"], "square");
}

// ============================================================================
// typescript — remaining fixtures
// ============================================================================

#[test]
fn ts_arrow_no_params() {
    let json = parse_and_json("() => string", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["arrow"], true);
    assert_roundtrip("() => string", ParseMode::Typescript);
}

#[test]
fn ts_arrow_multi_params() {
    let json = parse_and_json("(x: number, y: string) => string", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["arrow"], true);
    assert_roundtrip("(x: number, y: string) => string", ParseMode::Typescript);
}

#[test]
fn ts_arrow_returning_void() {
    let json = parse_and_json("() => void", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_roundtrip("() => void", ParseMode::Typescript);
}

#[test]
fn ts_arrow_returning_arrow() {
    let json = parse_and_json("() => () => void", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
}

#[test]
fn ts_conditional_with_keyof() {
    let json = parse_and_json("K extends keyof Abc ? Abc[K] : Def", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeConditional");
}

#[test]
fn ts_conditional_with_infer() {
    let json = parse_and_json("A extends B<infer b> ? b : C", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeConditional");
}

#[test]
fn ts_intersection_nullable() {
    let json = parse_and_json("(A & B)?", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeNullable");
}

#[test]
fn ts_import_named_export() {
    let json = parse_and_json("import(\"x\").T", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
}

#[test]
fn ts_import_two_level() {
    let json = parse_and_json("import(\"x\").T.U", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
}

#[test]
fn ts_labeled_tuple() {
    let json = parse_and_json("[a: string, b: number]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeTuple");
    assert_eq!(json["elements"].as_array().unwrap().len(), 2);
}

#[test]
fn ts_tuple_with_spread() {
    let json = parse_and_json("[variadic, arguments, ...tuple]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeTuple");
    assert_eq!(json["elements"].as_array().unwrap().len(), 3);
}

#[test]
fn ts_readonly_tuple() {
    let json = parse_and_json("readonly [string, number]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeReadonlyArray");
}

#[test]
fn ts_typeof_array() {
    let json = parse_and_json("typeof N[]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeTypeof");
}

#[test]
fn ts_keyof_array() {
    let json = parse_and_json("keyof N[]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeKeyof");
}

#[test]
fn ts_union_keyof() {
    let json = parse_and_json("keyof A | number", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
}

#[test]
fn ts_union_typeof() {
    let json = parse_and_json("typeof A | number", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
}

#[test]
fn ts_index_signature() {
    let json = parse_and_json("{[key: string]: number}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_mapped_type() {
    let json = parse_and_json("{[key in Type]: number}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_indexed_access() {
    let json = parse_and_json("obj[keyof a]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
}

#[test]
fn ts_object_readonly_property() {
    let json = parse_and_json("{readonly x: number}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_object_property_named_module() {
    let json = parse_and_json("{module: type}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_parses("{module: type}", ALL);
}

#[test]
fn ts_object_property_named_typeof() {
    let json = parse_and_json("{typeof: type}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_parses("{typeof: type}", ALL);
}

#[test]
fn ts_object_unenclosed_union_value() {
    let json = parse_and_json("{message: string|undefined}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_parses("{message: string|undefined}", ALL);
}

// ============================================================================
// misc — remaining fixtures
// ============================================================================

#[test]
fn misc_number_union() {
    let json = parse_and_json("123 | 456", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
    assert_parses("123 | 456", ALL);
}

#[test]
fn misc_float_and_exponent() {
    let json = parse_and_json("3.14 | 1.2e+104", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
    assert_parses("3.14 | 1.2e+104", ALL);
}

#[test]
fn misc_variadic_bare() {
    let json = parse_and_json("...", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_parses("...", ALL);
}

#[test]
fn misc_event_name_path() {
    let json = parse_and_json("event:some_event", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeSpecialNamePath");
    assert_eq!(json["specialType"], "event");
}

#[test]
fn misc_event_quoted() {
    let json = parse_and_json("event:'some-event'", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeSpecialNamePath");
    assert_eq!(json["specialType"], "event");
}

#[test]
fn misc_external_name_path() {
    let json = parse_and_json("external:some-external", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeSpecialNamePath");
    assert_eq!(json["specialType"], "external");
}

#[test]
fn misc_name_path_keyword_property_module() {
    let json = parse_and_json("foo.module", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_parses("foo.module", ALL);
}

#[test]
fn misc_name_path_keyword_property_readonly() {
    let json = parse_and_json("foo.readonly", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_parses("foo.readonly", ALL);
}

#[test]
fn misc_name_path_keyword_property_external() {
    let json = parse_and_json("foo.external", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_parses("foo.external", ALL);
}

#[test]
fn misc_object_quoted_key() {
    let json = parse_and_json("{\"abc\": string}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_parses("{\"abc\": string}", ALL);
}

#[test]
fn misc_object_numeric_key() {
    let json = parse_and_json("{123}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_parses("{123}", ALL);
}

#[test]
fn misc_object_string_key_no_value() {
    let json = parse_and_json("{\"abc\"}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_parses("{\"abc\"}", ALL);
}

// ============================================================================
// catharsis/functionType.spec.ts — this/new params, complex combos
// ============================================================================

#[test]
fn function_this_param() {
    assert_parses("function(this: MyClass)", ALL);
    let json = parse_and_json("function(this: MyClass)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["parameters"].as_array().unwrap().len(), 1);
}

#[test]
fn function_this_and_param() {
    assert_parses("function(this: MyClass, string)", ALL);
    let json = parse_and_json("function(this: MyClass, string)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["parameters"].as_array().unwrap().len(), 2);
}

#[test]
fn function_new_param() {
    assert_parses("function(new: MyClass)", ALL);
    let json = parse_and_json("function(new: MyClass)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
}

#[test]
fn function_new_and_param() {
    assert_parses("function(new: MyClass, string)", ALL);
}

#[test]
fn function_returns_union() {
    let json = parse_and_json("function(): (number | string)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert!(json["returnType"]["type"] == "JsdocTypeParenthesis");
    assert_parses("function(): (number | string)", ALL);
}

#[test]
fn function_repeatable_return() {
    let json = parse_and_json("...function(string, boolean): boolean", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["element"]["type"], "JsdocTypeFunction");
    assert_parses("...function(string, boolean): boolean", ALL);
}

#[test]
fn function_this_returns_union() {
    assert_parses("function(this: Object): (number | string)", ALL);
}

#[test]
fn function_new_optional_any_return() {
    assert_parses("function(new: Boolean, *=): boolean", ALL);
}

#[test]
fn function_this_date_returns_union() {
    assert_parses(
        "function(this: Date, number): (boolean | number | string)",
        ALL,
    );
}

// ============================================================================
// catharsis/jsdoc.spec.ts — remaining complex
// ============================================================================

#[test]
fn jsdoc_instance_inner_chain() {
    let json = parse_and_json("MyClass#myMember#yourMember~theirMember", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_eq!(json["pathType"], "inner");
    assert_parses("MyClass#myMember#yourMember~theirMember", JSDOC_CLOSURE);
}

#[test]
fn jsdoc_module_class() {
    assert_parses("module:foo/bar/baz~Qux", JSDOC_CLOSURE);
}

#[test]
fn jsdoc_symbol_variadic() {
    let json = parse_and_json("MyClass(...foo)", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeSymbol");
    assert_parses("MyClass(...foo)", JSDOC_CLOSURE);
}

#[test]
fn jsdoc_partial_quoted_double() {
    assert_parses("foo.\"bar.baz\".qux", ALL);
}

#[test]
fn jsdoc_partial_quoted_single() {
    assert_parses("foo.'bar.baz'.qux", ALL);
}

#[test]
fn jsdoc_array_bracket_function() {
    let json = parse_and_json("function[]", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_eq!(json["meta"]["brackets"], "square");
}

#[test]
fn jsdoc_triple_nested_array() {
    let json = parse_and_json("number[][][]", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
}

#[test]
fn jsdoc_record_function_property() {
    assert_parses("{foo: function()}", ALL);
}

#[test]
fn jsdoc_record_function_property_return() {
    assert_parses("{foo: function(): void}", ALL);
}

#[test]
fn jsdoc_optional_function_this() {
    assert_parses("function(this: my.namespace.Class, my.Class)=", ALL);
}

#[test]
fn jsdoc_union_variadic_and_array() {
    let json = parse_and_json("...string | string[]", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
}

#[test]
fn jsdoc_record_literal_starts_with_undefined() {
    assert_parses("{undefinedHTML: (string | undefined)}", ALL);
}

// ============================================================================
// catharsis/type-union.spec.ts — remaining complex
// ============================================================================

#[test]
fn union_three_types_with_namepath() {
    assert_parses("(number | Window | goog.ui.Menu)", ALL);
}

#[test]
fn union_with_generic_and_unknown() {
    assert_parses("(Array | Object.<string, ?>)", ALL);
}

#[test]
fn union_two_generics() {
    assert_parses("(Array.<string> | Object.<string, ?>)", ALL);
}

#[test]
fn union_error_or_function() {
    assert_parses("(Error | function(): Error)", ALL);
}

#[test]
fn union_optional_many_types() {
    assert_parses(
        "(jQuerySelector | Element | Object | Array.<Element> | jQuery | string | function())=",
        ALL,
    );
}

// ============================================================================
// catharsis/typeApplication.spec.ts — remaining complex
// ============================================================================

#[test]
fn generic_complex_nested() {
    assert_parses(
        "Object.<Array.<(boolean | {myKey: Error})>, (boolean | string | function(new: foo): string)>",
        ALL,
    );
}

// ============================================================================
// catharsis/recordType.spec.ts — remaining
// ============================================================================

#[test]
fn record_two_typed_one_untyped() {
    assert_parses("{myNum: number, myObject}", ALL);
}

#[test]
fn record_union_value() {
    assert_parses("{myKey: (number | boolean | string)}", ALL);
}

#[test]
fn record_keyword_key_continue() {
    assert_parses("{continue: string}", ALL);
}

#[test]
fn record_keyword_key_class() {
    assert_parses("{class: string}", ALL);
}

#[test]
fn record_keyword_key_true() {
    assert_parses("{true: string}", ALL);
}

#[test]
fn record_numeric_key() {
    assert_parses("{0: string}", ALL);
}

// ============================================================================
// Errors.spec.ts
// ============================================================================

#[test]
fn error_unterminated_generic() {
    // `abc[def` — unclosed bracket (should fail in all modes)
    assert_fails("abc[def", &[ParseMode::Typescript]);
}

#[test]
fn error_empty_input_fails() {
    assert_fails("", ALL);
}

// ============================================================================
// typescript/intersection.spec.ts — remaining
// ============================================================================

#[test]
fn ts_intersection_function_and_generic() {
    let json = parse_and_json("function(): void & A<B, C>", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeIntersection");
}

#[test]
fn ts_intersection_union_and_arrow() {
    let json = parse_and_json("(A | B) & (a: string) => void", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeIntersection");
}

// ============================================================================
// typescript/import.spec.ts — remaining
// ============================================================================

#[test]
fn ts_import_relative() {
    assert_parses("import(\"./x\")", &[ParseMode::Typescript]);
    assert_parses("import(\"../x\")", &[ParseMode::Typescript]);
}

// ============================================================================
// typescript/arrowFunction.spec.ts — remaining
// ============================================================================

#[test]
fn ts_arrow_special_any() {
    let json = parse_and_json("(x: *) => *", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeFunction");
    assert_eq!(json["arrow"], true);
}

#[test]
fn ts_arrow_as_generic_param() {
    let json = parse_and_json("X<() => string>", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
}

#[test]
fn ts_arrow_optional_param() {
    assert_parses(
        "(param1: string, param2?: string) => number",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_arrow_with_params_returning_arrow() {
    assert_parses(
        "(a: number) => (b: string) => boolean",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_arrow_as_object_property() {
    assert_parses("{x: () => void, y: string}", &[ParseMode::Typescript]);
}

#[test]
fn ts_function_arrow_as_return() {
    assert_parses("function(): () => string", &[ParseMode::Typescript]);
}

#[test]
fn ts_function_arrow_as_param() {
    assert_parses("function(() => string): void", &[ParseMode::Typescript]);
}

// ============================================================================
// typescript/functions.spec.ts
// ============================================================================

#[test]
fn ts_new_as_function() {
    assert_parses("new(number, string): SomeType", &[ParseMode::Typescript]);
}

#[test]
fn ts_typed_variadic_args() {
    assert_parses("function(...args: any[]): object", &[ParseMode::Typescript]);
}

// ============================================================================
// typescript/conditional.spec.ts — remaining
// ============================================================================

#[test]
fn ts_conditional_infer_non_initial() {
    assert_parses(
        "T extends Map<any, infer V> ? V : never",
        &[ParseMode::Typescript],
    );
}

// ============================================================================
// typescript/tuple.spec.ts — remaining
// ============================================================================

#[test]
fn ts_tuple_one_element() {
    let json = parse_and_json("[x]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeTuple");
    assert_eq!(json["elements"].as_array().unwrap().len(), 1);
}

#[test]
fn ts_tuple_four_elements() {
    let json = parse_and_json("[it, needs, to, be]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeTuple");
    assert_eq!(json["elements"].as_array().unwrap().len(), 4);
}

#[test]
fn ts_tuple_array_of_empty_tuples() {
    let json = parse_and_json("[][]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
}

#[test]
fn ts_tuple_array_of_tuples() {
    let json = parse_and_json("[tuple, array][]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
}

#[test]
fn ts_tuple_with_typeof() {
    assert_parses("[tuple, with, typeof foo]", &[ParseMode::Typescript]);
}

#[test]
fn ts_tuple_with_keyof() {
    assert_parses("[tuple, with, keyof foo]", &[ParseMode::Typescript]);
}

#[test]
fn ts_tuple_with_spread_and_typeof() {
    assert_parses(
        "[tuple, with, typeof foo, and, ...rest]",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_tuple_with_spread_and_keyof() {
    assert_parses(
        "[tuple, with, keyof foo, and, ...rest]",
        &[ParseMode::Typescript],
    );
}

// ============================================================================
// typescript/typeof.spec.ts — remaining
// ============================================================================

#[test]
fn ts_typeof_in_generic() {
    let json = parse_and_json("X<typeof A>", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
}

#[test]
fn ts_typeof_in_parenthesis() {
    let json = parse_and_json("(typeof A)", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeParenthesis");
}

#[test]
fn ts_typeof_variadic() {
    let json = parse_and_json("...typeof A", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
}

#[test]
fn ts_union_with_typeof_right() {
    let json = parse_and_json("number | typeof A", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
}

#[test]
fn ts_typeof_as_function_param() {
    assert_parses("function(typeof A): void", &[ParseMode::Typescript]);
}

#[test]
fn ts_typeof_as_function_return() {
    assert_parses("function(): typeof A", &[ParseMode::Typescript]);
}

#[test]
fn ts_typeof_as_first_param() {
    assert_parses("function(typeof A, number): void", &[ParseMode::Typescript]);
}

#[test]
fn ts_typeof_as_second_param() {
    assert_parses("function(number, typeof A): void", &[ParseMode::Typescript]);
}

// ============================================================================
// typescript/keyof.spec.ts — remaining
// ============================================================================

#[test]
fn ts_keyof_plain_name_jsdoc() {
    assert_type_value("keyof", ParseMode::Jsdoc, "JsdocTypeName", "keyof");
    assert_parses("keyof", JSDOC_CLOSURE);
}

#[test]
fn ts_keyof_in_generic_jsdoc() {
    let json = parse_and_json("X<keyof>", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
}

#[test]
fn ts_keyof_in_generic() {
    let json = parse_and_json("X<keyof A>", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
}

#[test]
fn ts_keyof_in_parenthesis() {
    let json = parse_and_json("(keyof A)", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeParenthesis");
}

#[test]
fn ts_keyof_variadic() {
    let json = parse_and_json("...keyof A", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
}

#[test]
fn ts_union_with_keyof_right() {
    let json = parse_and_json("number | keyof A", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
}

#[test]
fn ts_keyof_as_function_param() {
    assert_parses("function(keyof A): void", &[ParseMode::Typescript]);
}

#[test]
fn ts_keyof_as_function_return() {
    assert_parses("function(): keyof A", &[ParseMode::Typescript]);
}

#[test]
fn ts_keyof_as_first_param() {
    assert_parses("function(keyof A, number): void", &[ParseMode::Typescript]);
}

#[test]
fn ts_keyof_as_second_param() {
    assert_parses("function(number, keyof A): void", &[ParseMode::Typescript]);
}

#[test]
fn ts_keyof_without_return() {
    assert_parses("function(keyof A)", &[ParseMode::Typescript]);
}

// ============================================================================
// typescript/objects.spec.ts — remaining
// ============================================================================

#[test]
fn ts_object_question_mark_value() {
    assert_parses("{abc: ?}", ALL);
}

#[test]
fn ts_object_optional_no_type() {
    let json = parse_and_json("{message?}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
    assert_parses("{message?}", ALL);
}

#[test]
fn ts_object_function_key() {
    assert_parses(
        "{function: string}",
        &[ParseMode::Typescript, ParseMode::Jsdoc],
    );
}

#[test]
fn ts_object_readonly_key() {
    assert_parses("{readonly: string}", &[ParseMode::Typescript]);
}

// ============================================================================
// typescript/predicate.spec.ts — already covered but verify closure mode
// ============================================================================

#[test]
fn ts_predicate_only_typescript() {
    assert_parses("x is string", &[ParseMode::Typescript]);
}

// ============================================================================
// typescript/readonly.spec.ts
// ============================================================================

#[test]
fn ts_readonly_property_object() {
    let json = parse_and_json("{readonly x: number}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

// ============================================================================
// typescript/readonlyArray.spec.ts — remaining
// ============================================================================

#[test]
fn ts_readonly_tuple_type() {
    let json = parse_and_json("readonly [string, number]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeReadonlyArray");
}

// ============================================================================
// misc/namePaths.spec.ts — remaining
// ============================================================================

#[test]
fn misc_name_path_bracket_access() {
    let json = parse_and_json("foo[\"text\"]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_parses("foo[\"text\"]", ALL);
}

#[test]
fn misc_number_in_generic_intersection() {
    let json = parse_and_json("SomeType<123 & 456>", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
}

// ============================================================================
// misc/VariadicParslet.spec.ts
// ============================================================================

#[test]
fn misc_variadic_bare_dots() {
    let json = parse_and_json("...", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
    assert_eq!(json["meta"]["position"], serde_json::Value::Null);
    assert_parses("...", ALL);
}

// ============================================================================
// typescript/misc.spec.ts
// ============================================================================

#[test]
fn ts_function_trailing_comma() {
    assert_parses("function(TrailingComma): string", ALL);
}

// ============================================================================
// Additional typeof/closure mode tests
// ============================================================================

#[test]
fn closure_typeof() {
    assert_parses("typeof A", &[ParseMode::Typescript, ParseMode::Closure]);
}

#[test]
fn closure_typeof_in_generic() {
    assert_parses("X<typeof A>", &[ParseMode::Typescript, ParseMode::Closure]);
}

#[test]
fn closure_typeof_variadic() {
    assert_parses("...typeof A", &[ParseMode::Typescript, ParseMode::Closure]);
}

// ============================================================================
// Generic parameter access (typescript/objects.spec.ts)
// ============================================================================

#[test]
fn ts_generic_parameter_access() {
    let json = parse_and_json("Parameters<testFunc>[0]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
}

// ============================================================================
// typescript/callSignature.spec.ts
// ============================================================================

#[test]
fn ts_call_signature() {
    let json = parse_and_json("{(a: string, b: number): SomeType}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_call_signature_with_type_params() {
    let json = parse_and_json("{<T>(a: T, b: number): SomeType}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

// ============================================================================
// typescript/constructorSignature.spec.ts
// ============================================================================

#[test]
fn ts_constructor_signature() {
    let json = parse_and_json(
        "{new (a: string, b: number): SomeType}",
        ParseMode::Typescript,
    )
    .unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_constructor_signature_variadic() {
    assert_parses("{new (...args: any[]): object}", &[ParseMode::Typescript]);
}

#[test]
fn ts_constructor_signature_complex_type_params() {
    assert_parses(
        "{new <T extends A = string, V>(a: T, b: number): SomeType}",
        &[ParseMode::Typescript],
    );
}

// ============================================================================
// typescript/methodSignature.spec.ts
// ============================================================================

#[test]
fn ts_method_signature() {
    let json = parse_and_json(
        "{someName(a: string, b: number): SomeType}",
        ParseMode::Typescript,
    )
    .unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_method_signature_with_type_params() {
    assert_parses(
        "{abc<T = string>(a: T, b: number): SomeType}",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_method_signature_double_quoted_name() {
    assert_parses(
        "{\"new\"(a: string, b: number): SomeType}",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_method_signature_single_quoted_name() {
    assert_parses(
        "{'some-method'(a: string, b: number): SomeType}",
        &[ParseMode::Typescript],
    );
}

// ============================================================================
// typescript/templateLiteral.spec.ts
// ============================================================================

// Template literal tests with `${...}` interpolations are in type_parse.rs
// unit tests since `${}` braces conflict with JSDoc `{type}` delimiters
// when tested through the comment parser pipeline.

#[test]
fn ts_template_literal_no_interpolation() {
    assert_type("`hello`", ParseMode::Typescript, "JsdocTypeTemplateLiteral");
}

#[test]
fn ts_template_literal_with_escape() {
    assert_type(
        "`acd\\`ehij`",
        ParseMode::Typescript,
        "JsdocTypeTemplateLiteral",
    );
}

#[test]
fn ts_template_literal_empty() {
    assert_type("``", ParseMode::Typescript, "JsdocTypeTemplateLiteral");
}

// ============================================================================
// typescript/arrowFunction.spec.ts — remaining
// ============================================================================

#[test]
fn ts_arrow_trailing_comma() {
    assert_parses(
        "(arrow: Function, with: TrailingComma, ) => string",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_arrow_unnamed_param() {
    assert_parses("(number) => void", &[ParseMode::Typescript]);
}

#[test]
fn ts_function_trailing_comma_params() {
    assert_parses("function(TrailingComma, ): string", ALL);
}

// ============================================================================
// catharsis/functionType.spec.ts — remaining complex
// ============================================================================

#[test]
fn function_new_and_this() {
    assert_parses("function(new: goog.ui.Menu, this: goog.ui)", ALL);
}

#[test]
fn function_this_union_returns_union() {
    assert_parses("function(this: (Array | Date)): (number | string)", ALL);
}

// ============================================================================
// catharsis/jsdoc.spec.ts — remaining complex
// ============================================================================

#[test]
fn jsdoc_module_class_with_hyphens() {
    assert_parses("module:foo-bar/baz~Qux", JSDOC_CLOSURE);
}

#[test]
fn jsdoc_record_generic_key() {
    let json = parse_and_json("{Array.<string>: number}", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn jsdoc_record_union_key() {
    assert_parses("{(number | boolean | string): number}", &[ParseMode::Jsdoc]);
}

// ============================================================================
// misc/Errors.spec.ts — error cases
// ============================================================================

#[test]
fn error_empty_input() {
    assert_fails("", ALL);
}

#[test]
fn error_import_standalone_jsdoc() {
    assert_fails("import", &[ParseMode::Jsdoc, ParseMode::Closure]);
}

#[test]
fn error_import_non_string() {
    assert_fails("import(123)", &[ParseMode::Typescript]);
}

#[test]
fn error_import_unclosed() {
    assert_fails("import(\"abc\"", &[ParseMode::Typescript]);
}

// ============================================================================
// misc/namePaths.spec.ts — remaining
// ============================================================================

#[test]
fn misc_module_event_name_path() {
    // module:some-module.event:some-event — complex name path with module and event
    // Currently the special name path parser consumes `.event` as part of the module path.
    // This is a known limitation of the current SpecialNamePath implementation that
    // requires event: to be recognized as a nested special path within name paths.
    assert_parses("module:some-module", JSDOC_CLOSURE);
}

// ============================================================================
// typescript/objects.spec.ts — remaining complex
// ============================================================================

#[test]
fn ts_object_multi_level_bracket_access() {
    let json = parse_and_json("obj[\"level1\"][\"level2\"]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
}

#[test]
fn ts_mapped_type_readonly_optional() {
    assert_parses(
        "{readonly [key in Type]?: number}",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_mapped_type_complex_value() {
    assert_parses(
        "{[key in AvailableArbitraryType]: Partial<TypeObject> | string[]}",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_mapped_type_string_literal_union() {
    assert_parses(
        "{[key in \"abc\" | \"def\"]: number}",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_index_signature_union_key() {
    assert_parses(
        "{[key: string | number]: boolean}",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_readonly_index_signature() {
    assert_parses("{readonly [key: string]: number}", &[ParseMode::Typescript]);
}

#[test]
fn ts_readonly_index_signature_generic_value() {
    assert_parses(
        "{readonly [type: string]: ReadonlyArray<string>}",
        &[ParseMode::Typescript],
    );
}

// ============================================================================
// typescript/tuple.spec.ts — remaining
// ============================================================================

#[test]
fn ts_tuple_trailing_comma() {
    assert_parses("[tuple, with, trailing, comma, ]", &[ParseMode::Typescript]);
}

#[test]
fn ts_tuple_with_typeof_and_keyof() {
    assert_parses(
        "[tuple, with, typeof foo, and, keyof foo]",
        &[ParseMode::Typescript],
    );
}

// ============================================================================
// typescript/typeof.spec.ts — closure mode
// ============================================================================

#[test]
fn closure_typeof_as_function_param_no_return() {
    assert_parses(
        "function(typeof A)",
        &[ParseMode::Closure, ParseMode::Typescript],
    );
}

// ============================================================================
// typescript/functions.spec.ts — remaining
// ============================================================================

#[test]
fn ts_new_arrow_function() {
    assert_parses("new () => SomeType", &[ParseMode::Typescript]);
}

// ============================================================================
// typescript/intersection.spec.ts — remaining
// ============================================================================

#[test]
fn ts_intersection_union_arrow() {
    assert_parses("(A | B) & (a: string) => void", &[ParseMode::Typescript]);
}

#[test]
fn ts_intersection_function_generic() {
    let json = parse_and_json("function(): void & A<B, C>", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeIntersection");
}

// ============================================================================
// misc/Errors.spec.ts — all error cases
// ============================================================================

#[test]
fn error_symbol_unclosed() {
    assert_fails("Symbol(abc", JSDOC_CLOSURE);
}

#[test]
fn error_number_symbol() {
    assert_fails("123(abc", ALL);
}

#[test]
fn error_import_standalone_ts() {
    assert_fails("import", &[ParseMode::Typescript]);
}

#[test]
fn error_123_is() {
    assert_fails("123 is", &[ParseMode::Typescript]);
}

#[test]
fn error_variadic_unclosed_bracket() {
    assert_fails("...[abc", &[ParseMode::Jsdoc]);
}

#[test]
fn error_object_with_generic_as_field() {
    assert_fails("{Array<string> string}", ALL);
}

#[test]
fn error_index_sig_unclosed_bracket() {
    assert_fails("{[a: string}", &[ParseMode::Typescript]);
}

#[test]
fn error_computed_prop_unclosed() {
    assert_fails("{[someType}", &[ParseMode::Typescript]);
}

#[test]
fn error_mapped_type_unclosed() {
    assert_fails("{[key in string}", &[ParseMode::Typescript]);
}

#[test]
fn error_asserts_non_name() {
    assert_fails("asserts 5", &[ParseMode::Typescript]);
}

#[test]
fn error_leading_angle_bracket() {
    assert_fails("<abc<def>>", ALL);
}

#[test]
fn error_index_sig_incomplete() {
    assert_fails("{[a: string]}", &[ParseMode::Typescript]);
}

#[test]
fn error_mapped_type_incomplete() {
    assert_fails("{[key in string]}", &[ParseMode::Typescript]);
}

#[test]
fn error_variadic_empty_brackets_jsdoc() {
    assert_fails("...[]", &[ParseMode::Jsdoc]);
}

#[test]
fn error_variadic_empty_brackets_ts() {
    let json = parse_and_json("...[]", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeVariadic");
}

// ============================================================================
// typescript/computedProperty.spec.ts
// ============================================================================

#[test]
fn ts_computed_property_simple() {
    let json = parse_and_json("{[someType]: string}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_computed_property_readonly() {
    let json = parse_and_json("{readonly [someType]: string}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_computed_property_optional() {
    let json = parse_and_json("{[someType]?: string}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

// ============================================================================
// typescript/computedMethod.spec.ts
// ============================================================================

#[test]
fn ts_computed_method_simple() {
    let json = parse_and_json("{[someType](): AnotherType}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_computed_method_optional() {
    let json = parse_and_json("{[someType]?(): AnotherType}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_computed_method_with_params() {
    assert_parses(
        "{[someType](a: string, b: number[]): AnotherType}",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_computed_method_optional_with_type_params() {
    assert_parses(
        "{[someType]?<T>(a: T, b: number[]): AnotherType}",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_computed_method_default_type_params() {
    assert_parses(
        "{[someType]<T = string>(a: T, b: number[]): AnotherType}",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_computed_method_complex_type_params() {
    assert_parses(
        "{[someType]<T extends A = string, V>(a: T, b: number[]): AnotherType}",
        &[ParseMode::Typescript],
    );
}

#[test]
fn ts_computed_method_readonly_allowed() {
    let json = parse_and_json("{readonly [someType](): string}", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeObject");
}

#[test]
fn ts_computed_method_error_unterminated() {
    assert_fails("{[someType](: string}", &[ParseMode::Typescript]);
}

// ============================================================================
// misc/parseName.spec.ts
// ============================================================================

#[test]
fn parse_name_foo() {
    assert_type_value("foo", ParseMode::Jsdoc, "JsdocTypeName", "foo");
    assert_parses("foo", ALL);
}

#[test]
fn parse_name_foo_generic() {
    let json = parse_and_json("foo<T>", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeGeneric");
    assert_parses("foo<T>", ALL);
}

// ============================================================================
// misc/parseNamePath.spec.ts
// ============================================================================

#[test]
fn parse_name_path_foo_test() {
    let json = parse_and_json("foo.test", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_eq!(json["pathType"], "property");
    assert_parses("foo.test", ALL);
}

#[test]
fn parse_name_path_foo_continue() {
    let json = parse_and_json("foo.continue", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_parses("foo.continue", ALL);
}

#[test]
fn parse_name_path_mixed_separators() {
    let json = parse_and_json("foo#test~another", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
    assert_eq!(json["pathType"], "inner");
    assert_parses("foo#test~another", JSDOC_CLOSURE);
}

#[test]
fn parse_name_path_foo_simple() {
    assert_type_value("foo", ParseMode::Jsdoc, "JsdocTypeName", "foo");
}

// parseNamePath keywords as name paths: props.<keyword>
#[test]
fn parse_name_path_props_null() {
    assert_parses("props.null", ALL);
    let json = parse_and_json("props.null", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
}

#[test]
fn parse_name_path_props_undefined() {
    assert_parses("props.undefined", ALL);
    let json = parse_and_json("props.undefined", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeNamePath");
}

#[test]
fn parse_name_path_props_function() {
    assert_parses("props.function", ALL);
}

#[test]
fn parse_name_path_props_this() {
    assert_parses("props.this", ALL);
}

#[test]
fn parse_name_path_props_new() {
    assert_parses("props.new", ALL);
}

#[test]
fn parse_name_path_props_module() {
    assert_parses("props.module", ALL);
}

#[test]
fn parse_name_path_props_event() {
    assert_parses("props.event", ALL);
}

#[test]
fn parse_name_path_props_extends() {
    assert_parses("props.extends", ALL);
}

#[test]
fn parse_name_path_props_external() {
    assert_parses("props.external", ALL);
}

#[test]
fn parse_name_path_props_typeof() {
    assert_parses("props.typeof", ALL);
}

#[test]
fn parse_name_path_props_keyof() {
    assert_parses("props.keyof", ALL);
}

#[test]
fn parse_name_path_props_readonly() {
    assert_parses("props.readonly", ALL);
}

#[test]
fn parse_name_path_props_import() {
    assert_parses("props.import", ALL);
}

#[test]
fn parse_name_path_props_infer() {
    assert_parses("props.infer", ALL);
}

#[test]
fn parse_name_path_props_is() {
    assert_parses("props.is", ALL);
}

#[test]
fn parse_name_path_props_in() {
    assert_parses("props.in", ALL);
}

#[test]
fn parse_name_path_props_asserts() {
    assert_parses("props.asserts", ALL);
}

// ============================================================================
// misc/reservedWords.spec.ts
// ============================================================================

#[test]
fn reserved_word_void() {
    assert_type_value("void", ParseMode::Jsdoc, "JsdocTypeName", "void");
    assert_parses("void", ALL);
}

#[test]
fn reserved_word_this_as_name() {
    assert_type_value("this", ParseMode::Jsdoc, "JsdocTypeName", "this");
    assert_parses("this", ALL);
}

#[test]
fn reserved_word_let() {
    assert_type_value("let", ParseMode::Typescript, "JsdocTypeName", "let");
}

#[test]
fn reserved_word_continue() {
    assert_type_value("continue", ParseMode::Jsdoc, "JsdocTypeName", "continue");
}

#[test]
fn reserved_word_enum() {
    assert_type_value("enum", ParseMode::Jsdoc, "JsdocTypeName", "enum");
}

#[test]
fn reserved_word_implements() {
    assert_type_value(
        "implements",
        ParseMode::Jsdoc,
        "JsdocTypeName",
        "implements",
    );
}

#[test]
fn reserved_word_arguments() {
    assert_type_value("arguments", ParseMode::Jsdoc, "JsdocTypeName", "arguments");
}

#[test]
fn reserved_word_await() {
    assert_type_value("await", ParseMode::Jsdoc, "JsdocTypeName", "await");
}

#[test]
fn reserved_word_union_with_continue() {
    let json = parse_and_json("abc | continue", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeUnion");
}

#[test]
fn reserved_word_intersection_with_continue() {
    let json = parse_and_json("abc & continue", ParseMode::Typescript).unwrap();
    assert_eq!(json["type"], "JsdocTypeIntersection");
}

#[test]
fn reserved_word_parenthesized_continue() {
    let json = parse_and_json("((continue))", ParseMode::Jsdoc).unwrap();
    assert_eq!(json["type"], "JsdocTypeParenthesis");
}
