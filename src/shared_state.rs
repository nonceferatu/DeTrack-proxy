use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::tracker_blocker::TrackerBlocker;
use crate::ai_tracker::AITracker;

/// Statistics for a specific domain
#[derive(Clone, Debug)]
pub struct DomainStat {
    pub domain: String,
    pub requests: usize,
    pub blocked: usize,
    pub last_seen: DateTime<Utc>,
    pub bandwidth_saved: Arc<Mutex<u64>>,
}

/// Shared state between the proxy and the UI.
/// This is safe to clone and pass around because of Arc.
#[derive(Clone)]
pub struct SharedState {
    /// Whether the proxy is currently enabled.
    pub proxy_enabled: Arc<Mutex<bool>>,

    /// Whether request logging is currently enabled.
    pub log_enabled: Arc<Mutex<bool>>,

    /// Request logs storage
    pub logs: Arc<Mutex<Vec<String>>>,

    /// The active tracker blocker instance.
    pub blocker: Arc<Mutex<TrackerBlocker>>,

    /// Statistics about requests
    pub stats: Arc<Mutex<HashMap<String, DomainStat>>>,

    /// Total allowed requests
    pub allowed_count: Arc<Mutex<usize>>,

    /// Total blocked requests
    pub blocked_count: Arc<Mutex<usize>>,

    /// AI tracker for heuristic detection
    pub ai_tracker: Arc<Mutex<AITracker>>,
    
    /// AI-suggested trackers pending user review
    pub ai_suggested_trackers: Arc<Mutex<Vec<String>>>,

    /// Total bandwidth saved by blocking trackers
    pub bandwidth_saved: Arc<Mutex<u64>>, 
}

impl SharedState {
    pub fn new(blocker: TrackerBlocker) -> Self {
        Self {
            proxy_enabled: Arc::new(Mutex::new(true)),
            log_enabled: Arc::new(Mutex::new(true)),
            blocker: Arc::new(Mutex::new(blocker)),
            logs: Arc::new(Mutex::new(vec![])),
            stats: Arc::new(Mutex::new(HashMap::new())),
            allowed_count: Arc::new(Mutex::new(0)),
            blocked_count: Arc::new(Mutex::new(0)),
            ai_tracker: Arc::new(Mutex::new(AITracker::new())),
            ai_suggested_trackers: Arc::new(Mutex::new(Vec::new())),
            bandwidth_saved: Arc::new(Mutex::new(0)),
        }
    }

    // Proxy toggle
    pub fn enable_proxy(&self) {
        if let Ok(mut enabled) = self.proxy_enabled.lock() {
            *enabled = true;
        }
        self.append_log("▶️ Proxy enabled".to_string());
    }

    pub fn disable_proxy(&self) {
        if let Ok(mut enabled) = self.proxy_enabled.lock() {
            *enabled = false;
        }
        self.append_log("🛑 Proxy disabled".to_string());
    }

    pub fn is_proxy_enabled(&self) -> bool {
        self.proxy_enabled.lock().map(|v| *v).unwrap_or(false)
    }

    // Log toggle
    pub fn enable_logging(&self) {
        if let Ok(mut enabled) = self.log_enabled.lock() {
            *enabled = true;
        }
        self.append_log("📡 Logging enabled".to_string());
    }

    pub fn disable_logging(&self) {
        if let Ok(mut enabled) = self.log_enabled.lock() {
            *enabled = false;
        }
        self.append_log("📴 Logging disabled".to_string());
    }

    pub fn is_logging_enabled(&self) -> bool {
        self.log_enabled.lock().map(|v| *v).unwrap_or(false)
    }

    pub fn append_log(&self, entry: String) {
        let mut logs = match self.logs.lock() {
            Ok(logs) => logs,
            Err(_) => return, // Handle poisoned mutex
        };
        
        // Add timestamp to log entry
        let now = chrono::Local::now();
        let timestamped_entry = format!("[{}] {}", now.format("%H:%M:%S"), entry);
        
        logs.push(timestamped_entry);
        
        // Limit log size to prevent memory issues
        if logs.len() > 10000 {
            logs.remove(0); // Remove oldest log
        }
    }

