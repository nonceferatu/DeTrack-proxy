use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::tracker_blocker::TrackerBlocker;

/// Statistics for a specific domain
#[derive(Clone, Debug)]
pub struct DomainStat {
    pub domain: String,
    pub requests: usize,
    pub blocked: usize,
    pub last_seen: DateTime<Utc>,
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
    
    // Statistics methods
    
    pub fn record_request(&self, domain: &str, blocked: bool) {
        // Update domain stats
        if let Ok(mut stats) = self.stats.lock() {
            let entry = stats.entry(domain.to_string()).or_insert_with(|| DomainStat {
                domain: domain.to_string(),
                requests: 0,
                blocked: 0,
                last_seen: Utc::now(),
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
}