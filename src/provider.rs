use std::sync::Arc;
use dashmap::DashMap;

pub struct Provider {
    pub config: crate::config::ProviderConfig,
    pub state: std::sync::Mutex<ProviderState>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderState {
    Healthy,
    Cooldown { until: chrono::DateTime<chrono::Utc> },
    Unhealthy { recovery_at: chrono::DateTime<chrono::Utc> },
}

impl Provider {
    pub fn new(config: crate::config::ProviderConfig) -> Self {
        Self {
            config,
            state: std::sync::Mutex::new(ProviderState::Healthy),
        }
    }

    pub fn mark_quota_exhausted(&self, cooldown_secs: u64) {
        let mut state = self.state.lock().unwrap();
        let until = chrono::Utc::now() + chrono::Duration::seconds(cooldown_secs as i64);
        *state = ProviderState::Cooldown { until };
    }

    pub fn mark_failure(&self) -> bool {
        // For simplicity, just mark as unhealthy after threshold
        // In production, track failure count and threshold
        let mut state = self.state.lock().unwrap();
        let recovery_at = chrono::Utc::now() + chrono::Duration::seconds(600);
        *state = ProviderState::Unhealthy { recovery_at };
        false // return true if reached threshold
    }

    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        *state = ProviderState::Healthy;
    }

    pub fn is_available(&self) -> bool {
        let state = self.state.lock().unwrap();
        match &*state {
            ProviderState::Healthy => true,
            ProviderState::Cooldown { until } => chrono::Utc::now() > *until,
            ProviderState::Unhealthy { recovery_at } => chrono::Utc::now() > *recovery_at,
        }
    }
}

pub struct ProviderManager {
    providers: DashMap<String, Arc<Provider>>,
    health_config: crate::config::HealthConfig,
}

impl ProviderManager {
    pub fn new(config: &crate::config::Config) -> Self {
        let providers = DashMap::new();
        for provider_config in &config.provider_configs {
            let provider = Arc::new(Provider::new(provider_config.clone()));
            providers.insert(provider_config.name.clone(), provider);
        }
        Self {
            providers,
            health_config: config.health.clone(),
        }
    }

    pub fn get_available(&self) -> Vec<(String, Arc<Provider>)> {
        self.providers
            .iter()
            .filter(|entry| entry.value().is_available())
            .map(|entry| (entry.key().clone(), Arc::clone(entry.value())))
            .collect()
    }

    pub fn get_by_name(&self, name: &str) -> Option<Arc<Provider>> {
        self.providers.get(name).map(|entry| Arc::clone(entry.value()))
    }

    pub fn mark_quota_exhausted(&self, name: &str) {
        if let Some(provider) = self.providers.get(name) {
            provider.mark_quota_exhausted(self.health_config.cooldown_secs);
        }
    }

    pub fn mark_failure(&self, name: &str) {
        if let Some(provider) = self.providers.get(name) {
            provider.mark_failure();
        }
    }
}
