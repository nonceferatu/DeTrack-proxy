use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::path::Path;
use eframe::{egui, App, Frame, CreationContext};
use egui::{Color32, RichText, Ui};
use image;

use detrack_proxy::{
    shared_state::SharedState,
    tracker_blocker::TrackerBlocker,
    run_proxy::run_proxy,
};

// Add derive for PartialEq to fix comparison issues
#[derive(PartialEq)]
enum Tab {
    Dashboard,
    Logs,
    BlockList,
    Settings,
    About,
    AI,
}

struct RequestViewerApp {
    state: Arc<SharedState>,
    selected_tab: Tab,
    log_filter: String,
    new_domain: String,
    show_blocked_only: bool,
    max_logs: usize,
    auto_scroll: bool,
    ai_suggestions_showing: bool,
    logo_texture: Option<egui::TextureHandle>,
}

impl RequestViewerApp {
    fn new(state: Arc<SharedState>) -> Self {
        Self {
            state,
            selected_tab: Tab::Dashboard,
            log_filter: String::new(),
            new_domain: String::new(),
            show_blocked_only: false,
            max_logs: 1000,
            auto_scroll: true,
            ai_suggestions_showing: true,
            logo_texture: None,
        }
    }

    fn render_dashboard(&mut self, ui: &mut Ui) {
        ui.heading("Dashboard");
        ui.add_space(10.0);

        // Status and controls
        ui.horizontal(|ui| {
            let enabled = self.state.is_proxy_enabled();
            let status_text = if enabled {
                RichText::new("🟢 Proxy Running").color(Color32::GREEN)
            } else {
                RichText::new("🔴 Proxy Stopped").color(Color32::RED)
            };
            ui.label(status_text);
            
            if ui.button(if enabled { "🚫 Stop Proxy" } else { "▶️ Start Proxy" }).clicked() {
                if enabled {
                    self.state.disable_proxy();
                } else {
                    self.state.enable_proxy();
                }
            }
            
            let logging = self.state.is_logging_enabled();
            if ui.button(if logging { "📴 Disable Logging" } else { "📡 Enable Logging" }).clicked() {
                if logging {
                    self.state.disable_logging();
                } else {
                    self.state.enable_logging();
                }
            }
            
            if ui.button("💨 Clear Logs").clicked() {
                self.state.clear_logs();
            }
        });
        
        ui.add_space(16.0);
        
        // Stats overview
        ui.heading("Request Statistics");
        
        egui::Grid::new("stats_grid").num_columns(2).spacing([40.0, 8.0]).show(ui, |ui| {
            // Get stats
            let allowed = self.state.get_allowed_count();
            let blocked = self.state.get_blocked_count();
            let total = allowed + blocked;
            
            ui.label("Total Requests:");
            ui.label(format!("{}", total));
            ui.end_row();
            
            ui.label("Allowed Requests:");
            ui.label(RichText::new(format!("{}", allowed)).color(Color32::GREEN));
            ui.end_row();
            
            ui.label("Blocked Requests:");
            ui.label(RichText::new(format!("{}", blocked)).color(Color32::RED));
            ui.end_row();
            
            ui.label("Block Rate:");
            let block_rate = if total > 0 {
                (blocked as f32 / total as f32) * 100.0
            } else {
                0.0
            };
            ui.label(format!("{:.1}%", block_rate));
            ui.end_row();
            
            // Get domain stats
            let domain_stats = self.state.get_stats();
            
            ui.label("Unique Domains:");
            ui.label(format!("{}", domain_stats.len()));
            ui.end_row();
        });
        
        ui.add_space(16.0);
        
        // Recent activity
        ui.heading("Recent Activity");
        ui.add_space(8.0);
        
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            let logs = self.state.get_logs();
            let logs_to_show = logs.iter().rev().take(10);
            
            for log in logs_to_show {
                let text = if log.contains("Blocked") || log.contains("🚫") {
                    RichText::new(log.clone()).color(Color32::RED)
                } else if log.contains("Allowed") || log.contains("✅") {
                    RichText::new(log.clone()).color(Color32::GREEN)
                } else {
                    RichText::new(log.clone())
                };
                ui.label(text);
            }
        });


        // Bandwidth section
        ui.add_space(16.0);

        ui.heading("Bandwidth Savings");
        ui.add_space(8.0);

        let saved_bytes = self.state.get_bandwidth_saved();
        ui.label(format!("Total Saved: {:.2} MB", 
        saved_bytes as f64 / 1_000_000.0));
    }

    fn render_logs(&mut self, ui: &mut Ui) {
        ui.heading("Request Logs");
        ui.add_space(10.0);
        
        // Log controls
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.log_filter);
            
            ui.checkbox(&mut self.show_blocked_only, "Blocked Only");
            
            ui.label("Max logs:");
            ui.add(egui::Slider::new(&mut self.max_logs, 10..=10000).logarithmic(true));
            
            ui.checkbox(&mut self.auto_scroll, "Auto-scroll");
            
            if ui.button("💨 Clear Logs").clicked() {
                self.state.clear_logs();
            }
        });
        
        ui.add_space(8.0);
        
        // Log viewer
        let logs = self.state.get_logs();
        let filtered_logs: Vec<&String> = logs.iter()
            .filter(|log| {
                if self.show_blocked_only && !log.contains("Blocked") && !log.contains("🚫") {
                    return false;
                }
                if !self.log_filter.is_empty() {
                    return log.to_lowercase().contains(&self.log_filter.to_lowercase());
                }
                true
            })
            .rev() // Most recent first
            .take(self.max_logs)
            .collect();
        
        let log_panel_height = ui.available_height() - 50.0;
        let scroll_area = egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .max_height(log_panel_height);
        
        scroll_area.show(ui, |ui| {
            for log in &filtered_logs {
                // Fix the dereference issue by cloning the string
                let log_text = (*log).clone();
                let text = if log_text.contains("Blocked") || log_text.contains("🚫") {
                    RichText::new(log_text).color(Color32::RED)
                } else if log_text.contains("Allowed") || log_text.contains("✅") {
                    RichText::new(log_text).color(Color32::GREEN)
                } else {
                    RichText::new(log_text)
                };
                ui.label(text);
            }
        });
        
        ui.label(format!("Displaying {} of {} logs", filtered_logs.len(), logs.len()));
    }

    fn render_blocklist(&mut self, ui: &mut Ui) {
        ui.heading("Tracker Blocklist");
        ui.add_space(16.0);
        
        // Add new domain
        ui.horizontal(|ui| {
            ui.label("Add domain:");
            let response = ui.text_edit_singleline(&mut self.new_domain);
            
            let add_pressed = ui.button("Add").clicked();
            if (add_pressed || response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) 
                && !self.new_domain.is_empty() {
                // Add domain to blocklist
                match self.state.add_tracker(&self.new_domain) {
                    Ok(()) => {
                        // Clear input on success
                        self.new_domain.clear();
                    },
                    Err(e) => {
                        // Log error
                        self.state.append_log(format!("❌ Error adding tracker: {}", e));
                    }
                }
            }
        });
        
        ui.add_space(16.0);
        
        // Blocklist viewer
        match self.state.get_trackers() {
            Ok(trackers) => {
                ui.label(format!("Current blocked domains: {}", trackers.len()));
                
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, domain) in trackers.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}. {}", i + 1, domain));
                            
                            if ui.button("❌").clicked() {
                                // Remove domain from blocklist
                                if let Err(e) = self.state.remove_tracker(domain) {
                                    self.state.append_log(format!("❌ Error removing tracker: {}", e));
                                }
                            }
                        });
                    }
                });
            },
            Err(e) => {
                ui.label(RichText::new(format!("❌ Error loading trackers: {}", e)).color(Color32::RED));
            }
        }
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        // Import/Export controls
        ui.heading("Import/Export");
        
        ui.horizontal(|ui| {
            if ui.button("Import Trackers").clicked() {
                // This would require file dialog - not implemented yet
                // In a real app, you'd use a native file dialog here
                self.state.append_log("Import trackers requested - Not implemented yet".to_string());
            }
            
            if ui.button("Export Trackers").clicked() {
                // This would require file dialog - not implemented yet
                self.state.append_log("Export trackers requested - Not implemented yet".to_string());
            }
        });
    }

    fn render_settings(&mut self, ui: &mut Ui) {
        ui.heading("Proxy Settings");
        ui.add_space(16.0);
        
        // Proxy status
        let enabled = self.state.is_proxy_enabled();
        ui.horizontal(|ui| {
            ui.label("Proxy Status:");
            let status_text = if enabled {
                RichText::new("Running").color(Color32::GREEN)
            } else {
                RichText::new("Stopped").color(Color32::RED)
            };
            ui.label(status_text);
        });
        
        ui.add_space(8.0);
        
        // Proxy controls
        if ui.button(if enabled { "🚫 Stop Proxy" } else { "▶️ Start Proxy" }).clicked() {
            if enabled {
                self.state.disable_proxy();
            } else {
                self.state.enable_proxy();
            }
        }
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        // Logging settings
        ui.heading("Logging Settings");
        ui.add_space(8.0);
        
        let logging = self.state.is_logging_enabled();
        ui.horizontal(|ui| {
            ui.label("Logging Status:");
            let status_text = if logging {
                RichText::new("Enabled").color(Color32::GREEN)
            } else {
                RichText::new("Disabled").color(Color32::RED)
            };
            ui.label(status_text);
        });
        
        ui.add_space(8.0);
        
        // Logging controls
        if ui.button(if logging { "📴 Disable Logging" } else { "📡 Enable Logging" }).clicked() {
            if logging {
                self.state.disable_logging();
            } else {
                self.state.enable_logging();
            }
        }
        
        if ui.button("💨 Clear Logs").clicked() {
            self.state.clear_logs();
        }
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        // Connection settings
        ui.heading("Connection Settings");
        ui.add_space(8.0);
        
        ui.label("Proxy Address: 127.0.0.1:8100");
        ui.label("Configure your browser to use this address for HTTP/HTTPS proxy.");
        
        ui.add_space(16.0);
        
        ui.collapsing("Browser Setup Instructions", |ui| {
            ui.heading("Chrome / Edge");
            ui.label("1. Open Settings -> Advanced -> System -> Open your computer's proxy settings");
            ui.label("2. In Windows, switch 'Use a proxy server' to ON");
            ui.label("3. Set Address to 127.0.0.1 and Port to 8100");
            ui.label("4. Click Save");
            
            ui.add_space(8.0);
            
            ui.heading("Firefox");
            ui.label("1. Open Settings -> General -> Network Settings");
            ui.label("2. Select 'Manual proxy configuration'");
            ui.label("3. Set HTTP Proxy to 127.0.0.1 and Port to 8100");
            ui.label("4. Check 'Also use this proxy for HTTPS'");
            ui.label("5. Click OK");
        });
    }

    fn render_about(&mut self, ui: &mut Ui) {
        ui.heading("About DeTrack Proxy");
        ui.add_space(16.0);
        
        ui.label("DeTrack Proxy is a privacy-focused HTTP/HTTPS proxy that blocks trackers and ads.");
        ui.label("Version: 0.1.0");
        ui.add_space(8.0);
        
        ui.horizontal(|ui| {
            ui.label("Source code:");
            ui.hyperlink("https://github.com/nonceferatu/DeTrack-proxy");
        });
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        ui.heading("Features");
        ui.add_space(8.0);
        
        ui.label("• Block known trackers and ad servers");
        ui.label("• View and filter HTTP request logs");
        ui.label("• Customize blocking rules");
        ui.label("• Minimal performance impact");
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        ui.heading("Setup Instructions");
        ui.add_space(8.0);
        
        ui.label("1. Set your browser's HTTP and HTTPS proxy to 127.0.0.1:8100");
        ui.label("2. Enable the proxy using the controls in the Dashboard tab");
        ui.label("3. Browse the web with reduced tracking!");
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        ui.heading("Credits");
        ui.add_space(8.0);
        
        ui.label("DeTrack Proxy uses a curated list of known trackers and ad servers.");
        ui.label("Special thanks to the open source projects that made this possible.");
    }

    fn render_ai_tab(&mut self, ui: &mut Ui) {
        ui.heading("AI Tracker Detection");
        ui.add_space(16.0);
        
        // AI Status
        let enabled = self.state.is_ai_detection_enabled();
        ui.horizontal(|ui| {
            ui.label("AI Detection Status:");
            let status_text = if enabled {
                RichText::new("Enabled").color(Color32::GREEN)
            } else {
                RichText::new("Disabled").color(Color32::RED)
            };
            ui.label(status_text);
        });
        
        ui.add_space(8.0);
        
        // AI Controls
        if ui.button(if enabled { "🔴 Disable AI" } else { "🟢 Enable AI" }).clicked() {
            if enabled {
                self.state.disable_ai_detection();
            } else {
                self.state.enable_ai_detection();
            }
        }
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        // AI Sensitivity
        ui.heading("AI Sensitivity");
        ui.add_space(8.0);
        
        let mut threshold = self.state.get_ai_confidence_threshold();
        ui.horizontal(|ui| {
            ui.label("Detection Threshold:");
            if ui.add(egui::Slider::new(&mut threshold, 0.0..=1.0).text("Confidence")).changed() {
                self.state.set_ai_confidence_threshold(threshold);
            }
        });
        
        ui.label(
            if threshold < 0.4 {
                "Low threshold: More trackers detected but higher chance of false positives"
            } else if threshold > 0.7 {
                "High threshold: Only high-confidence trackers detected, fewer false positives"
            } else {
                "Balanced threshold: Moderate detection with reasonable accuracy"
            }
        );
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        // AI Statistics
        ui.heading("AI Detection Statistics");
        ui.add_space(8.0);
        
        let (detections, false_positives, false_negatives) = self.state.get_ai_stats();
        
        egui::Grid::new("ai_stats_grid").num_columns(2).spacing([40.0, 8.0]).show(ui, |ui| {
            ui.label("Total Detections:");
            ui.label(format!("{}", detections));
            ui.end_row();
            
            ui.label("False Positives (Rejected):");
            ui.label(format!("{}", false_positives));
            ui.end_row();
            
            ui.label("False Negatives (Manually Added):");
            ui.label(format!("{}", false_negatives));
            ui.end_row();
            
            ui.label("Accuracy:");
            let accuracy = if detections + false_negatives > 0 {
                100.0 - (false_positives as f32 / (detections + false_negatives) as f32 * 100.0)
            } else {
                100.0
            };
            ui.label(format!("{:.1}%", accuracy));
            ui.end_row();
        });
        
        if ui.button("Reset Statistics").clicked() {
            self.state.reset_ai_stats();
        }
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        // AI Suggested Trackers
        ui.heading("AI-Suggested Trackers");
        ui.add_space(8.0);
        
        let suggestions = self.state.get_ai_suggested_trackers();
        
        ui.label(format!("Pending suggestions: {}", suggestions.len()));
        
        if suggestions.is_empty() {
            ui.label("No suggestions yet. AI will suggest trackers as it detects them.");
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for domain in &suggestions {
                    ui.horizontal(|ui| {
                        ui.label(domain);
                        
                        if ui.button("✅ Approve").clicked() {
                            if let Err(e) = self.state.approve_ai_suggestion(domain) {
                                self.state.append_log(format!("❌ Error approving suggestion: {}", e));
                            }
                        }
                        
                        if ui.button("❌ Reject").clicked() {
                            self.state.reject_ai_suggestion(domain);
                        }
                    });
                }
            });
            
            if ui.button("Clear All Suggestions").clicked() {
                self.state.clear_ai_suggested_trackers();
            }
        }
        
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
        
        // Explanation of AI detection
        ui.heading("How AI Detection Works");
        ui.add_space(8.0);
        
        ui.label("The AI detection system uses fingerprinting and heuristics to identify trackers:");
        ui.add_space(4.0);
        
        egui::Grid::new("ai_features_grid").num_columns(2).spacing([20.0, 8.0]).show(ui, |ui| {
            ui.label("• Tracking Parameters");
            ui.label("Detects common tracking query parameters (UTM, fbclid, etc.)");
            ui.end_row();
            
            ui.label("• Domain Entropy");
            ui.label("Identifies randomly generated domains common in tracking networks");
            ui.end_row();
            
            ui.label("• Path Analysis");
            ui.label("Recognizes suspicious paths like /pixel, /beacon, /track");
            ui.end_row();
            
            ui.label("• Third-Party Status");
            ui.label("Detects resources loaded from domains different than the page");
            ui.end_row();
            
            ui.label("• Keyword Detection");
            ui.label("Identifies tracking-related terms in URLs and paths");
            ui.end_row();
        });
        
        ui.add_space(8.0);
        ui.label("When potential trackers are detected, they're added to the suggestion queue above for your review.");
    }
}

