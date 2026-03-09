pub struct SandboxConfig {
    pub workspace_path: String,
}

impl SandboxConfig {
    pub fn new(workspace_path: &str) -> Self {
        Self { workspace_path: workspace_path.to_string() }
    }

    /// Static validation of Python code before execution.
    /// Returns Ok(()) if safe, Err(message) if blocked.
    pub fn validate_code(&self, code: &str) -> Result<(), String> {
        // Check forbidden modules
        let forbidden_modules = ["subprocess", "importlib", "multiprocessing"];
        for module in &forbidden_modules {
            if code.contains(&format!("import {}", module))
                || code.contains(&format!("from {} ", module))
                || code.contains(&format!("__import__('{}')", module))
                || code.contains(&format!("__import__(\"{}\")", module)) {
                return Err(format!("Module '{}' is not allowed for security reasons", module));
            }
        }

        // Check dangerous os calls
        let dangerous_calls = [
            "os.system", "os.popen", "os.exec", "os.fork", "os.spawn",
            "os.kill", "os.remove", "os.unlink", "os.rmdir",
        ];
        for call in &dangerous_calls {
            if code.contains(call) {
                return Err(format!("'{}' is not allowed for security reasons", call));
            }
        }

        // Check standalone dangerous calls (eval/exec/compile)
        // But allow things like pd.eval(), df.eval(), np.exec()
        for func in &["eval", "exec", "compile"] {
            if contains_standalone(code, func) {
                return Err(format!("Standalone '{}()' is not allowed. Use library-specific methods instead.", func));
            }
        }

        // Check bypass attempts
        let bypass_patterns = [
            "builtins.__import__", "__inkess_builtin_open__", "__builtins__",
            "importlib.import_module", "globals()[", "locals()[",
            "_original_open",
        ];
        for pat in &bypass_patterns {
            if code.contains(pat) {
                return Err(format!("'{}' is not allowed for security reasons", pat));
            }
        }

        Ok(())
    }

