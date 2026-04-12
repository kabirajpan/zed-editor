use egui::{Ui, Color32, RichText};
use crate::ai::mason::{MasonManager, LspServiceStatus};

pub struct ExtensionsPanel;

impl ExtensionsPanel {
    pub fn render(ui: &mut Ui, mason: &mut MasonManager, sender: &tokio::sync::mpsc::UnboundedSender<crate::ai::mason::MasonEvent>) {
        ui.vertical(|ui| {
            ui.heading(RichText::new("🧩 Extensions Manager").strong().color(Color32::WHITE));
            ui.add_space(8.0);
            ui.label("Manage your local Language Servers (Mason).");
            ui.separator();
            ui.add_space(4.0);

            let mut sorted_keys: Vec<_> = mason.registry.keys().cloned().collect();
            sorted_keys.sort();

            let mut pending_install = None;

            for name in sorted_keys {
                let ext = mason.registry.get_mut(&name).unwrap();
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&ext.name).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            render_status_badge(ui, &ext.status);
                        });
                    });
                    
                    ui.add_space(4.0);
                    
                    ui.horizontal(|ui| {
                        match &ext.status {
                            LspServiceStatus::NotInstalled => {
                                if ui.button("📥 Install").clicked() {
                                    pending_install = Some(name.clone());
                                }
                            }
                            LspServiceStatus::Downloading(p) => {
                                ui.add(egui::ProgressBar::new(*p).show_percentage());
                            }
                            LspServiceStatus::Installed | LspServiceStatus::Running | LspServiceStatus::Paused => {
                                if ui.button("🗑 Uninstall").clicked() {
                                    ext.status = LspServiceStatus::NotInstalled;
                                }
                                
                                if ext.status == LspServiceStatus::Running {
                                    if ui.button("⏸ Pause").clicked() {
                                        ext.status = LspServiceStatus::Paused;
                                    }
                                } else if ext.status == LspServiceStatus::Paused {
                                    if ui.button("▶ Resume").clicked() {
                                        ext.status = LspServiceStatus::Running;
                                    }
                                }
                            }
                            LspServiceStatus::Error(e) => {
                                ui.label(RichText::new(e).color(Color32::RED).small());
                                if ui.button("🔄 Retry").clicked() {
                                    ext.status = LspServiceStatus::NotInstalled;
                                }
                            }
                        }
                    });
                });
                ui.add_space(4.0);
            }

            if let Some(name) = pending_install {
                if let Some(ext) = mason.registry.get_mut(&name) {
                    ext.status = LspServiceStatus::Downloading(0.0);
                }
                mason.trigger_install(name, sender.clone());
            }
        });
    }
}

fn render_status_badge(ui: &mut Ui, status: &LspServiceStatus) {
    let (text, color) = match status {
        LspServiceStatus::NotInstalled => ("Offline", Color32::GRAY),
        LspServiceStatus::Downloading(_) => ("Installing", Color32::KHAKI),
        LspServiceStatus::Installed => ("Ready", Color32::LIGHT_BLUE),
        LspServiceStatus::Running => ("Running", Color32::GREEN),
        LspServiceStatus::Paused => ("Paused", Color32::GOLD),
        LspServiceStatus::Error(_) => ("Error", Color32::LIGHT_RED),
    };
    
    ui.label(RichText::new(text).small().color(color));
}
