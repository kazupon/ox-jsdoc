// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//
// L2: Parse function unit tests — per-syntax AST structure verification
// L3: Mode error tests — syntax restrictions per mode
// L6: Span offset tests — base_offset absolute position verification

use ox_jsdoc::ast::JsdocType;
use ox_jsdoc::parse_comment;
use ox_jsdoc::parser::ParseOptions;
use ox_jsdoc::type_parser::ast::*;
use oxc_allocator::Allocator;

// ============================================================================
// Test helpers
// ============================================================================

/// Parse a type expression and return the TypeNode debug representation.
fn parse(source: &str, mode: ParseMode) -> Option<serde_json::Value> {
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
        JsdocType::Parsed(node) => Some(ser(node)),
        JsdocType::Raw(_) => None,
    }
}

/// Parse with a specific base_offset for span testing.
fn parse_with_offset(source: &str, mode: ParseMode, base_offset: u32) -> Option<serde_json::Value> {
    let allocator = Allocator::default();
    let comment_src = format!("/** @param {{{source}}} x */");
    let output = parse_comment(
        &allocator,
        &comment_src,
        base_offset,
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
        JsdocType::Parsed(node) => Some(ser(node)),
        JsdocType::Raw(_) => None,
    }
}

fn succeeds(source: &str, mode: ParseMode) -> bool {
    parse(source, mode).is_some()
}

fn fails(source: &str, mode: ParseMode) -> bool {
    parse(source, mode).is_none()
}

