//! Sub-declaration analysis helpers.
//!
//! Provides generic tree-walking utilities that inspect the interior of a
//! top-level Elm declaration:
//!
//! - [`compute_site_keys`] — stable string keys for a slice of case-expression
//!   sites (moved here from `variant.rs`; variant.rs uses it via
//!   `crate::analysis::compute_site_keys`).
//! - [`collect_let_sites`] — enumerate every let-binding inside a declaration
//!   with its scope path, line, and byte span.
//! - [`collect_case_sites_generic`] — enumerate every case expression inside a
//!   declaration with scrutinee text and branch details.
//! - [`collect_binders_in_scope`] — compute the set of names visible at a given
//!   byte offset inside a declaration (function params + enclosing let
//!   bindings + pattern-introduced names).

use std::collections::{BTreeMap, HashSet};
use tree_sitter::Node;

// ---------------------------------------------------------------------------
// SiteKeyable — abstraction for compute_site_keys
// ---------------------------------------------------------------------------

/// Trait implemented by anything that participates in the site-key algorithm.
/// [`compute_site_keys`] is generic over this so it can be used by both
/// `variant.rs` (whose `CaseSite` is private) and future callers.
pub trait SiteKeyable {
    /// The bare function (or let-binding) name containing the site.
    fn function_name(&self) -> &str;
    /// The display path of the file (e.g. `"src/Main.elm"`).
    fn display_path(&self) -> &str;
    /// The byte offset of the site's start within the file source.
    fn byte_start(&self) -> usize;
}

// ---------------------------------------------------------------------------
// compute_site_keys
// ---------------------------------------------------------------------------

/// Assign stable string keys to a slice of case-expression sites.
///
/// Keys use progressive qualification:
///
/// 1. Bare function name (`"update"`) — when every site has a unique function
///    name across the whole slice.
/// 2. `"update#N"` — when multiple sites share a function name in a single
///    file (ordinals are 1-based, source-ordered by `byte_start`).
/// 3. `"src/Main.elm:update"` — when the same function name appears in more
///    than one file but only once per file.
/// 4. `"src/Main.elm:update#N"` — when both conditions apply.
///
/// Ordinals are 1-based and source-ordered by [`SiteKeyable::byte_start`].
pub fn compute_site_keys<S: SiteKeyable>(sites: &[S]) -> Vec<String> {
    let mut keys = vec![String::new(); sites.len()];

    // Group indices by bare function name.
    let mut by_function: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, site) in sites.iter().enumerate() {
        by_function.entry(site.function_name()).or_default().push(i);
    }

    for (fname, indices) in by_function {
        if indices.len() == 1 {
            keys[indices[0]] = fname.to_string();
            continue;
        }

        // Subgroup by file display path.
        let mut by_file: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for &i in &indices {
            by_file.entry(sites[i].display_path()).or_default().push(i);
        }

        if by_file.len() == 1 {
            // All sites in one file: disambiguate with #N.
            let mut sorted = indices;
            sorted.sort_by_key(|&i| sites[i].byte_start());
            for (ord, i) in sorted.into_iter().enumerate() {
                keys[i] = format!("{}#{}", fname, ord + 1);
            }
        } else {
            // Sites span multiple files: prefix with display path.
            for (file, file_indices) in by_file {
                if file_indices.len() == 1 {
                    keys[file_indices[0]] = format!("{}:{}", file, fname);
                } else {
                    let mut sorted = file_indices;
                    sorted.sort_by_key(|&i| sites[i].byte_start());
                    for (ord, i) in sorted.into_iter().enumerate() {
                        keys[i] = format!("{}:{}#{}", file, fname, ord + 1);
                    }
                }
            }
        }
    }

    keys
}

// ---------------------------------------------------------------------------
// LetSite / collect_let_sites
// ---------------------------------------------------------------------------

/// A single let-binding found inside a top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetSite {
    /// The binding name (from `functionDeclarationLeft`'s first
    /// `lower_case_identifier`).
    pub name: String,
    /// 1-indexed absolute file line of the `value_declaration` (or its
    /// preceding `type_annotation` if one is present) start.
    pub line: usize,
    /// Chain of enclosing names from the top-level declaration down to (but
    /// not including) this binding, outermost first. The first element is
    /// always the top-level declaration name.
    pub scope_path: Vec<String>,
    /// Byte range `(start, end)` of the entire binding.  If the binding has
    /// a preceding `type_annotation` sibling with the same name, the span
    /// begins at the annotation start.
    pub node_span: (usize, usize),
}

