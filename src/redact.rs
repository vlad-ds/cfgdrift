use serde_json::Value;

const SENSITIVE_TERMS: &[&str] = &[
    "apikey",
    "api_key",
    "auth",
    "bearer",
    "certificate",
    "clientsecret",
    "client_secret",
    "cookie",
    "credential",
    "jwt",
    "passwd",
    "password",
    "privatekey",
    "private_key",
    "secret",
    "session",
    "sshkey",
    "ssh_key",
    "token",
];

pub fn maybe_redact(path: &str, value: &Value, redact: bool) -> Value {
    if redact && is_sensitive_path(path) {
        Value::String("<redacted>".to_string())
    } else {
        value.clone()
    }
}

fn is_sensitive_path(path: &str) -> bool {
    let compact = path
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>()
        .to_ascii_lowercase();

    let tokens = path
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();

    SENSITIVE_TERMS.iter().any(|term| {
        compact.contains(term)
            || tokens
                .iter()
                .any(|token| token == term || token.ends_with(term))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_password_paths() {
        assert_eq!(
            maybe_redact("database.password", &json!("hunter2"), true),
            json!("<redacted>")
        );
    }

    #[test]
    fn leaves_non_sensitive_paths() {
        assert_eq!(maybe_redact("database.pool", &json!(8), true), json!(8));
    }
}
