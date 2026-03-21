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
    health_config: std::sync::RwLock<crate::config::HealthConfig>,
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
            health_config: std::sync::RwLock::new(config.health.clone()),
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
            let cooldown_secs = self.health_config.read().unwrap().cooldown_secs;
            provider.mark_quota_exhausted(cooldown_secs);
        }
    }

    pub fn mark_failure(&self, name: &str) {
        if let Some(provider) = self.providers.get(name) {
            provider.mark_failure();
        }
    }

    pub fn reset(&self, name: &str) {
        if let Some(provider) = self.providers.get(name) {
            provider.reset();
        }
    }

    pub fn get_all_states(&self) -> Vec<ProviderStatus> {
        self.providers
            .iter()
            .map(|entry| {
                let state = entry.value().state.lock().unwrap();
                let state_str = match &*state {
                    ProviderState::Healthy => "healthy",
                    ProviderState::Cooldown { until } => {
                        if chrono::Utc::now() < *until {
                            "cooldown"
                        } else {
                            "healthy"
                        }
                    }
                    ProviderState::Unhealthy { recovery_at } => {
                        if chrono::Utc::now() < *recovery_at {
                            "unhealthy"
                        } else {
                            "healthy"
                        }
                    }
                };
                ProviderStatus {
                    name: entry.key().clone(),
                    state: state_str.to_string(),
                }
            })
            .collect()
    }
pub fn reload(&self, config: &crate::config::Config) {
        // Update health config
        *self.health_config.write().unwrap() = config.health.clone();

        // Get current provider names
        let current_names: std::collections::HashSet<_> = self.providers.iter()
            .map(|entry| entry.key().clone())
            .collect();

        // Get new provider names
        let new_names: std::collections::HashSet<_> = config.provider_configs.iter()
            .map(|p| p.name.clone())
            .collect();

        // Remove providers that no longer exist
        for name in current_names.difference(&new_names) {
            self.providers.remove(name);
            tracing::info!(provider = %name, "Removed provider");
        }

        // Add or update providers
        for provider_config in &config.provider_configs {
            if self.providers.contains_key(&provider_config.name) {
                // Update existing provider's config - need to replace the whole provider
                let provider = Arc::new(Provider::new(provider_config.clone()));
                self.providers.insert(provider_config.name.clone(), provider);
                tracing::info!(provider = %provider_config.name, "Updated provider config");
            } else {
                // Add new provider
                let provider = Arc::new(Provider::new(provider_config.clone()));
                self.providers.insert(provider_config.name.clone(), provider);
                tracing::info!(provider = %provider_config.name, "Added new provider");
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderStatus {
    pub name: String,
    pub state: String,
}