/// Walk every `let_in_expr` descendant of `decl_node` and collect all
/// `value_declaration` children as [`LetSite`]s.
///
/// `decl_node` should be the `value_declaration` node of the top-level
/// declaration being analysed; the `functionDeclarationLeft` name becomes
/// the first element of every `scope_path`.
///
/// `source` is the full file source (UTF-8 bytes).
pub fn collect_let_sites(decl_node: Node, source: &str) -> Vec<LetSite> {
    let top_name = extract_value_decl_name(&decl_node, source.as_bytes()).unwrap_or_default();
    let mut results = Vec::new();
    // Walk the body of the top-level declaration.
    if let Some(body) = decl_node.child_by_field_name("body") {
        walk_for_let_sites(body, source.as_bytes(), &[top_name], &mut results);
    }
    results
}

/// Recursive descent: visit every `let_in_expr` reachable from `node`,
/// collecting bindings with `parent_scope` as context.
fn walk_for_let_sites(
    node: Node,
    source: &[u8],
    parent_scope: &[String],
    results: &mut Vec<LetSite>,
) {
    if node.kind() == "let_in_expr" {
        // Collect bindings declared directly in this let block.
        visit_let_block(node, source, parent_scope, results);
        // Recurse into the `in …` body.
        if let Some(body) = node.child_by_field_name("body") {
            walk_for_let_sites(body, source, parent_scope, results);
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_for_let_sites(child, source, parent_scope, results);
    }
}

/// Process all `value_declaration` (and adjacent `type_annotation`) children
/// of a `let_in_expr` node, registering them as `LetSite`s and recursing
/// into their bodies.
fn visit_let_block(
    let_node: Node,
    source: &[u8],
    parent_scope: &[String],
    results: &mut Vec<LetSite>,
) {
    let children: Vec<Node> = let_node.named_children(&mut let_node.walk()).collect();

    let mut i = 0;
    while i < children.len() {
        let child = children[i];
        match child.kind() {
            "type_annotation" => {
                // If the next sibling is a value_declaration for the same name,
                // treat annotation + declaration together.
                let ann_name = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source).ok())
                    .map(str::to_owned);

                if let Some(ann_name) = ann_name
                    && i + 1 < children.len()
                    && children[i + 1].kind() == "value_declaration"
                {
                    let val = children[i + 1];
                    let val_name = extract_value_decl_name(&val, source).unwrap_or_default();
                    if val_name == ann_name {
                        let line = child.start_position().row + 1;
                        let span = (child.start_byte(), val.end_byte());
                        results.push(LetSite {
                            name: val_name.clone(),
                            line,
                            scope_path: parent_scope.to_vec(),
                            node_span: span,
                        });
                        // Recurse into the binding body.
                        let child_scope = scope_append(parent_scope, &val_name);
                        if let Some(body) = val.child_by_field_name("body") {
                            walk_for_let_sites(body, source, &child_scope, results);
                        }
                        i += 2;
                        continue;
                    }
                }
                i += 1;
            }
            "value_declaration" => {
                let name = extract_value_decl_name(&child, source).unwrap_or_default();
                let line = child.start_position().row + 1;
                let span = (child.start_byte(), child.end_byte());
                results.push(LetSite {
                    name: name.clone(),
                    line,
                    scope_path: parent_scope.to_vec(),
                    node_span: span,
                });
                let child_scope = scope_append(parent_scope, &name);
                if let Some(body) = child.child_by_field_name("body") {
                    walk_for_let_sites(body, source, &child_scope, results);
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
}

fn scope_append(parent: &[String], name: &str) -> Vec<String> {
    let mut v = parent.to_vec();
    v.push(name.to_owned());
    v
}

// ---------------------------------------------------------------------------
// CaseSite / CaseBranch / collect_case_sites_generic
// ---------------------------------------------------------------------------

/// A single branch inside a [`CaseSite`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseBranch {
    /// Trimmed pattern text (e.g. `"Increment"`, `"( x, Just y )"`).
    pub pattern_text: String,
    /// Byte range `(start, end)` of the pattern node.
    pub pattern_span: (usize, usize),
    /// Byte range `(start, end)` of the branch body expression.
    pub body_span: (usize, usize),
    /// 1-indexed absolute file line of the branch pattern.
    pub line: usize,
}

/// A case expression found inside a top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSite {
    /// Trimmed text of the scrutinee expression (between `case` and `of`).
    pub scrutinee_text: String,
    /// Byte range `(start, end)` of the scrutinee expression node.
    pub scrutinee_span: (usize, usize),
    /// 1-indexed absolute file line of the `case` keyword.
    pub line: usize,
    /// All branches of this case expression, in source order.
    pub branches: Vec<CaseBranch>,
    /// Byte range `(start, end)` of the entire `case_of_expr` node.
    pub node_span: (usize, usize),
}

/// Walk every `case_of_expr` descendant of `decl_node` and return a
/// [`CaseSite`] for each one.
///
/// `source` is the full file source (UTF-8 bytes).
pub fn collect_case_sites_generic(decl_node: Node, source: &str) -> Vec<CaseSite> {
    let mut results = Vec::new();
    walk_for_case_sites(decl_node, source.as_bytes(), &mut results);
    results
}

fn walk_for_case_sites(node: Node, source: &[u8], results: &mut Vec<CaseSite>) {
    if node.kind() == "case_of_expr"
        && let Some(site) = parse_case_site(node, source)
    {
        results.push(site);
        // Still descend into the node's children below to catch nested case exprs.
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_for_case_sites(child, source, results);
    }
}

fn parse_case_site(node: Node, source: &[u8]) -> Option<CaseSite> {
    let scrutinee_node = node.child_by_field_name("expr")?;
    let scrutinee_text = scrutinee_node.utf8_text(source).ok()?.trim().to_string();
    let scrutinee_span = (scrutinee_node.start_byte(), scrutinee_node.end_byte());
    let line = node.start_position().row + 1;
    let node_span = (node.start_byte(), node.end_byte());

    let mut branches = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "case_of_branch"
            && let Some(branch) = parse_case_branch(child, source)
        {
            branches.push(branch);
        }
    }

    Some(CaseSite {
        scrutinee_text,
        scrutinee_span,
        line,
        branches,
        node_span,
    })
}

