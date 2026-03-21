//! Shared runtime helpers for invoking nomadterm and locating tool config roots.

/// Cached nomadterm invocation prefix (computed once per process lifetime).
static NOMADTERM_PREFIX: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
    if std::env::var("NOMADTERM_DEV_ROOT").is_ok() {
        return vec!["nomadterm".into()];
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Ok(resolved) = exe.canonicalize() {
            let has_uv = resolved.components().any(|c| c.as_os_str() == "uv");
            if has_uv {
                return vec!["uvx".into(), "nomadterm".into()];
            }
        }
    }

    vec!["nomadterm".into()]
});

/// Detect nomadterm invocation prefix based on execution context.
pub(crate) fn get_nomadterm_prefix() -> Vec<String> {
    NOMADTERM_PREFIX.clone()
}

/// Get the base directory for tool config files (e.g. .codex/, .gemini/).
pub(crate) fn tool_config_root() -> std::path::PathBuf {
    let env: std::collections::HashMap<String, String> = std::env::vars().collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let (nomadterm_dir, _) = crate::paths::resolve_nomadterm_dir_from_env(&env, &cwd);
    nomadterm_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
}

/// Build nomadterm command string for prompts, config, and hook commands.
pub(crate) fn build_nomadterm_command() -> String {
    get_nomadterm_prefix().join(" ")
}

/// Set terminal title via escape codes written to /dev/tty.
pub(crate) fn set_terminal_title(instance_name: &str) {
    let title = format!("nomadterm: {}", instance_name);
    if let Ok(mut tty) = std::fs::OpenOptions::new().write(true).open("/dev/tty") {
        use std::io::Write;
        let _ = write!(tty, "\x1b]1;{}\x07\x1b]2;{}\x07", title, title);
    }
}

#[cfg(test)]
mod tests {
    use crate::hooks::test_helpers::EnvGuard;
    use serial_test::serial;

    #[test]
    #[serial]
    fn tool_config_root_uses_home_when_nomadterm_dir_has_no_parent() {
        let _guard = EnvGuard::new();
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();

        unsafe {
            std::env::set_var("HOME", &home);
            std::env::set_var("NOMADTERM_DIR", "/");
        }

        assert_eq!(super::tool_config_root(), home);
    }

    #[test]
    #[serial]
    fn tool_config_root_uses_parent_of_resolved_nomadterm_dir() {
        let _guard = EnvGuard::new();
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let home = temp.path().join("home");
        let sandbox = workspace.join(".sandbox");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&sandbox).unwrap();

        let prev_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&workspace).unwrap();
        unsafe {
            std::env::set_var("HOME", &home);
            std::env::set_var("NOMADTERM_DIR", ".sandbox/.nomadterm");
        }

        let root = super::tool_config_root();
        let expected = sandbox.canonicalize().unwrap();

        std::env::set_current_dir(prev_cwd).unwrap();
        assert_eq!(root, expected);
    }
}
