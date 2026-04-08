/// Smart Clipboard Manager for Zed.
///
/// Ensures internal copy-paste is instantaneous and consistent (preserving line-copy metadata),
/// while remaining synchronized with the OS clipboard.

pub struct ClipboardManager {
    /// The text currently stored in Zed's internal "high-priority" clipboard.
    internal_text: String,
    /// Whether the internal text represents a "whole line" copy.
    is_line: bool,
    /// Whether the internal buffer is newer than any potential OS change.
    /// This is set to true on Copy, and false when the window loses focus.
    is_internal_new: bool,
}

impl ClipboardManager {
    pub fn new() -> Self {
        Self {
            internal_text: String::new(),
            is_line: false,
            is_internal_new: false,
        }
    }

    /// Update both the internal and OS clipboards.
    pub fn copy(&mut self, text: String, is_line: bool, ctx: &egui::Context) {
        self.internal_text = text.clone();
        self.is_line = is_line;
        self.is_internal_new = true;

        // Sync to OS clipboard
        ctx.output_mut(|o| o.copied_text = text);
    }

    /// Retrieve text from the clipboard.
    /// Prefers the internal buffer if it was recently updated within this application session.
    pub fn paste(&mut self, os_text: String) -> (String, bool) {
        if self.is_internal_new {
            // We just copied this in Zed; trust it absolutely.
            (self.internal_text.clone(), self.is_line)
        } else if os_text == self.internal_text {
            // OS matches internal; use internal for metadata (is_line).
            (self.internal_text.clone(), self.is_line)
        } else {
            // Content must have come from outside Zed (e.g. Chrome).
            // Sync it to internal for future comparisons.
            self.internal_text = os_text.clone();
            self.is_line = false;
            (os_text, false)
        }
    }

    /// Mark internal buffer as "stale" (potential for OS clipboard to have changed).
    pub fn invalidate_internal(&mut self) {
        self.is_internal_new = false;
    }

    /// Get internal state (for manual inspection or debugging)
    pub fn internal_state(&self) -> (&str, bool) {
        (&self.internal_text, self.is_line)
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