fn parse_case_branch(node: Node, source: &[u8]) -> Option<CaseBranch> {
    let pattern_node = node.child_by_field_name("pattern")?;
    let body_node = node.child_by_field_name("expr")?;

    let pattern_text = pattern_node.utf8_text(source).ok()?.trim().to_string();
    let pattern_span = (pattern_node.start_byte(), pattern_node.end_byte());
    let body_span = (body_node.start_byte(), body_node.end_byte());
    let line = pattern_node.start_position().row + 1;

    Some(CaseBranch {
        pattern_text,
        pattern_span,
        body_span,
        line,
    })
}

// ---------------------------------------------------------------------------
// collect_binders_in_scope
// ---------------------------------------------------------------------------

/// Return the set of all names visible at byte offset `position` inside
/// `decl_node`.
///
/// Includes:
/// - Function parameters of `decl_node` (from `functionDeclarationLeft`).
/// - Let-binding names in every `let_in_expr` that structurally encloses
///   `position` (i.e., `position` is within `let_in_expr`'s byte range).
///   All sibling bindings within the same let-block are in scope (Elm does
///   not require forward declaration within a let).
/// - Pattern-introduced names from enclosing case branches whose body spans
///   contain `position`.
/// - Parameter patterns from enclosing lambda expressions.
///
/// Sibling let-bindings in let-blocks that do NOT contain `position` are
/// excluded.
pub fn collect_binders_in_scope(decl_node: Node, source: &str, position: usize) -> HashSet<String> {
    let bytes = source.as_bytes();
    let mut names = HashSet::new();

    // Top-level function parameters.
    if let Some(fdl) = decl_node.child_by_field_name("functionDeclarationLeft") {
        collect_fdl_params(fdl, bytes, &mut names);
    }

    // Walk the body for enclosing scope constructs.
    if let Some(body) = decl_node.child_by_field_name("body") {
        visit_scope(body, bytes, position, &mut names);
    }

    names
}

