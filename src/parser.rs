use crate::{Declaration, DeclarationKind, FileSummary};
use anyhow::{Context, Result, bail};
use std::path::Path;
use tree_sitter::{Node, Parser, Tree};

pub fn parse(source: &str) -> Result<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_elm::LANGUAGE.into())
        .context("failed to load Elm grammar")?;
    parser
        .parse(source, None)
        .context("failed to parse Elm source")
}

/// Locate the first ERROR or MISSING node in a tree and return its
/// 1-indexed `(line, col)` position. Returns `None` if the tree is clean.
pub fn first_error_location(tree: &Tree, source: &str) -> Option<(usize, usize)> {
    fn walk<'a>(node: Node<'a>) -> Option<Node<'a>> {
        if node.is_error() || node.is_missing() {
            return Some(node);
        }
        if !node.has_error() {
            return None;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = walk(child) {
                return Some(found);
            }
        }
        None
    }

    let node = walk(tree.root_node())?;
    let start = node.start_byte();
    Some(byte_offset_to_line_col(source, start))
}

fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let clamped = offset.min(source.len());
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= clamped {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Parse `source` and fail if tree-sitter produced any ERROR or MISSING
/// nodes. Intended for write-path preconditions — commands that mutate
/// Elm sources MUST call this (or an equivalent helper) before operating
/// on the file, so we never splice edits into a damaged CST.
pub fn ensure_clean_parse(source: &str, file: &Path) -> Result<Tree> {
    let tree = parse(source)?;
    if tree.root_node().has_error() {
        let where_ = match first_error_location(&tree, source) {
            Some((line, col)) => format!(" at {line}:{col}"),
            None => String::new(),
        };
        bail!(
            "refusing to edit {}: file has pre-existing parse errors{where_}",
            file.display()
        );
    }
    Ok(tree)
}

pub fn extract_summary(tree: &Tree, source: &str) -> FileSummary {
    let root = tree.root_node();
    let module_line = extract_module_line(&root, source);
    let imports = extract_imports(&root, source);
    let declarations = extract_declarations(&root, source);

    FileSummary {
        module_line,
        imports,
        declarations,
    }
}

fn extract_module_line(root: &Node, source: &str) -> String {
    root.child_by_field_name("moduleDeclaration")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .unwrap_or("")
        .to_string()
}

fn extract_imports(root: &Node, source: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() == "import_clause"
            && let Ok(text) = child.utf8_text(source.as_bytes())
        {
            let text = text.strip_prefix("import ").unwrap_or(text);
            imports.push(text.to_string());
        }
    }
    imports
}

fn extract_declarations(root: &Node, source: &str) -> Vec<Declaration> {
    let children: Vec<Node> = root.named_children(&mut root.walk()).collect();

    let mut declarations = Vec::new();
    let mut i = 0;

    while i < children.len() {
        let node = children[i];
        match node.kind() {
            "type_annotation" => {
                let name = node_field_text(&node, "name", source).unwrap_or_default();
                let type_expr = node_field_text(&node, "typeExpression", source);

                if i + 1 < children.len() && children[i + 1].kind() == "value_declaration" {
                    let val_node = children[i + 1];
                    let doc = find_doc_comment(&children, i, source);
                    let start_node = doc.as_ref().map(|(n, _)| *n).unwrap_or(node);
                    declarations.push(Declaration {
                        name,
                        kind: DeclarationKind::Function,
                        type_annotation: type_expr,
                        doc_comment: doc.map(|(_, s)| s),
                        start_line: start_node.start_position().row + 1,
                        end_line: val_node.end_position().row + 1,
                    });
                    i += 2;
                    continue;
                }

                let doc = find_doc_comment(&children, i, source);
                let start_node = doc.as_ref().map(|(n, _)| *n).unwrap_or(node);
                declarations.push(Declaration {
                    name,
                    kind: DeclarationKind::Function,
                    type_annotation: type_expr,
                    doc_comment: doc.map(|(_, s)| s),
                    start_line: start_node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                });
            }
            "value_declaration" => {
                let name = extract_value_decl_name(&node, source).unwrap_or_default();
                let doc = find_doc_comment(&children, i, source);
                let start_node = doc.as_ref().map(|(n, _)| *n).unwrap_or(node);
                declarations.push(Declaration {
                    name,
                    kind: DeclarationKind::Function,
                    type_annotation: None,
                    doc_comment: doc.map(|(_, s)| s),
                    start_line: start_node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                });
            }
            "type_declaration" => {
                let name = node_field_text(&node, "name", source).unwrap_or_default();
                let doc = find_doc_comment(&children, i, source);
                let start_node = doc.as_ref().map(|(n, _)| *n).unwrap_or(node);
                declarations.push(Declaration {
                    name,
                    kind: DeclarationKind::Type,
                    type_annotation: None,
                    doc_comment: doc.map(|(_, s)| s),
                    start_line: start_node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                });
            }
            "type_alias_declaration" => {
                let name = node_field_text(&node, "name", source).unwrap_or_default();
                let doc = find_doc_comment(&children, i, source);
                let start_node = doc.as_ref().map(|(n, _)| *n).unwrap_or(node);
                declarations.push(Declaration {
                    name,
                    kind: DeclarationKind::TypeAlias,
                    type_annotation: None,
                    doc_comment: doc.map(|(_, s)| s),
                    start_line: start_node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                });
            }
            "port_annotation" => {
                let name = node_field_text(&node, "name", source).unwrap_or_default();
                let type_expr = node_field_text(&node, "typeExpression", source);
                let doc = find_doc_comment(&children, i, source);
                let start_node = doc.as_ref().map(|(n, _)| *n).unwrap_or(node);
                declarations.push(Declaration {
                    name,
                    kind: DeclarationKind::Port,
                    type_annotation: type_expr,
                    doc_comment: doc.map(|(_, s)| s),
                    start_line: start_node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                });
            }
            _ => {}
        }
        i += 1;
    }

    declarations
}

