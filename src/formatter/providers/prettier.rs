use crate::formatter::{FormatError, FormatResult, FormatterProvider};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub struct PrettierProvider {
    additional_args: Vec<String>,
}

impl PrettierProvider {
    pub fn new() -> Self {
        Self {
            additional_args: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.additional_args = args;
        self
    }
}

impl Default for PrettierProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatterProvider for PrettierProvider {
    fn name(&self) -> &str {
        "prettier"
    }

    fn supported_extensions(&self) -> &[&str] {
        &["js", "jsx", "ts", "tsx", "json", "css", "html", "md"]
    }

    fn is_available(&self) -> bool {
        Command::new("prettier").arg("--version").output().is_ok()
    }

    fn format(&self, text: &str, file_path: Option<&Path>) -> FormatResult {
        let mut cmd = Command::new("prettier");
        cmd.args(&self.additional_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(path) = file_path {
            if let Some(path_str) = path.to_str() {
                cmd.arg("--stdin-filepath").arg(path_str);
            }
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| FormatError::ExecutionFailed(e.to_string()))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| FormatError::ExecutionFailed(e.to_string()))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| FormatError::ExecutionFailed(e.to_string()))?;

        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|e| FormatError::InvalidOutput(e.to_string()))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(FormatError::ExecutionFailed(format!(
                "prettier failed: {}",
                stderr
            )))
        }
    }
}
