// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Analyzer phase for consumer-oriented facts.

use crate::ast::{BlockTagBody, DescriptionPart, JSDocComment, TagValueToken};

#[derive(Debug)]
pub struct AnalysisOutput<'a> {
    pub tag_count: usize,
    pub tag_names: Vec<&'a str>,
    pub parameter_names: Vec<&'a str>,
    pub custom_tag_names: Vec<&'a str>,
    pub has_inline_tags: bool,
}

pub fn analyze_comment<'a>(comment: &'a JSDocComment<'a>) -> AnalysisOutput<'a> {
    let mut tag_names = Vec::new();
    let mut parameter_names = Vec::new();
    let mut custom_tag_names = Vec::new();

    let has_inline_tags = comment.description.as_ref().is_some_and(|description| {
        description
            .parts
            .iter()
            .any(|part| matches!(part, DescriptionPart::InlineTag(_)))
    });

    for tag in &comment.tags {
        let tag_name = tag.tag_name.value;
        tag_names.push(tag_name);
        if !is_known_builtin_tag(tag_name) {
            custom_tag_names.push(tag_name);
        }

        let Some(body) = tag.body.as_ref() else {
            continue;
        };

        match body.as_ref() {
            BlockTagBody::Generic(body) => {
                if is_parameter_like_tag(tag_name)
                    && let Some(value) = body.value.as_ref()
                {
                    match value.as_ref() {
                        TagValueToken::Parameter(parameter) => {
                            parameter_names.push(parameter.path.raw);
                        }
                        TagValueToken::Raw(_) | TagValueToken::NamePath(_) => {}
                    }
                }
            }
            BlockTagBody::Borrows(_) => {}
        }
    }

    AnalysisOutput {
        tag_count: comment.tags.len(),
        tag_names,
        parameter_names,
        custom_tag_names,
        has_inline_tags,
    }
}

fn is_parameter_like_tag(tag_name: &str) -> bool {
    matches!(tag_name, "param" | "arg" | "argument" | "property" | "prop")
}

fn is_known_builtin_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "alias"
            | "arg"
            | "argument"
            | "borrows"
            | "emits"
            | "event"
            | "fires"
            | "lends"
            | "listens"
            | "memberOf"
            | "memberof"
            | "memberof!"
            | "mixes"
            | "module"
            | "name"
            | "namespace"
            | "param"
            | "prop"
            | "property"
            | "requires"
            | "return"
            | "returns"
            | "throw"
            | "throws"
            | "type"
            | "variation"
    )
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;

    use crate::parse_comment;
    use crate::parser::ParseOptions;

    use super::analyze_comment;

    #[test]
    fn collects_simple_consumer_facts() {
        let allocator = Allocator::default();
        let parsed = parse_comment(
            &allocator,
            "/** See {@link UserService}.\n * @param {string} id\n * @VueI18nSee Docs\n */",
            0,
            ParseOptions::default(),
        );
        let comment = parsed.comment.expect("expected comment");

        let analysis = analyze_comment(&comment);

        assert_eq!(analysis.tag_count, 2);
        assert_eq!(analysis.tag_names, vec!["param", "VueI18nSee"]);
        assert_eq!(analysis.parameter_names, vec!["id"]);
        assert_eq!(analysis.custom_tag_names, vec!["VueI18nSee"]);
        assert!(analysis.has_inline_tags);
    }
}
