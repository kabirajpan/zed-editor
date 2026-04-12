/// Focus management and panel state for the Zed Text Editor.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusTarget {
    Editor,
    LeftPanel,   // File Tree, etc.
    RightPanel,  // Chat, Investigator, etc.
    BottomPanel, // Terminal, Logs, etc.
    SearchBar,
}

impl FocusTarget {
    fn cycle_order(active_panels: &ActivePanels) -> Vec<FocusTarget> {
        let mut order = vec![FocusTarget::Editor];
        if active_panels.left_open { order.push(FocusTarget::LeftPanel); }
        if active_panels.right_open { order.push(FocusTarget::RightPanel); }
        if active_panels.bottom_open { order.push(FocusTarget::BottomPanel); }
        if active_panels.search_bar_open { order.push(FocusTarget::SearchBar); }
        order
    }

    pub fn label(&self) -> &'static str {
        match self {
            FocusTarget::Editor => "Editor",
            FocusTarget::LeftPanel => "Side Bar (Left)",
            FocusTarget::RightPanel => "Intelligence (Right)",
            FocusTarget::BottomPanel => "Status (Bottom)",
            FocusTarget::SearchBar => "Search",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ActivePanels {
    pub left_open: bool,
    pub right_open: bool,
    pub bottom_open: bool,
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
        // Hide indicator on first real keystroke
    }
}

impl Default for FocusManager {
    fn default() -> Self { Self::new() }
}
