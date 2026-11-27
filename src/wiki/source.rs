//! File-local code declarations; no import, type, or macro expansion resolution.
use std::{fs, path::Path};

use std::collections::{HashMap, HashSet};
#[derive(Default)]
pub(super) struct SourceCache {
    files: HashMap<std::path::PathBuf, Result<HashSet<String>, String>>,
}
impl SourceCache {
    pub(super) fn check(&mut self, path: &Path, symbol: &str) -> Result<(), String> {
        let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let declarations = self.files.entry(key).or_insert_with(|| parse_source(path));
        check_symbol(declarations.as_ref().map_err(Clone::clone)?, symbol)
    }
}
pub(super) fn check_symbol(declarations: &HashSet<String>, symbol: &str) -> Result<(), String> {
    if declarations.contains(symbol) {
        Ok(())
    } else {
        Err(format!("declaration not found: {symbol}"))
    }
}
fn parse_source(path: &Path) -> Result<HashSet<String>, String> {
    let source = fs::read_to_string(path).map_err(|e| format!("cannot read source: {e}"))?;
    let language = match path.extension().and_then(|s| s.to_str()) {
        Some("ts") => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        Some("tsx") => tree_sitter_typescript::LANGUAGE_TSX,
        Some("rs") => tree_sitter_rust::LANGUAGE,
        _ => return Err("unsupported source language (supported: ts, tsx, rs)".into()),
    };
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language.into())
        .map_err(|e| e.to_string())?;
    let tree = parser.parse(&source, None).ok_or("source parsing failed")?;
    if tree.root_node().has_error() {
        return Err("source contains syntax errors".into());
    }
    let mut names = Vec::new();
    if path.extension().is_some_and(|ext| ext == "rs") {
        collect_rust(tree.root_node(), source.as_bytes(), "", &mut names);
    } else {
        collect_declarations(tree.root_node(), source.as_bytes(), "", &mut names);
    }
    Ok(names.into_iter().collect())
}

fn collect_declarations(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    prefix: &str,
    out: &mut Vec<String>,
) {
    let kind = node.kind();
    let declaration = matches!(
        kind,
        "function_declaration"
            | "function_signature"
            | "generator_function_declaration"
            | "class_declaration"
            | "abstract_class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration"
            | "internal_module"
            | "variable_declarator"
            | "method_definition"
            | "method_signature"
            | "public_field_definition"
            | "property_signature"
            | "enum_assignment"
    );
    let mut scope = prefix.to_string();
    if declaration {
        if let Some(name) = node.child_by_field_name("name") {
            if !matches!(
                name.kind(),
                "identifier" | "type_identifier" | "property_identifier"
            ) {
                return;
            }
            let name = name.utf8_text(source).unwrap_or("");
            scope = if prefix.is_empty() {
                name.into()
            } else {
                format!("{prefix}.{name}")
            };
            out.push(scope.clone());
        }
    }
    // Do not expose locals, inherited members, or inferred object members.
    if matches!(
        kind,
        "function_declaration"
            | "generator_function_declaration"
            | "method_definition"
            | "variable_declarator"
            | "arrow_function"
            | "function_expression"
    ) {
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_declarations(child, source, &scope, out);
    }
}

// Only descend into declaration containers, never function bodies or macro tokens.
fn collect_rust(node: tree_sitter::Node<'_>, source: &[u8], prefix: &str, out: &mut Vec<String>) {
    if node.kind() == "impl_item" {
        let Some(ty) = node.child_by_field_name("type") else {
            return;
        };
        let ty = if ty.kind() == "generic_type" {
            let Some(ty) = ty.child_by_field_name("type") else {
                return;
            };
            ty
        } else {
            ty
        };
        if !matches!(ty.kind(), "type_identifier" | "scoped_type_identifier") {
            return;
        }
        let scope = qualify(prefix, ty.utf8_text(source).unwrap_or(""));
        if let Some(body) = node.child_by_field_name("body") {
            collect_rust(body, source, &scope, out);
        }
        return;
    }
    let named = matches!(
        node.kind(),
        "function_item"
            | "function_signature_item"
            | "struct_item"
            | "enum_item"
            | "enum_variant"
            | "trait_item"
            | "type_item"
            | "associated_type"
            | "const_item"
            | "static_item"
            | "mod_item"
            | "union_item"
            | "field_declaration"
            | "macro_definition"
    );
    if named {
        let Some(name) = node.child_by_field_name("name") else {
            return;
        };
        let scope = qualify(prefix, name.utf8_text(source).unwrap_or(""));
        out.push(scope.clone());
        if matches!(
            node.kind(),
            "struct_item" | "enum_item" | "enum_variant" | "trait_item" | "mod_item" | "union_item"
        ) {
            if let Some(body) = node.child_by_field_name("body") {
                collect_rust(body, source, &scope, out);
            }
        }
        return;
    }
    if matches!(
        node.kind(),
        "source_file"
            | "declaration_list"
            | "field_declaration_list"
            | "enum_variant_list"
            | "foreign_mod_item"
    ) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect_rust(child, source, prefix, out);
        }
    }
}

fn qualify(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.into()
    } else {
        format!("{prefix}::{name}")
    }
}