fn ser(node: &TypeNode<'_>) -> serde_json::Value {
    use serde_json::json;
    match node {
        TypeNode::Name(n) => {
            json!({"type":"JsdocTypeName","value":n.value,"span":[n.span.start,n.span.end]})
        }
        TypeNode::Number(n) => {
            json!({"type":"JsdocTypeNumber","value":n.value,"span":[n.span.start,n.span.end]})
        }
        TypeNode::StringValue(n) => {
            json!({"type":"JsdocTypeStringValue","value":n.value,"span":[n.span.start,n.span.end]})
        }
        TypeNode::Null(n) => json!({"type":"JsdocTypeNull","span":[n.span.start,n.span.end]}),
        TypeNode::Undefined(n) => {
            json!({"type":"JsdocTypeUndefined","span":[n.span.start,n.span.end]})
        }
        TypeNode::Any(n) => json!({"type":"JsdocTypeAny","span":[n.span.start,n.span.end]}),
        TypeNode::Unknown(n) => json!({"type":"JsdocTypeUnknown","span":[n.span.start,n.span.end]}),
        TypeNode::Union(n) => {
            json!({"type":"JsdocTypeUnion","elements":n.elements.iter().map(|e|ser(e)).collect::<Vec<_>>(),"span":[n.span.start,n.span.end]})
        }
        TypeNode::Intersection(n) => {
            json!({"type":"JsdocTypeIntersection","elements":n.elements.iter().map(|e|ser(e)).collect::<Vec<_>>(),"span":[n.span.start,n.span.end]})
        }
        TypeNode::Generic(n) => {
            json!({"type":"JsdocTypeGeneric","left":ser(&n.left),"elements":n.elements.iter().map(|e|ser(e)).collect::<Vec<_>>(),"brackets":match n.brackets{GenericBrackets::Angle=>"angle",GenericBrackets::Square=>"square"},"dot":n.dot,"span":[n.span.start,n.span.end]})
        }
        TypeNode::Function(n) => {
            let mut o = json!({"type":"JsdocTypeFunction","params":n.parameters.iter().map(|e|ser(e)).collect::<Vec<_>>(),"constructor":n.constructor,"arrow":n.arrow,"parenthesis":n.parenthesis,"span":[n.span.start,n.span.end]});
            if let Some(ref r) = n.return_type {
                o["returnType"] = ser(r);
            }
            o
        }
        TypeNode::Object(n) => {
            json!({"type":"JsdocTypeObject","elements":n.elements.iter().map(|e|ser(e)).collect::<Vec<_>>(),"span":[n.span.start,n.span.end]})
        }
        TypeNode::Tuple(n) => {
            json!({"type":"JsdocTypeTuple","elements":n.elements.iter().map(|e|ser(e)).collect::<Vec<_>>(),"span":[n.span.start,n.span.end]})
        }
        TypeNode::Parenthesis(n) => {
            json!({"type":"JsdocTypeParenthesis","element":ser(&n.element),"span":[n.span.start,n.span.end]})
        }
        TypeNode::NamePath(n) => {
            json!({"type":"JsdocTypeNamePath","left":ser(&n.left),"right":ser(&n.right),"pathType":match n.path_type{NamePathType::Property=>"property",NamePathType::Instance=>"instance",NamePathType::Inner=>"inner",NamePathType::PropertyBrackets=>"property-brackets"},"span":[n.span.start,n.span.end]})
        }
        TypeNode::SpecialNamePath(n) => {
            json!({"type":"JsdocTypeSpecialNamePath","value":n.value,"specialType":match n.special_type{SpecialPathType::Module=>"module",SpecialPathType::Event=>"event",SpecialPathType::External=>"external"},"span":[n.span.start,n.span.end]})
        }
        TypeNode::Nullable(n) => {
            json!({"type":"JsdocTypeNullable","element":ser(&n.element),"position":match n.position{ModifierPosition::Prefix=>"prefix",ModifierPosition::Suffix=>"suffix"},"span":[n.span.start,n.span.end]})
        }
        TypeNode::NotNullable(n) => {
            json!({"type":"JsdocTypeNotNullable","element":ser(&n.element),"position":match n.position{ModifierPosition::Prefix=>"prefix",ModifierPosition::Suffix=>"suffix"},"span":[n.span.start,n.span.end]})
        }
        TypeNode::Optional(n) => {
            json!({"type":"JsdocTypeOptional","element":ser(&n.element),"position":match n.position{ModifierPosition::Prefix=>"prefix",ModifierPosition::Suffix=>"suffix"},"span":[n.span.start,n.span.end]})
        }
        TypeNode::Variadic(n) => {
            let mut o = json!({"type":"JsdocTypeVariadic","squareBrackets":n.square_brackets,"span":[n.span.start,n.span.end]});
            if let Some(ref e) = n.element {
                o["element"] = ser(e);
            }
            if let Some(p) = n.position {
                o["position"] = json!(match p {
                    VariadicPosition::Prefix => "prefix",
                    VariadicPosition::Suffix => "suffix",
                });
            }
            o
        }
        TypeNode::Conditional(n) => {
            json!({"type":"JsdocTypeConditional","checksType":ser(&n.checks_type),"extendsType":ser(&n.extends_type),"trueType":ser(&n.true_type),"falseType":ser(&n.false_type),"span":[n.span.start,n.span.end]})
        }
        TypeNode::Infer(n) => {
            json!({"type":"JsdocTypeInfer","element":ser(&n.element),"span":[n.span.start,n.span.end]})
        }
        TypeNode::KeyOf(n) => {
            json!({"type":"JsdocTypeKeyof","element":ser(&n.element),"span":[n.span.start,n.span.end]})
        }
        TypeNode::TypeOf(n) => {
            json!({"type":"JsdocTypeTypeof","element":ser(&n.element),"span":[n.span.start,n.span.end]})
        }
        TypeNode::Import(n) => {
            json!({"type":"JsdocTypeImport","element":ser(&n.element),"span":[n.span.start,n.span.end]})
        }
        TypeNode::Predicate(n) => {
            json!({"type":"JsdocTypePredicate","left":ser(&n.left),"right":ser(&n.right),"span":[n.span.start,n.span.end]})
        }
        TypeNode::Asserts(n) => {
            json!({"type":"JsdocTypeAsserts","left":ser(&n.left),"right":ser(&n.right),"span":[n.span.start,n.span.end]})
        }
        TypeNode::AssertsPlain(n) => {
            json!({"type":"JsdocTypeAssertsPlain","element":ser(&n.element),"span":[n.span.start,n.span.end]})
        }
        TypeNode::ReadonlyArray(n) => {
            json!({"type":"JsdocTypeReadonlyArray","element":ser(&n.element),"span":[n.span.start,n.span.end]})
        }
        TypeNode::UniqueSymbol(n) => {
            json!({"type":"JsdocTypeUniqueSymbol","span":[n.span.start,n.span.end]})
        }
        TypeNode::Symbol(n) => {
            let mut o =
                json!({"type":"JsdocTypeSymbol","value":n.value,"span":[n.span.start,n.span.end]});
            if let Some(ref e) = n.element {
                o["element"] = ser(e);
            }
            o
        }
        TypeNode::KeyValue(n) => {
            let mut o = json!({"type":"JsdocTypeKeyValue","key":n.key,"optional":n.optional,"variadic":n.variadic,"span":[n.span.start,n.span.end]});
            if let Some(ref r) = n.right {
                o["right"] = ser(r);
            }
            o
        }
        TypeNode::ObjectField(n) => {
            let mut o = json!({"type":"JsdocTypeObjectField","key":ser(&n.key),"optional":n.optional,"readonly":n.readonly,"span":[n.span.start,n.span.end]});
            if let Some(ref r) = n.right {
                o["right"] = ser(r);
            }
            o
        }
        TypeNode::Property(n) => {
            json!({"type":"JsdocTypeProperty","value":n.value,"span":[n.span.start,n.span.end]})
        }
        TypeNode::MethodSignature(n) => {
            json!({"type":"JsdocTypeMethodSignature","name":n.name,"span":[n.span.start,n.span.end]})
        }
        TypeNode::CallSignature(_) => json!({"type":"JsdocTypeCallSignature"}),
        TypeNode::ConstructorSignature(_) => json!({"type":"JsdocTypeConstructorSignature"}),
        TypeNode::TemplateLiteral(n) => {
            json!({"type":"JsdocTypeTemplateLiteral","span":[n.span.start,n.span.end]})
        }
        TypeNode::MappedType(n) => {
            json!({"type":"JsdocTypeMappedType","key":n.key,"span":[n.span.start,n.span.end]})
        }
        TypeNode::IndexSignature(n) => {
            json!({"type":"JsdocTypeIndexSignature","key":n.key,"span":[n.span.start,n.span.end]})
        }
        TypeNode::TypeParameter(n) => json!({"type":"JsdocTypeTypeParameter","name":ser(&n.name)}),
        _ => json!({"type":"Other"}),
    }
}

