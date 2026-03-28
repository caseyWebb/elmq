pub mod imports;
pub mod move_decl;
pub mod parser;
pub mod project;
pub mod refs;
pub mod writer;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileSummary {
    pub module_line: String,
    pub imports: Vec<String>,
    pub declarations: Vec<Declaration>,
}

impl FileSummary {
    pub fn find_declaration(&self, name: &str) -> Option<&Declaration> {
        self.declarations.iter().find(|d| d.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Declaration {
    pub name: String,
    pub kind: DeclarationKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_comment: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclarationKind {
    Function,
    Type,
    TypeAlias,
    Port,
}

impl std::fmt::Display for DeclarationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeclarationKind::Function => write!(f, "function"),
            DeclarationKind::Type => write!(f, "type"),
            DeclarationKind::TypeAlias => write!(f, "type_alias"),
            DeclarationKind::Port => write!(f, "port"),
        }
    }
}
