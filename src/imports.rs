use std::collections::HashMap;
use tree_sitter::Node;

/// How a single item is exposed from an import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExposedItem {
    /// A value or function name, e.g. `div`
    Value(String),
    /// A type name without constructors, e.g. `Html`
    Type(String),
    /// A type name with all constructors exposed, e.g. `Msg(..)`
    TypeWithConstructors(String),
}

impl ExposedItem {
    pub fn name(&self) -> &str {
        match self {
            ExposedItem::Value(n) | ExposedItem::Type(n) | ExposedItem::TypeWithConstructors(n) => {
                n
            }
        }
    }
}

/// How a single module is imported in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleImport {
    /// The canonical module name, e.g. "Html.Attributes"
    pub module_name: String,
    /// Alias if present, e.g. Some("Attr")
    pub alias: Option<String>,
    /// Explicitly exposed items
    pub exposed: Vec<ExposedItem>,
    /// True if `exposing (..)` is used
    pub has_exposing_all: bool,
    /// The full import line text (for rendering back)
    pub raw_line: String,
}

/// Tracks how a file imports and references other modules.
#[derive(Debug, Clone)]
pub struct ImportContext {
    /// All imports keyed by canonical module name.
    imports: HashMap<String, ModuleImport>,
}

/// Elm modules that are auto-imported (always in scope without explicit import).
const AUTO_IMPORTED_MODULES: &[&str] = &[
    "Basics",
    "List",
    "Maybe",
    "Result",
    "String",
    "Char",
    "Tuple",
    "Debug",
    "Platform",
    "Platform.Cmd",
    "Platform.Sub",
];

impl ImportContext {
    /// Build an `ImportContext` from a tree-sitter parse root.
    pub fn from_tree(root: &Node, source: &str) -> Self {
        let mut imports = HashMap::new();
        let mut cursor = root.walk();

        for child in root.named_children(&mut cursor) {
            if child.kind() != "import_clause" {
                continue;
            }

            let Some(module_name_node) = child.child_by_field_name("moduleName") else {
                continue;
            };
            let Ok(module_name) = module_name_node.utf8_text(source.as_bytes()) else {
                continue;
            };

            let raw_line = child.utf8_text(source.as_bytes()).unwrap_or("").to_string();

            let alias = child
                .child_by_field_name("asClause")
                .and_then(|as_clause| as_clause.child_by_field_name("name"))
                .and_then(|name_node| name_node.utf8_text(source.as_bytes()).ok())
                .map(|s| s.to_string());

            let (exposed, has_exposing_all) = child
                .child_by_field_name("exposing")
                .map(|exposing_list| extract_exposed_items(&exposing_list, source))
                .unwrap_or_default();

            imports.insert(
                module_name.to_string(),
                ModuleImport {
                    module_name: module_name.to_string(),
                    alias,
                    exposed,
                    has_exposing_all,
                    raw_line,
                },
            );
        }

        ImportContext { imports }
    }

    /// Create an empty context (for new files with no imports yet).
    pub fn empty() -> Self {
        ImportContext {
            imports: HashMap::new(),
        }
    }

    /// Get the import entry for a specific module.
    pub fn get(&self, module: &str) -> Option<&ModuleImport> {
        self.imports.get(module)
    }

    /// Iterate over all imports.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ModuleImport)> {
        self.imports.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Resolve a module prefix (alias or full name) to the canonical module name.
    /// Also recognizes auto-imported modules.
    pub fn resolve_prefix(&self, prefix: &str) -> Option<&str> {
        // Check explicit imports first — alias match.
        for (canonical, imp) in &self.imports {
            if let Some(ref alias) = imp.alias
                && alias == prefix
            {
                return Some(canonical);
            }
            if imp.module_name == prefix {
                return Some(canonical);
            }
        }

        // Check auto-imported modules.
        if let Some(module) = AUTO_IMPORTED_MODULES.iter().find(|&&m| m == prefix) {
            return Some(module);
        }

        None
    }

