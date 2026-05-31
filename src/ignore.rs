use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};

#[derive(Debug, Clone)]
pub struct IgnoreMatcher {
    patterns: Vec<String>,
    globset: GlobSet,
}

impl IgnoreMatcher {
    pub fn empty() -> Self {
        Self::from_patterns(&[]).expect("empty glob set must be valid")
    }

    pub fn from_sources(cli_patterns: &[String], ignore_file: Option<&Path>) -> Result<Self> {
        let mut patterns = cli_patterns.to_vec();

        if let Some(path) = ignore_file {
            let contents = fs::read_to_string(path)
                .with_context(|| format!("could not read ignore file {}", path.display()))?;
            patterns.extend(
                contents
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty() && !line.starts_with('#'))
                    .map(ToOwned::to_owned),
            );
        }

        Self::from_patterns(&patterns)
    }

    pub fn from_patterns(patterns: &[String]) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            builder.add(Glob::new(pattern).with_context(|| {
                format!("invalid ignore pattern {pattern:?}; cfgdrift uses glob syntax")
            })?);
        }

        Ok(Self {
            patterns: patterns.to_vec(),
            globset: builder.build()?,
        })
    }

    pub fn is_match(&self, file: &str, path: &str, location: &str) -> bool {
        if self.patterns.is_empty() {
            return false;
        }

        self.globset.is_match(file)
            || self.globset.is_match(path)
            || self.globset.is_match(location)
            || self.patterns.iter().any(|pattern| {
                raw_match(pattern, file) || raw_match(pattern, path) || raw_match(pattern, location)
            })
    }
}

fn raw_match(pattern: &str, value: &str) -> bool {
    if pattern == value {
        return true;
    }

    if value.ends_with(pattern) {
        let prefix_len = value.len().saturating_sub(pattern.len());
        return value[..prefix_len]
            .chars()
            .next_back()
            .is_some_and(|ch| matches!(ch, '.' | ':' | '/'));
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_field_patterns_match_suffixes() {
        let matcher = IgnoreMatcher::from_patterns(&["password".to_string()]).unwrap();
        assert!(matcher.is_match(
            "app.toml",
            "database.credentials.password",
            "app.toml:database.credentials.password"
        ));
    }

    #[test]
    fn glob_patterns_match_locations() {
        let matcher =
            IgnoreMatcher::from_patterns(&["config/*.toml:*password".to_string()]).unwrap();
        assert!(matcher.is_match(
            "config/app.toml",
            "database.password",
            "config/app.toml:database.password"
        ));
    }
}
