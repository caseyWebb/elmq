//! Multi-argument output framing for batch-capable subcommands.
//!
//! The shared rule: when exactly one entry is provided the output is bare
//! (identical to the pre-batching single-argument output). When two or more
//! entries are provided, each is emitted as a `## <arg>` header block in
//! input order, with `error: <msg>` bodies on failure.
//!
//! Exit-code policy: `0` on all-success, `2` if any entry failed.

use std::fmt::Write as _;

/// A per-argument result: success carries the body text that would have been
/// printed by the single-argument form of the command; failure carries the
/// error message that should render inline.
pub type ItemResult = Result<String, String>;

/// Format an ordered list of per-argument results into a single output string
/// and compute the exit code.
///
/// - If `entries` has exactly one item: bare body (no header). The body is
///   whatever the single-arg call would have printed. Exit `2` on failure,
///   `0` on success. On failure the body is `error: <msg>`.
/// - If `entries` has two or more items: each entry is a `## <arg>\n<body>`
///   block separated by blank lines, in input order. Exit `2` if any entry
///   failed, `0` otherwise.
///
/// The returned string does not include a trailing newline beyond whatever
/// the bodies already contain; callers should print it with `print!`, not
/// `println!`, and the helper ensures multi-entry output ends with a newline.
pub fn format_results(entries: &[(String, ItemResult)]) -> (String, i32) {
    let any_failed = entries.iter().any(|(_, r)| r.is_err());
    let exit_code = if any_failed { 2 } else { 0 };

    if entries.len() == 1 {
        let (_, result) = &entries[0];
        let body = match result {
            Ok(s) => s.clone(),
            Err(msg) => format!("error: {msg}\n"),
        };
        return (body, exit_code);
    }

    let mut out = String::new();
    for (i, (arg, result)) in entries.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let _ = writeln!(out, "## {arg}");
        match result {
            Ok(body) => {
                out.push_str(body);
                if !body.ends_with('\n') {
                    out.push('\n');
                }
            }
            Err(msg) => {
                let _ = writeln!(out, "error: {msg}");
            }
        }
    }
    (out, exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_entry_success_is_bare() {
        let entries = vec![("Foo.elm".to_string(), Ok("view : Html Msg\n".to_string()))];
        let (out, code) = format_results(&entries);
        assert_eq!(out, "view : Html Msg\n");
        assert_eq!(code, 0);
    }

    #[test]
    fn single_entry_failure_is_bare_error() {
        let entries = vec![("Foo.elm".to_string(), Err("file not found".to_string()))];
        let (out, code) = format_results(&entries);
        assert_eq!(out, "error: file not found\n");
        assert_eq!(code, 2);
    }

    #[test]
    fn two_entries_all_success() {
        let entries = vec![
            ("A.elm".to_string(), Ok("aaa\n".to_string())),
            ("B.elm".to_string(), Ok("bbb\n".to_string())),
        ];
        let (out, code) = format_results(&entries);
        assert_eq!(out, "## A.elm\naaa\n\n## B.elm\nbbb\n");
        assert_eq!(code, 0);
    }

    #[test]
    fn three_entries_middle_fails() {
        let entries = vec![
            ("a".to_string(), Ok("aaa\n".to_string())),
            ("b".to_string(), Err("boom".to_string())),
            ("c".to_string(), Ok("ccc\n".to_string())),
        ];
        let (out, code) = format_results(&entries);
        assert_eq!(out, "## a\naaa\n\n## b\nerror: boom\n\n## c\nccc\n",);
        assert_eq!(code, 2);
    }

    #[test]
    fn input_order_preserved() {
        let entries = vec![
            ("z".to_string(), Ok("1\n".to_string())),
            ("a".to_string(), Ok("2\n".to_string())),
            ("m".to_string(), Ok("3\n".to_string())),
        ];
        let (out, _) = format_results(&entries);
        let z_pos = out.find("## z").unwrap();
        let a_pos = out.find("## a").unwrap();
        let m_pos = out.find("## m").unwrap();
        assert!(z_pos < a_pos);
        assert!(a_pos < m_pos);
    }

    #[test]
    fn body_without_trailing_newline_gets_one() {
        let entries = vec![
            ("a".to_string(), Ok("no-newline".to_string())),
            ("b".to_string(), Ok("bbb\n".to_string())),
        ];
        let (out, _) = format_results(&entries);
        assert_eq!(out, "## a\nno-newline\n\n## b\nbbb\n");
    }
}
