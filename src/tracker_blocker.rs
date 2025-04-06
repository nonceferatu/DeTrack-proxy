use std::collections::HashSet;
use std::fs;
use std::io::{self}; // Removed unused Write import
use std::path::{Path, PathBuf};

pub struct TrackerBlocker {
    trackers: HashSet<String>,
    tracker_file_path: PathBuf,
}

impl TrackerBlocker {
    pub fn new<P: AsRef<Path>>(tracker_file: P) -> std::io::Result<Self> {
        let file_path = tracker_file.as_ref().to_path_buf();
        let content = match fs::read_to_string(&file_path) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // Create an empty file if it doesn't exist
                fs::create_dir_all(file_path.parent().unwrap_or(Path::new("./")))?;
                fs::write(&file_path, "")?;
                String::new()
            },
            Err(e) => return Err(e),
        };
        
        let trackers = content
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty() && !line.starts_with('#')
            })
            .map(|line| line.trim().to_lowercase())
            .collect();
        
        println!("✅ Loaded {} trackers from {:?}", 
                 content.lines().filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#')).count(), 
                 file_path);
        
        Ok(Self { 
            trackers,
            tracker_file_path: file_path,
        })
    }

    pub fn is_blocked(&self, host: &str) -> bool {
        if self.trackers.is_empty() {
            // If no trackers are loaded, don't block anything
            return false;
        }
        
        let host = host.to_lowercase();
        
        // Exact match
        if self.trackers.contains(&host) {
            println!("🚫 Blocked exact match: {}", host);
            return true;
        }
        
        // Check for domain suffix matches
        // e.g., if "example.com" is blocked, "sub.example.com" should also be blocked
        for tracker in &self.trackers {
            if host.ends_with(&format!(".{}", tracker)) {
                println!("🚫 Blocked domain suffix match: {} (matches {})", host, tracker);
                return true;
            }
        }
        
        println!("✅ Allowed: {}", host);
        false
    }
    
    // Add a new tracker to the list
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
    
    // Remove a tracker from the list
    pub fn remove_tracker(&mut self, domain: &str) -> io::Result<()> {
        let domain = domain.trim().to_lowercase();
        
        // Remove from in-memory set
        self.trackers.remove(&domain);
        
        // Save to file
        self.save_trackers()
    }
    
    // Save the current tracker list to file
    fn save_trackers(&self) -> io::Result<()> {
        // Sort trackers for consistent file format
        let mut sorted_trackers: Vec<&String> = self.trackers.iter().collect();
        sorted_trackers.sort();
        
        // Prepare file content
        let content = format!(
            "# Tracker list for DeTrack Proxy\n\
             # Updated: {}\n\
             # Format: One domain per line\n\
             {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            sorted_trackers.iter().map(|s| s.as_str()).collect::<Vec<&str>>().join("\n")
        );
        
        // Write to file
        fs::write(&self.tracker_file_path, content)
    }
    
    // Get a vector of all trackers
    pub fn get_trackers(&self) -> Vec<String> {
        let mut trackers: Vec<String> = self.trackers.iter().cloned().collect();
        trackers.sort();
        trackers
    }
    
    // Get the number of trackers
    pub fn tracker_count(&self) -> usize {
        self.trackers.len()
    }
    
    // Print all loaded trackers for debugging
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
    
    // Import trackers from another file
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
    
    // Export trackers to another file
    pub fn export_trackers<P: AsRef<Path>>(&self, export_file: P) -> io::Result<usize> {
        let mut sorted_trackers: Vec<&String> = self.trackers.iter().collect();
        sorted_trackers.sort();
        
        // Prepare file content
        let content = format!(
            "# Exported tracker list from DeTrack Proxy\n\
             # Exported: {}\n\
             # Total domains: {}\n\
             {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            sorted_trackers.len(),
            sorted_trackers.iter().map(|s| s.as_str()).collect::<Vec<&str>>().join("\n")
        );
        
        // Write to file
        fs::write(export_file, content)?;
        
        Ok(sorted_trackers.len())
    }
}