use std::collections::HashSet;
use std::fs;
use std::io::{self}; 
use std::path::{Path, PathBuf};
use chrono::Local;
use url::Url;

pub struct TrackerBlocker {
    trackers: HashSet<String>,
    tracker_file_path: PathBuf,
    tracking_params: HashSet<String>,
}

impl TrackerBlocker {
    /// Create a new TrackerBlocker from a file path
    /// 
    /// # Arguments
    /// * `tracker_file` - Path to the tracker list file
    /// 
    /// # Behavior
    /// - If file doesn't exist, creates an empty file
    /// - Loads trackers, ignoring empty lines and comments
    /// - Converts trackers to lowercase
    pub fn new<P: AsRef<Path>>(tracker_file: P) -> std::io::Result<Self> {
        let file_path = tracker_file.as_ref().to_path_buf();
        
        // Ensure directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Read file content, create if not exists
        let content = match fs::read_to_string(&file_path) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                fs::write(&file_path, "")?;
                String::new()
            },
            Err(e) => return Err(e),
        };
        
        // Parse trackers, ignoring comments and empty lines
        let trackers = content
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty() && !line.starts_with('#')
            })
            .map(|line| line.trim().to_lowercase())
            .collect();
        
        // Predefined tracking parameters
        let tracking_params = [
            "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
            "fbclid", "gclid", "msclkid", "dclid", "twclid", 
            "_ga", "_hsenc", "_openstat", "ref", "referrer", "source",
            "mc_cid", "mc_eid", // Mailchimp
            "wickedid", // Wicked Reports
            "yclid", // Yandex
        ].iter().map(|&s| s.to_string()).collect();

        Ok(Self { 
            trackers,
            tracker_file_path: file_path,
            tracking_params,
        })
    }

    /// Check if a host is blocked
    /// 
    /// # Behavior
    /// - If no trackers are loaded, nothing is blocked
    /// - Checks for exact and subdomain matches
    pub fn is_blocked(&self, host: &str) -> bool {
        if self.trackers.is_empty() {
            return false;
        }
        
        let host = host.to_lowercase();
        
        // Exact match
        if self.trackers.contains(&host) {
            println!("🚫 Blocked exact match: {}", host);
            return true;
        }
        
        // Domain suffix matches
        for tracker in &self.trackers {
            if host.ends_with(&format!(".{}", tracker)) {
                println!("🚫 Blocked domain suffix match: {} (matches {})", host, tracker);
                return true;
            }
        }
        
        println!("✅ Allowed: {}", host);
        false
    }
    
    /// Add a new tracker to the list
    pub fn add_tracker(&mut self, domain: &str) -> io::Result<()> {
        let domain = domain.trim().to_lowercase();
        
        // Don't add if it already exists
        if self.trackers.contains(&domain) {
            return Ok(());
        }
        
        // Add to in-memory set
        self.trackers.insert(domain.clone());
        
        // Save to file
        self.save_trackers()
    }
    
    /// Remove a tracker from the list
    pub fn remove_tracker(&mut self, domain: &str) -> io::Result<()> {
        let domain = domain.trim().to_lowercase();
        
        // Remove from in-memory set
        self.trackers.remove(&domain);
        
        // Save to file
        self.save_trackers()
    }
    
    /// Save current tracker list to file
    fn save_trackers(&self) -> io::Result<()> {
        // Sort trackers for consistent file format
        let mut sorted_trackers: Vec<&String> = self.trackers.iter().collect();
        sorted_trackers.sort();
        
        // Prepare file content with header
        let content = format!(
            "# Tracker list for DeTrack Proxy\n\
             # Updated: {}\n\
             # Format: One domain per line\n\
             {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            sorted_trackers.iter().map(|s| s.as_str()).collect::<Vec<&str>>().join("\n")
        );
        
        // Write to file
        fs::write(&self.tracker_file_path, content)
    }
    
    /// Get a sorted vector of all trackers
    pub fn get_trackers(&self) -> Vec<String> {
        let mut trackers: Vec<String> = self.trackers.iter().cloned().collect();
        trackers.sort();
        trackers
    }
    
    /// Get the number of trackers
    pub fn tracker_count(&self) -> usize {
        self.trackers.len()
    }
    
    /// Print all loaded trackers (for debugging)
    pub fn print_loaded_trackers(&self) {
        println!("====== Loaded Trackers: ======");
        println!("Total trackers: {}", self.trackers.len());
        
        let mut sorted_trackers: Vec<&String> = self.trackers.iter().collect();
        sorted_trackers.sort();
        
        for tracker in sorted_trackers {
            println!("  - {}", tracker);
        }
        println!("==============================");
    }
    
    /// Import trackers from another file
    pub fn import_trackers<P: AsRef<Path>>(&mut self, import_file: P) -> io::Result<usize> {
        let content = fs::read_to_string(import_file)?;
        
        let mut added_count = 0;
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            let domain = line.to_lowercase();
            if !self.trackers.contains(&domain) {
                self.trackers.insert(domain);
                added_count += 1;
            }
        }
        
        // Only save if we added any
        if added_count > 0 {
            self.save_trackers()?;
        }
        
        Ok(added_count)
    }
    
    /// Export trackers to another file
    pub fn export_trackers<P: AsRef<Path>>(&self, export_file: P) -> io::Result<usize> {
        let mut sorted_trackers: Vec<&String> = self.trackers.iter().collect();
        sorted_trackers.sort();
        
        // Prepare file content
        let content = format!(
            "# Exported tracker list from DeTrack Proxy\n\
             # Exported: {}\n\
             # Total domains: {}\n\
             {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            sorted_trackers.len(),
            sorted_trackers.iter().map(|s| s.as_str()).collect::<Vec<&str>>().join("\n")
        );
        
        // Write to file
        fs::write(export_file, content)?;
        
        Ok(sorted_trackers.len())
    }

    /// Check if a parameter is a tracking parameter
    pub fn is_tracking_parameter(&self, param_name: &str) -> bool {
        self.tracking_params.contains(&param_name.to_lowercase())
    }

    /// Clean URL by removing tracking parameters
    pub fn clean_url(&self, url_str: &str) -> String {
        match Url::parse(url_str) {
            Ok(mut parsed_url) => {
                // Get existing query parameters
                let mut new_query_pairs = Vec::new();
                let pairs = parsed_url.query_pairs();
                
                // Filter out tracking parameters
                for (key, value) in pairs {
                    if !self.is_tracking_parameter(&key) {
                        new_query_pairs.push((key.to_string(), value.to_string()));
                    }
                }
                
                // Clear existing query
                parsed_url.set_query(None);
                
                // Add back non-tracking parameters
                if !new_query_pairs.is_empty() {
                    let query_string = new_query_pairs
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<String>>()
                        .join("&");
                        
                    parsed_url.set_query(Some(&query_string));
                }
                
                parsed_url.to_string()
            },
            Err(_) => {
                // Return original if parsing fails
                url_str.to_string()
            }
        }
    }
}

// Optional: Implement Default for easier initialization
impl Default for TrackerBlocker {
    fn default() -> Self {
        // Attempt to create with a default tracker list file
        Self::new("trackers.txt").unwrap_or_else(|_| Self {
            trackers: HashSet::new(),
            tracker_file_path: PathBuf::from("trackers.txt"),
            tracking_params: [
                "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
                "fbclid", "gclid", "msclkid", "dclid", "twclid", 
                "_ga", "_hsenc", "_openstat", "ref", "referrer", "source",
            ].iter().map(|&s| s.to_string()).collect(),
        })
    }
}