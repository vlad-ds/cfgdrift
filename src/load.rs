use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Number, Value};
use walkdir::{DirEntry, WalkDir};

pub fn supported_formats() -> Vec<&'static str> {
    vec!["json", "yaml", "yml", "toml", "env", ".env", ".env.*"]
}

pub fn load_config_file(path: &Path) -> Result<Value> {
    let format = ConfigFormat::detect(path)
        .ok_or_else(|| anyhow!("unsupported config format for {}", path.display()))?;
    let contents = fs::read_to_string(path)
        .with_context(|| format!("could not read config file {}", path.display()))?;

    parse_config(&contents, format).with_context(|| format!("could not parse {}", path.display()))
}

pub fn load_config_dir(root: &Path) -> Result<BTreeMap<String, Value>> {
    let mut docs = BTreeMap::new();

    for entry in WalkDir::new(root).into_iter().filter_entry(should_visit) {
        let entry =
            entry.with_context(|| format!("could not walk directory {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        if ConfigFormat::detect(entry.path()).is_none() {
            continue;
        }

        let relative = entry
            .path()
            .strip_prefix(root)
            .with_context(|| format!("could not relativize {}", entry.path().display()))?;
        let key = relative.to_string_lossy().replace('\\', "/");
        let value = load_config_file(entry.path())?;
        docs.insert(key, value);
    }

    Ok(docs)
}

fn should_visit(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    !entry.file_type().is_dir() || !matches!(name.as_ref(), ".git" | "target" | "node_modules")
}

fn parse_config(contents: &str, format: ConfigFormat) -> Result<Value> {
    match format {
        ConfigFormat::Json => Ok(serde_json::from_str(contents)?),
        ConfigFormat::Yaml => yaml_to_json(serde_yaml::from_str(contents)?),
        ConfigFormat::Toml => Ok(serde_json::to_value(toml::from_str::<toml::Value>(
            contents,
        )?)?),
        ConfigFormat::Env => parse_env(contents),
    }
}

fn parse_env(contents: &str) -> Result<Value> {
    let mut map = Map::new();

    for item in dotenvy::from_read_iter(contents.as_bytes()) {
        let (key, value) = item?;
        map.insert(key, Value::String(value));
    }

    Ok(Value::Object(map))
}

fn yaml_to_json(value: serde_yaml::Value) -> Result<Value> {
    Ok(match value {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(value) => Value::Bool(value),
        serde_yaml::Value::Number(number) => yaml_number_to_json(number),
        serde_yaml::Value::String(value) => Value::String(value),
        serde_yaml::Value::Sequence(values) => Value::Array(
            values
                .into_iter()
                .map(yaml_to_json)
                .collect::<Result<Vec<_>>>()?,
        ),
        serde_yaml::Value::Mapping(mapping) => {
            let mut object = Map::new();
            for (key, value) in mapping {
                object.insert(yaml_key_to_string(key)?, yaml_to_json(value)?);
            }
            Value::Object(object)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(tagged.value)?,
    })
}

fn yaml_number_to_json(number: serde_yaml::Number) -> Value {
    if let Some(value) = number.as_i64() {
        Value::Number(value.into())
    } else if let Some(value) = number.as_u64() {
        Value::Number(value.into())
    } else if let Some(value) = number.as_f64() {
        Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(number.to_string()))
    } else {
        Value::String(number.to_string())
    }
}

fn yaml_key_to_string(key: serde_yaml::Value) -> Result<String> {
    match key {
        serde_yaml::Value::String(value) => Ok(value),
        serde_yaml::Value::Bool(value) => Ok(value.to_string()),
        serde_yaml::Value::Number(value) => Ok(value.to_string()),
        serde_yaml::Value::Null => Ok("null".to_string()),
        other => {
            let rendered = serde_yaml::to_string(&other)?;
            let key = rendered
                .lines()
                .filter(|line| *line != "...")
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();

            if key.is_empty() {
                bail!("yaml mapping key rendered to an empty string");
            }
            Ok(key)
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ConfigFormat {
    Json,
    Yaml,
    Toml,
    Env,
}

impl ConfigFormat {
    fn detect(path: &Path) -> Option<Self> {
        let file_name = path.file_name()?.to_str()?;
        if file_name == ".env" || file_name.starts_with(".env.") {
            return Some(Self::Env);
        }

        match path.extension().and_then(|extension| extension.to_str()) {
            Some("json") => Some(Self::Json),
            Some("yaml" | "yml") => Some(Self::Yaml),
            Some("toml") => Some(Self::Toml),
            Some("env") => Some(Self::Env),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_dotenv_content() {
        let parsed = parse_config(
            r#"
APP_ENV=production
QUOTED="hello world"
# ignored
"#,
            ConfigFormat::Env,
        )
        .unwrap();

        assert_eq!(
            parsed,
            json!({
                "APP_ENV": "production",
                "QUOTED": "hello world"
            })
        );
    }

    #[test]
    fn converts_yaml_mapping_to_json() {
        let parsed = parse_config("database:\n  pool: 8\n", ConfigFormat::Yaml).unwrap();
        assert_eq!(parsed, json!({"database": {"pool": 8}}));
    }
}
