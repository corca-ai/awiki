use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Severity {
    Off,
    Warn,
    Error,
}

#[derive(Debug, Serialize)]
pub(crate) struct Diagnostic {
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) rule: String,
    pub(crate) severity: Severity,
    pub(crate) message: String,
}
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = if self.severity == Severity::Error {
            "error"
        } else {
            "warn"
        };
        write!(
            f,
            "{}:{}:{}: {}[{}]: {}",
            self.path, self.line, self.column, level, self.rule, self.message
        )
    }
}
pub(crate) struct Diagnostics {
    pub(crate) items: Vec<Diagnostic>,
    pub(crate) documents: usize,
}
impl Diagnostics {
    pub(crate) fn has_errors(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.severity == Severity::Error)
    }
    pub(crate) fn render_text(&self) -> String {
        self.items
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub(crate) fn render_json(&self) -> Result<String, String> {
        serde_json::to_string(&serde_json::json!({
            "schema_version": 1, "documents": self.documents,
            "errors": self.items.iter().filter(|d| d.severity == Severity::Error).count(),
            "warnings": self.items.iter().filter(|d| d.severity == Severity::Warn).count(),
            "diagnostics": self.items,
        }))
        .map_err(|e| e.to_string())
    }
    pub(crate) fn sort(&mut self) {
        self.items.sort_by(|a, b| {
            (&a.path, a.line, a.column, &a.rule, &a.message)
                .cmp(&(&b.path, b.line, b.column, &b.rule, &b.message))
        });
    }
}
