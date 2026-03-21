//! `nomadterm hooks` command — add/remove/status for tool hooks.
//!
//!
//! Manages hook installation across Claude, Gemini, Codex, and OpenCode.

use crate::db::NomadtermDb;
use crate::shared::CommandContext;

/// Parsed arguments for `nomadterm hooks`.
#[derive(clap::Parser, Debug)]
#[command(name = "hooks", about = "Manage tool hooks")]
pub struct HooksArgs {
    /// Subcommand and arguments (status/add/remove [tool])
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Valid tool names for hooks management.
const HOOK_TOOLS: &[&str] = &["claude", "gemini", "codex", "opencode"];

/// Get hook installation status for each tool.
fn get_tool_status() -> Vec<(&'static str, bool, String)> {
    let claude_installed = crate::hooks::claude::verify_claude_hooks_installed(None, false);
    let claude_path = crate::hooks::claude::get_claude_settings_path()
        .to_string_lossy()
        .to_string();

    let gemini_installed = crate::hooks::gemini::verify_gemini_hooks_installed(false);
    let gemini_path = crate::hooks::gemini::get_gemini_settings_path()
        .to_string_lossy()
        .to_string();

    let codex_installed = crate::hooks::codex::verify_codex_hooks_installed(false);
    let codex_path = crate::hooks::codex::get_codex_config_path()
        .to_string_lossy()
        .to_string();

    let opencode_installed = crate::hooks::opencode::verify_opencode_plugin_installed();
    let opencode_path = crate::hooks::opencode::get_opencode_plugin_path()
        .to_string_lossy()
        .to_string();

    vec![
        ("claude", claude_installed, claude_path),
        ("gemini", gemini_installed, gemini_path),
        ("codex", codex_installed, codex_path),
        ("opencode", opencode_installed, opencode_path),
    ]
}

/// Show hook installation status for all tools.
fn cmd_hooks_status() -> i32 {
    let status = get_tool_status();
    for (tool, installed, path) in &status {
        if *installed {
            println!("{tool}:  installed    ({path})");
        } else {
            println!("{tool}:  not installed");
        }
    }
    0
}

/// Add hooks for specified tool(s).
fn cmd_hooks_add(argv: &[String]) -> i32 {
    // Get auto_approve from config
    let include_permissions = crate::config::load_config_snapshot().core.auto_approve;

    // Determine which tools to install
    let tools: Vec<&str> = if argv.is_empty() {
        // Auto-detect current tool
        let current = detect_current_tool();
        if HOOK_TOOLS.contains(&current) {
            vec![current]
        } else {
            HOOK_TOOLS.to_vec()
        }
    } else if argv[0] == "all" {
        HOOK_TOOLS.to_vec()
    } else if HOOK_TOOLS.contains(&argv[0].as_str()) {
        vec![argv[0].as_str()]
    } else {
        eprintln!("Error: Unknown tool: {}", argv[0]);
        eprintln!("Valid options: claude, gemini, codex, opencode, all");
        return 1;
    };

    // Check current status before installing
    let pre_status = get_tool_status();

    // Install hooks — propagate error detail where available
    // Outcome: "already" = was already installed, "added" = newly added, "failed" = error
    enum AddResult {
        Already,
        Added,
        Failed(Option<String>),
    }
    let mut results: Vec<(&str, AddResult)> = Vec::new();
    for tool in &tools {
        let already = pre_status
            .iter()
            .find(|(t, _, _)| t == tool)
            .map(|(_, installed, _)| *installed)
            .unwrap_or(false);
        if already {
            results.push((tool, AddResult::Already));
            continue;
        }
        let outcome = match *tool {
            "claude" => {
                if crate::hooks::claude::setup_claude_hooks(include_permissions) {
                    AddResult::Added
                } else {
                    AddResult::Failed(None)
                }
            }
            "gemini" => {
                if crate::hooks::gemini::setup_gemini_hooks(include_permissions) {
                    AddResult::Added
                } else {
                    AddResult::Failed(None)
                }
            }
            "codex" => {
                if crate::hooks::codex::setup_codex_hooks(include_permissions) {
                    AddResult::Added
                } else {
                    AddResult::Failed(None)
                }
            }
            "opencode" => match crate::hooks::opencode::install_opencode_plugin() {
                Ok(true) => AddResult::Added,
                Ok(false) => AddResult::Failed(None),
                Err(e) => AddResult::Failed(Some(e.to_string())),
            },
            _ => AddResult::Failed(None),
        };
        results.push((tool, outcome));
    }

    // Report results
    let post_status = get_tool_status();
    let mut added_count = 0;
    let mut fail_count = 0;
    for (tool, outcome) in &results {
        let path = post_status
            .iter()
            .find(|(t, _, _)| t == tool)
            .map(|(_, _, p)| p.as_str())
            .unwrap_or("");
        match outcome {
            AddResult::Already => println!("{tool} hooks already installed  ({path})"),
            AddResult::Added => {
                println!("Added {tool} hooks  ({path})");
                added_count += 1;
            }
            AddResult::Failed(Some(e)) => {
                eprintln!("Failed to add {tool} hooks: {e}");
                fail_count += 1;
            }
            AddResult::Failed(None) => {
                eprintln!("Failed to add {tool} hooks");
                fail_count += 1;
            }
        }
    }

    if added_count > 0 {
        println!();
        if tools.len() == 1 {
            let tool_name = match tools[0] {
                "claude" => "Claude Code",
                "gemini" => "Gemini CLI",
                "codex" => "Codex",
                "opencode" => "OpenCode",
                other => other,
            };
            println!("Restart {tool_name} to activate hooks.");
        } else {
            println!("Restart the tool(s) to activate hooks.");
        }
    }

    if fail_count > 0 { 1 } else { 0 }
}

/// Remove hooks for specified tool(s). Called from both `nomadterm hooks remove` and `nomadterm reset hooks`.
pub fn cmd_hooks_remove(argv: &[String]) -> i32 {
    // Determine which tools to remove
    let tools: Vec<&str> = if argv.is_empty() || (argv.len() == 1 && argv[0] == "all") {
        HOOK_TOOLS.to_vec()
    } else if HOOK_TOOLS.contains(&argv[0].as_str()) {
        vec![argv[0].as_str()]
    } else {
        eprintln!("Error: Unknown tool: {}", argv[0]);
        eprintln!("Valid options: claude, gemini, codex, opencode, all");
        return 1;
    };

    // Check current status before removing
    let pre_status = get_tool_status();

    // Remove hooks — propagate error detail where available
    // Outcome: "already" = was not installed, "removed" = newly removed, "failed" = error
    enum RemoveResult {
        AlreadyRemoved,
        Removed,
        Failed(Option<String>),
    }
    let mut results: Vec<(&str, RemoveResult)> = Vec::new();
    for tool in &tools {
        let installed = pre_status
            .iter()
            .find(|(t, _, _)| t == tool)
            .map(|(_, installed, _)| *installed)
            .unwrap_or(false);
        if !installed {
            results.push((tool, RemoveResult::AlreadyRemoved));
            continue;
        }
        let outcome = match *tool {
            "claude" => {
                if crate::hooks::claude::remove_claude_hooks() {
                    RemoveResult::Removed
                } else {
                    RemoveResult::Failed(None)
                }
            }
            "gemini" => {
                if crate::hooks::gemini::remove_gemini_hooks() {
                    RemoveResult::Removed
                } else {
                    RemoveResult::Failed(None)
                }
            }
            "codex" => {
                if crate::hooks::codex::remove_codex_hooks() {
                    RemoveResult::Removed
                } else {
                    RemoveResult::Failed(None)
                }
            }
            "opencode" => match crate::hooks::opencode::remove_opencode_plugin() {
                Ok(()) => RemoveResult::Removed,
                Err(e) => RemoveResult::Failed(Some(e.to_string())),
            },
            _ => RemoveResult::Failed(None),
        };
        results.push((tool, outcome));
    }

    // Report results
    let mut fail_count = 0;
    for (tool, outcome) in &results {
        match outcome {
            RemoveResult::AlreadyRemoved => println!("{tool} hooks already removed"),
            RemoveResult::Removed => println!("Removed {tool} hooks"),
            RemoveResult::Failed(Some(e)) => {
                eprintln!("Failed to remove {tool} hooks: {e}");
                fail_count += 1;
            }
            RemoveResult::Failed(None) => {
                eprintln!("Failed to remove {tool} hooks");
                fail_count += 1;
            }
        }
    }

    if fail_count > 0 { 1 } else { 0 }
}

/// Detect current AI tool from environment.
fn detect_current_tool() -> &'static str {
    crate::shared::detect_current_tool_from_env()
}

