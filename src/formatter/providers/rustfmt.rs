use crate::formatter::{FormatError, FormatResult, FormatterProvider};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub struct RustfmtProvider {
    additional_args: Vec<String>,
}

impl RustfmtProvider {
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

impl Default for RustfmtProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatterProvider for RustfmtProvider {
    fn name(&self) -> &str {
        "rustfmt"
    }

    fn supported_extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn is_available(&self) -> bool {
        Command::new("rustfmt").arg("--version").output().is_ok()
    }

    fn format(&self, text: &str, _file_path: Option<&Path>) -> FormatResult {
        let mut child = Command::new("rustfmt")
            .args(&self.additional_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| FormatError::ExecutionFailed(e.to_string()))?;

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| FormatError::ExecutionFailed(e.to_string()))?;
        }

        // Wait for process and collect output
        let output = child
            .wait_with_output()
            .map_err(|e| FormatError::ExecutionFailed(e.to_string()))?;

        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|e| FormatError::InvalidOutput(e.to_string()))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(FormatError::ExecutionFailed(format!(
                "rustfmt failed: {}",
                stderr
            )))
        }
    }
}
