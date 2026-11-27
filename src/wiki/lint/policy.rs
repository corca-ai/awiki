use super::diagnostic::Severity;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::collections::BTreeMap;

const RULES: &[&str] = &[
    "orphan",
    "island",
    "link-only-line",
    "broken-document-link",
    "first-block-paragraph",
    "broken-code-reference",
    "broken-reference-link",
    "broken-local-link",
];
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    #[serde(default)]
    pub(super) lint: Policy,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    #[serde(default)]
    rules: BTreeMap<String, Severity>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    overrides: Vec<Override>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Override {
    files: Vec<String>,
    rules: BTreeMap<String, Severity>,
}
pub(super) struct CompiledPolicy {
    rules: BTreeMap<String, Severity>,
    exclude: GlobSet,
    overrides: Vec<(GlobSet, BTreeMap<String, Severity>)>,
}
impl Policy {
    pub(super) fn validate(&self) -> Result<CompiledPolicy, String> {
        for rule in self
            .rules
            .keys()
            .chain(self.overrides.iter().flat_map(|o| o.rules.keys()))
        {
            if !RULES.contains(&rule.as_str()) {
                return Err(format!("unknown lint rule: {rule}"));
            }
        }
        Ok(CompiledPolicy {
            rules: self.rules.clone(),
            exclude: globs(&self.exclude)?,
            overrides: self
                .overrides
                .iter()
                .map(|o| Ok((globs(&o.files)?, o.rules.clone())))
                .collect::<Result<_, String>>()?,
        })
    }
}
impl CompiledPolicy {
    pub(super) fn severity(&self, path: &str, rule: &str) -> Severity {
        if self.exclude.is_match(path) {
            return Severity::Off;
        }
        let mut value = self.rules.get(rule).copied().unwrap_or_else(|| {
            if ["orphan", "island", "link-only-line"].contains(&rule) {
                Severity::Error
            } else {
                Severity::Off
            }
        });
        for (files, rules) in &self.overrides {
            if files.is_match(path) {
                if let Some(next) = rules.get(rule) {
                    value = *next;
                }
            }
        }
        value
    }
}
fn globs(patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).map_err(|e| e.to_string())?);
    }
    builder.build().map_err(|e| e.to_string())
}