    /// Resolve a bare (unqualified) name to the module that exposes it.
    /// Returns `None` if the name isn't explicitly exposed by any import.
    pub fn resolve_bare(&self, name: &str) -> Option<&str> {
        for (canonical, imp) in &self.imports {
            if imp.has_exposing_all {
                // We can't resolve bare names from `exposing (..)` — skip.
                continue;
            }
            for item in &imp.exposed {
                match item {
                    ExposedItem::Value(n) | ExposedItem::Type(n) if n == name => {
                        return Some(canonical);
                    }
                    ExposedItem::TypeWithConstructors(_n) if _n == name => {
                        // The type itself is exposed.
                        return Some(canonical);
                    }
                    ExposedItem::TypeWithConstructors(_) => {
                        // Constructors of this type are also bare-accessible,
                        // but we can't enumerate them from the import alone.
                        // The caller handles constructor resolution separately.
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Produce the correct syntax for referencing `module.name` in this file's import context.
    /// - If the name is explicitly exposed → bare name
    /// - If the module has an alias → "Alias.name"
    /// - Otherwise → "Module.name"
    pub fn emit_ref(&self, module: &str, name: &str) -> String {
        if let Some(imp) = self.imports.get(module) {
            // Check if the name is explicitly exposed (bare reference).
            for item in &imp.exposed {
                if item.name() == name {
                    return name.to_string();
                }
            }
            // Use alias if available.
            if let Some(ref alias) = imp.alias {
                return format!("{alias}.{name}");
            }
            // Fall back to full module name.
            return format!("{}.{name}", imp.module_name);
        }

        // Module not imported — emit fully qualified.
        format!("{module}.{name}")
    }

    /// Ensure an import exists for the given module. If the module is already imported,
    /// keep the existing style. If not, add it using the provided style hint.
    /// No-op for auto-imported modules.
    pub fn ensure_import(&mut self, module: &str, style_hint: &ModuleImport) {
        // Skip auto-imported modules.
        if AUTO_IMPORTED_MODULES.contains(&module) {
            return;
        }
        // If already imported, keep existing.
        if self.imports.contains_key(module) {
            return;
        }
        self.imports.insert(module.to_string(), style_hint.clone());
    }

    /// Render the import block as sorted text lines.
    pub fn render_imports(&self) -> String {
        let mut lines: Vec<&str> = self
            .imports
            .values()
            .map(|imp| imp.raw_line.as_str())
            .collect();
        lines.sort();
        lines.join("\n")
    }
}

/// Extract exposed items from an exposing list node.
fn extract_exposed_items(exposing_list: &Node, source: &str) -> (Vec<ExposedItem>, bool) {
    let mut items = Vec::new();
    let mut has_all = false;
    let mut cursor = exposing_list.walk();

    for child in exposing_list.named_children(&mut cursor) {
        match child.kind() {
            "double_dot" => {
                has_all = true;
            }
            "exposed_value" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    items.push(ExposedItem::Value(text.to_string()));
                }
            }
            "exposed_type" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    let name = text.split('(').next().unwrap_or(text).trim();
                    if name != ".." {
                        if text.contains("(..)") {
                            items.push(ExposedItem::TypeWithConstructors(name.to_string()));
                        } else {
                            items.push(ExposedItem::Type(name.to_string()));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    (items, has_all)
}

/// Check if a module name is auto-imported in Elm.
pub fn is_auto_imported(module: &str) -> bool {
    AUTO_IMPORTED_MODULES.contains(&module)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn ctx_from_source(source: &str) -> ImportContext {
        let tree = parser::parse(source).unwrap();
        ImportContext::from_tree(&tree.root_node(), source)
    }

    #[test]
    fn test_from_tree_basic() {
        let ctx = ctx_from_source(
            "module Main exposing (..)\n\nimport Html exposing (Html, div, text)\nimport Html.Attributes as Attr\n",
        );
        assert_eq!(ctx.imports.len(), 2);
        let html = ctx.get("Html").unwrap();
        assert_eq!(html.alias, None);
        assert_eq!(html.exposed.len(), 3);
        assert!(!html.has_exposing_all);

        let attr = ctx.get("Html.Attributes").unwrap();
        assert_eq!(attr.alias.as_deref(), Some("Attr"));
        assert!(attr.exposed.is_empty());
    }

    #[test]
    fn test_from_tree_exposing_all() {
        let ctx = ctx_from_source("module Main exposing (..)\n\nimport Html exposing (..)\n");
        let html = ctx.get("Html").unwrap();
        assert!(html.has_exposing_all);
    }

    #[test]
    fn test_from_tree_type_with_constructors() {
        let ctx =
            ctx_from_source("module Main exposing (..)\n\nimport Maybe exposing (Maybe(..))\n");
        let maybe = ctx.get("Maybe").unwrap();
        assert_eq!(maybe.exposed.len(), 1);
        assert_eq!(
            maybe.exposed[0],
            ExposedItem::TypeWithConstructors("Maybe".to_string())
        );
    }

    #[test]
    fn test_resolve_prefix_full_name() {
        let ctx = ctx_from_source("module Main exposing (..)\n\nimport Html.Attributes\n");
        assert_eq!(
            ctx.resolve_prefix("Html.Attributes"),
            Some("Html.Attributes")
        );
    }

    #[test]
    fn test_resolve_prefix_alias() {
        let ctx = ctx_from_source("module Main exposing (..)\n\nimport Html.Attributes as Attr\n");
        assert_eq!(ctx.resolve_prefix("Attr"), Some("Html.Attributes"));
    }

    #[test]
    fn test_resolve_prefix_auto_import() {
        let ctx = ctx_from_source("module Main exposing (..)\n");
        assert_eq!(ctx.resolve_prefix("String"), Some("String"));
        assert_eq!(ctx.resolve_prefix("Maybe"), Some("Maybe"));
        assert_eq!(ctx.resolve_prefix("Platform.Cmd"), Some("Platform.Cmd"));
    }

    #[test]
    fn test_resolve_prefix_unknown() {
        let ctx = ctx_from_source("module Main exposing (..)\n");
        assert_eq!(ctx.resolve_prefix("Http"), None);
    }

    #[test]
    fn test_resolve_bare_exposed_value() {
        let ctx =
            ctx_from_source("module Main exposing (..)\n\nimport Html exposing (div, text)\n");
        assert_eq!(ctx.resolve_bare("div"), Some("Html"));
        assert_eq!(ctx.resolve_bare("text"), Some("Html"));
    }

    #[test]
    fn test_resolve_bare_exposed_type() {
        let ctx = ctx_from_source("module Main exposing (..)\n\nimport Html exposing (Html)\n");
        assert_eq!(ctx.resolve_bare("Html"), Some("Html"));
    }

    #[test]
    fn test_resolve_bare_unknown() {
        let ctx = ctx_from_source("module Main exposing (..)\n\nimport Html exposing (div)\n");
        assert_eq!(ctx.resolve_bare("span"), None);
    }

    #[test]
    fn test_resolve_bare_exposing_all_skipped() {
        let ctx = ctx_from_source("module Main exposing (..)\n\nimport Html exposing (..)\n");
        // exposing (..) doesn't resolve bare names
        assert_eq!(ctx.resolve_bare("div"), None);
    }

    #[test]
    fn test_emit_ref_exposed() {
        let ctx =
            ctx_from_source("module Main exposing (..)\n\nimport Html exposing (div, text)\n");
        assert_eq!(ctx.emit_ref("Html", "div"), "div");
    }

    #[test]
    fn test_emit_ref_aliased() {
        let ctx = ctx_from_source("module Main exposing (..)\n\nimport Html.Attributes as Attr\n");
        assert_eq!(ctx.emit_ref("Html.Attributes", "class"), "Attr.class");
    }

    #[test]
    fn test_emit_ref_qualified() {
        let ctx = ctx_from_source("module Main exposing (..)\n\nimport Http\n");
        assert_eq!(ctx.emit_ref("Http", "get"), "Http.get");
    }

    #[test]
    fn test_emit_ref_not_imported() {
        let ctx = ctx_from_source("module Main exposing (..)\n");
        assert_eq!(ctx.emit_ref("Http", "get"), "Http.get");
    }

    #[test]
    fn test_ensure_import_new() {
        let mut ctx = ctx_from_source("module Main exposing (..)\n");
        let hint = ModuleImport {
            module_name: "Http".to_string(),
            alias: None,
            exposed: vec![],
            has_exposing_all: false,
            raw_line: "import Http".to_string(),
        };
        ctx.ensure_import("Http", &hint);
        assert!(ctx.get("Http").is_some());
    }

    #[test]
    fn test_ensure_import_existing_preserved() {
        let mut ctx =
            ctx_from_source("module Main exposing (..)\n\nimport Json.Decode as Decode\n");
        let hint = ModuleImport {
            module_name: "Json.Decode".to_string(),
            alias: Some("D".to_string()),
            exposed: vec![],
            has_exposing_all: false,
            raw_line: "import Json.Decode as D".to_string(),
        };
        ctx.ensure_import("Json.Decode", &hint);
        // Should keep existing "as Decode", not use hint's "as D"
        assert_eq!(
            ctx.get("Json.Decode").unwrap().alias.as_deref(),
            Some("Decode")
        );
    }

    #[test]
    fn test_ensure_import_auto_imported_noop() {
        let mut ctx = ctx_from_source("module Main exposing (..)\n");
        let hint = ModuleImport {
            module_name: "String".to_string(),
            alias: None,
            exposed: vec![],
            has_exposing_all: false,
            raw_line: "import String".to_string(),
        };
        ctx.ensure_import("String", &hint);
        // Should not be added (auto-imported)
        assert!(ctx.get("String").is_none());
    }

    #[test]
    fn test_render_imports_sorted() {
        let ctx = ctx_from_source(
            "module Main exposing (..)\n\nimport Html\nimport Array\nimport Json.Decode as D\n",
        );
        let rendered = ctx.render_imports();
        assert_eq!(
            rendered,
            "import Array\nimport Html\nimport Json.Decode as D"
        );
    }
}