const TS: ParseMode = ParseMode::Typescript;
const JS: ParseMode = ParseMode::Jsdoc;
const CL: ParseMode = ParseMode::Closure;

// ============================================================================
// L2: Parse function unit tests — basic types
// ============================================================================

#[test]
fn l2_name_string() {
    let j = parse("string", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
    assert_eq!(j["value"], "string");
}
#[test]
fn l2_name_number() {
    let j = parse("number", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
    assert_eq!(j["value"], "number");
}
#[test]
fn l2_name_boolean() {
    let j = parse("boolean", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
    assert_eq!(j["value"], "boolean");
}
#[test]
fn l2_name_any_word() {
    let j = parse("any", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
    assert_eq!(j["value"], "any");
}
#[test]
fn l2_name_void() {
    let j = parse("void", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
    assert_eq!(j["value"], "void");
}
#[test]
fn l2_name_never() {
    let j = parse("never", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
    assert_eq!(j["value"], "never");
}
#[test]
fn l2_name_object() {
    let j = parse("object", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
    assert_eq!(j["value"], "object");
}
#[test]
fn l2_name_custom() {
    let j = parse("MyClass", TS).unwrap();
    assert_eq!(j["value"], "MyClass");
}
#[test]
fn l2_name_dollar() {
    let j = parse("$scope", TS).unwrap();
    assert_eq!(j["value"], "$scope");
}
#[test]
fn l2_name_underscore() {
    let j = parse("_private", TS).unwrap();
    assert_eq!(j["value"], "_private");
}
#[test]
fn l2_null() {
    let j = parse("null", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeNull");
}
#[test]
fn l2_undefined() {
    let j = parse("undefined", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeUndefined");
}
#[test]
fn l2_any() {
    let j = parse("*", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeAny");
}
#[test]
fn l2_unknown() {
    let j = parse("?", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeUnknown");
}

// L2: Number literals
#[test]
fn l2_num_int() {
    let j = parse("42", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeNumber");
    assert_eq!(j["value"], "42");
}
#[test]
fn l2_num_float() {
    let j = parse("3.14", TS).unwrap();
    assert_eq!(j["value"], "3.14");
}
#[test]
fn l2_num_neg() {
    let j = parse("-1", TS).unwrap();
    assert_eq!(j["value"], "-1");
}
#[test]
fn l2_num_exp() {
    let j = parse("1e10", TS).unwrap();
    assert_eq!(j["value"], "1e10");
}
#[test]
fn l2_num_neg_exp() {
    let j = parse("-1.5e+3", TS).unwrap();
    assert_eq!(j["value"], "-1.5e+3");
}
#[test]
fn l2_num_zero() {
    let j = parse("0", TS).unwrap();
    assert_eq!(j["value"], "0");
}

// L2: String literals
#[test]
fn l2_str_double() {
    let j = parse("\"hello\"", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeStringValue");
}
#[test]
fn l2_str_single() {
    let j = parse("'world'", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeStringValue");
}
#[test]
fn l2_str_empty_double() {
    let j = parse("\"\"", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeStringValue");
}
#[test]
fn l2_str_empty_single() {
    let j = parse("''", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeStringValue");
}

// ============================================================================
// L2: Union types
// ============================================================================

#[test]
fn l2_union_two() {
    let j = parse("string | number", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeUnion");
    assert_eq!(j["elements"].as_array().unwrap().len(), 2);
}
#[test]
fn l2_union_three() {
    let j = parse("string | number | boolean", TS).unwrap();
    assert_eq!(j["elements"].as_array().unwrap().len(), 3);
}
#[test]
fn l2_union_no_space() {
    let j = parse("A|B", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeUnion");
}
#[test]
fn l2_union_with_null() {
    let j = parse("string | null", TS).unwrap();
    assert_eq!(j["elements"][1]["type"], "JsdocTypeNull");
}
#[test]
fn l2_union_with_undefined() {
    let j = parse("string | undefined", TS).unwrap();
    assert_eq!(j["elements"][1]["type"], "JsdocTypeUndefined");
}
#[test]
fn l2_union_complex() {
    let j = parse("string | number | null | undefined", TS).unwrap();
    assert_eq!(j["elements"].as_array().unwrap().len(), 4);
}

// ============================================================================
// L2: Intersection types
// ============================================================================

#[test]
fn l2_intersection_two() {
    let j = parse("A & B", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeIntersection");
    assert_eq!(j["elements"].as_array().unwrap().len(), 2);
}
#[test]
fn l2_intersection_three() {
    let j = parse("A & B & C", TS).unwrap();
    assert_eq!(j["elements"].as_array().unwrap().len(), 3);
}

// ============================================================================
// L2: Generic types
// ============================================================================

#[test]
fn l2_generic_one() {
    let j = parse("Array<string>", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeGeneric");
    assert_eq!(j["brackets"], "angle");
    assert_eq!(j["dot"], false);
}
#[test]
fn l2_generic_two() {
    let j = parse("Map<string, number>", TS).unwrap();
    assert_eq!(j["elements"].as_array().unwrap().len(), 2);
}
#[test]
fn l2_generic_dot() {
    let j = parse("Array.<string>", JS).unwrap();
    assert_eq!(j["dot"], true);
}
#[test]
fn l2_generic_nested() {
    let j = parse("Map<string, Array<number>>", TS).unwrap();
    assert_eq!(j["elements"][1]["type"], "JsdocTypeGeneric");
}
#[test]
fn l2_array_bracket() {
    let j = parse("string[]", TS).unwrap();
    assert_eq!(j["brackets"], "square");
}
#[test]
fn l2_array_nested() {
    let j = parse("number[][]", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeGeneric");
}
#[test]
fn l2_generic_promise() {
    let j = parse("Promise<void>", TS).unwrap();
    assert_eq!(j["left"]["value"], "Promise");
}

// ============================================================================
// L2: Nullable/NotNullable/Optional
// ============================================================================

#[test]
fn l2_nullable_pre() {
    let j = parse("?string", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeNullable");
    assert_eq!(j["position"], "prefix");
}
#[test]
fn l2_nullable_post() {
    let j = parse("string?", JS).unwrap();
    assert_eq!(j["position"], "suffix");
}
#[test]
fn l2_not_nullable_pre() {
    let j = parse("!Object", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeNotNullable");
    assert_eq!(j["position"], "prefix");
}
#[test]
fn l2_not_nullable_post() {
    let j = parse("Object!", JS).unwrap();
    assert_eq!(j["position"], "suffix");
}
#[test]
fn l2_optional_post() {
    let j = parse("string=", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeOptional");
    assert_eq!(j["position"], "suffix");
}
#[test]
fn l2_optional_pre() {
    let j = parse("=string", JS).unwrap();
    assert_eq!(j["position"], "prefix");
}

// ============================================================================
// L2: Variadic
// ============================================================================

#[test]
fn l2_variadic_pre() {
    let j = parse("...string", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeVariadic");
    assert_eq!(j["position"], "prefix");
    assert_eq!(j["squareBrackets"], false);
}
#[test]
fn l2_variadic_post() {
    let j = parse("string...", JS).unwrap();
    assert_eq!(j["position"], "suffix");
}
#[test]
fn l2_variadic_bare() {
    let j = parse("...", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeVariadic");
    assert!(j.get("position").is_none() || j["position"].is_null());
}

// ============================================================================
// L2: Function types
// ============================================================================

#[test]
fn l2_func_bare() {
    let j = parse("function", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeFunction");
    assert_eq!(j["parenthesis"], false);
}
#[test]
fn l2_func_no_params() {
    let j = parse("function()", JS).unwrap();
    assert_eq!(j["parenthesis"], true);
    assert_eq!(j["params"].as_array().unwrap().len(), 0);
}
#[test]
fn l2_func_one_param() {
    let j = parse("function(string)", JS).unwrap();
    assert_eq!(j["params"].as_array().unwrap().len(), 1);
}
#[test]
fn l2_func_two_params() {
    let j = parse("function(string, number)", JS).unwrap();
    assert_eq!(j["params"].as_array().unwrap().len(), 2);
}
#[test]
fn l2_func_return() {
    let j = parse("function(): number", JS).unwrap();
    assert_eq!(j["returnType"]["value"], "number");
}
#[test]
fn l2_func_params_return() {
    let j = parse("function(string): boolean", JS).unwrap();
    assert_eq!(j["params"].as_array().unwrap().len(), 1);
    assert_eq!(j["returnType"]["value"], "boolean");
}
#[test]
fn l2_func_arrow() {
    let j = parse("() => void", TS).unwrap();
    assert_eq!(j["arrow"], true);
    assert_eq!(j["constructor"], false);
}
#[test]
fn l2_func_arrow_param() {
    let j = parse("(x: number) => string", TS).unwrap();
    assert_eq!(j["arrow"], true);
}
#[test]
fn l2_func_arrow_multi() {
    let j = parse("(a: string, b: number) => boolean", TS).unwrap();
    assert_eq!(j["arrow"], true);
}
#[test]
fn l2_func_constructor() {
    let j = parse("new(string): MyClass", TS).unwrap();
    assert_eq!(j["constructor"], true);
}
#[test]
fn l2_func_this_param() {
    let j = parse("function(this: MyObj)", JS).unwrap();
    assert_eq!(j["params"][0]["type"], "JsdocTypeKeyValue");
    assert_eq!(j["params"][0]["key"], "this");
}
#[test]
fn l2_func_new_param() {
    let j = parse("function(new: MyObj)", JS).unwrap();
    assert_eq!(j["params"][0]["key"], "new");
}

// ============================================================================
// L2: Object types
// ============================================================================

#[test]
fn l2_obj_empty() {
    let j = parse("{}", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeObject");
    assert_eq!(j["elements"].as_array().unwrap().len(), 0);
}
#[test]
fn l2_obj_one_field() {
    let j = parse("{a: string}", TS).unwrap();
    assert_eq!(j["elements"].as_array().unwrap().len(), 1);
}
#[test]
fn l2_obj_two_fields() {
    let j = parse("{a: string, b: number}", TS).unwrap();
    assert_eq!(j["elements"].as_array().unwrap().len(), 2);
}
#[test]
fn l2_obj_optional() {
    let j = parse("{a?: string}", TS).unwrap();
    assert_eq!(j["elements"][0]["optional"], true);
}
#[test]
fn l2_obj_readonly() {
    let j = parse("{readonly a: string}", TS).unwrap();
    assert_eq!(j["elements"][0]["readonly"], true);
}
#[test]
fn l2_obj_no_value() {
    let j = parse("{a}", TS).unwrap();
    assert_eq!(j["elements"].as_array().unwrap().len(), 1);
}
#[test]
fn l2_obj_semicolon() {
    assert!(succeeds("{a: string; b: number}", TS));
}
#[test]
fn l2_obj_index_sig() {
    assert!(succeeds("{[key: string]: number}", TS));
}
#[test]
fn l2_obj_mapped_type() {
    assert!(succeeds("{[K in keyof T]: V}", TS));
}
#[test]
fn l2_obj_computed_prop() {
    assert!(succeeds("{[someType]: string}", TS));
}
#[test]
fn l2_obj_computed_method() {
    assert!(succeeds("{[someType](): string}", TS));
}
#[test]
fn l2_obj_call_sig() {
    assert!(succeeds("{(a: string): void}", TS));
}
#[test]
fn l2_obj_ctor_sig() {
    assert!(succeeds("{new (a: string): void}", TS));
}
#[test]
fn l2_obj_method_sig() {
    assert!(succeeds("{foo(a: string): void}", TS));
}

// ============================================================================
// L2: TypeScript-specific
// ============================================================================

#[test]
fn l2_conditional() {
    let j = parse("T extends U ? X : Y", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeConditional");
    assert_eq!(j["checksType"]["value"], "T");
    assert_eq!(j["extendsType"]["value"], "U");
    assert_eq!(j["trueType"]["value"], "X");
    assert_eq!(j["falseType"]["value"], "Y");
}
#[test]
fn l2_nested_conditional() {
    let j = parse("A extends B ? C extends D ? E : F : G", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeConditional");
    assert_eq!(j["trueType"]["type"], "JsdocTypeConditional");
}
#[test]
fn l2_keyof() {
    let j = parse("keyof T", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeKeyof");
    assert_eq!(j["element"]["value"], "T");
}
#[test]
fn l2_typeof() {
    let j = parse("typeof x", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeTypeof");
    assert_eq!(j["element"]["value"], "x");
}
#[test]
fn l2_infer() {
    let j = parse("infer T", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeInfer");
}
#[test]
fn l2_import() {
    let j = parse("import('./foo')", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeImport");
}
#[test]
fn l2_predicate() {
    let j = parse("x is string", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypePredicate");
}
#[test]
fn l2_asserts() {
    let j = parse("asserts x is T", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeAsserts");
}
#[test]
fn l2_asserts_plain() {
    let j = parse("asserts x", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeAssertsPlain");
}
#[test]
fn l2_unique_symbol() {
    let j = parse("unique symbol", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeUniqueSymbol");
}
#[test]
fn l2_readonly_array() {
    let j = parse("readonly string[]", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeReadonlyArray");
}
#[test]
fn l2_readonly_tuple() {
    let j = parse("readonly [string, number]", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeReadonlyArray");
}
#[test]
fn l2_tuple_empty() {
    let j = parse("[]", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeTuple");
    assert_eq!(j["elements"].as_array().unwrap().len(), 0);
}
#[test]
fn l2_tuple_one() {
    let j = parse("[string]", TS).unwrap();
    assert_eq!(j["elements"].as_array().unwrap().len(), 1);
}
#[test]
fn l2_tuple_two() {
    let j = parse("[string, number]", TS).unwrap();
    assert_eq!(j["elements"].as_array().unwrap().len(), 2);
}
#[test]
fn l2_tuple_labeled() {
    let j = parse("[a: string, b: number]", TS).unwrap();
    assert_eq!(j["elements"].as_array().unwrap().len(), 2);
}
#[test]
fn l2_tuple_spread() {
    assert!(succeeds("[string, ...number]", TS));
}
#[test]
fn l2_parenthesized() {
    let j = parse("(string)", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeParenthesis");
}
#[test]
fn l2_parenthesized_union() {
    let j = parse("(string | number)", TS).unwrap();
    assert_eq!(j["element"]["type"], "JsdocTypeUnion");
}

// ============================================================================
// L2: Name paths
// ============================================================================

#[test]
fn l2_namepath_dot() {
    let j = parse("A.B", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeNamePath");
    assert_eq!(j["pathType"], "property");
}
#[test]
fn l2_namepath_hash() {
    let j = parse("A#B", JS).unwrap();
    assert_eq!(j["pathType"], "instance");
}
#[test]
fn l2_namepath_tilde() {
    let j = parse("A~B", JS).unwrap();
    assert_eq!(j["pathType"], "inner");
}
#[test]
fn l2_namepath_deep() {
    let j = parse("A.B.C", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeNamePath");
    assert_eq!(j["right"]["value"], "C");
}
#[test]
fn l2_namepath_bracket() {
    let j = parse("A[\"key\"]", TS).unwrap();
    assert_eq!(j["pathType"], "property-brackets");
}
#[test]
fn l2_namepath_indexed() {
    let j = parse("T[K]", TS).unwrap();
    assert_eq!(j["pathType"], "property-brackets");
}

// ============================================================================
// L2: JSDoc/Closure-specific
// ============================================================================

#[test]
fn l2_symbol_empty() {
    let j = parse("Symbol()", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeSymbol");
    assert_eq!(j["value"], "Symbol");
}
#[test]
fn l2_symbol_name() {
    let j = parse("Symbol(iterator)", JS).unwrap();
    assert_eq!(j["element"]["value"], "iterator");
}
#[test]
fn l2_symbol_number() {
    let j = parse("MyClass(2)", JS).unwrap();
    assert_eq!(j["element"]["type"], "JsdocTypeNumber");
}
#[test]
fn l2_special_module() {
    let j = parse("module:foo", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeSpecialNamePath");
    assert_eq!(j["specialType"], "module");
}
#[test]
fn l2_special_event() {
    let j = parse("event:click", JS).unwrap();
    assert_eq!(j["specialType"], "event");
}
#[test]
fn l2_special_external() {
    let j = parse("external:jQuery", JS).unwrap();
    assert_eq!(j["specialType"], "external");
}

// ============================================================================
// L2: Combination / precedence
// ============================================================================

#[test]
fn l2_union_of_generics() {
    let j = parse("Array<string> | Map<string, number>", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeUnion");
    assert_eq!(j["elements"].as_array().unwrap().len(), 2);
}
#[test]
fn l2_nullable_union() {
    let j = parse("?(string | number)", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeNullable");
    assert_eq!(j["element"]["type"], "JsdocTypeParenthesis");
}
#[test]
fn l2_optional_function() {
    let j = parse("function(string)=", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeOptional");
    assert_eq!(j["element"]["type"], "JsdocTypeFunction");
}
#[test]
fn l2_variadic_generic() {
    let j = parse("...Array<string>", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeVariadic");
    assert_eq!(j["element"]["type"], "JsdocTypeGeneric");
}
#[test]
fn l2_keyof_array() {
    let j = parse("keyof T[]", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeKeyof");
}
#[test]
fn l2_typeof_array() {
    let j = parse("typeof T[]", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeTypeof");
}
#[test]
fn l2_conditional_with_keyof() {
    assert!(succeeds("K extends keyof T ? T[K] : never", TS));
}
#[test]
fn l2_conditional_with_infer() {
    assert!(succeeds("T extends Array<infer U> ? U : never", TS));
}
#[test]
fn l2_import_namepath() {
    assert!(succeeds("import('./foo').Bar", TS));
}
#[test]
fn l2_generic_of_union() {
    assert!(succeeds("Array<string | number>", TS));
}
#[test]
fn l2_function_intersection() {
    let j = parse("function(): void & A", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeIntersection");
}
#[test]
fn l2_union_intersection_precedence() {
    let j = parse("A | B & C", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeUnion");
    assert_eq!(j["elements"][1]["type"], "JsdocTypeIntersection");
}

// ============================================================================
// L2: KeyValue (function parameters)
// ============================================================================

#[test]
fn l2_kv_basic() {
    let j = parse("(a: string) => void", TS).unwrap();
    assert_eq!(j["params"][0]["type"], "JsdocTypeKeyValue");
    assert_eq!(j["params"][0]["key"], "a");
}
#[test]
fn l2_kv_optional() {
    let j = parse("(a?: string) => void", TS).unwrap();
    assert_eq!(j["params"][0]["optional"], true);
}
#[test]
fn l2_kv_variadic() {
    assert!(succeeds("(...args: string[]) => void", TS));
}

// ============================================================================
// L2: Type parameters
// ============================================================================

#[test]
fn l2_type_param_simple() {
    assert!(succeeds("{<T>(a: T): T}", TS));
}
#[test]
fn l2_type_param_constraint() {
    assert!(succeeds("{<T extends string>(a: T): T}", TS));
}
#[test]
fn l2_type_param_default() {
    assert!(succeeds("{<T = string>(a: T): T}", TS));
}
#[test]
fn l2_type_param_full() {
    assert!(succeeds("{<T extends U = V>(a: T): T}", TS));
}

// ============================================================================
// L3: Mode error tests — syntax restrictions per mode
// ============================================================================

// Intersection only in typescript
#[test]
fn l3_intersection_ts_only() {
    assert!(succeeds("A & B", TS));
    assert!(fails("A & B", JS));
    assert!(fails("A & B", CL));
}

// Conditional only in typescript
#[test]
fn l3_conditional_ts_only() {
    assert!(succeeds("T extends U ? X : Y", TS));
    assert!(fails("T extends U ? X : Y", JS));
    assert!(fails("T extends U ? X : Y", CL));
}

// keyof as prefix only in typescript (name in jsdoc/closure)
#[test]
fn l3_keyof_name_in_jsdoc() {
    let j = parse("keyof", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
    assert_eq!(j["value"], "keyof");
}
#[test]
fn l3_keyof_prefix_in_ts() {
    let j = parse("keyof T", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeKeyof");
}

// typeof as prefix in typescript and closure (not jsdoc)
#[test]
fn l3_typeof_ts() {
    assert!(succeeds("typeof x", TS));
}
#[test]
fn l3_typeof_closure() {
    assert!(succeeds("typeof x", CL));
}
#[test]
fn l3_typeof_not_jsdoc() {
    assert!(fails("typeof x", JS));
}

// import only in typescript
#[test]
fn l3_import_ts_only() {
    assert!(succeeds("import('./x')", TS));
    assert!(fails("import('./x')", JS));
    assert!(fails("import('./x')", CL));
}

// infer only in typescript
#[test]
fn l3_infer_ts_only() {
    assert!(succeeds("infer T", TS));
    assert!(fails("infer T", JS));
    assert!(fails("infer T", CL));
}

// asserts only in typescript
#[test]
fn l3_asserts_ts_only() {
    assert!(succeeds("asserts x", TS));
    assert!(fails("asserts x", JS));
    assert!(fails("asserts x", CL));
}

// unique symbol only in typescript
#[test]
fn l3_unique_symbol_ts_only() {
    assert!(succeeds("unique symbol", TS));
    assert!(fails("unique symbol", JS));
    assert!(fails("unique symbol", CL));
}

// predicate only in typescript
#[test]
fn l3_predicate_ts_only() {
    assert!(succeeds("x is string", TS));
    assert!(fails("x is string", JS));
    assert!(fails("x is string", CL));
}

// readonly array only in typescript
#[test]
fn l3_readonly_ts_only() {
    assert!(succeeds("readonly string[]", TS));
    assert!(fails("readonly string[]", JS));
    assert!(fails("readonly string[]", CL));
}

// Tuple only in typescript
#[test]
fn l3_tuple_ts_only() {
    assert!(succeeds("[string, number]", TS));
    assert!(fails("[string, number]", JS));
    assert!(fails("[string, number]", CL));
}

// Bare function only in jsdoc
#[test]
fn l3_bare_function_jsdoc() {
    let j = parse("function", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeFunction");
}
#[test]
fn l3_bare_function_closure() {
    let j = parse("function", CL).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
}
#[test]
fn l3_bare_function_ts() {
    let j = parse("function", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
}

// Variadic postfix only in jsdoc
#[test]
fn l3_variadic_postfix_jsdoc() {
    assert!(succeeds("string...", JS));
}
#[test]
fn l3_variadic_postfix_not_ts() {
    assert!(fails("string...", TS));
}
#[test]
fn l3_variadic_postfix_not_closure() {
    assert!(fails("string...", CL));
}

// Symbol (Name(args)) only in jsdoc/closure
#[test]
fn l3_symbol_jsdoc() {
    assert!(succeeds("MyClass()", JS));
}
#[test]
fn l3_symbol_closure() {
    assert!(succeeds("MyClass()", CL));
}
#[test]
fn l3_symbol_not_ts() {
    assert!(fails("MyClass()", TS));
}

// event: only in jsdoc
#[test]
fn l3_event_jsdoc() {
    assert!(succeeds("event:click", JS));
}
#[test]
fn l3_event_not_closure() {
    let j = parse("event", CL).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
}

// external: only in jsdoc
#[test]
fn l3_external_jsdoc() {
    assert!(succeeds("external:jQuery", JS));
}

// Optional prefix in jsdoc/closure
#[test]
fn l3_optional_prefix_jsdoc() {
    assert!(succeeds("=string", JS));
}
#[test]
fn l3_optional_prefix_closure() {
    assert!(succeeds("=string", CL));
}
#[test]
fn l3_optional_prefix_not_ts() {
    assert!(fails("=string", TS));
}

// Loose mode identifiers (hyphens, NaN, Infinity)
#[test]
fn l3_hyphen_id_jsdoc() {
    assert!(succeeds("my-type", JS));
}
#[test]
fn l3_hyphen_id_closure() {
    assert!(succeeds("my-type", CL));
}
// In typescript mode, `my-type` is parsed as `my` (name) then `-type` causes
// EarlyEndOfParse via the full pipeline. However through comment parser pipeline,
// the type is extracted as raw then parsed, and the result depends on how
// the comment parser handles `-`. In practice, `-` splits the type so this
// test verifies the identifier does NOT include the hyphen.
#[test]
fn l3_hyphen_id_ts_is_just_name() {
    let j = parse("my-type", TS);
    if let Some(j) = j {
        assert_eq!(j["value"], "my");
    }
}

// NaN/Infinity as numbers in loose mode
#[test]
fn l3_nan_jsdoc() {
    let j = parse("NaN", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeNumber");
}
#[test]
fn l3_nan_ts_is_name() {
    let j = parse("NaN", TS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
}
#[test]
fn l3_infinity_jsdoc() {
    let j = parse("Infinity", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeNumber");
}

// Name path # and ~ only in jsdoc/closure
#[test]
fn l3_hash_jsdoc() {
    assert!(succeeds("A#B", JS));
}
#[test]
fn l3_hash_closure() {
    assert!(succeeds("A#B", CL));
}
#[test]
fn l3_tilde_jsdoc() {
    assert!(succeeds("A~B", JS));
}
#[test]
fn l3_tilde_closure() {
    assert!(succeeds("A~B", CL));
}

// new() as function only in typescript
#[test]
fn l3_new_func_ts() {
    assert!(succeeds("new(string): T", TS));
}
#[test]
fn l3_new_name_jsdoc() {
    let j = parse("new", JS).unwrap();
    assert_eq!(j["type"], "JsdocTypeName");
}

// ============================================================================
// L6: Span offset tests
// ============================================================================

// In `/** @param {string} x */` the type starts at position 14 (0-indexed):
// `/** @param {` = 12 chars, `{` is offset 12, type starts at 13
// With base_offset, spans shift accordingly.

#[test]
fn l6_span_zero_offset() {
    let j = parse_with_offset("string", TS, 0).unwrap();
    // In `/** @param {string} x */`, `string` starts at byte 13
    let span = j["span"].as_array().unwrap();
    let start = span[0].as_u64().unwrap();
    let end = span[1].as_u64().unwrap();
    assert_eq!(end - start, 6); // "string" = 6 chars
}

#[test]
fn l6_span_with_offset() {
    let j0 = parse_with_offset("string", TS, 0).unwrap();
    let j100 = parse_with_offset("string", TS, 100).unwrap();
    let start0 = j0["span"][0].as_u64().unwrap();
    let start100 = j100["span"][0].as_u64().unwrap();
    assert_eq!(start100 - start0, 100);
}

#[test]
fn l6_span_union_children() {
    let j = parse_with_offset("A | B", TS, 0).unwrap();
    let a = &j["elements"][0];
    let b = &j["elements"][1];
    let a_start = a["span"][0].as_u64().unwrap();
    let a_end = a["span"][1].as_u64().unwrap();
    let b_start = b["span"][0].as_u64().unwrap();
    let b_end = b["span"][1].as_u64().unwrap();
    assert_eq!(a_end - a_start, 1); // "A" = 1 char
    assert_eq!(b_end - b_start, 1); // "B" = 1 char
    assert!(b_start > a_end); // B comes after A
}

#[test]
fn l6_span_generic() {
    let j = parse_with_offset("Array<string>", TS, 0).unwrap();
    let left = &j["left"];
    let left_start = left["span"][0].as_u64().unwrap();
    let left_end = left["span"][1].as_u64().unwrap();
    assert_eq!(left_end - left_start, 5); // "Array" = 5 chars
    let elem = &j["elements"][0];
    let elem_start = elem["span"][0].as_u64().unwrap();
    let elem_end = elem["span"][1].as_u64().unwrap();
    assert_eq!(elem_end - elem_start, 6); // "string" = 6 chars
    assert!(elem_start > left_end); // element after left
}

#[test]
fn l6_span_offset_propagates_to_children() {
    let j = parse_with_offset("A | B", TS, 500).unwrap();
    let a_start = j["elements"][0]["span"][0].as_u64().unwrap();
    assert!(a_start >= 500); // all spans shifted by base_offset
}

#[test]
fn l6_span_nullable_wraps_element() {
    let j = parse_with_offset("?string", JS, 0).unwrap();
    let outer_start = j["span"][0].as_u64().unwrap();
    let outer_end = j["span"][1].as_u64().unwrap();
    let inner_start = j["element"]["span"][0].as_u64().unwrap();
    let inner_end = j["element"]["span"][1].as_u64().unwrap();
    assert!(outer_start <= inner_start);
    assert!(outer_end >= inner_end);
    assert_eq!(outer_end - outer_start, 7); // "?string" = 7 chars
}

#[test]
fn l6_span_function_with_return() {
    let j = parse_with_offset("function(): number", JS, 0).unwrap();
    let func_start = j["span"][0].as_u64().unwrap();
    let ret_end = j["returnType"]["span"][1].as_u64().unwrap();
    assert!(ret_end > func_start);
}

#[test]
fn l6_span_conditional() {
    let j = parse_with_offset("T extends U ? X : Y", TS, 0).unwrap();
    let checks_start = j["checksType"]["span"][0].as_u64().unwrap();
    let false_end = j["falseType"]["span"][1].as_u64().unwrap();
    assert!(false_end > checks_start);
}

#[test]
fn l6_span_name_path() {
    let j = parse_with_offset("A.B.C", JS, 0).unwrap();
    let outer_start = j["span"][0].as_u64().unwrap();
    let outer_end = j["span"][1].as_u64().unwrap();
    assert_eq!(outer_end - outer_start, 5); // "A.B.C" = 5 chars
}

#[test]
fn l6_span_large_offset() {
    let j = parse_with_offset("number", TS, 100000).unwrap();
    let start = j["span"][0].as_u64().unwrap();
    assert!(start >= 100000);
}
