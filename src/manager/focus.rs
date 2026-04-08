/// Focus management and panel state for the Zed Text Editor.
///
/// Moved to the manager layer to allow shared access across different UI components.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusTarget {
    Editor,
    Terminal,
    FileTree,
    SearchBar,
}

impl FocusTarget {
    fn cycle_order(active_panels: &ActivePanels) -> Vec<FocusTarget> {
        let mut order = vec![FocusTarget::Editor];
        if active_panels.terminal_open { order.push(FocusTarget::Terminal); }
        if active_panels.file_tree_open { order.push(FocusTarget::FileTree); }
        if active_panels.search_bar_open { order.push(FocusTarget::SearchBar); }
        order
    }

    pub fn label(&self) -> &'static str {
        match self {
            FocusTarget::Editor => "Editor",
            FocusTarget::Terminal => "Terminal",
            FocusTarget::FileTree => "File Tree",
            FocusTarget::SearchBar => "Search",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ActivePanels {
    pub terminal_open: bool,
    pub file_tree_open: bool,
    pub search_bar_open: bool,
}

#[derive(Debug)]
pub struct FocusManager {
    current: FocusTarget,
    pub show_indicator: bool,
}

impl FocusManager {
    pub fn new() -> Self {
        Self {
            current: FocusTarget::Editor,
            show_indicator: false,
        }
    }

    pub fn current(&self) -> FocusTarget { self.current }
    pub fn is_focused(&self, target: FocusTarget) -> bool { self.current == target }
    pub fn set(&mut self, target: FocusTarget) { self.current = target; }

    pub fn handle_tab(&mut self, shift: bool, active_panels: &ActivePanels) -> bool {
        let order = FocusTarget::cycle_order(active_panels);
        if order.len() <= 1 { return false; }

        let current_idx = order.iter().position(|&t| t == self.current).unwrap_or(0);
        let next_idx = if shift {
            if current_idx == 0 { order.len() - 1 } else { current_idx - 1 }
        } else {
            (current_idx + 1) % order.len()
        };

        self.current = order[next_idx];
        self.show_indicator = true;
        true
    }

    pub fn status_label(&self) -> String {
        format!("[{}]", self.current.label())
    }

    pub fn on_key_pressed(&mut self) {
        // Keep indicator visible while navigating with keyboard;
        // hide it on first "real" keystroke so it doesn't clutter editing
    }
}

impl Default for FocusManager {
    fn default() -> Self { Self::new() }
}
