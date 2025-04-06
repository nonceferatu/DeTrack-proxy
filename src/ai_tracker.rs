use url::Url;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

/// AI Tracker Detection module for DeTrack Proxy
/// Uses fingerprinting and heuristic methods to identify potential trackers
#[derive(Debug, Clone)]
pub struct AITracker {
    // Configuration
    enabled: bool,
    confidence_threshold: f32,
    
    // Model parameters (would be learned/tuned over time)
    feature_weights: FeatureWeights,
    
    // Learning data
    known_trackers: Vec<String>,
    known_legitimate: Vec<String>,
    
    // Cache for previous decisions to improve performance
    decision_cache: HashMap<String, bool>,
    
    // Statistics
    detection_count: usize,
    false_positive_count: usize,
    false_negative_count: usize,
}

#[derive(Debug, Clone)]
struct FeatureWeights {
    tracking_param_weight: f32,
    suspicious_path_weight: f32,
    numeric_id_weight: f32,
    domain_entropy_weight: f32,
    third_party_weight: f32,
    suspicious_keywords_weight: f32,
    path_depth_weight: f32,
    query_count_weight: f32,
}

impl Default for FeatureWeights {
    fn default() -> Self {
        Self {
            tracking_param_weight: 0.7,
            suspicious_path_weight: 0.5,
            numeric_id_weight: 0.3,
            domain_entropy_weight: 0.4,
            third_party_weight: 0.6,
            suspicious_keywords_weight: 0.8,
            path_depth_weight: 0.2,
            query_count_weight: 0.3,
        }
    }
}

#[derive(Debug)]
struct RequestFeatures {
    has_tracking_params: bool,
    has_suspicious_path: bool,
    has_numeric_id: bool,
    domain_entropy: f32,
    is_third_party: bool,
    has_suspicious_keywords: bool,
    path_depth: usize,
    query_param_count: usize,
}

impl AITracker {
    /// Create a new AI tracker detector
    pub fn new() -> Self {
        Self {
            enabled: true,
            confidence_threshold: 0.65,
            feature_weights: FeatureWeights::default(),
            known_trackers: Vec::new(),
            known_legitimate: Vec::new(),
            decision_cache: HashMap::new(),
            detection_count: 0,
            false_positive_count: 0,
            false_negative_count: 0,
        }
    }
    
