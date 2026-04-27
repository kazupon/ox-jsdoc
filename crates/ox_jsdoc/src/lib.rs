// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Core parser crate for ox-jsdoc.

pub mod analyzer;
pub mod ast;
pub mod parser;
pub mod serializer;
pub mod type_parser;
pub mod validator;

pub use analyzer::{AnalysisOutput, analyze_comment};
pub use parser::text::parsed_preserving_whitespace;
pub use parser::{ParseOptions, ParseOutput, ParseTypeOutput, parse_comment, parse_type};
pub use serializer::{
    SerializeOptions, SpacingMode, serialize_comment_json, serialize_comment_json_with_options,
};
pub use type_parser::ast::ParseMode;
pub use validator::{ValidationMode, ValidationOptions, ValidationOutput, validate_comment};
