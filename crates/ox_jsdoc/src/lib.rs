//! Core parser crate for ox-jsdoc.

pub mod analyzer;
pub mod ast;
pub mod parser;
pub mod serializer;
pub mod validator;

pub use parser::{ParseOptions, ParseOutput, parse_comment};