/// Collect parameter names from a `function_declaration_left` node.
///
/// The first `lower_case_identifier` child is the function name itself and is
/// skipped; remaining `lower_case_identifier`s and pattern nodes are params.
fn collect_fdl_params(fdl: Node, source: &[u8], names: &mut HashSet<String>) {
    let mut cursor = fdl.walk();
    let mut saw_fn_name = false;
    for child in fdl.named_children(&mut cursor) {
        if child.kind() == "lower_case_identifier" {
            if !saw_fn_name {
                saw_fn_name = true; // skip the fn name
            } else if let Ok(text) = child.utf8_text(source) {
                names.insert(text.to_string());
            }
        } else {
            // Pattern parameter (tuple, record, etc.)
            collect_pattern_binders(child, source, names);
        }
    }
}

/// Recursively visit `node`, adding to `names` all binders that are in scope
/// at `position`.  Only descends into nodes whose byte range contains
/// `position`.
fn visit_scope(node: Node, source: &[u8], position: usize, names: &mut HashSet<String>) {
    // Skip branches that don't contain the target position.
    if !node_contains(node, position) {
        return;
    }

    match node.kind() {
        "let_in_expr" => {
            // All sibling value_declarations in this let-block are in scope
            // when position is inside the let_in_expr.
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "value_declaration"
                    && let Some(name) = extract_value_decl_name(&child, source)
                {
                    names.insert(name);
                }
            }
            // Recurse: check whether position is inside a binding body (nested
            // let) or the `in …` body.
            let mut cursor2 = node.walk();
            for child in node.named_children(&mut cursor2) {
                if child.kind() == "value_declaration" || child.kind() == "let_in_expr" {
                    visit_scope(child, source, position, names);
                }
            }
            if let Some(body) = node.child_by_field_name("body") {
                visit_scope(body, source, position, names);
            }
        }
        "case_of_branch" => {
            // Pattern binders are only in scope inside the branch body.
            if let (Some(pattern), Some(expr)) = (
                node.child_by_field_name("pattern"),
                node.child_by_field_name("expr"),
            ) && node_contains(expr, position)
            {
                collect_pattern_binders(pattern, source, names);
                visit_scope(expr, source, position, names);
            }
        }
        "anonymous_function_expr" => {
            // Lambda parameters are visible inside the lambda body.
            if let Some(body_expr) = node.child_by_field_name("expr")
                && node_contains(body_expr, position)
            {
                // Collect all named children up to the body as param patterns.
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if child == body_expr {
                        break;
                    }
                    collect_pattern_binders(child, source, names);
                }
                visit_scope(body_expr, source, position, names);
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                visit_scope(child, source, position, names);
            }
        }
    }
}

/// Returns `true` if `node`'s byte range contains `position` (inclusive of
/// `end_byte`).
fn node_contains(node: Node, position: usize) -> bool {
    node.start_byte() <= position && position <= node.end_byte()
}