    /// Load AI tracker from file - simplified version without serde
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        // Skip loading for now - just create a new instance
        // In a real implementation, you would parse a custom format here
        println!("Note: AI model loading from file not implemented in this version");
        Ok(Self::new())
    }
    
    /// Save AI tracker to file - simplified version without serde
    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        // Skip saving for now
        // In a real implementation, you would serialize to a custom format here
        println!("Note: AI model saving to file not implemented in this version");
        Ok(())
    }
    
    /// Enable AI detection
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    
    /// Disable AI detection
    pub fn disable(&mut self) {
        self.enabled = false;
    }
    
    /// Check if AI detection is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    /// Set confidence threshold (0.0 to 1.0)
    pub fn set_confidence_threshold(&mut self, threshold: f32) {
        self.confidence_threshold = threshold.max(0.0).min(1.0);
    }
    
    /// Get current confidence threshold
    pub fn get_confidence_threshold(&self) -> f32 {
        self.confidence_threshold
    }
    
    /// Analyze if a request is likely a tracker
    pub fn is_likely_tracker(&mut self, url: &str, host: &str, referer: Option<&str>) -> bool {
        if !self.enabled {
            return false;
        }
        
        // Check cache first for performance
        if let Some(&decision) = self.decision_cache.get(url) {
            return decision;
        }
        
        // Check if it's a known tracker
        if self.known_trackers.contains(&host.to_string()) {
            self.decision_cache.insert(url.to_string(), true);
            self.detection_count += 1;
            return true;
        }
        
        // Check if it's known to be legitimate
        if self.known_legitimate.contains(&host.to_string()) {
            self.decision_cache.insert(url.to_string(), false);
            return false;
        }
        
        // Extract features from the request
        let features = self.extract_features(url, host, referer);
        
        // Calculate confidence score
        let confidence = self.calculate_confidence(&features);
        
        // Make decision based on confidence threshold
        let is_tracker = confidence >= self.confidence_threshold;
        
        // Cache the decision
        self.decision_cache.insert(url.to_string(), is_tracker);
        
        // Update statistics if it's a tracker
        if is_tracker {
            self.detection_count += 1;
        }
        
        is_tracker
    }
    
    /// Report a false positive (something that was marked as tracker but isn't)
    pub fn report_false_positive(&mut self, domain: &str) {
        self.false_positive_count += 1;
        
        // Add to known legitimate domains
        if !self.known_legitimate.contains(&domain.to_string()) {
            self.known_legitimate.push(domain.to_string());
        }
        
        // Remove from known trackers if present
        self.known_trackers.retain(|d| d != domain);
        
        // Clear cache entry
        self.decision_cache.remove(domain);
    }
    
    /// Report a false negative (something that wasn't marked as tracker but is)
    pub fn report_false_negative(&mut self, domain: &str) {
        self.false_negative_count += 1;
        
        // Add to known trackers
        if !self.known_trackers.contains(&domain.to_string()) {
            self.known_trackers.push(domain.to_string());
        }
        
        // Remove from known legitimate if present
        self.known_legitimate.retain(|d| d != domain);
        
        // Clear cache entry
        self.decision_cache.remove(domain);
    }
    
    /// Extract features from a request
    fn extract_features(&self, url: &str, host: &str, referer: Option<&str>) -> RequestFeatures {
        // Parse URL
        let parsed_url = match Url::parse(url) {
            Ok(url) => url,
            Err(_) => return RequestFeatures {
                has_tracking_params: false,
                has_suspicious_path: false,
                has_numeric_id: false,
                domain_entropy: 0.0,
                is_third_party: false,
                has_suspicious_keywords: false,
                path_depth: 0,
                query_param_count: 0,
            },
        };
        
        // Check for tracking parameters
        let query_pairs: Vec<_> = parsed_url.query_pairs().collect();
        let has_tracking_params = query_pairs.iter().any(|(k, _)| {
            let key = k.to_lowercase();
            key.contains("utm_") || 
            key.contains("fbclid") || 
            key.contains("gclid") || 
            key.contains("msclkid") || 
            key.contains("dclid") ||
            key.contains("twclid") ||
            key.contains("_ga") ||
            key.contains("ref")
        });
        
        // Count query parameters
        let query_param_count = query_pairs.len();
        
        // Check path for suspicious patterns
        let path = parsed_url.path();
        let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let path_depth = path_segments.len();
        
        // Check for numeric IDs
        let has_numeric_id = path_segments.iter()
            .any(|segment| segment.chars().all(|c| c.is_numeric()) && segment.len() > 5);
        
        // Check for suspicious path patterns
        let has_suspicious_path = path.contains("/pixel") || 
                                path.contains("/track") || 
                                path.contains("/collect") ||
                                path.contains("/beacon") ||
                                path.contains("/1x1.gif") ||
                                path.contains("/1x1.png") ||
                                path.contains("/impression");
        
        // Calculate domain entropy (more random = more likely to be a tracker)
        let domain_entropy = Self::calculate_entropy(host);
        
        // Check if it's a third-party request
        let is_third_party = match referer {
            Some(referer_url) => {
                if let Ok(referer_parsed) = Url::parse(referer_url) {
                    if let Some(referer_host) = referer_parsed.host_str() {
                        !host.ends_with(referer_host) && !referer_host.ends_with(host)
                    } else {
                        true
                    }
                } else {
                    true
                }
            },
            None => false, // Can't determine without referer
        };
        
        // Check for suspicious keywords
        let url_lower = url.to_lowercase();
        let has_suspicious_keywords = ["analytics", "tracker", "pixel", "stat", "metrics", "telemetry", "beacon", "counter"]
            .iter()
            .any(|&keyword| url_lower.contains(keyword));
            
        RequestFeatures {
            has_tracking_params,
            has_suspicious_path,
            has_numeric_id,
            domain_entropy,
            is_third_party,
            has_suspicious_keywords,
            path_depth,
            query_param_count,
        }
    }
    
    /// Calculate confidence score based on features
    fn calculate_confidence(&self, features: &RequestFeatures) -> f32 {
        let mut confidence = 0.0;
        
        // Add weighted feature contributions
        if features.has_tracking_params {
            confidence += self.feature_weights.tracking_param_weight;
        }
        
        if features.has_suspicious_path {
            confidence += self.feature_weights.suspicious_path_weight;
        }
        
        if features.has_numeric_id {
            confidence += self.feature_weights.numeric_id_weight;
        }
        
        // Normalize entropy to 0-1 and add contribution
        let normalized_entropy = (features.domain_entropy / 4.5).min(1.0);
        confidence += normalized_entropy * self.feature_weights.domain_entropy_weight;
        
        if features.is_third_party {
            confidence += self.feature_weights.third_party_weight;
        }
        
        if features.has_suspicious_keywords {
            confidence += self.feature_weights.suspicious_keywords_weight;
        }
        
        // Path depth - normalize to 0-1 range with diminishing returns
        let normalized_path_depth = (features.path_depth as f32 / 10.0).min(1.0);
        confidence += normalized_path_depth * self.feature_weights.path_depth_weight;
        
        // Query parameter count - normalize to 0-1 range with diminishing returns
        let normalized_query_count = (features.query_param_count as f32 / 20.0).min(1.0);
        confidence += normalized_query_count * self.feature_weights.query_count_weight;
        
        // Normalize final confidence to 0-1 range
        confidence = (confidence / 3.0).min(1.0);
        
        confidence
    }
    
    /// Calculate Shannon entropy of a string
    fn calculate_entropy(text: &str) -> f32 {
        let text = text.to_lowercase();
        let len = text.len() as f32;
        
        if len == 0.0 {
            return 0.0;
        }
        
        let mut char_counts = HashMap::new();
        
        // Count occurrences of each character
        for c in text.chars() {
            *char_counts.entry(c).or_insert(0) += 1;
        }
        
        // Calculate entropy
        let mut entropy = 0.0;
        for &count in char_counts.values() {
            let probability = count as f32 / len;
            entropy -= probability * probability.log2();
        }
        
        entropy
    }
    
    /// Get statistics
    pub fn get_stats(&self) -> (usize, usize, usize) {
        (self.detection_count, self.false_positive_count, self.false_negative_count)
    }
    
    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.detection_count = 0;
        self.false_positive_count = 0;
        self.false_negative_count = 0;
    }
    
    /// Get list of detected trackers
    pub fn get_detected_domains(&self) -> Vec<String> {
        self.decision_cache.iter()
            .filter(|(_, &is_tracker)| is_tracker)
            .map(|(domain, _)| domain.clone())
            .collect()
    }
    
    /// Clear the decision cache
    pub fn clear_cache(&mut self) {
        self.decision_cache.clear();
    }
}

impl Default for AITracker {
    fn default() -> Self {
        Self::new()
    }
}