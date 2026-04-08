pub fn is_jsdoc_block(source_text: &str) -> bool {
    source_text.starts_with("/**")
}

pub fn has_closing_block(source_text: &str) -> bool {
    source_text.ends_with("*/")
}
