// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("headers:check") => check_headers(),
        Some(command) => {
            eprintln!("unknown xtask command: {command}");
            print_usage();
            ExitCode::from(2)
        }
        None => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn print_usage() {
    eprintln!("usage: cargo run -p xtask -- headers:check");
}

fn check_headers() -> ExitCode {
    let root = env::current_dir().expect("failed to resolve current directory");
    let mut files = Vec::new();
    collect_rust_files(&root, &mut files);

    let mut failures = Vec::new();
    for file in files {
        if let Err(error) = check_header(&file) {
            failures.push(error);
        }
    }

    if failures.is_empty() {
        println!("header check passed");
        return ExitCode::SUCCESS;
    }

    eprintln!("header check failed:");
    for failure in failures {
        eprintln!("  - {failure}");
    }
    ExitCode::FAILURE
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| {
        panic!("failed to read directory {}: {error}", dir.display());
    });

    for entry in entries {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if path.is_dir() {
            if should_skip_dir(&file_name) {
                continue;
            }
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn should_skip_dir(name: &str) -> bool {
    matches!(name, ".git" | "node_modules" | "refers" | "target")
}

fn check_header(path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("{}: failed to read file: {error}", path.display()))?;
    let header = parse_leading_line_comment_header(&content);

    let Some(author) = header.author else {
        return Err(format!("{}: missing @author header", path.display()));
    };
    if author.is_empty() {
        return Err(format!(
            "{}: @author header must not be empty",
            path.display()
        ));
    }

    let Some(license) = header.license else {
        return Err(format!("{}: missing @license header", path.display()));
    };
    if license != "MIT" {
        return Err(format!(
            "{}: @license must be MIT, found {license:?}",
            path.display()
        ));
    }

    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Header<'a> {
    author: Option<&'a str>,
    license: Option<&'a str>,
}

fn parse_leading_line_comment_header(content: &str) -> Header<'_> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut header = Header::default();

    for line in content.lines() {
        let trimmed = line.trim_start();
        let Some(comment) = trimmed.strip_prefix("//") else {
            break;
        };
        let comment = comment.trim();

        if let Some(value) = comment.strip_prefix("@author") {
            header.author = Some(value.trim());
        } else if let Some(value) = comment.strip_prefix("@license") {
            header.license = Some(value.trim());
        }
    }

    header
}

#[cfg(test)]
mod tests {
    use super::{Header, parse_leading_line_comment_header};

    #[test]
    fn parses_author_and_license() {
        let header = parse_leading_line_comment_header(
            "// @author someone\n// @license MIT\n//\n\nfn main() {}\n",
        );

        assert_eq!(
            header,
            Header {
                author: Some("someone"),
                license: Some("MIT"),
            }
        );
    }

    #[test]
    fn stops_at_first_non_line_comment() {
        let header = parse_leading_line_comment_header(
            "fn main() {}\n// @author too late\n// @license MIT\n",
        );

        assert_eq!(header, Header::default());
    }

    #[test]
    fn allows_any_non_empty_author() {
        let header = parse_leading_line_comment_header("// @author Jane Doe\n// @license MIT\n");

        assert_eq!(header.author, Some("Jane Doe"));
    }
}