impl App for RequestViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // Load the logo texture if not already loaded
        if self.logo_texture.is_none() {
            let logo_path = Path::new("assets/DeTrack_logo.png");
            if logo_path.exists() {
                // Load image using the image crate
                if let Ok(img) = image::open(logo_path) {
                    let img_rgba8 = img.to_rgba8();
                    let size = [img_rgba8.width() as _, img_rgba8.height() as _];
                    
                    // Create a Vec<u8> to hold the image data
                    let image_data = img_rgba8.as_raw().to_vec();
                    
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        size,
                        &image_data,
                    );
                    
                    // Create texture handle
                    let texture = ctx.load_texture(
                        "logo",
                        color_image,
                        Default::default(),
                    );
                    
                    self.logo_texture = Some(texture);
                }
            }
        }

        // Force a repaint to update UI frequently
        ctx.request_repaint_after(Duration::from_millis(500));
        
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Display logo if loaded - fixed for egui 0.31.1
                if let Some(texture) = &self.logo_texture {
                    // For egui 0.31.1, we need to create a tuple of (TextureId, Vec2)
                    let image_source = (texture.id(), egui::vec2(32.0, 32.0));
                    ui.add(egui::Image::new(image_source));
                    ui.add_space(8.0);
                }
                
                ui.heading("DeTrack Proxy");
                ui.add_space(32.0);
                
                // Navigation tabs
                ui.selectable_value(&mut self.selected_tab, Tab::Dashboard, "📊 Dashboard");
                ui.selectable_value(&mut self.selected_tab, Tab::Logs, "📝 Logs");
                ui.selectable_value(&mut self.selected_tab, Tab::BlockList, "🚫 Blocklist");
                ui.selectable_value(&mut self.selected_tab, Tab::AI, "🔍 AI");
                ui.selectable_value(&mut self.selected_tab, Tab::Settings, "🔧 Settings");
                ui.selectable_value(&mut self.selected_tab, Tab::About, "❓ About");
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let enabled = self.state.is_proxy_enabled();
                    let color = if enabled { Color32::GREEN } else { Color32::RED };
                    let status = if enabled { "Running" } else { "Stopped" };
                    ui.colored_label(color, status);
                    ui.label("Status:");
                });
            });
        });
        
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Proxy Address: 127.0.0.1:8100");
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Show AI status if enabled
                    if self.state.is_ai_detection_enabled() {
                        // Show number of AI suggestions if any
                        let suggestions = self.state.get_ai_suggested_trackers();
                        if !suggestions.is_empty() {
                            ui.label(RichText::new(format!("🤖 {} suggestions", suggestions.len()))
                                .color(Color32::LIGHT_BLUE));
                        } else {
                            ui.label(RichText::new("🤖 AI Active").color(Color32::LIGHT_BLUE));
                        }
                    }
                    
                    // Get domain stats from logs (simple approach)
                    let domain_count = match self.state.get_stats().len() {
                        0 => "No domains tracked yet".to_string(),
                        count => format!("{} domains tracked", count),
                    };
                    ui.label(domain_count);
                    
                    // Get request counts
                    let logs = self.state.get_logs();
                    if !logs.is_empty() {
                        ui.label(format!("{} logs", logs.len()));
                    }
                });
            });
        });
        
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.selected_tab {
                Tab::Dashboard => self.render_dashboard(ui),
                Tab::Logs => self.render_logs(ui),
                Tab::BlockList => self.render_blocklist(ui),
                Tab::Settings => self.render_settings(ui),
                Tab::About => self.render_about(ui),
                Tab::AI => self.render_ai_tab(ui),
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    // Setup the tracker blocker and shared state
    let blocker = TrackerBlocker::new("tracker_lists/test_trackers.txt")
        .expect("Failed to load tracker list");
    
    // Print loaded trackers for debugging
    blocker.print_loaded_trackers();
    
    let state = Arc::new(SharedState::new(blocker));

    // Start proxy in background thread with Tokio runtime
    let state_for_proxy = Arc::clone(&state);
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        if let Err(e) = rt.block_on(run_proxy(state_for_proxy)) {
            eprintln!("❌ Proxy failed to start: {:?}", e);
        }
    });

    // Launch the egui desktop app with correct options for the newer eframe version
    let mut native_options = eframe::NativeOptions::default();
    
    // Icon loading logic
    let icon_data = match image::open("assets/DeTrack_logo.png") {
        Ok(img) => {
            let img_rgba8 = img.to_rgba8();
            let width = img_rgba8.width();
            let height = img_rgba8.height();
            let rgba = img_rgba8.into_raw();
            
            Some(Arc::new(egui::IconData {
                width,
                height,
                rgba,
            }))
        },
        Err(_) => None
    };
    
    // Set the icon if loaded successfully
    native_options.viewport.icon = icon_data;

    // Set viewport options
    native_options.viewport.inner_size = Some(egui::vec2(900.0, 650.0));
    native_options.viewport.min_inner_size = Some(egui::vec2(600.0, 400.0));
    
    // Set window title and other basic properties
    native_options.viewport.title = Some("DeTrack Proxy".to_string());
    
    eframe::run_native(
        "DeTrack Proxy",
        native_options,
        Box::new(|_cc: &CreationContext| {
            Ok(Box::new(RequestViewerApp::new(Arc::clone(&state))))
        }),
    )
}