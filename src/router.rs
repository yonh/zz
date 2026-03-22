use rand::{rng, RngExt};
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// Model routing rule for pattern-based provider selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRule {
    pub id: String,
    pub pattern: String,
    pub target_provider: String,
}

pub enum RoutingStrategy {
    Failover,
    RoundRobin,
    WeightedRandom,
    QuotaAware,
    Manual,
}

pub struct Router {
    strategy: RoutingStrategy,
    round_robin_index: std::sync::atomic::AtomicUsize,
    pinned_provider: Option<String>,
}

impl Router {
    pub fn new(strategy: &str) -> Self {
        let strategy = match strategy {
            "round-robin" => RoutingStrategy::RoundRobin,
            "weighted-random" => RoutingStrategy::WeightedRandom,
            "quota-aware" => RoutingStrategy::QuotaAware,
            "manual" => RoutingStrategy::Manual,
            _ => RoutingStrategy::Failover,
        };
        Self {
            strategy,
            round_robin_index: std::sync::atomic::AtomicUsize::new(0),
            pinned_provider: None,
        }
    }

    pub fn with_pinned_provider(mut self, provider: Option<String>) -> Self {
        self.pinned_provider = provider;
        self
    }

    pub fn select_provider(
        &self,
        providers: &[(String, Arc<crate::provider::Provider>)],
    ) -> Option<(String, Arc<crate::provider::Provider>)> {
        if providers.is_empty() {
            return None;
        }

        match &self.strategy {
            RoutingStrategy::Failover => {
                // Sort by priority, then pick first
                let mut sorted = providers.to_vec();
                sorted.sort_by_key(|(_, p)| p.config.priority);
                sorted.first().cloned()
            }
            RoutingStrategy::RoundRobin => {
                let idx = self.round_robin_index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                providers.get(idx % providers.len()).cloned()
            }
            RoutingStrategy::WeightedRandom => {
                // Simple random selection for now
                let mut rng = rng();
                providers.get(rng.random_range(0..providers.len())).cloned()
            }
            RoutingStrategy::QuotaAware => {
                // Select provider with lowest token usage (if available)
                // For now, fall back to failover behavior
                let mut sorted = providers.to_vec();
                sorted.sort_by_key(|(_, p)| p.config.priority);
                sorted.first().cloned()
            }
            RoutingStrategy::Manual => {
                // Use pinned provider if set and available
                if let Some(ref name) = &self.pinned_provider {
                    providers.iter()
                        .find(|(n, _)| n == name)
                        .map(|(n, p)| (n.clone(), Arc::clone(p)))
                } else {
                    providers.first().cloned()
                }
            }
        }
    }

    /// Select provider based on model rules
    pub fn select_provider_by_model(
        &self,
        model: &str,
        providers: &[(String, Arc<crate::provider::Provider>)],
        rules: &[ModelRule],
    ) -> Option<(String, Arc<crate::provider::Provider>)> {
        // Check model rules first
        for rule in rules {
            if glob_match(&rule.pattern, model) {
                // Find the target provider
                return providers.iter()
                    .find(|(name, _)| name == &rule.target_provider)
                    .map(|(name, p)| (name.clone(), Arc::clone(p)));
            }
        }

        // Fall back to default strategy
        self.select_provider(providers)
    }
}

/// Simple glob pattern matching
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    let mut pattern_idx = 0;
    let mut text_idx = 0;

    while pattern_idx < pattern_chars.len() && text_idx < text_chars.len() {
        match pattern_chars[pattern_idx] {
            '*' => {
                // Match any characters
                if pattern_idx + 1 == pattern_chars.len() {
                    // Trailing * matches rest
                    return true;
                }
                // Try to match * with remaining text
                while text_idx < text_chars.len() {
                    if glob_match(&pattern[pattern_idx..], &text[text_idx..]) {
                        return true;
                    }
                    text_idx += 1;
                }
                return false;
            }
            '?' => {
                // Match single character
                text_idx += 1;
            }
            c => {
                if text_idx < text_chars.len() && text_chars[text_idx] == c {
                    text_idx += 1;
                } else {
                    return false;
                }
            }
        }
        pattern_idx += 1;
    }

    pattern_idx == pattern_chars.len() && text_idx == text_chars.len()
}