fn node_field_text(node: &Node, field: &str, source: &str) -> Option<String> {
    let child = node.child_by_field_name(field)?;
    Some(child.utf8_text(source.as_bytes()).ok()?.to_string())
}

fn extract_value_decl_name(node: &Node, source: &str) -> Option<String> {
    let fdl = node.child_by_field_name("functionDeclarationLeft")?;
    let mut cursor = fdl.walk();
    for child in fdl.named_children(&mut cursor) {
        if child.kind() == "lower_case_identifier" {
            return Some(child.utf8_text(source.as_bytes()).ok()?.to_string());
        }
    }
    None
}

/// Extract the declaration name from a source string by parsing it.
/// Used by `set` to determine the name of the declaration being upserted.
pub fn extract_declaration_name(source: &str) -> Option<String> {
    let tree = parse(source).ok()?;
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "type_annotation" => {
                return node_field_text(&child, "name", source);
            }
            "value_declaration" => {
                return extract_value_decl_name(&child, source);
            }
            "type_declaration" | "type_alias_declaration" => {
                return node_field_text(&child, "name", source);
            }
            "port_annotation" => {
                return node_field_text(&child, "name", source);
            }
            _ => {}
        }
    }
    None
}

fn find_doc_comment<'a>(
    children: &[Node<'a>],
    decl_index: usize,
    source: &str,
) -> Option<(Node<'a>, String)> {
    if decl_index == 0 {
        return None;
    }
    let prev = children[decl_index - 1];
    if prev.kind() == "block_comment" {
        let text = prev.utf8_text(source.as_bytes()).ok()?;
        if text.starts_with("{-|") {
            return Some((prev, text.to_string()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ELM: &str = r#"module Main exposing (Model, Msg(..), update, view)

import Html exposing (Html, div, text)
import Html.Attributes as Attr


{-| The model for our app -}
type alias Model =
    { count : Int
    , name : String
    }


{-| Messages for the update function -}
type Msg
    = Increment
    | Decrement
    | Reset


update : Msg -> Model -> Model
update msg model =
    case msg of
        Increment ->
            { model | count = model.count + 1 }

        Decrement ->
            { model | count = model.count - 1 }

        Reset ->
            { model | count = 0 }


view : Model -> Html Msg
view model =
    div []
        [ text (String.fromInt model.count) ]


helper x =
    x + 1
"#;

    #[test]
    fn test_extract_summary() {
        let tree = parse(SAMPLE_ELM).unwrap();
        let summary = extract_summary(&tree, SAMPLE_ELM);

        assert_eq!(
            summary.module_line,
            "module Main exposing (Model, Msg(..), update, view)"
        );

        assert_eq!(summary.imports.len(), 2);
        assert_eq!(summary.imports[0], "Html exposing (Html, div, text)");
        assert_eq!(summary.imports[1], "Html.Attributes as Attr");

        assert_eq!(summary.declarations.len(), 5);
    }

    #[test]
    fn test_declarations() {
        let tree = parse(SAMPLE_ELM).unwrap();
        let summary = extract_summary(&tree, SAMPLE_ELM);
        let decls = &summary.declarations;

        // Model type alias
        assert_eq!(decls[0].name, "Model");
        assert_eq!(decls[0].kind, DeclarationKind::TypeAlias);
        assert!(decls[0].doc_comment.is_some());

        // Msg type
        assert_eq!(decls[1].name, "Msg");
        assert_eq!(decls[1].kind, DeclarationKind::Type);
        assert!(decls[1].doc_comment.is_some());

        // update function
        assert_eq!(decls[2].name, "update");
        assert_eq!(decls[2].kind, DeclarationKind::Function);
        assert_eq!(
            decls[2].type_annotation.as_deref(),
            Some("Msg -> Model -> Model")
        );

        // view function
        assert_eq!(decls[3].name, "view");
        assert_eq!(decls[3].kind, DeclarationKind::Function);

        // helper function (no type annotation)
        assert_eq!(decls[4].name, "helper");
        assert_eq!(decls[4].kind, DeclarationKind::Function);
        assert!(decls[4].type_annotation.is_none());
    }

    #[test]
    fn test_empty_module() {
        let source = "module Main exposing (..)\n";
        let tree = parse(source).unwrap();
        let summary = extract_summary(&tree, source);
        assert_eq!(summary.module_line, "module Main exposing (..)");
        assert!(summary.imports.is_empty());
        assert!(summary.declarations.is_empty());
    }

    #[test]
    fn test_port_declaration() {
        let source = r#"port module Main exposing (..)

port sendMessage : String -> Cmd msg
"#;
        let tree = parse(source).unwrap();
        let summary = extract_summary(&tree, source);
        assert_eq!(summary.module_line, "port module Main exposing (..)");
        assert_eq!(summary.declarations.len(), 1);
        assert_eq!(summary.declarations[0].name, "sendMessage");
        assert_eq!(summary.declarations[0].kind, DeclarationKind::Port);
        assert_eq!(
            summary.declarations[0].type_annotation.as_deref(),
            Some("String -> Cmd msg")
        );
    }

    #[test]
    fn test_find_declaration_found() {
        let tree = parse(SAMPLE_ELM).unwrap();
        let summary = extract_summary(&tree, SAMPLE_ELM);

        let decl = summary.find_declaration("update");
        assert!(decl.is_some());
        let decl = decl.unwrap();
        assert_eq!(decl.name, "update");
        assert_eq!(decl.kind, DeclarationKind::Function);
    }

    #[test]
    fn test_find_declaration_not_found() {
        let tree = parse(SAMPLE_ELM).unwrap();
        let summary = extract_summary(&tree, SAMPLE_ELM);

        assert!(summary.find_declaration("nonExistent").is_none());
    }

    #[test]
    fn test_extract_declaration_name_function() {
        assert_eq!(
            extract_declaration_name(
                "update : Msg -> Model -> Model\nupdate msg model =\n    model"
            ),
            Some("update".to_string())
        );
    }

    #[test]
    fn test_extract_declaration_name_type() {
        assert_eq!(
            extract_declaration_name("type Msg\n    = Increment\n    | Decrement"),
            Some("Msg".to_string())
        );
    }

    #[test]
    fn test_extract_declaration_name_type_alias() {
        assert_eq!(
            extract_declaration_name("type alias Model =\n    { count : Int }"),
            Some("Model".to_string())
        );
    }

    #[test]
    fn test_extract_declaration_name_port() {
        assert_eq!(
            extract_declaration_name("port sendMessage : String -> Cmd msg"),
            Some("sendMessage".to_string())
        );
    }

    #[test]
    fn test_extract_declaration_name_value_no_annotation() {
        assert_eq!(
            extract_declaration_name("helper x =\n    x + 1"),
            Some("helper".to_string())
        );
    }

    #[test]
    fn test_extract_declaration_name_unparseable() {
        assert_eq!(extract_declaration_name("not valid elm at all {{{"), None);
    }

    #[test]
    fn test_first_error_location_clean_tree() {
        let tree = parse(SAMPLE_ELM).unwrap();
        assert_eq!(first_error_location(&tree, SAMPLE_ELM), None);
    }

    #[test]
    fn test_first_error_location_unclosed_let() {
        let source = "module Main exposing (..)\n\nfoo =\n    let\n        x = 1\n";
        let tree = parse(source).unwrap();
        let loc = first_error_location(&tree, source);
        assert!(loc.is_some(), "expected an error location for unclosed let");
        let (line, _col) = loc.unwrap();
        assert!(
            (3..=6).contains(&line),
            "error line {line} should fall inside the let block"
        );
    }

    #[test]
    fn test_first_error_location_malformed_annotation() {
        let source = "module Main exposing (..)\n\nfoo : Int ->\nfoo = 1\n";
        let tree = parse(source).unwrap();
        let loc = first_error_location(&tree, source);
        assert!(
            loc.is_some(),
            "expected an error location for dangling type arrow"
        );
    }

    #[test]
    fn test_first_error_location_picks_first_by_offset() {
        let source = "module Main exposing (..)\n\nfoo =\n    let\n\nbar =\n    case\n";
        let tree = parse(source).unwrap();
        let loc = first_error_location(&tree, source);
        assert!(loc.is_some());
        let (line, _) = loc.unwrap();
        assert!(
            line <= 5,
            "first error should be reported before the second broken construct, got line {line}"
        );
    }

    #[test]
    fn test_ensure_clean_parse_accepts_valid_source() {
        let path = std::path::Path::new("Sample.elm");
        assert!(ensure_clean_parse(SAMPLE_ELM, path).is_ok());
    }

    #[test]
    fn test_ensure_clean_parse_rejects_broken_source() {
        let path = std::path::Path::new("Broken.elm");
        let source = "module Main exposing (..)\n\nfoo =\n    let\n        x = 1\n";
        let err = ensure_clean_parse(source, path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Broken.elm"), "message: {msg}");
        assert!(msg.contains("pre-existing parse errors"), "message: {msg}");
    }
}