/// Recursively extract all bound names from a pattern node.
pub fn collect_pattern_binders(node: Node, source: &[u8], names: &mut HashSet<String>) {
    match node.kind() {
        "lower_pattern" | "lower_case_identifier" => {
            if let Ok(text) = node.utf8_text(source) {
                let t = text.trim();
                if !t.is_empty() && t != "_" {
                    names.insert(t.to_string());
                }
            }
        }
        "anything_pattern" => {
            // `_` — binds nothing
        }
        _ => {
            // Recurse into composite patterns.
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_pattern_binders(child, source, names);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Extract the name from a `value_declaration` node via
/// `functionDeclarationLeft` → first `lower_case_identifier`.
fn extract_value_decl_name(node: &Node, source: &[u8]) -> Option<String> {
    let fdl = node.child_by_field_name("functionDeclarationLeft")?;
    let mut cursor = fdl.walk();
    for child in fdl.named_children(&mut cursor) {
        if child.kind() == "lower_case_identifier" {
            return Some(child.utf8_text(source).ok()?.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Parse source and return the first top-level `value_declaration` with
    /// the given name.
    fn find_top_level_decl<'tree>(
        root: tree_sitter::Node<'tree>,
        source: &[u8],
        name: &str,
    ) -> Option<tree_sitter::Node<'tree>> {
        let mut cursor = root.walk();
        for child in root.named_children(&mut cursor) {
            if child.kind() == "value_declaration"
                && let Some(n) = extract_value_decl_name(&child, source)
                && n == name
            {
                return Some(child);
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // compute_site_keys — smoke tests via a lightweight adapter
    // -----------------------------------------------------------------------

    struct TestSite {
        function: String,
        display: String,
        start: usize,
    }

    impl SiteKeyable for TestSite {
        fn function_name(&self) -> &str {
            &self.function
        }
        fn display_path(&self) -> &str {
            &self.display
        }
        fn byte_start(&self) -> usize {
            self.start
        }
    }

    fn ts(display: &str, function: &str, start: usize) -> TestSite {
        TestSite {
            function: function.to_string(),
            display: display.to_string(),
            start,
        }
    }

    #[test]
    fn site_key_single_is_bare_function() {
        let sites = vec![ts("src/Main.elm", "update", 100)];
        assert_eq!(compute_site_keys(&sites), vec!["update"]);
    }

    #[test]
    fn site_key_distinct_functions_stay_bare() {
        let sites = vec![
            ts("src/Main.elm", "update", 100),
            ts("src/Main.elm", "view", 200),
        ];
        assert_eq!(compute_site_keys(&sites), vec!["update", "view"]);
    }

    #[test]
    fn site_key_same_function_same_file_gets_ordinals() {
        // byte 500 is listed first in the slice but sorts later → gets #2
        let sites = vec![
            ts("src/Main.elm", "update", 500),
            ts("src/Main.elm", "update", 100),
        ];
        assert_eq!(compute_site_keys(&sites), vec!["update#2", "update#1"]);
    }

    #[test]
    fn site_key_same_function_different_files_gets_file_prefix() {
        let sites = vec![
            ts("src/Main.elm", "update", 100),
            ts("src/Page.elm", "update", 200),
        ];
        assert_eq!(
            compute_site_keys(&sites),
            vec!["src/Main.elm:update", "src/Page.elm:update"]
        );
    }

    #[test]
    fn site_key_full_qualification() {
        let sites = vec![
            ts("src/Main.elm", "update", 500),
            ts("src/Main.elm", "update", 100),
            ts("src/Page.elm", "update", 200),
        ];
        let keys = compute_site_keys(&sites);
        assert_eq!(
            keys,
            vec![
                "src/Main.elm:update#2",
                "src/Main.elm:update#1",
                "src/Page.elm:update",
            ]
        );
    }

    // -----------------------------------------------------------------------
    // (a) nested let bindings — distinct LetSites with different scope_path
    // -----------------------------------------------------------------------

    #[test]
    fn nested_let_bindings_distinct_scope_paths() {
        let source = r#"module Main exposing (..)

outer x =
    let
        helperA =
            let
                h = 1
            in
            h

        helperB =
            let
                h = 2
            in
            h
    in
    helperA + helperB
"#;
        let tree = parse(source).unwrap();
        let root = tree.root_node();
        let bytes = source.as_bytes();

        let decl = find_top_level_decl(root, bytes, "outer").expect("outer not found");
        let sites = collect_let_sites(decl, source);

        let names: Vec<&str> = sites.iter().map(|s| s.name.as_str()).collect();

        assert!(
            names.contains(&"helperA"),
            "expected helperA; got {names:?}"
        );
        assert!(
            names.contains(&"helperB"),
            "expected helperB; got {names:?}"
        );
        assert_eq!(
            names.iter().filter(|&&n| n == "h").count(),
            2,
            "expected two 'h' bindings; got {names:?}"
        );

        let h_sites: Vec<&LetSite> = sites.iter().filter(|s| s.name == "h").collect();
        assert_eq!(h_sites.len(), 2);

        // The two 'h' bindings must have different scope paths.
        assert_ne!(
            h_sites[0].scope_path, h_sites[1].scope_path,
            "two 'h' bindings must have different scope_paths"
        );

        let paths: Vec<&Vec<String>> = h_sites.iter().map(|s| &s.scope_path).collect();
        let expect_a: Vec<String> = vec!["outer".into(), "helperA".into()];
        let expect_b: Vec<String> = vec!["outer".into(), "helperB".into()];
        assert!(
            paths.contains(&&expect_a),
            "expected scope_path [outer, helperA]; got {paths:?}"
        );
        assert!(
            paths.contains(&&expect_b),
            "expected scope_path [outer, helperB]; got {paths:?}"
        );
    }

    // -----------------------------------------------------------------------
    // (b) collect_case_sites_generic — tuple scrutinee + nested constructor
    // -----------------------------------------------------------------------

    #[test]
    fn case_site_tuple_scrutinee_and_nested_constructor() {
        let source = r#"module Main exposing (..)

toLabel pair =
    case pair of
        ( 0, Just x ) ->
            "zero-just"

        ( n, Nothing ) ->
            "other"
"#;
        let tree = parse(source).unwrap();
        let root = tree.root_node();
        let bytes = source.as_bytes();

        let decl = find_top_level_decl(root, bytes, "toLabel").expect("toLabel not found");
        let sites = collect_case_sites_generic(decl, source);

        assert_eq!(sites.len(), 1, "expected exactly one case site");
        let site = &sites[0];

        assert_eq!(
            site.scrutinee_text, "pair",
            "scrutinee_text: {:?}",
            site.scrutinee_text
        );
        assert_eq!(site.branches.len(), 2, "expected 2 branches");

        let first = &site.branches[0];
        assert!(
            first.pattern_text.contains("0") && first.pattern_text.contains("Just"),
            "expected first pattern to contain '0' and 'Just'; got {:?}",
            first.pattern_text
        );

        // Spans must be non-empty.
        assert!(first.pattern_span.0 < first.pattern_span.1);
        assert!(first.body_span.0 < first.body_span.1);

        // Byte-exact: text at pattern_span == pattern_text (trimmed).
        let pat_str = std::str::from_utf8(&bytes[first.pattern_span.0..first.pattern_span.1])
            .unwrap()
            .trim();
        assert_eq!(pat_str, first.pattern_text);

        // Scrutinee span check.
        let scr_str = std::str::from_utf8(&bytes[site.scrutinee_span.0..site.scrutinee_span.1])
            .unwrap()
            .trim();
        assert_eq!(scr_str, site.scrutinee_text);
    }

    // -----------------------------------------------------------------------
    // (c) collect_binders_in_scope
    // -----------------------------------------------------------------------

    #[test]
    fn binders_in_scope_excludes_sibling_let_bindings_outside_position() {
        // `process items` has:
        //   - param: `items`
        //   - outer let: bindings `a` and `b`
        //   - `a` itself contains a nested let with binding `inner`
        //
        // At a position inside `a`'s nested body: items, a, b, inner are visible.
        // At a position inside `b`'s value expression: items, a, b are visible,
        //   but `inner` is NOT (it's only in scope inside `a`'s nested let).
        let source = r#"module Main exposing (..)

process items =
    let
        a =
            let
                inner = 99
            in
            inner + 1

        b = 2
    in
    a + b
"#;
        let tree = parse(source).unwrap();
        let root = tree.root_node();
        let bytes = source.as_bytes();

        let decl = find_top_level_decl(root, bytes, "process").expect("process not found");

        // Position inside `inner + 1` (the body of `a`'s nested let).
        let inner_body_pos = source
            .find("inner + 1")
            .expect("could not locate 'inner + 1'");
        let pos_inside_a = inner_body_pos + 2; // mid-word in "inner"

        let binders_a = collect_binders_in_scope(decl, source, pos_inside_a);

        assert!(
            binders_a.contains("items"),
            "items missing; got {binders_a:?}"
        );
        assert!(binders_a.contains("a"), "a missing; got {binders_a:?}");
        assert!(binders_a.contains("b"), "b missing; got {binders_a:?}");
        assert!(
            binders_a.contains("inner"),
            "inner missing; got {binders_a:?}"
        );

        // Position inside `b = 2` — specifically at the `2`.
        // Find `b = 2` line and position past the `= `.
        let b_eq_pos = source.find("        b = 2").expect("b = 2 not found");
        let pos_inside_b = b_eq_pos + "        b = ".len() + 1; // past the `2`

        let binders_b = collect_binders_in_scope(decl, source, pos_inside_b);

        assert!(
            binders_b.contains("items"),
            "items missing at b; got {binders_b:?}"
        );
        assert!(binders_b.contains("a"), "a missing at b; got {binders_b:?}");
        assert!(binders_b.contains("b"), "b missing at b; got {binders_b:?}");
        assert!(
            !binders_b.contains("inner"),
            "'inner' should NOT be in scope at b's body; got {binders_b:?}"
        );
    }
}
