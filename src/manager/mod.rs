pub mod clipboard;
pub mod focus;

pub use clipboard::ClipboardManager;
pub use focus::{ActivePanels, FocusManager, FocusTarget};

/// The central authority for application-wide state.
///
/// Refactored out of GuiApp to ensure that core logic like focus and
/// clipboard management can be shared across multiple windows or panels
/// in the future.
pub struct GlobalManager {
    pub clipboard: ClipboardManager,
    pub focus: FocusManager,
    pub panels: ActivePanels,
}

impl GlobalManager {
    pub fn new() -> Self {
        Self {
            clipboard: ClipboardManager::new(),
            focus: FocusManager::new(),
            panels: ActivePanels::default(),
        }
    }
}

impl Default for GlobalManager {
    fn default() -> Self {
        Self::new()
    }
}
