
pub mod right;
pub mod extensions_panel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTab {
    Chat,
    Investigator,
    Extensions,
    FileTree,
    Terminal,
}

pub struct PanelManager {
    pub right_panel: right::RightPanel,
}

impl PanelManager {
    pub fn new() -> Self {
        Self {
            right_panel: right::RightPanel::new(),
        }
    }
}