    /// Generate Python preamble injected before user code.
    /// Sets up encoding, preloads packages, restricts file writes to workspace.
    pub fn preamble(&self) -> String {
        let workspace = self.workspace_path.replace('\\', "\\\\").replace('\'', "\\'");
        format!(r#"
# === Inkess Sandbox Preamble ===
import sys, os
sys.setrecursionlimit(1000)

# Pre-import common packages (suppress import errors)
try:
    import pandas as pd
except ImportError:
    pass
try:
    import numpy as np
except ImportError:
    pass

# Restrict file writes to workspace directory
__inkess_builtin_open__ = open
_workspace = r'{workspace}'

def _safe_open(path, mode='r', *args, **kwargs):
    if any(m in mode for m in ('w', 'a', 'x')):
        abs_path = os.path.realpath(path)
        if _workspace and not abs_path.startswith(_workspace):
            raise PermissionError(f"Cannot write outside workspace: {{path}}")
    return __inkess_builtin_open__(path, mode, *args, **kwargs)

import builtins
builtins.open = _safe_open

# === End Preamble ===
"#)
    }
}

/// Check if code contains a standalone call to func (not method call like pd.eval)
fn contains_standalone(code: &str, func: &str) -> bool {
    let pattern = format!("{}(", func);
    for (i, _) in code.match_indices(&pattern) {
        if i == 0 {
            return true;
        }
        let prev_char = code.as_bytes()[i - 1];
        // If preceded by '.', it's a method call (pd.eval, df.exec) — allowed
        // If preceded by letter/digit/underscore, it's part of another name — skip
        if prev_char == b'.' || prev_char.is_ascii_alphanumeric() || prev_char == b'_' {
            continue;
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox() -> SandboxConfig {
        SandboxConfig::new("/tmp/test-workspace")
    }

    // --- validate_code tests ---

    #[test]
    fn test_safe_code_passes() {
        assert!(sandbox().validate_code("print('hello')").is_ok());
        assert!(sandbox().validate_code("import pandas as pd\ndf = pd.read_csv('data.csv')").is_ok());
        assert!(sandbox().validate_code("x = 1 + 2\nprint(x)").is_ok());
    }

    #[test]
    fn test_import_subprocess_blocked() {
        assert!(sandbox().validate_code("import subprocess").is_err());
        assert!(sandbox().validate_code("from subprocess import run").is_err());
    }

    #[test]
    fn test_import_importlib_blocked() {
        assert!(sandbox().validate_code("import importlib").is_err());
        assert!(sandbox().validate_code("from importlib import import_module").is_err());
    }

    #[test]
    fn test_import_multiprocessing_blocked() {
        assert!(sandbox().validate_code("import multiprocessing").is_err());
    }

    #[test]
    fn test_os_system_blocked() {
        assert!(sandbox().validate_code("os.system('rm -rf /')").is_err());
    }

    #[test]
    fn test_os_popen_blocked() {
        assert!(sandbox().validate_code("os.popen('ls')").is_err());
    }

    #[test]
    fn test_eval_standalone_blocked() {
        assert!(sandbox().validate_code("eval('1+1')").is_err());
        assert!(sandbox().validate_code("result = eval(expr)").is_err());
    }

    #[test]
    fn test_eval_method_allowed() {
        // pd.eval() and df.eval() should NOT be blocked
        assert!(sandbox().validate_code("pd.eval('a + b')").is_ok());
        assert!(sandbox().validate_code("df.eval('col1 > 0')").is_ok());
    }

    #[test]
    fn test_exec_standalone_blocked() {
        assert!(sandbox().validate_code("exec('print(1)')").is_err());
    }

    #[test]
    fn test_compile_standalone_blocked() {
        assert!(sandbox().validate_code("compile('code', 'f', 'exec')").is_err());
    }

    #[test]
    fn test_builtins_blocked() {
        assert!(sandbox().validate_code("__builtins__").is_err());
        assert!(sandbox().validate_code("builtins.__import__('os')").is_err());
    }

    #[test]
    fn test_dunder_import_blocked() {
        assert!(sandbox().validate_code("__import__('subprocess')").is_err());
    }

    #[test]
    fn test_inkess_builtin_open_blocked() {
        // Prevent user code from accessing the real open
        assert!(sandbox().validate_code("__inkess_builtin_open__('file')").is_err());
    }

    // --- preamble tests ---

    #[test]
    fn test_preamble_contains_builtin_open() {
        let preamble = sandbox().preamble();
        assert!(preamble.contains("__inkess_builtin_open__"));
    }

    #[test]
    fn test_preamble_contains_realpath() {
        let preamble = sandbox().preamble();
        assert!(preamble.contains("os.path.realpath"));
    }

    #[test]
    fn test_preamble_contains_workspace() {
        let sb = SandboxConfig::new("/home/user/project");
        let preamble = sb.preamble();
        assert!(preamble.contains("/home/user/project"));
    }

    // --- contains_standalone tests ---

    #[test]
    fn test_contains_standalone_bare_call() {
        assert!(contains_standalone("eval('code')", "eval"));
        assert!(contains_standalone("x = eval(expr)", "eval"));
    }

    #[test]
    fn test_contains_standalone_method_call_not_matched() {
        assert!(!contains_standalone("pd.eval('a+b')", "eval"));
        assert!(!contains_standalone("df.exec()", "exec"));
    }

    #[test]
    fn test_contains_standalone_part_of_name_not_matched() {
        assert!(!contains_standalone("my_eval(x)", "eval"));
        assert!(!contains_standalone("evaluation(x)", "eval"));
    }

    #[test]
    fn test_contains_standalone_at_start() {
        assert!(contains_standalone("eval()", "eval"));
    }

    #[test]
    fn test_contains_standalone_with_spaces() {
        assert!(contains_standalone("result = eval(expr)", "eval"));
        assert!(contains_standalone("  eval('x')", "eval"));
    }
}
