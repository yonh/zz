use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub routing: RoutingConfig,
    pub health: HealthConfig,
    #[serde(rename = "providers")]
    pub provider_configs: Vec<ProviderConfig>,
    #[serde(default)]
    pub observability: ObservabilityConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub request_journal: RequestJournalConfig,
    #[serde(default)]
    pub timing: TimingConfig,
    #[serde(default)]
    pub tracing: TracingConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimingConfig {
    #[serde(default = "default_timing_enabled")]
    pub enabled: bool,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn default_timing_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct TracingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_tracing_sampling_mode")]
    pub sampling_mode: String,
    #[serde(default = "default_tracing_base_rate")]
    pub base_rate: f64,
    #[serde(default = "default_tracing_slow_threshold_ms")]
    pub slow_threshold_ms: u64,
    #[serde(default = "default_tracing_error_sampling")]
    pub error_sampling: f64,
    #[serde(default = "default_tracing_storage_dir")]
    pub storage_dir: String,
    #[serde(default = "default_tracing_retention_days")]
    pub retention_days: u64,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sampling_mode: default_tracing_sampling_mode(),
            base_rate: default_tracing_base_rate(),
            slow_threshold_ms: default_tracing_slow_threshold_ms(),
            error_sampling: default_tracing_error_sampling(),
            storage_dir: default_tracing_storage_dir(),
            retention_days: default_tracing_retention_days(),
        }
    }
}

fn default_tracing_sampling_mode() -> String {
    "adaptive".to_string()
}

fn default_tracing_base_rate() -> f64 {
    0.01
}

fn default_tracing_slow_threshold_ms() -> u64 {
    3000
}

fn default_tracing_error_sampling() -> f64 {
    1.0
}

fn default_tracing_storage_dir() -> String {
    "logs/traces".to_string()
}

fn default_tracing_retention_days() -> u64 {
    3
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestJournalConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_request_journal_storage_dir")]
    pub storage_dir: String,
    #[serde(default = "default_request_journal_retention_days")]
    pub retention_days: u64,
    #[serde(default = "default_redact_headers")]
    pub redact_headers: Vec<String>,
}

impl Default for RequestJournalConfig {
    fn default() -> Self {
        Self {
            enabled: true, // 默认启用，方便调试和问题排查
            storage_dir: default_request_journal_storage_dir(),
            retention_days: default_request_journal_retention_days(),
            redact_headers: default_redact_headers(),
        }
    }
}

fn default_request_journal_storage_dir() -> String {
    "logs/request-journal".to_string()
}

fn default_request_journal_retention_days() -> u64 {
    7
}

fn default_redact_headers() -> Vec<String> {
    vec![
        "authorization".to_string(),
        "x-api-key".to_string(),
        "cookie".to_string(),
        "set-cookie".to_string(),
    ]
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
    #[serde(default)]
    pub pinned_provider: Option<String>,
    #[serde(default)]
    pub rules: Vec<ModelRuleConfig>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct ModelRuleConfig {
    pub pattern: String,
    pub target_provider: String,
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
    #[serde(default)]
    pub token_budget: Option<u64>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
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
        Self::load_from_str(&contents)
    }

    pub fn load_from_str(contents: &str) -> Result<Self, anyhow::Error> {
        let config: Config = toml::from_str(contents)?;

        // Validate required fields
        for provider in &config.provider_configs {
            if provider.name.is_empty() {
                return Err(anyhow::anyhow!("Provider name is required"));
            }
            if provider.base_url.is_empty() {
                return Err(anyhow::anyhow!(
                    "Provider base_url is required for {}",
                    provider.name
                ));
            }
            if provider.api_key.is_empty() {
                return Err(anyhow::anyhow!(
                    "Provider api_key is required for {}",
                    provider.name
                ));
            }
        }

        Ok(config)
    }
}