pub fn cmd_hooks(_db: &NomadtermDb, args: &HooksArgs, _ctx: Option<&CommandContext>) -> i32 {
    let argv = &args.args;
    if argv.is_empty() {
        // No args = show status
        return cmd_hooks_status();
    }

    let first = argv[0].as_str();

    if first == "--help" || first == "-h" {
        println!(
            "nomadterm hooks - Manage tool hooks for nomadterm integration\n\n\
             Hooks enable automatic message delivery and status tracking. Without hooks,\n\
             you can still use nomadterm in ad-hoc mode (run nomadterm start in any ai tool).\n\n\
             Usage:\n  \
             nomadterm hooks                  Show hook status for all tools\n  \
             nomadterm hooks status           Same as above\n  \
             nomadterm hooks add [tool]       Add hooks (claude|gemini|codex|opencode|all)\n  \
             nomadterm hooks remove [tool]    Remove hooks (claude|gemini|codex|opencode|all)\n\n\
             Examples:\n  \
             nomadterm hooks add claude       Add Claude Code hooks only\n  \
             nomadterm hooks add              Auto-detect tool or add all\n  \
             nomadterm hooks remove all       Remove all hooks\n\n\
             After adding, restart the tool to activate hooks."
        );
        return 0;
    }

    let sub_argv = argv[1..].to_vec();

    match first {
        "status" => cmd_hooks_status(),
        "add" | "install" => cmd_hooks_add(&sub_argv),
        "remove" | "uninstall" => cmd_hooks_remove(&sub_argv),
        _ => {
            eprintln!("Error: Unknown hooks subcommand: {first}");
            eprintln!("Usage: nomadterm hooks [status|add|remove] [tool]");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_current_tool_default() {
        // In test env, none of the AI tool vars should be set
        // (unless running inside one, which is fine — it'll detect it)
        let tool = detect_current_tool();
        assert!(
            ["claude", "gemini", "codex", "opencode", "adhoc"].contains(&tool),
            "unexpected tool: {tool}"
        );
    }
}