    pub fn get_logs(&self) -> Vec<String> {
        match self.logs.lock() {
            Ok(logs) => logs.clone(),
            Err(_) => vec![], // Return empty vector on error
        }
    }

    pub fn clear_logs(&self) {
        if let Ok(mut logs) = self.logs.lock() {
            logs.clear();
        }
        self.append_log("🧹 Logs cleared".to_string());
    }

    // Method to track bandwidth
    pub fn track_bandwidth(&self, bytes: u64, blocked: bool) {
        if blocked {
            if let Ok(mut saved) = self.bandwidth_saved.lock() {
                *saved += bytes;
            }
        }
    }

    // Method to get total bandwidth saved
    pub fn get_bandwidth_saved(&self) -> u64 {
        self.bandwidth_saved.lock().map(|s| *s).unwrap_or(0)
    }
    
    
    // Statistics methods
    
    pub fn record_request(&self, domain: &str, blocked: bool) {
        // Update domain stats
        if let Ok(mut stats) = self.stats.lock() {
            let entry = stats.entry(domain.to_string()).or_insert_with(|| DomainStat {
                domain: domain.to_string(),
                requests: 0,
                blocked: 0,
                last_seen: Utc::now(),
                bandwidth_saved: Arc::new(Mutex::new(0)), 
            });
            
            entry.requests += 1;
            if blocked {
                entry.blocked += 1;
            }
            entry.last_seen = Utc::now();
        }
        
        // Update global counters
        if blocked {
            if let Ok(mut count) = self.blocked_count.lock() {
                *count += 1;
            }
        } else {
            if let Ok(mut count) = self.allowed_count.lock() {
                *count += 1;
            }
        }
    }
    
    pub fn get_stats(&self) -> HashMap<String, DomainStat> {
        match self.stats.lock() {
            Ok(stats) => stats.clone(),
            Err(_) => HashMap::new(), // Return empty map on error
        }
    }
    
    pub fn get_allowed_count(&self) -> usize {
        self.allowed_count.lock().map(|v| *v).unwrap_or(0)
    }
    
    pub fn get_blocked_count(&self) -> usize {
        self.blocked_count.lock().map(|v| *v).unwrap_or(0)
    }
    
    pub fn reset_stats(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            stats.clear();
        }
        
        if let Ok(mut count) = self.allowed_count.lock() {
            *count = 0;
        }
        
        if let Ok(mut count) = self.blocked_count.lock() {
            *count = 0;
        }
        
