use crate::{Declaration, DeclarationKind, FileSummary};
use anyhow::{Context, Result};
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
}
