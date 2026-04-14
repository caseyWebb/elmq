use crate::FileSummary;
use crate::analysis;
use crate::parser;
use anyhow::{Result, bail};
use tree_sitter::Node;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// How to address a parameter for removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgTarget {
    /// 1-indexed position in the parameter list.
    Position(usize),
    /// Parameter name.
    Name(String),
}

// ---------------------------------------------------------------------------
// add_function_arg
// ---------------------------------------------------------------------------

/// Add a new parameter to the function declaration `decl_name`.
///
/// `at` is 1-indexed. Allowed range: `1..=N+1` where `N` is the current
/// parameter count. If the declaration has a type signature, `type_opt` is
/// required and is inserted into the arrow chain at position `at`. If there
/// is no signature, `type_opt` is silently ignored.
pub fn add_function_arg(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    at: usize,
    name: &str,
    type_opt: Option<&str>,
) -> Result<String> {
    let decl = summary
        .find_declaration(decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{}' not found", decl_name))?;

    let tree = parser::parse(source)?;
    let root = tree.root_node();

    // Locate value_declaration and optional type_annotation CST nodes.
    let (ann_node, val_node) = find_decl_nodes(&root, source, decl_name, decl.start_line)?;

    // Collect current params from the FDL.
    let fdl = val_node
        .child_by_field_name("functionDeclarationLeft")
        .ok_or_else(|| anyhow::anyhow!("'{}' has no functionDeclarationLeft", decl_name))?;

    let params = collect_fdl_params_nodes(&fdl, source);
    let n = params.len(); // current param count (name nodes only, not fn name)

    if at < 1 || at > n + 1 {
        bail!(
            "'--at {}' exceeds parameter count; valid range is 1..={} ({} existing params, +1 for append)",
            at,
            n + 1,
            n
        );
    }

    // Validate / require --type when there's a signature. We also validate a
    // user-supplied --type even when the target is untyped, so "silently
    // ignored" doesn't let syntactically broken types through.
    let has_sig = ann_node.is_some();
    if has_sig {
        let type_str = type_opt.ok_or_else(|| {
            anyhow::anyhow!("'{}' has a type signature; --type is required", decl_name)
        })?;
        parser::parse_arg_type(type_str)?;
    } else if let Some(type_str) = type_opt {
        parser::parse_arg_type(type_str)?;
    }

    // Build the new source by applying replacements rear-to-front.
    let mut result = source.to_string();

    // ---- Splice the type signature (if present) ----
    if let Some(ann) = ann_node {
        let type_str = type_opt.unwrap(); // validated above
        let new_sig = insert_into_signature(&ann, source, at, type_str)?;
        // Replace the annotation node bytes.
        let start = ann.start_byte();
        let end = ann.end_byte();
        result.replace_range(start..end, &new_sig);
        // Re-parse to get fresh offsets for the FDL (source has shifted).
        // We'll do this by re-parsing; but it's simpler to apply sig first,
        // then recalc the FDL position from scratch.
    }

    // ---- Splice the parameter name into the FDL ----
    // Re-parse (possibly updated) result to get fresh FDL offsets.
    {
        let tree2 = parser::parse(&result)?;
        let root2 = tree2.root_node();
        let (_, val2) = find_decl_nodes(&root2, &result, decl_name, decl.start_line)?;
        let fdl2 = val2
            .child_by_field_name("functionDeclarationLeft")
            .ok_or_else(|| anyhow::anyhow!("FDL disappeared after sig update"))?;
        let params2 = collect_fdl_params_nodes(&fdl2, &result);

        // Determine insertion byte position and whether to add a leading or
        // trailing space around the new name token.
        //
        // Inserting *before* an existing param: put `"name "` at that param's
        // start byte so the new name pushes the existing one to the right.
        //
        // Appending (after the last param or the fn name when zero params exist):
        // put `" name"` right after the anchor end byte so we get a space
        // between the anchor and the new token.
        let text_to_insert;
        let insert_byte;
        if at - 1 < params2.len() {
            // Insert before the param currently at position `at`.
            insert_byte = params2[at - 1].start_byte();
            text_to_insert = format!("{} ", name);
        } else {
            // Append: insert after the last param (or after the fn name if no params).
            let fn_name_node = fdl2
                .named_children(&mut fdl2.walk())
                .next()
                .ok_or_else(|| anyhow::anyhow!("FDL has no children"))?;
            insert_byte = if params2.is_empty() {
                fn_name_node.end_byte()
            } else {
                params2.last().unwrap().end_byte()
            };
            text_to_insert = format!(" {}", name);
        };

        result.insert_str(insert_byte, &text_to_insert);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// remove_function_arg
// ---------------------------------------------------------------------------

/// Remove a single parameter from the function declaration `decl_name`.
pub fn remove_function_arg(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    target: &ArgTarget,
) -> Result<String> {
    remove_function_args_batch(source, summary, decl_name, std::slice::from_ref(target))
}

// ---------------------------------------------------------------------------
// remove_function_args_batch
// ---------------------------------------------------------------------------

/// Remove multiple parameters from the function declaration `decl_name`.
///
/// All targets must be the same variant (all `Position` or all `Name`).
/// Positions are resolved up front against original indices; rear-to-front
/// processing avoids index-shift bugs.
pub fn remove_function_args_batch(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    targets: &[ArgTarget],
) -> Result<String> {
    if targets.is_empty() {
        return Ok(source.to_string());
    }

    // Validate all-same variant.
    let all_position = targets.iter().all(|t| matches!(t, ArgTarget::Position(_)));
    let all_name = targets.iter().all(|t| matches!(t, ArgTarget::Name(_)));
    if !all_position && !all_name {
        bail!("rm arg targets must all use --at or all use --name, not a mix of both");
    }

    let decl = summary
        .find_declaration(decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{}' not found", decl_name))?;

    let tree = parser::parse(source)?;
    let root = tree.root_node();
    let (ann_node, val_node) = find_decl_nodes(&root, source, decl_name, decl.start_line)?;

    let fdl = val_node
        .child_by_field_name("functionDeclarationLeft")
        .ok_or_else(|| anyhow::anyhow!("'{}' has no functionDeclarationLeft", decl_name))?;

    let params = collect_fdl_params_nodes(&fdl, source);
    let n = params.len();

    // Resolve all targets to 1-indexed positions up front.
    let mut positions: Vec<usize> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    if all_position {
        for t in targets {
            if let ArgTarget::Position(p) = t {
                if *p < 1 || *p > n {
                    errors.push(format!(
                        "'--at {}' is out of range; valid range is 1..={} ({} params)",
                        p, n, n
                    ));
                } else {
                    positions.push(*p);
                }
            }
        }
    } else {
        // All Name
        for t in targets {
            if let ArgTarget::Name(nm) = t {
                let found = params.iter().enumerate().find_map(|(i, node)| {
                    node.utf8_text(source.as_bytes())
                        .ok()
                        .filter(|&text| text == nm.as_str())
                        .map(|_| i + 1) // 1-indexed
                });
                match found {
                    Some(pos) => positions.push(pos),
                    None => errors.push(format!("parameter '{}' not found in '{}'", nm, decl_name)),
                }
            }
        }
    }

    if !errors.is_empty() {
        bail!("{}", errors.join("; "));
    }

    // De-duplicate and sort rear-to-front for safe splicing.
    positions.sort_unstable();
    positions.dedup();
    positions.reverse(); // rear-to-front

    // Build byte-range replacement list.
    // Collect (start_byte, end_byte) for param tokens plus their surrounding whitespace.
    // We need to remove the param name token AND the separating space.
    let mut byte_removals: Vec<(usize, usize)> = Vec::new();

    // For sig removals: we'll remove the entire sig text and rebuild it.
    let has_sig = ann_node.is_some();

    // Compute signature arrow parts if we have a sig.
    let sig_parts: Option<Vec<String>> = if has_sig {
        let ann = ann_node.unwrap();
        let type_expr_node = ann
            .child_by_field_name("typeExpression")
            .ok_or_else(|| anyhow::anyhow!("type_annotation has no typeExpression"))?;
        let type_expr_text = type_expr_node.utf8_text(source.as_bytes())?;
        Some(split_arrow_chain(type_expr_text))
    } else {
        None
    };

    // Apply rear-to-front removals for FDL params.
    // We also need to handle the whitespace token next to each param.
    // Gather all param byte ranges first (1-indexed, per the original params list).
    let param_ranges: Vec<(usize, usize)> = params
        .iter()
        .map(|n| (n.start_byte(), n.end_byte()))
        .collect();

    // For each position (already rear-to-front), figure out which bytes to remove:
    // The param token itself plus one trailing space (if there's a next token),
    // or one leading space (if it's the last param).
    for &pos in &positions {
        let idx = pos - 1; // 0-indexed
        let (pstart, pend) = param_ranges[idx];

        // Determine whether to eat the leading or trailing space.
        // If there's a param after this one: eat the trailing space.
        // If this is the last param: eat the leading space.
        // Handle the case where it's the only param (eat trailing space from fn name end).
        let (remove_start, remove_end) = if n > 1 && idx + 1 < n {
            // Not the last param: eat trailing space (pend..pend+1 is a space).
            (pstart, pend + 1)
        } else if idx > 0 {
            // Last param (and not only param): eat leading space.
            (pstart - 1, pend)
        } else {
            // Only param: eat trailing space if present.
            if pend < source.len() && source.as_bytes()[pend] == b' ' {
                (pstart, pend + 1)
            } else {
                (pstart, pend)
            }
        };
        byte_removals.push((remove_start, remove_end));
    }

    // Apply FDL removals rear-to-front (positions are already rear-to-front).
    let mut result = source.to_string();
    for (rs, re) in &byte_removals {
        result.replace_range(*rs..*re, "");
    }

    // Apply signature removals if we have a sig.
    if let Some(mut parts) = sig_parts {
        // We need fresh parse offsets after FDL edits.
        let tree3 = parser::parse(&result)?;
        let root3 = tree3.root_node();
        let (ann3, _) = find_decl_nodes(&root3, &result, decl_name, decl.start_line)?;
        let ann3 =
            ann3.ok_or_else(|| anyhow::anyhow!("type annotation disappeared after FDL edit"))?;

        // Remove the parts at the collected positions.
        // positions is rear-to-front, but parts is indexed forward.
        // We need to remove by original position — sort forward first.
        let mut forward_positions = positions.clone();
        forward_positions.reverse(); // back to ascending
        // Remove rear-to-front from parts to preserve indices.
        let mut rev_pos = forward_positions.clone();
        rev_pos.reverse();
        for p in rev_pos {
            let idx = p - 1;
            if idx < parts.len() {
                parts.remove(idx);
            }
        }

        // Rebuild the type expression text.
        let new_type_text = parts.join(" -> ");

        // Rebuild the full annotation: `<name> : <new_type_text>`.
        let ann_name = ann3
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(result.as_bytes()).ok())
            .unwrap_or(decl_name);
        let new_ann = format!("{} : {}", ann_name, new_type_text);

        let start = ann3.start_byte();
        let end = ann3.end_byte();
        result.replace_range(start..end, &new_ann);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// rename_function_arg
// ---------------------------------------------------------------------------

/// Rename a parameter in the function definition from `old` to `new`,
/// updating every reference in the function body. The type signature is
/// not modified.
pub fn rename_function_arg(
    source: &str,
    summary: &FileSummary,
    decl_name: &str,
    old: &str,
    new: &str,
) -> Result<String> {
    let decl = summary
        .find_declaration(decl_name)
        .ok_or_else(|| anyhow::anyhow!("declaration '{}' not found", decl_name))?;

    let tree = parser::parse(source)?;
    let root = tree.root_node();
    let (_, val_node) = find_decl_nodes(&root, source, decl_name, decl.start_line)?;

    let fdl = val_node
        .child_by_field_name("functionDeclarationLeft")
        .ok_or_else(|| anyhow::anyhow!("'{}' has no functionDeclarationLeft", decl_name))?;

    let params = collect_fdl_params_nodes(&fdl, source);

    // Find the parameter named `old`.
    let old_param_node = params
        .iter()
        .find(|n| n.utf8_text(source.as_bytes()).ok() == Some(old))
        .ok_or_else(|| anyhow::anyhow!("parameter '{}' not found in '{}'", old, decl_name))?;

    // Collision check: collect binders in scope at a position inside the body.
    let body_node = val_node
        .child_by_field_name("body")
        .ok_or_else(|| anyhow::anyhow!("'{}' has no body for scope analysis", decl_name))?;
    let position_in_body = body_node.start_byte() + 1;
    let binders = analysis::collect_binders_in_scope(val_node, source, position_in_body);

    if binders.contains(new) && new != old {
        bail!("'{}' is already in scope", new);
    }

    // Shadowing guard: if any inner let binding or pattern introduces a
    // binder named `old` within the body, rewriting every `value_qid` whose
    // text equals `old` would incorrectly rewrite references that refer to
    // the inner binding. Elm 0.19 disallows shadowing, so valid code can't
    // reach this branch; error cleanly rather than produce wrong output.
    if inner_shadows_name(&body_node, source.as_bytes(), old) {
        bail!(
            "cannot rename parameter '{old}' in '{decl_name}': an inner let binding or pattern shadows it (Elm 0.19 disallows shadowing; fix the shadowing first)"
        );
    }

    // Collect replacements: old_param_node in FDL + all value_qid in body with text == old.
    let mut replacements: Vec<(usize, usize)> = Vec::new();

    // The param name in the FDL.
    replacements.push((old_param_node.start_byte(), old_param_node.end_byte()));

    // References in the body.
    collect_value_refs(&body_node, source, old, &mut replacements);

    // Also check for pattern bindings in the body (case branches, lambda args)
    // that introduce `old` — don't rename those inner binders, but we should not
    // rename references shadowed by them either. However, the spec says to rename
    // every reference within the function body that refers to the parameter. Since
    // Elm doesn't have shadowing in practice, we do a simple name-exact pass here.
    // (The collision check above already guards against new conflicting with
    // any in-scope name, so this is safe.)

    // Sort and apply rear-to-front.
    replacements.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let mut result = source.to_string();
    for (start, end) in replacements {
        result.replace_range(start..end, new);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Recursively walk `node` looking for any binding position (let binding
/// name, `case_of_branch` pattern variable, lambda argument pattern) whose
/// text equals `name`. Returns true at the first hit.
fn inner_shadows_name(node: &Node, source: &[u8], name: &str) -> bool {
    // A `value_declaration` inside a `let_in_expr` binds its name.
    if node.kind() == "value_declaration"
        && let Some(fdl) = node.child_by_field_name("functionDeclarationLeft")
        && let Some(first) = {
            let mut c = fdl.walk();
            fdl.named_children(&mut c).next()
        }
        && first.utf8_text(source).ok() == Some(name)
    {
        return true;
    }
    // Pattern-introduced names (case branches, lambda args, function sub-decl
    // pattern parameters). Tree-sitter surfaces these as `lower_pattern` nodes
    // whose text is the bound identifier.
    if node.kind() == "lower_pattern" && node.utf8_text(source).ok() == Some(name) {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if inner_shadows_name(&child, source, name) {
            return true;
        }
    }
    false
}

/// Walk all named children of `root` to find the `type_annotation` and
/// `value_declaration` nodes for `decl_name`. We use `start_line` as a hint
/// to anchor to the right pair when multiple same-named declarations might
/// exist (shouldn't happen in valid Elm, but guards us).
fn find_decl_nodes<'tree>(
    root: &Node<'tree>,
    source: &str,
    decl_name: &str,
    start_line: usize,
) -> Result<(Option<Node<'tree>>, Node<'tree>)> {
    let mut cursor = root.walk();
    let children: Vec<Node> = root.named_children(&mut cursor).collect();

    let mut i = 0;
    while i < children.len() {
        let node = children[i];
        if node.kind() == "type_annotation" {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or_default();
            if name == decl_name
                && i + 1 < children.len()
                && children[i + 1].kind() == "value_declaration"
            {
                let val = children[i + 1];
                return Ok((Some(node), val));
            }
        } else if node.kind() == "value_declaration" {
            let vname = node
                .child_by_field_name("functionDeclarationLeft")
                .and_then(|fdl| {
                    let mut c = fdl.walk();
                    fdl.named_children(&mut c)
                        .find(|n| n.kind() == "lower_case_identifier")
                })
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or_default();
            if vname == decl_name {
                let row = node.start_position().row + 1;
                // Accept if the row is near start_line (within a few lines for
                // annotation-less decls).
                if row >= start_line.saturating_sub(2) {
                    return Ok((None, node));
                }
            }
        }
        i += 1;
    }
    bail!("could not locate CST nodes for declaration '{}'", decl_name)
}

/// Collect the parameter name nodes from a `function_declaration_left` node.
/// The first `lower_case_identifier` is the function name itself and is skipped.
/// Returns only identifier nodes (not pattern nodes like tuple/record destructures).
/// For simplicity we handle the common case of simple identifier parameters.
fn collect_fdl_params_nodes<'tree>(fdl: &Node<'tree>, _source: &str) -> Vec<Node<'tree>> {
    let mut cursor = fdl.walk();
    let mut params = Vec::new();
    let mut saw_fn_name = false;
    for child in fdl.named_children(&mut cursor) {
        if child.kind() == "lower_case_identifier" {
            if !saw_fn_name {
                saw_fn_name = true;
                continue; // skip function name
            }
            params.push(child);
        } else {
            // Pattern parameter (tuple, record, etc.) — track it too so
            // position-based addressing works. We don't support renaming
            // inside patterns here, but we need to count them.
            if saw_fn_name {
                params.push(child);
            }
        }
    }
    params
}

/// Split a type expression string by the top-level `->` arrows, respecting
/// parentheses depth and not splitting inside parenthesized sub-types.
///
/// Examples:
/// - `"Msg -> Model -> Model"` → `["Msg", "Model", "Model"]`
/// - `"(a -> b) -> c"` → `["(a -> b)", "c"]`
/// - `"Bool"` → `["Bool"]`
///
/// This operates purely on the text representation of the right-hand side of
/// a type annotation. Because `->` in Elm is right-associative, the raw text
/// `A -> B -> C` is equivalent to `A -> (B -> C)`, but both render as the
/// same flat arrow chain in the source. We split at the top-level `->` only.
pub(crate) fn split_arrow_chain(type_expr: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize; // paren/bracket depth
    let mut current = String::new();
    let bytes = type_expr.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'{' => {
                depth += 1;
                current.push(bytes[i] as char);
                i += 1;
            }
            b')' | b'}' => {
                depth = depth.saturating_sub(1);
                current.push(bytes[i] as char);
                i += 1;
            }
            b'-' if depth == 0 && i + 1 < bytes.len() && bytes[i + 1] == b'>' => {
                // Top-level arrow.
                parts.push(current.trim().to_string());
                current = String::new();
                i += 2; // skip `->`
                // Skip leading whitespace after arrow.
                while i < bytes.len() && bytes[i] == b' ' {
                    i += 1;
                }
            }
            c => {
                current.push(c as char);
                i += 1;
            }
        }
    }
    let tail = current.trim().to_string();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

/// Insert a new type string at position `at` (1-indexed) in the signature's
/// arrow chain, respecting the existing whitespace and structure. Returns the
/// new full annotation text (just the `name : type` text, without trailing
/// newline).
fn insert_into_signature<'tree>(
    ann_node: &Node<'tree>,
    source: &str,
    at: usize,
    new_type: &str,
) -> Result<String> {
    let name = ann_node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .ok_or_else(|| anyhow::anyhow!("type_annotation has no name"))?;

    let type_expr_node = ann_node
        .child_by_field_name("typeExpression")
        .ok_or_else(|| anyhow::anyhow!("type_annotation has no typeExpression"))?;
    let type_expr_text = type_expr_node.utf8_text(source.as_bytes())?;

    let mut parts = split_arrow_chain(type_expr_text);
    // `at` is 1-indexed: insert before the existing element at that position.
    // at=1 → index 0, at=N+1 → append at end.
    let insert_idx = (at - 1).min(parts.len());
    parts.insert(insert_idx, new_type.to_string());

    Ok(format!("{} : {}", name, parts.join(" -> ")))
}

/// Recursively collect all `value_qid` nodes inside `node` whose text equals
/// `target`, appending their byte ranges to `out`.
fn collect_value_refs(node: &Node, source: &str, target: &str, out: &mut Vec<(usize, usize)>) {
    if node.kind() == "value_qid"
        && let Ok(text) = node.utf8_text(source.as_bytes())
        && text == target
    {
        out.push((node.start_byte(), node.end_byte()));
        return; // don't recurse into the qid's children
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_value_refs(&child, source, target, out);
    }
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

    // Helper: parse source, get summary, call add_function_arg, return result string.
    fn add_arg(
        source: &str,
        decl: &str,
        at: usize,
        name: &str,
        ty: Option<&str>,
    ) -> Result<String> {
        let summary = make_summary(source);
        add_function_arg(source, &summary, decl, at, name, ty)
    }

    fn rm_arg_pos(source: &str, decl: &str, positions: &[usize]) -> Result<String> {
        let summary = make_summary(source);
        let targets: Vec<ArgTarget> = positions.iter().map(|&p| ArgTarget::Position(p)).collect();
        remove_function_args_batch(source, &summary, decl, &targets)
    }

    fn rm_arg_name(source: &str, decl: &str, names: &[&str]) -> Result<String> {
        let summary = make_summary(source);
        let targets: Vec<ArgTarget> = names
            .iter()
            .map(|&n| ArgTarget::Name(n.to_string()))
            .collect();
        remove_function_args_batch(source, &summary, decl, &targets)
    }

    fn rename_arg(source: &str, decl: &str, old: &str, new: &str) -> Result<String> {
        let summary = make_summary(source);
        rename_function_arg(source, &summary, decl, old, new)
    }

    // (a) Add typed arg at position 2 to a 2-arg typed function — sig and def both updated.
    #[test]
    fn test_add_typed_arg_position_2_typed_fn() {
        let source = "module Main exposing (..)\n\nupdate : Msg -> Model -> Model\nupdate msg model =\n    model\n";
        let result = add_arg(source, "update", 2, "flag", Some("Bool")).unwrap();
        assert!(
            result.contains("update : Msg -> Bool -> Model -> Model"),
            "sig not updated: {result}"
        );
        assert!(
            result.contains("update msg flag model ="),
            "def not updated: {result}"
        );
    }

    // (b) Add untyped arg to an untyped function — definition only.
    #[test]
    fn test_add_untyped_arg_no_sig() {
        let source = "module Main exposing (..)\n\nlogImpl level msg =\n    msg\n";
        let result = add_arg(source, "logImpl", 1, "tag", None).unwrap();
        // No signature created.
        assert!(!result.contains("logImpl :"), "unexpected sig: {result}");
        assert!(
            result.contains("logImpl tag level msg ="),
            "def not updated: {result}"
        );
    }

    // (c) --type missing on typed function errors with exact message.
    #[test]
    fn test_add_arg_missing_type_on_typed_fn() {
        let source = "module Main exposing (..)\n\nupdate : Msg -> Model -> Model\nupdate msg model =\n    model\n";
        let err = add_arg(source, "update", 2, "flag", None).unwrap_err();
        assert!(
            err.to_string()
                .contains("'update' has a type signature; --type is required"),
            "unexpected error: {err}"
        );
    }

    // (d) --at 1 prepends.
    #[test]
    fn test_add_arg_at_1_prepends() {
        let source = "module Main exposing (..)\n\nf a b =\n    a\n";
        let result = add_arg(source, "f", 1, "x", None).unwrap();
        assert!(result.contains("f x a b ="), "not prepended: {result}");
    }

    // (e) --at N+1 appends.
    #[test]
    fn test_add_arg_append() {
        let source = "module Main exposing (..)\n\nf a b =\n    a\n";
        // N=2, at=3 should append.
        let result = add_arg(source, "f", 3, "z", None).unwrap();
        assert!(result.contains("f a b z ="), "not appended: {result}");
    }

    // (f) --at N+2 errors with range hint.
    #[test]
    fn test_add_arg_out_of_range() {
        let source = "module Main exposing (..)\n\nf a b c =\n    a\n";
        // N=3, at=5 is N+2 → error.
        let err = add_arg(source, "f", 5, "x", None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("'--at 5' exceeds parameter count"),
            "unexpected: {msg}"
        );
        assert!(msg.contains("1..=4"), "no range hint: {msg}");
    }

    // (g) Remove by position 2 updates sig + def correctly.
    #[test]
    fn test_remove_by_position_updates_sig_and_def() {
        let source = "module Main exposing (..)\n\nupdate : Msg -> Bool -> Model -> Model\nupdate msg flag model =\n    model\n";
        let result = rm_arg_pos(source, "update", &[2]).unwrap();
        assert!(
            result.contains("update : Msg -> Model -> Model"),
            "sig wrong: {result}"
        );
        assert!(result.contains("update msg model ="), "def wrong: {result}");
    }

    // (h) Remove by name updates sig + def correctly.
    #[test]
    fn test_remove_by_name_updates_sig_and_def() {
        let source = "module Main exposing (..)\n\nupdate : Msg -> Bool -> Model -> Model\nupdate msg flag model =\n    model\n";
        let result = rm_arg_name(source, "update", &["flag"]).unwrap();
        assert!(
            result.contains("update : Msg -> Model -> Model"),
            "sig wrong: {result}"
        );
        assert!(result.contains("update msg model ="), "def wrong: {result}");
    }

    // (i) Multi-position remove with rear-to-front processing.
    //     `rm --at 2 --at 4` on a 4-arg function removes originally-2nd and -4th.
    #[test]
    fn test_multi_position_remove_rear_to_front() {
        let source =
            "module Main exposing (..)\n\nfn : A -> B -> C -> D -> E\nfn a b c d =\n    a\n";
        let result = rm_arg_pos(source, "fn", &[2, 4]).unwrap();
        // Should keep a, c (originally 1st and 3rd).
        assert!(result.contains("fn a c ="), "def wrong: {result}");
        // Sig: originally A->B->C->D->E, remove 2nd(B) and 4th(D): A->C->E
        assert!(result.contains("fn : A -> C -> E"), "sig wrong: {result}");
    }

    // (j) Rename arg updates body references word-boundary correctly
    //     (no mangling of `oldValue` when renaming `old`).
    #[test]
    fn test_rename_arg_body_reference_word_boundary() {
        let source = "module Main exposing (..)\n\nupdate m model =\n    m + model.count\n";
        let result = rename_arg(source, "update", "m", "msg").unwrap();
        assert!(result.contains("update msg model ="), "def wrong: {result}");
        assert!(result.contains("msg + model.count"), "body wrong: {result}");
        // `model` must NOT be renamed (it's a different name).
        assert!(result.contains("model.count"), "model mangled: {result}");
    }

    // (k) Rename arg with collision errors.
    #[test]
    fn test_rename_arg_collision_errors() {
        let source = "module Main exposing (..)\n\nupdate msg model =\n    let\n        model = 42\n    in\n    msg\n";
        let err = rename_arg(source, "update", "msg", "model").unwrap_err();
        assert!(
            err.to_string().contains("'model' is already in scope"),
            "unexpected error: {err}"
        );
    }

    // Extra: split_arrow_chain handles parenthesized sub-types.
    #[test]
    fn test_split_arrow_chain_with_parens() {
        let parts = split_arrow_chain("(a -> b) -> c -> d");
        assert_eq!(parts, vec!["(a -> b)", "c", "d"]);
    }

    // Extra: split_arrow_chain single type.
    #[test]
    fn test_split_arrow_chain_single() {
        let parts = split_arrow_chain("Bool");
        assert_eq!(parts, vec!["Bool"]);
    }

    // Extra: add typed arg at position 1 (prepend) to typed fn.
    #[test]
    fn test_add_typed_arg_at_1_typed_fn() {
        let source =
            "module Main exposing (..)\n\nview : Model -> Html Msg\nview model =\n    model\n";
        let result = add_arg(source, "view", 1, "ctx", Some("Context")).unwrap();
        assert!(
            result.contains("view : Context -> Model -> Html Msg"),
            "sig wrong: {result}"
        );
        assert!(result.contains("view ctx model ="), "def wrong: {result}");
    }

    // Extra: add typed arg at end (append) to typed fn.
    #[test]
    fn test_add_typed_arg_append_typed_fn() {
        let source =
            "module Main exposing (..)\n\nview : Model -> Html Msg\nview model =\n    model\n";
        // N=1, at=2 appends.
        let result = add_arg(source, "view", 2, "extra", Some("Int")).unwrap();
        assert!(
            result.contains("view : Model -> Int -> Html Msg"),
            "sig wrong: {result}"
        );
        assert!(result.contains("view model extra ="), "def wrong: {result}");
    }
}
