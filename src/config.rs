use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub routing: RoutingConfig,
    pub health: HealthConfig,
    #[serde(rename = "providers")]
    pub provider_configs: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_listen")]
    pub listen: String,
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoutingConfig {
    #[serde(default = "default_routing_strategy")]
    pub strategy: String,
    #[serde(default = "default_retry_on_failure")]
    pub retry_on_failure: bool,
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HealthConfig {
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: usize,
    #[serde(default = "default_recovery_secs")]
    pub recovery_secs: u64,
    #[serde(default = "default_cooldown_secs")]
    pub cooldown_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub priority: usize,
    #[serde(default)]
    pub weight: usize,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

fn default_listen() -> String {
    "127.0.0.1:9090".to_string()
}

fn default_request_timeout_secs() -> u64 {
    300
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_routing_strategy() -> String {
    "failover".to_string()
}

fn default_retry_on_failure() -> bool {
    true
}

fn default_max_retries() -> usize {
    3
}

fn default_failure_threshold() -> usize {
    3
}

fn default_recovery_secs() -> u64 {
    600
}

fn default_cooldown_secs() -> u64 {
    60
}

impl Config {
    pub fn load(path: &str) -> Result<Self, anyhow::Error> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;

        // Validate required fields
        for provider in &config.provider_configs {
            if provider.name.is_empty() {
                return Err(anyhow::anyhow!("Provider name is required"));
            }
            if provider.base_url.is_empty() {
                return Err(anyhow::anyhow!("Provider base_url is required for {}", provider.name));
            }
            if provider.api_key.is_empty() {
                return Err(anyhow::anyhow!("Provider api_key is required for {}", provider.name));
            }
        }

        Ok(config)
    }
}
