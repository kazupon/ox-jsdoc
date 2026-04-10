// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Validator phase for `ox-jsdoc`.
//!
//! Parser-level recovery and source-shape extraction live in `parser`.
//! Tag-specific and mode-specific checks live here.

use oxc_diagnostics::OxcDiagnostic;

use crate::ast::{BlockTag, BlockTagBody, JSDocComment};

/// Validation rule set.
///
/// Modes are intentionally coarse for now. They let callers choose broad tag
/// semantics without changing parser behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    JSDoc,
    Closure,
    TypeScript,
    Permissive,
}

/// Validator configuration.
#[derive(Debug, Clone, Copy)]
pub struct ValidationOptions {
    /// Rule set used when interpreting known tags.
    pub mode: ValidationMode,
    /// When true, unknown tags are treated as extension points.
    pub allow_unknown_tags: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            mode: ValidationMode::JSDoc,
            allow_unknown_tags: true,
        }
    }
}

/// Validation diagnostics for a parsed comment.
#[derive(Debug)]
pub struct ValidationOutput {
    /// Diagnostics emitted by tag-level semantic validation.
    pub diagnostics: Vec<OxcDiagnostic>,
}

/// Semantic validation failures after syntax parsing succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatorDiagnosticKind {
    UnknownTag,
    MissingTypeExpression,
    MissingTagValue,
    InvalidBorrowsShape,
}

/// Validate tag-level requirements for a parsed comment.
pub fn validate_comment(
    comment: &JSDocComment<'_>,
    options: ValidationOptions,
) -> ValidationOutput {
    let mut diagnostics = Vec::new();

    for tag in &comment.tags {
        validate_tag(tag, options, &mut diagnostics);
    }

    ValidationOutput { diagnostics }
}

fn validate_tag(
    tag: &BlockTag<'_>,
    options: ValidationOptions,
    diagnostics: &mut Vec<OxcDiagnostic>,
) {
    let Some(spec) = lookup_tag_spec(tag.tag_name.value, options.mode) else {
        // Unknown tags are common in framework ecosystems, so this is
        // configurable instead of hard-coded as an error.
        if !options.allow_unknown_tags {
            diagnostics.push(diagnostic(
                ValidatorDiagnosticKind::UnknownTag,
                tag.tag_name.value,
            ));
        }
        return;
    };

    let Some(body) = tag.body.as_ref() else {
        // Missing bodies can imply both missing type and missing value,
        // depending on the tag's declared shape.
        if spec.type_required {
            diagnostics.push(diagnostic(
                ValidatorDiagnosticKind::MissingTypeExpression,
                tag.tag_name.value,
            ));
        }
        if spec.value_required {
            diagnostics.push(diagnostic(
                ValidatorDiagnosticKind::MissingTagValue,
                tag.tag_name.value,
            ));
        }
        return;
    };

    match body.as_ref() {
        BlockTagBody::Generic(body) => {
            // Generic bodies are intentionally simple. Specialized validation
            // can still use `raw_body` for source-shape checks such as borrows.
            if spec.type_required && body.type_expression.is_none() {
                diagnostics.push(diagnostic(
                    ValidatorDiagnosticKind::MissingTypeExpression,
                    tag.tag_name.value,
                ));
            }
            if spec.value_required && body.value.is_none() {
                diagnostics.push(diagnostic(
                    ValidatorDiagnosticKind::MissingTagValue,
                    tag.tag_name.value,
                ));
            }
            if spec.requires_borrows_shape {
                let raw_body = tag
                    .raw_body
                    .as_ref()
                    .map(|raw| raw.value)
                    .unwrap_or_default()
                    .trim();
                if !raw_body.contains(" as ") {
                    diagnostics.push(diagnostic(
                        ValidatorDiagnosticKind::InvalidBorrowsShape,
                        tag.tag_name.value,
                    ));
                }
            }
        }
        BlockTagBody::Borrows(_) => {}
    }
}

#[derive(Debug, Clone, Copy)]
struct TagSpec {
    /// Whether the tag requires a `{...}` type expression.
    type_required: bool,
    /// Whether the tag requires a value token after the type expression.
    value_required: bool,
    /// Whether the raw body must follow `source as target`.
    requires_borrows_shape: bool,
}

