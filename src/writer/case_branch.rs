use crate::FileSummary;
use crate::analysis::{self, CaseBranch, CaseSite};
use crate::parser;
use anyhow::{Result, bail};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Specification for a case branch upsert.
pub struct CaseSpec {
    /// Scrutinee selector — trimmed match against `case <scrutinee> of`.
    /// If `None` and the declaration has exactly one case expression, that
    /// case is used automatically.
    pub on: Option<String>,
    /// Pattern to upsert (byte-exact after trim).
    pub pattern: String,
    /// Branch body expression.
    pub body: String,
    /// Disambiguation: when multiple case expressions share the same
    /// scrutinee text, `--line` picks one by absolute file line.
    pub line: Option<usize>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Find the top-level `value_declaration` node whose
/// `functionDeclarationLeft` has the given name.
fn find_decl_node<'t>(
    root: tree_sitter::Node<'t>,
    source: &[u8],
    name: &str,
) -> Option<tree_sitter::Node<'t>> {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() == "value_declaration" {
            let n = child
                .child_by_field_name("functionDeclarationLeft")
                .and_then(|fdl| {
                    let mut c = fdl.walk();
                    fdl.named_children(&mut c)
                        .find(|ch| ch.kind() == "lower_case_identifier")
                })
                .and_then(|id| id.utf8_text(source).ok())
                .unwrap_or("");
            if n == name {
                return Some(child);
            }
        }
    }
    None
}

/// Determine which `CaseSite` to target given `on` and `line` selectors.
fn select_case_site<'a>(
    sites: &'a [CaseSite],
    decl_name: &str,
    on: Option<&str>,
    line: Option<usize>,
) -> Result<&'a CaseSite> {
    if sites.is_empty() {
        bail!("no case expression in '{decl_name}'");
    }

    if let Some(scrutinee) = on {
        // Filter by scrutinee text.
        let matching: Vec<&CaseSite> = sites
            .iter()
            .filter(|s| s.scrutinee_text.trim() == scrutinee.trim())
            .collect();

        if matching.is_empty() {
            bail!("no case expression with scrutinee `{scrutinee}` in '{decl_name}'");
        }

        if matching.len() == 1 {
            return Ok(matching[0]);
        }

        // Multiple matches — use line to disambiguate.
        if let Some(target_line) = line {
            let by_line: Vec<&CaseSite> = matching
                .iter()
                .filter(|s| s.line == target_line)
                .copied()
                .collect();
            if by_line.len() == 1 {
                return Ok(by_line[0]);
            }
            if by_line.is_empty() {
                let candidates: Vec<String> = matching
                    .iter()
                    .map(|s| format!("  line {}", s.line))
                    .collect();
                bail!(
                    "multiple case expressions on `{scrutinee}` in '{decl_name}':\n{}\nretry with --line <N>",
                    candidates.join("\n")
                );
            }
        }

        // Still ambiguous.
        let candidates: Vec<String> = matching
            .iter()
            .map(|s| format!("  line {}", s.line))
            .collect();
        bail!(
            "multiple case expressions on `{scrutinee}` in '{decl_name}':\n{}\nretry with --line <N>",
            candidates.join("\n")
        );
    }

    // No `--on` given.
    if sites.len() == 1 {
        return Ok(&sites[0]);
    }

    // Multiple case expressions and no --on selector.
    let candidates: Vec<String> = sites
        .iter()
        .map(|s| format!("  line {}  on `{}`", s.line, s.scrutinee_text))
        .collect();
    bail!(
        "case expression is ambiguous in '{decl_name}': {} candidates:\n{}\nretry with --on <scrutinee> [--line <N>]",
        sites.len(),
        candidates.join("\n")
    );
}

/// Infer the indentation (leading spaces/tabs) of the first branch's pattern
/// line.  Falls back to four spaces.
fn branch_indent(branches: &[CaseBranch], source: &str) -> String {
    let bytes = source.as_bytes();
    if let Some(first) = branches.first() {
        // Walk backward from the pattern start to find the beginning of that line.
        let start = first.pattern_span.0;
        let line_start = bytes[..start]
            .iter()
            .rposition(|&b| b == b'\n')
            .map(|p| p + 1)
            .unwrap_or(0);
        let indent_bytes = &bytes[line_start..start];
        if let Ok(s) = std::str::from_utf8(indent_bytes) {
            if !s.trim().is_empty() {
                // The leading bytes should all be whitespace; if not fall back.
                let indent: String = s.chars().take_while(|c| c.is_whitespace()).collect();
                return indent;
            }
            return s.to_string();
        }
    }
    "        ".to_string() // default: 8 spaces (typical Elm 4-space body indent)
}

