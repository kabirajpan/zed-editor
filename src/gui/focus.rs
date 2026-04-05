/// Focus management for the Zed Text Editor GUI.
///
/// Designed to scale — adding a terminal panel, file tree, search bar,
/// or any future pane is a one-liner: add a variant to `FocusTarget`.
///
/// Usage pattern:
///   1. Call `focus_manager.handle_tab(modifiers)` to cycle focus
///   2. Call `focus_manager.is_focused(FocusTarget::Editor)` to gate input
///   3. Call `focus_manager.set(FocusTarget::Terminal)` on panel click
///   4. In egui panels, call `ui.set_enabled(...)` or skip input based on focus

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusTarget {
    Editor,
    Terminal, // Not yet built — reserved for future use
    FileTree, // Not yet built — reserved for future use
    SearchBar, // Not yet built — reserved for future use
              // Add more panes here as you build them — no other code needs to change
}

impl FocusTarget {
    /// Ordered list of focusable targets for Tab cycling.
    /// Only includes targets that are currently active/visible.
    /// You'll pass `active_panels` from GuiApp to control this.
    fn cycle_order(active_panels: &ActivePanels) -> Vec<FocusTarget> {
        let mut order = vec![FocusTarget::Editor];

        if active_panels.terminal_open {
            order.push(FocusTarget::Terminal);
        }
        if active_panels.file_tree_open {
            order.push(FocusTarget::FileTree);
        }
        if active_panels.search_bar_open {
            order.push(FocusTarget::SearchBar);
        }

        order
    }

    /// Human-readable label for status bar / debug display
    pub fn label(&self) -> &'static str {
        match self {
            FocusTarget::Editor => "Editor",
            FocusTarget::Terminal => "Terminal",
            FocusTarget::FileTree => "File Tree",
            FocusTarget::SearchBar => "Search",
        }
    }
}

/// Tracks which optional panels are currently open/visible.
/// GuiApp owns this and updates it as panels are toggled.
#[derive(Debug, Clone, Default)]
pub struct ActivePanels {
    pub terminal_open: bool,
    pub file_tree_open: bool,
    pub search_bar_open: bool,
}

/// The core focus manager. GuiApp owns exactly one of these.
#[derive(Debug)]
pub struct FocusManager {
    current: FocusTarget,
    /// Whether focus indicator should be visually shown (e.g. highlight border)
    pub show_indicator: bool,
}

impl FocusManager {
    pub fn new() -> Self {
        Self {
            current: FocusTarget::Editor,
            show_indicator: false,
        }
    }

    /// Get the currently focused target
    pub fn current(&self) -> FocusTarget {
        self.current
    }

    /// Check if a specific target is focused
    pub fn is_focused(&self, target: FocusTarget) -> bool {
        self.current == target
    }

    /// Explicitly set focus to a target (e.g. on mouse click)
    pub fn set(&mut self, target: FocusTarget) {
        self.current = target;
    }

    /// Handle Tab key press — cycles forward through active panels.
    /// Call this ONLY when the Tab key is pressed and no panel should insert a tab.
    /// Returns true if focus was cycled (i.e. Tab was consumed as focus-switch).
    ///
    /// When only the Editor is active (no terminal/tree open), Tab is NEVER
    /// consumed here — it should insert spaces/tab in the editor instead.
    pub fn handle_tab(&mut self, shift: bool, active_panels: &ActivePanels) -> bool {
        let order = FocusTarget::cycle_order(active_panels);

        // Only 1 focusable target → Tab should NOT cycle focus, let editor handle it
        if order.len() <= 1 {
            return false;
        }

        let current_idx = order.iter().position(|&t| t == self.current).unwrap_or(0);

        let next_idx = if shift {
            // Shift+Tab → cycle backward
            if current_idx == 0 {
                order.len() - 1
            } else {
                current_idx - 1
            }
        } else {
            // Tab → cycle forward
            (current_idx + 1) % order.len()
        };

        self.current = order[next_idx];
        self.show_indicator = true;
        true
    }

    /// Call this whenever a non-Tab key is pressed (hides the focus ring after keyboard nav)
    pub fn on_key_pressed(&mut self) {
        // Keep indicator visible while navigating with keyboard;
        // hide it on first "real" keystroke so it doesn't clutter editing
        // Uncomment below if you want indicator to hide on typing:
        // self.show_indicator = false;
    }

    /// Returns a visual indicator string for the status bar, e.g. "[Editor]"
    pub fn status_label(&self) -> String {
        format!("[{}]", self.current.label())
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience: egui border color to draw around a focused panel
/// Call this in your panel's frame setup when `show_indicator` is true.
pub fn focus_border_color(is_focused: bool) -> egui::Color32 {
    if is_focused {
        egui::Color32::from_rgb(100, 160, 255) // Blue highlight
    } else {
        egui::Color32::from_rgb(50, 50, 60) // Subtle inactive border
    }
}
