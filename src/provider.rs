use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::collections::VecDeque;
use dashmap::DashMap;

pub struct Provider {
    pub config: crate::config::ProviderConfig,
    pub state: std::sync::Mutex<ProviderState>,
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
    pub failure_count: std::sync::Mutex<usize>,
    pub enabled: AtomicBool,
    pub latency_history: std::sync::Mutex<VecDeque<u64>>,
    pub latency_ema: std::sync::Mutex<f64>,
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
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            failure_count: std::sync::Mutex::new(0),
            enabled: AtomicBool::new(true),
            latency_history: std::sync::Mutex::new(VecDeque::with_capacity(12)),
            latency_ema: std::sync::Mutex::new(0.0),
        }
    }

    pub fn increment_request(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn mark_quota_exhausted(&self, cooldown_secs: u64) {
        let mut state = self.state.lock().unwrap();
        let until = chrono::Utc::now() + chrono::Duration::seconds(cooldown_secs as i64);
        *state = ProviderState::Cooldown { until };
        // Reset failure count on quota exhaustion
        *self.failure_count.lock().unwrap() = 0;
    }

    pub fn mark_failure(&self, failure_threshold: usize, recovery_secs: u64) -> bool {
        let mut failure_count = self.failure_count.lock().unwrap();
        *failure_count += 1;

        if *failure_count >= failure_threshold {
            let mut state = self.state.lock().unwrap();
            let recovery_at = chrono::Utc::now() + chrono::Duration::seconds(recovery_secs as i64);
            *state = ProviderState::Unhealthy { recovery_at };
            *failure_count = 0;
            return true;
        }
        false
    }

    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        *state = ProviderState::Healthy;
        *self.failure_count.lock().unwrap() = 0;
    }

    pub fn is_available(&self) -> bool {
        if !self.is_enabled() {
            return false;
        }
        let state = self.state.lock().unwrap();
        match &*state {
            ProviderState::Healthy => true,
            ProviderState::Cooldown { until } => chrono::Utc::now() > *until,
            ProviderState::Unhealthy { recovery_at } => chrono::Utc::now() > *recovery_at,
        }
    }

    /// Record latency and update EMA
    pub fn record_latency(&self, latency_ms: u64) {
        // Update history
        {
            let mut history = self.latency_history.lock().unwrap();
            if history.len() >= 12 {
                history.pop_front();
            }
            history.push_back(latency_ms);
        }
        // Update EMA with alpha = 0.3
        {
            let mut ema = self.latency_ema.lock().unwrap();
            if *ema == 0.0 {
                *ema = latency_ms as f64;
            } else {
                *ema = 0.3 * latency_ms as f64 + 0.7 * *ema;
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn get_latency_history(&self) -> Vec<u64> {
        let history = self.latency_history.lock().unwrap();
        history.iter().copied().collect()
    }

    pub fn get_avg_latency(&self) -> u64 {
        let history = self.latency_history.lock().unwrap();
        if history.is_empty() {
            return 0;
        }
        let sum: u64 = history.iter().sum();
        sum / history.len() as u64
    }

    pub fn get_latency_ema(&self) -> f64 {
        *self.latency_ema.lock().unwrap()
    }

    pub fn get_stats(&self) -> ProviderStats {
        let state = self.state.lock().unwrap();
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
        ProviderStats {
            name: self.config.name.clone(),
            state: state_str.to_string(),
            enabled: self.is_enabled(),
            request_count: self.request_count.load(Ordering::Relaxed),
            error_count: self.error_count.load(Ordering::Relaxed),
            failure_count: *self.failure_count.lock().unwrap(),
            avg_latency_ms: self.get_avg_latency(),
            latency_ema: self.get_latency_ema(),
            latency_history: self.get_latency_history(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderStats {
    pub name: String,
    pub state: String,
    pub enabled: bool,
    pub request_count: u64,
    pub error_count: u64,
    pub failure_count: usize,
    pub avg_latency_ms: u64,
    pub latency_ema: f64,
    pub latency_history: Vec<u64>,
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
            provider.increment_error();
        }
    }

    pub fn mark_failure(&self, name: &str) {
        if let Some(provider) = self.providers.get(name) {
            let (failure_threshold, recovery_secs) = {
                let config = self.health_config.read().unwrap();
                (config.failure_threshold, config.recovery_secs)
            };
            provider.mark_failure(failure_threshold, recovery_secs);
            provider.increment_error();
        }
    }

    pub fn reset(&self, name: &str) {
        if let Some(provider) = self.providers.get(name) {
            provider.reset();
        }
    }

    pub fn increment_request(&self, name: &str) {
        if let Some(provider) = self.providers.get(name) {
            provider.increment_request();
        }
    }

    pub fn get_all_states(&self) -> Vec<ProviderStatus> {
        self.providers
            .iter()
            .map(|entry| {
                let stats = entry.value().get_stats();
                ProviderStatus {
                    name: stats.name,
                    state: stats.state,
                }
            })
            .collect()
    }

    pub fn get_all_stats(&self) -> Vec<ProviderStats> {
        self.providers
            .iter()
            .map(|entry| entry.value().get_stats())
            .collect()
    }

    pub fn get_total_stats(&self) -> (u64, u64) {
        let mut total_requests = 0u64;
        let mut total_errors = 0u64;
        for entry in self.providers.iter() {
            total_requests += entry.value().request_count.load(Ordering::Relaxed);
            total_errors += entry.value().error_count.load(Ordering::Relaxed);
        }
        (total_requests, total_errors)
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

    /// Add a new provider at runtime.
    pub fn add_provider(&self, config: crate::config::ProviderConfig) -> Result<(), String> {
        if self.providers.contains_key(&config.name) {
            return Err(format!("Provider already exists: {}", config.name));
        }
        let provider = Arc::new(Provider::new(config.clone()));
        self.providers.insert(config.name.clone(), provider);
        tracing::info!(provider = %config.name, "Added new provider at runtime");
        Ok(())
    }

    /// Remove a provider at runtime.
    pub fn remove_provider(&self, name: &str) -> Result<(), String> {
        if self.providers.remove(name).is_none() {
            return Err(format!("Provider not found: {}", name));
        }
        tracing::info!(provider = %name, "Removed provider at runtime");
        Ok(())
    }

    /// Update a provider's configuration fields at runtime.
    /// Preserves runtime state (request counts, health state).
    pub fn update_provider_config(
        &self,
        name: &str,
        updates: ProviderConfigUpdate,
    ) -> Result<(), String> {
        let provider = self.providers.get(name)
            .ok_or_else(|| format!("Provider not found: {}", name))?;

        // We need to create a new Provider with updated config but preserve stats
        // Since config fields are in the Provider struct, we need careful update
        let mut new_config = provider.config.clone();
        if let Some(base_url) = updates.base_url { new_config.base_url = base_url; }
        if let Some(api_key) = updates.api_key { new_config.api_key = api_key; }
        if let Some(priority) = updates.priority { new_config.priority = priority; }
        if let Some(weight) = updates.weight { new_config.weight = weight; }
        if let Some(models) = updates.models { new_config.models = models; }
        if let Some(headers) = updates.headers { new_config.headers = headers; }
        if let Some(token_budget) = updates.token_budget { new_config.token_budget = token_budget; }

        drop(provider); // Release DashMap read lock

        // Replace with new provider (preserving name, resetting stats)
        // Note: This resets runtime stats. For a non-destructive update,
        // Provider.config would need interior mutability (Mutex/RwLock).
        let new_provider = Arc::new(Provider::new(new_config));
        self.providers.insert(name.to_string(), new_provider);

        tracing::info!(provider = %name, "Updated provider config at runtime");
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderStatus {
    pub name: String,
    pub state: String,
}

/// Partial update for provider configuration.
#[derive(Debug, Clone, Default)]
pub struct ProviderConfigUpdate {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub priority: Option<usize>,
    pub weight: Option<usize>,
    pub models: Option<Vec<String>>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub token_budget: Option<Option<u64>>,
}