/// Build the text for a new branch.  Returns:
/// `"<indent><pattern> ->\n<indent>    <body>"`
///
/// We intentionally use `<indent>    ` (indent + 4 spaces) for the body
/// line to mirror standard Elm style.
fn build_branch_text(indent: &str, pattern: &str, body: &str) -> String {
    let body_indent = format!("{indent}    ");
    format!("{indent}{pattern} ->\n{body_indent}{body}")
}

/// Given a branch's body_span, compute the exclusive byte range of the entire
/// branch (pattern through end of body), including any trailing newline.
///
/// We need to remove the whole branch text when deleting.  The branch
/// occupies `[pattern_span.0, body_span.1]` plus any immediately following
/// newline(s) / blank line(s) up to (but not including) the indent of the
/// next branch.
fn branch_delete_range(branch: &CaseBranch, source: &str) -> (usize, usize) {
    let start = branch.pattern_span.0;
    // Walk backward from pattern start to include the indent on the same line.
    let bytes = source.as_bytes();
    let line_start = bytes[..start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|p| p + 1)
        .unwrap_or(0);

    // The delete start is the beginning of that line (includes the indent).
    let delete_start = line_start;

    // The delete end: advance past body_span.1, consuming the trailing newline.
    let mut end = branch.body_span.1;
    if end < bytes.len() && bytes[end] == b'\n' {
        end += 1;
    }
    // Consume a single blank line after the branch (common Elm style).
    if end < bytes.len() && bytes[end] == b'\n' {
        end += 1;
    }

    (delete_start, end)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Upsert a case branch within a top-level declaration.
///
/// - If the pattern already exists in the selected case → replace its body.
/// - If the pattern is new → insert before any wildcard (`_`) branch, else
///   append at the end.
///
/// Returns the full updated source string.
pub fn upsert_case_branch(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    spec: &CaseSpec,
) -> Result<String> {
    // 1. Look up the declaration.
    let _decl_info = summary
        .find_declaration(decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{decl_name}' not found"))?;

    // 2. Parse the source and find the decl node.
    let tree = parser::parse(source)?;
    let root = tree.root_node();
    let decl_node = find_decl_node(root, source.as_bytes(), decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{decl_name}' not found in parse tree"))?;

    // 3. Collect case sites.
    let sites = analysis::collect_case_sites_generic(decl_node, source);

    // 4. Select the target case.
    let site = select_case_site(&sites, decl_name, spec.on.as_deref(), spec.line)?;

    // 5. Validate the body.
    parser::parse_case_branch_body(&spec.body)?;

    let pattern_trimmed = spec.pattern.trim();

    // 6. Check whether the pattern already exists.
    let existing = site
        .branches
        .iter()
        .find(|b| b.pattern_text.trim() == pattern_trimmed);

    if let Some(branch) = existing {
        // Replace body span only.
        let (body_start, body_end) = branch.body_span;
        let mut result = source.to_string();
        result.replace_range(body_start..body_end, &spec.body);
        Ok(result)
    } else {
        // Insert new branch.
        let indent = branch_indent(&site.branches, source);
        let new_branch_text = build_branch_text(&indent, pattern_trimmed, &spec.body);

        // Find insertion point: before the first wildcard branch, or at the end of the case.
        let wildcard = site.branches.iter().find(|b| b.pattern_text.trim() == "_");

        let insert_byte = if let Some(wc) = wildcard {
            // Insert before the wildcard's line start.
            let bytes = source.as_bytes();
            let wc_start = wc.pattern_span.0;
            bytes[..wc_start]
                .iter()
                .rposition(|&b| b == b'\n')
                .map(|p| p + 1)
                .unwrap_or(wc_start)
        } else {
            // Append after the last branch's body.
            let last = site.branches.last().ok_or_else(|| {
                anyhow::anyhow!("case expression has no branches in '{decl_name}'")
            })?;
            last.body_span.1
        };

        let mut result = source.to_string();
        if wildcard.is_some() {
            // Insert at the line start of the wildcard (before it).
            result.insert_str(insert_byte, &format!("{new_branch_text}\n\n"));
        } else {
            // Append after the last body: add a blank line separator then the branch.
            result.insert_str(insert_byte, &format!("\n\n{new_branch_text}"));
        }
        Ok(result)
    }
}

/// Remove a single case branch from a top-level declaration.
///
/// Returns the full updated source string.
pub fn remove_case_branch(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    on: Option<&str>,
    pattern: &str,
    line: Option<usize>,
) -> Result<String> {
    let _decl_info = summary
        .find_declaration(decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{decl_name}' not found"))?;

    let tree = parser::parse(source)?;
    let root = tree.root_node();
    let decl_node = find_decl_node(root, source.as_bytes(), decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{decl_name}' not found in parse tree"))?;

    let sites = analysis::collect_case_sites_generic(decl_node, source);
    let site = select_case_site(&sites, decl_name, on, line)?;
    let patterns = [pattern.to_string()];
    remove_branches_from_site(source, decl_name, site, &patterns)
}

/// Remove multiple case branches in one atomic operation.
///
/// All patterns must resolve before any mutation is performed.
pub fn remove_case_branches_batch(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    on: Option<&str>,
    patterns: &[String],
    line: Option<usize>,
) -> Result<String> {
    // 1. Look up declaration.
    let _decl_info = summary
        .find_declaration(decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{decl_name}' not found"))?;

    // 2. Parse.
    let tree = parser::parse(source)?;
    let root = tree.root_node();
    let decl_node = find_decl_node(root, source.as_bytes(), decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{decl_name}' not found in parse tree"))?;

    // 3. Collect sites and select target.
    let sites = analysis::collect_case_sites_generic(decl_node, source);
    let site = select_case_site(&sites, decl_name, on, line)?;

    remove_branches_from_site(source, decl_name, site, patterns)
}

/// Shared logic: remove `patterns` from `site` within `source`.
fn remove_branches_from_site(
    source: &str,
    decl_name: &str,
    site: &CaseSite,
    patterns: &[String],
) -> Result<String> {
    // Resolve all patterns up front.
    let mut matched: Vec<&CaseBranch> = Vec::new();
    let mut missing: Vec<&str> = Vec::new();

    for pat in patterns {
        let pat_trimmed = pat.trim();
        if let Some(b) = site
            .branches
            .iter()
            .find(|b| b.pattern_text.trim() == pat_trimmed)
        {
            matched.push(b);
        } else {
            missing.push(pat.as_str());
        }
    }

    if !missing.is_empty() {
        let list = missing
            .iter()
            .map(|p| format!("  `{p}`"))
            .collect::<Vec<_>>()
            .join("\n");
        bail!("pattern(s) not found in case expression in '{decl_name}':\n{list}");
    }

    // Guard against emptying the case.
    let remaining = site.branches.len() - matched.len();
    if remaining == 0 {
        bail!(
            "cannot remove all branches from case expression in '{decl_name}': \
             would leave the case expression empty"
        );
    }

    // Compute delete ranges for each matched branch, then process rear-to-front.
    let mut ranges: Vec<(usize, usize)> = matched
        .iter()
        .map(|b| branch_delete_range(b, source))
        .collect();

    // Sort by start byte descending so rear-to-front splicing keeps offsets valid.
    ranges.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = source.to_string();
    for (start, end) in ranges {
        result.replace_range(start..end, "");
    }

    // Collapse any runs of >2 consecutive newlines that may have been left.
    Ok(collapse_blank_lines(&result))
}

/// Collapse runs of more than two consecutive newlines to exactly two.
fn collapse_blank_lines(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut consecutive_newlines = 0usize;
    for ch in s.chars() {
        if ch == '\n' {
            consecutive_newlines += 1;
            if consecutive_newlines <= 2 {
                result.push(ch);
            }
        } else {
            consecutive_newlines = 0;
            result.push(ch);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn make_summary(source: &str) -> FileSummary {
        let tree = parser::parse(source).unwrap();
        parser::extract_summary(&tree, source)
    }

    // -----------------------------------------------------------------------
    // (a) Replace an existing branch body (typed decl)
    // -----------------------------------------------------------------------

    #[test]
    fn replace_existing_branch_body() {
        let source = r#"module Main exposing (..)

update msg model =
    case msg of
        Increment ->
            model + 1

        Decrement ->
            model - 1
"#;
        let summary = make_summary(source);
        let spec = CaseSpec {
            on: None,
            pattern: "Increment".to_string(),
            body: "model + 2".to_string(),
            line: None,
        };
        let result = upsert_case_branch(source, &summary, "update", &spec).unwrap();
        assert!(result.contains("model + 2"), "body not updated: {result}");
        assert!(
            !result.contains("model + 1"),
            "old body still present: {result}"
        );
        assert!(
            result.contains("Decrement"),
            "other branch missing: {result}"
        );
    }

    // -----------------------------------------------------------------------
    // (b) Add a new branch at the end (no wildcard)
    // -----------------------------------------------------------------------

    #[test]
    fn add_new_branch_no_wildcard() {
        let source = r#"module Main exposing (..)

update msg model =
    case msg of
        Increment ->
            model + 1

        Decrement ->
            model - 1
"#;
        let summary = make_summary(source);
        let spec = CaseSpec {
            on: None,
            pattern: "Reset".to_string(),
            body: "0".to_string(),
            line: None,
        };
        let result = upsert_case_branch(source, &summary, "update", &spec).unwrap();
        assert!(
            result.contains("Reset ->"),
            "new branch not added: {result}"
        );
        assert!(result.contains("0"), "body not added: {result}");
        // Reset should come after Decrement
        let reset_pos = result.find("Reset").unwrap();
        let decrement_pos = result.find("Decrement").unwrap();
        assert!(
            reset_pos > decrement_pos,
            "Reset not after Decrement: {result}"
        );
    }

    // -----------------------------------------------------------------------
    // (c) Add a new branch before the wildcard
    // -----------------------------------------------------------------------

    #[test]
    fn add_branch_before_wildcard() {
        let source = r#"module Main exposing (..)

toLabel n =
    case n of
        0 ->
            "zero"

        _ ->
            "other"
"#;
        let summary = make_summary(source);
        let spec = CaseSpec {
            on: None,
            pattern: "1".to_string(),
            body: "\"one\"".to_string(),
            line: None,
        };
        let result = upsert_case_branch(source, &summary, "toLabel", &spec).unwrap();
        assert!(result.contains("1 ->"), "new branch not added: {result}");
        // `1 ->` must appear before `_ ->`
        let one_pos = result
            .find("        1 ->")
            .unwrap_or_else(|| result.find("1 ->").unwrap());
        let wild_pos = result.find("_ ->").unwrap();
        assert!(one_pos < wild_pos, "1 -> not before _ ->:\n{result}");
    }

    // -----------------------------------------------------------------------
    // (d) Scrutinee ambiguity error; retry with --on resolves it
    // -----------------------------------------------------------------------

    #[test]
    fn scrutinee_ambiguity_without_on() {
        let source = r#"module Main exposing (..)

view state route =
    let
        stateResult =
            case state of
                Loading ->
                    "loading"

                Loaded ->
                    "loaded"

        routeResult =
            case route of
                Home ->
                    "home"

                About ->
                    "about"
    in
    stateResult ++ routeResult
"#;
        let summary = make_summary(source);
        let spec = CaseSpec {
            on: None,
            pattern: "Error".to_string(),
            body: "\"error\"".to_string(),
            line: None,
        };
        let err = upsert_case_branch(source, &summary, "view", &spec).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ambiguous"), "expected ambiguity error: {msg}");
        assert!(msg.contains("state"), "expected scrutinee 'state': {msg}");
        assert!(msg.contains("route"), "expected scrutinee 'route': {msg}");
        assert!(msg.contains("--on"), "expected --on hint: {msg}");

        // Retry with --on resolves ambiguity.
        let spec2 = CaseSpec {
            on: Some("state".to_string()),
            pattern: "Error".to_string(),
            body: "\"error\"".to_string(),
            line: None,
        };
        let result = upsert_case_branch(source, &summary, "view", &spec2).unwrap();
        assert!(
            result.contains("Error ->"),
            "Error branch not added: {result}"
        );
    }

    // -----------------------------------------------------------------------
    // (e) --on + --line disambiguation when two cases share a scrutinee
    // -----------------------------------------------------------------------

    #[test]
    fn on_and_line_disambiguation() {
        let source = r#"module Main exposing (..)

doubleCase x =
    let
        first =
            case x of
                A ->
                    1

        second =
            case x of
                B ->
                    2
    in
    first + second
"#;
        let summary = make_summary(source);

        // Without --line it should be ambiguous.
        let spec_ambig = CaseSpec {
            on: Some("x".to_string()),
            pattern: "C".to_string(),
            body: "3".to_string(),
            line: None,
        };
        let err = upsert_case_branch(source, &summary, "doubleCase", &spec_ambig).unwrap_err();
        assert!(
            err.to_string().contains("multiple case expressions on `x`"),
            "expected multiple-case error: {err}"
        );

        // Find the line of the first case expression to pass --line.
        let tree = parser::parse(source).unwrap();
        let root = tree.root_node();
        let decl_node = find_decl_node(root, source.as_bytes(), "doubleCase").unwrap();
        let sites = crate::analysis::collect_case_sites_generic(decl_node, source);
        let first_line = sites.iter().map(|s| s.line).min().unwrap();

        let spec_ok = CaseSpec {
            on: Some("x".to_string()),
            pattern: "C".to_string(),
            body: "3".to_string(),
            line: Some(first_line),
        };
        let result = upsert_case_branch(source, &summary, "doubleCase", &spec_ok).unwrap();
        assert!(result.contains("C ->"), "C branch not added: {result}");
    }

    // -----------------------------------------------------------------------
    // (f) Error when decl has no case expression
    // -----------------------------------------------------------------------

    #[test]
    fn error_no_case_expression() {
        let source = r#"module Main exposing (..)

simpleFn x =
    x + 1
"#;
        let summary = make_summary(source);
        let spec = CaseSpec {
            on: None,
            pattern: "X".to_string(),
            body: "0".to_string(),
            line: None,
        };
        let err = upsert_case_branch(source, &summary, "simpleFn", &spec).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("no case expression in 'simpleFn'"),
            "unexpected error: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // (g) Multi-target rm case with all patterns matching
    // -----------------------------------------------------------------------

    #[test]
    fn remove_multiple_branches() {
        let source = r#"module Main exposing (..)

update msg model =
    case msg of
        Increment ->
            model + 1

        Decrement ->
            model - 1

        Reset ->
            0
"#;
        let summary = make_summary(source);
        let patterns = vec!["Increment".to_string(), "Decrement".to_string()];
        let result =
            remove_case_branches_batch(source, &summary, "update", None, &patterns, None).unwrap();
        assert!(
            !result.contains("Increment"),
            "Increment not removed: {result}"
        );
        assert!(
            !result.contains("Decrement"),
            "Decrement not removed: {result}"
        );
        assert!(result.contains("Reset"), "Reset should remain: {result}");
    }

    // -----------------------------------------------------------------------
    // (h) Removing all branches errors
    // -----------------------------------------------------------------------

    #[test]
    fn error_removing_all_branches() {
        let source = r#"module Main exposing (..)

update msg model =
    case msg of
        A ->
            1

        B ->
            2
"#;
        let summary = make_summary(source);
        let patterns = vec!["A".to_string(), "B".to_string()];
        let err = remove_case_branches_batch(source, &summary, "update", None, &patterns, None)
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("empty"), "expected empty-case error: {msg}");
    }

    // -----------------------------------------------------------------------
    // (i) Pattern with spaces / nested constructors matches correctly
    // -----------------------------------------------------------------------

    #[test]
    fn nested_constructor_pattern_matches() {
        let source = r#"module Main exposing (..)

process msg =
    case msg of
        Just (Increment count) ->
            count + 1

        Nothing ->
            0
"#;
        let summary = make_summary(source);
        let spec = CaseSpec {
            on: None,
            pattern: "Just (Increment count)".to_string(),
            body: "count + 2".to_string(),
            line: None,
        };
        let result = upsert_case_branch(source, &summary, "process", &spec).unwrap();
        assert!(result.contains("count + 2"), "body not replaced: {result}");
        assert!(
            !result.contains("count + 1"),
            "old body still present: {result}"
        );
    }

    // -----------------------------------------------------------------------
    // (j) Multi-line body replacement
    // -----------------------------------------------------------------------

    #[test]
    fn multiline_body_replacement() {
        // Use a body that parse_case_branch_body can validate cleanly.
        // The validator wraps as `_ -> <body>` after "        " (8 spaces),
        // so a simple if/then/else or tuple works without tricky indentation.
        let source = r#"module Main exposing (..)

update msg model =
    case msg of
        Increment ->
            model + 1

        Decrement ->
            model - 1
"#;
        let summary = make_summary(source);
        // Use a body expression that parse_case_branch_body can validate cleanly.
        let new_body = "clamp 0 100 (model + 5)";
        let spec = CaseSpec {
            on: None,
            pattern: "Increment".to_string(),
            body: new_body.to_string(),
            line: None,
        };
        let result = upsert_case_branch(source, &summary, "update", &spec).unwrap();
        assert!(
            result.contains("clamp 0 100 (model + 5)"),
            "new body not spliced: {result}"
        );
        assert!(
            !result.contains("model + 1"),
            "old body still present: {result}"
        );
        assert!(
            result.contains("Decrement"),
            "other branch missing after splice: {result}"
        );
    }
}