/// Look up the base requirements for a tag in the requested validation mode.
fn lookup_tag_spec(tag_name: &str, mode: ValidationMode) -> Option<TagSpec> {
    let base = match tag_name {
        "param" | "arg" | "argument" | "property" | "prop" => TagSpec {
            type_required: false,
            value_required: true,
            requires_borrows_shape: false,
        },
        "returns" | "return" | "throws" | "throw" | "type" => TagSpec {
            type_required: true,
            value_required: false,
            requires_borrows_shape: false,
        },
        "borrows" => TagSpec {
            type_required: false,
            value_required: false,
            requires_borrows_shape: true,
        },
        "memberof" | "memberof!" | "variation" | "name" | "alias" | "module" | "namespace"
        | "event" | "fires" | "listens" | "emits" | "mixes" | "lends" | "requires" => TagSpec {
            type_required: false,
            value_required: true,
            requires_borrows_shape: false,
        },
        _ => return None,
    };

    Some(adjust_spec_for_mode(base, mode, tag_name))
}

/// Apply mode-specific refinements to the shared tag table.
fn adjust_spec_for_mode(spec: TagSpec, mode: ValidationMode, tag_name: &str) -> TagSpec {
    match (mode, tag_name) {
        (ValidationMode::TypeScript, "type") | (ValidationMode::Closure, "type") => TagSpec {
            type_required: true,
            ..spec
        },
        (ValidationMode::TypeScript, "typedef") => TagSpec {
            type_required: true,
            value_required: false,
            requires_borrows_shape: false,
        },
        _ => spec,
    }
}

fn diagnostic(kind: ValidatorDiagnosticKind, tag_name: &str) -> OxcDiagnostic {
    let message = match kind {
        ValidatorDiagnosticKind::UnknownTag => {
            format!("unknown tag `@{tag_name}` for the current validator configuration")
        }
        ValidatorDiagnosticKind::MissingTypeExpression => {
            format!("tag `@{tag_name}` requires a type expression")
        }
        ValidatorDiagnosticKind::MissingTagValue => {
            format!("tag `@{tag_name}` requires a value")
        }
        ValidatorDiagnosticKind::InvalidBorrowsShape => {
            format!("tag `@{tag_name}` must use the `source as target` shape")
        }
    };

    OxcDiagnostic::error(message)
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;

    use crate::parse_comment;
    use crate::parser::ParseOptions;

    use super::{ValidationMode, ValidationOptions, validate_comment};

    #[test]
    fn emits_missing_value_for_param() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** @param {string} */",
            0,
            ParseOptions::default(),
        );
        let comment = parsed.comment.expect("expected comment");

        let output = validate_comment(&comment, ValidationOptions::default());

        assert_eq!(output.diagnostics.len(), 1);
        assert!(
            output.diagnostics[0]
                .to_string()
                .contains("requires a value")
        );
    }

    #[test]
    fn emits_missing_type_for_returns() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** @returns user */",
            0,
            ParseOptions::default(),
        );
        let comment = parsed.comment.expect("expected comment");

        let output = validate_comment(&comment, ValidationOptions::default());

        assert_eq!(output.diagnostics.len(), 1);
        assert!(
            output.diagnostics[0]
                .to_string()
                .contains("requires a type expression")
        );
    }

    #[test]
    fn emits_invalid_borrows_shape() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** @borrows source */",
            0,
            ParseOptions::default(),
        );
        let comment = parsed.comment.expect("expected comment");

        let output = validate_comment(&comment, ValidationOptions::default());

        assert_eq!(output.diagnostics.len(), 1);
        assert!(
            output.diagnostics[0]
                .to_string()
                .contains("source as target")
        );
    }

    #[test]
    fn rejects_unknown_tag_when_configured() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** @VueI18nSee Docs */",
            0,
            ParseOptions::default(),
        );
        let comment = parsed.comment.expect("expected comment");

        let output = validate_comment(
            &comment,
            ValidationOptions {
                mode: ValidationMode::JSDoc,
                allow_unknown_tags: false,
            },
        );

        assert_eq!(output.diagnostics.len(), 1);
        assert!(output.diagnostics[0].to_string().contains("unknown tag"));
    }

    #[test]
    fn allows_unknown_tag_by_default() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** @VueI18nSee Docs */",
            0,
            ParseOptions::default(),
        );
        let comment = parsed.comment.expect("expected comment");

        let output = validate_comment(&comment, ValidationOptions::default());

        assert!(output.diagnostics.is_empty());
    }
}
