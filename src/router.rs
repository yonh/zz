use rand::{rng, RngExt};
use std::sync::Arc;

pub enum RoutingStrategy {
    Failover,
    RoundRobin,
    WeightedRandom,
}

pub struct Router {
    strategy: RoutingStrategy,
    round_robin_index: std::sync::atomic::AtomicUsize,
}

impl Router {
    pub fn new(strategy: &str) -> Self {
        let strategy = match strategy {
            "round-robin" => RoutingStrategy::RoundRobin,
            "weighted-random" => RoutingStrategy::WeightedRandom,
            _ => RoutingStrategy::Failover,
        };
        Self {
            strategy,
            round_robin_index: std::sync::atomic::AtomicUsize::new(0),
        }
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
        }
    }
}
