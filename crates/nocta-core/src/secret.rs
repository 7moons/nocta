use zeroize::Zeroizing;

/// Backend-only secret value. It has no `Serialize`/`Display` implementation, and `Debug` is redacted,
/// so it cannot reach IPC snapshots, logs or error messages by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(Zeroizing<String>);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretString([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn debug_output_is_redacted() {
        let secret = SecretString::new("synthetic-access-code");
        assert_eq!(secret.expose(), "synthetic-access-code");
        assert!(!format!("{secret:?}").contains("synthetic"));
    }
}
