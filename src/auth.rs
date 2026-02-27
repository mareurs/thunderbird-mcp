use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Cannot determine home directory")]
    NoHome,
    #[error("Auth token not found at {paths:?}. Is Thunderbird running with the MCP extension?")]
    NotFound { paths: Vec<PathBuf> },
}

pub fn find_token() -> Result<String, AuthError> {
    let home = dirs::home_dir().ok_or(AuthError::NoHome)?;
    find_token_in(&home)
}

// Testable inner function — accepts home dir as parameter
pub fn find_token_in(home: &std::path::Path) -> Result<String, AuthError> {
    let candidates = [
        home.join(".thunderbird-mcp-auth"),
        home.join("snap/thunderbird/common/.thunderbird-mcp-auth"),
    ];
    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            return Ok(content.trim().to_string());
        }
    }
    Err(AuthError::NotFound { paths: candidates.to_vec() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_token(dir: &std::path::Path, rel: &str, token: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, token).unwrap();
    }

    #[test]
    fn finds_token_at_home_path() {
        let tmp = TempDir::new().unwrap();
        write_token(tmp.path(), ".thunderbird-mcp-auth", "token-abc");
        let result = find_token_in(tmp.path()).unwrap();
        assert_eq!(result, "token-abc");
    }

    #[test]
    fn finds_token_at_snap_path() {
        let tmp = TempDir::new().unwrap();
        write_token(tmp.path(), "snap/thunderbird/common/.thunderbird-mcp-auth", "token-snap");
        let result = find_token_in(tmp.path()).unwrap();
        assert_eq!(result, "token-snap");
    }

    #[test]
    fn prefers_home_over_snap() {
        let tmp = TempDir::new().unwrap();
        write_token(tmp.path(), ".thunderbird-mcp-auth", "token-home");
        write_token(tmp.path(), "snap/thunderbird/common/.thunderbird-mcp-auth", "token-snap");
        let result = find_token_in(tmp.path()).unwrap();
        assert_eq!(result, "token-home");
    }

    #[test]
    fn trims_whitespace() {
        let tmp = TempDir::new().unwrap();
        write_token(tmp.path(), ".thunderbird-mcp-auth", "  token-xyz\n");
        let result = find_token_in(tmp.path()).unwrap();
        assert_eq!(result, "token-xyz");
    }

    #[test]
    fn returns_error_when_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = find_token_in(tmp.path());
        assert!(matches!(result, Err(AuthError::NotFound { .. })));
    }
}
