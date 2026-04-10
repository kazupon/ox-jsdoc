// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Core parser crate for ox-jsdoc.

pub mod analyzer;
pub mod ast;
pub mod parser;
pub mod serializer;
pub mod validator;

pub use analyzer::{AnalysisOutput, analyze_comment};
pub use parser::{ParseOptions, ParseOutput, parse_comment};
pub use serializer::serialize_comment_json;
pub use validator::{ValidationMode, ValidationOptions, ValidationOutput, validate_comment};