        self.append_log("📊 Statistics reset".to_string());
    }
    
    // Tracker management methods
    
    pub fn add_tracker(&self, domain: &str) -> Result<(), String> {
        if let Ok(mut blocker) = self.blocker.lock() {
            match blocker.add_tracker(domain) {
                Ok(()) => {
                    self.append_log(format!("➕ Added tracker: {}", domain));
                    Ok(())
                },
                Err(e) => Err(format!("Failed to add tracker: {}", e)),
            }
        } else {
            Err("Failed to lock blocker".to_string())
        }
    }
    
    pub fn remove_tracker(&self, domain: &str) -> Result<(), String> {
        if let Ok(mut blocker) = self.blocker.lock() {
            match blocker.remove_tracker(domain) {
                Ok(()) => {
                    self.append_log(format!("➖ Removed tracker: {}", domain));
                    Ok(())
                },
                Err(e) => Err(format!("Failed to remove tracker: {}", e)),
            }
        } else {
            Err("Failed to lock blocker".to_string())
        }
    }
    
    pub fn get_trackers(&self) -> Result<Vec<String>, String> {
        if let Ok(blocker) = self.blocker.lock() {
            Ok(blocker.get_trackers())
        } else {
            Err("Failed to lock blocker".to_string())
        }
    }

    // AI tracker methods

    pub fn enable_ai_detection(&self) {
        if let Ok(mut tracker) = self.ai_tracker.lock() {
            tracker.enable();
        }
        self.append_log("🤖 AI tracker detection enabled".to_string());
    }
    
    pub fn disable_ai_detection(&self) {
        if let Ok(mut tracker) = self.ai_tracker.lock() {
            tracker.disable();
        }
        self.append_log("🤖 AI tracker detection disabled".to_string());
    }
    
    pub fn is_ai_detection_enabled(&self) -> bool {
        if let Ok(tracker) = self.ai_tracker.lock() {
            tracker.is_enabled()
        } else {
            false
        }
    }
    
    pub fn set_ai_confidence_threshold(&self, threshold: f32) {
        if let Ok(mut tracker) = self.ai_tracker.lock() {
            tracker.set_confidence_threshold(threshold);
        }
        self.append_log(format!("🤖 AI confidence threshold set to {:.2}", threshold));
    }
    
    pub fn get_ai_confidence_threshold(&self) -> f32 {
        if let Ok(tracker) = self.ai_tracker.lock() {
            tracker.get_confidence_threshold()
        } else {
            0.65 // Default
        }
    }
    
    pub fn add_ai_suggested_tracker(&self, domain: &str) {
        if let Ok(mut suggested) = self.ai_suggested_trackers.lock() {
            if !suggested.contains(&domain.to_string()) {
                suggested.push(domain.to_string());
                self.append_log(format!("🤖 Added domain to AI suggestions: {}", domain));
            }
        }
    }
    
    pub fn get_ai_suggested_trackers(&self) -> Vec<String> {
        if let Ok(suggested) = self.ai_suggested_trackers.lock() {
            suggested.clone()
        } else {
            Vec::new()
        }
    }
    
    pub fn clear_ai_suggested_trackers(&self) {
        if let Ok(mut suggested) = self.ai_suggested_trackers.lock() {
            suggested.clear();
        }
        self.append_log("🤖 Cleared AI suggested trackers".to_string());
    }
    
    pub fn approve_ai_suggestion(&self, domain: &str) -> Result<(), String> {
        // First add to blocklist
        self.add_tracker(domain)?;
        
        // Then remove from suggestions
        if let Ok(mut suggested) = self.ai_suggested_trackers.lock() {
            suggested.retain(|d| d != domain);
        }
        
        // Finally, inform the AI that its suggestion was correct
        if let Ok(mut tracker) = self.ai_tracker.lock() {
            tracker.report_false_negative(domain);
        }
        
        self.append_log(format!("✅ Approved AI-suggested tracker: {}", domain));
        Ok(())
    }
    
    pub fn reject_ai_suggestion(&self, domain: &str) {
        if let Ok(mut suggested) = self.ai_suggested_trackers.lock() {
            suggested.retain(|d| d != domain);
        }
        
        // Inform the AI that its suggestion was incorrect
        if let Ok(mut tracker) = self.ai_tracker.lock() {
            tracker.report_false_positive(domain);
        }
        
        self.append_log(format!("❌ Rejected AI-suggested tracker: {}", domain));
    }
    
    pub fn get_ai_stats(&self) -> (usize, usize, usize) {
        if let Ok(tracker) = self.ai_tracker.lock() {
            tracker.get_stats()
        } else {
            (0, 0, 0)
        }
    }
    
    pub fn reset_ai_stats(&self) {
        if let Ok(mut tracker) = self.ai_tracker.lock() {
            tracker.reset_stats();
        }
        self.append_log("🤖 Reset AI tracker statistics".to_string());
    }
    
    pub fn save_ai_model<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<()> {
        if let Ok(tracker) = self.ai_tracker.lock() {
            tracker.save(path)?;
            self.append_log("💾 Saved AI model to file".to_string());
        }
        Ok(())
    }
    
    pub fn load_ai_model<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<()> {
        if let Ok(model) = AITracker::load(path) {
            if let Ok(mut tracker) = self.ai_tracker.lock() {
                *tracker = model;
            }
            self.append_log("📂 Loaded AI model from file".to_string());
        }
        Ok(())
    }
}