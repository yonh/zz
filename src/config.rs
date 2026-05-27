use serde::Deserialize;

use crate::converter::ApiType;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub routing: RoutingConfig,
    pub health: HealthConfig,
    #[serde(rename = "providers")]
    pub provider_configs: Vec<ProviderConfig>,
    #[serde(default)]
    pub observability: ObservabilityConfig,
    #[serde(default)]
    pub admin: AdminConfig,
    #[serde(default)]
    pub compat: CompatConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_allowed_origins")]
    pub allowed_origins: Vec<String>,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            allowed_origins: default_allowed_origins(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CompatConfig {
    #[serde(default)]
    pub claude_code_openai: ClaudeCodeOpenAICompatConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ClaudeCodeOpenAICompatConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_claude_code_match_paths")]
    pub match_paths: Vec<String>,
    #[serde(default = "default_claude_code_target_api")]
    pub target_api_type: String,
}

fn default_allowed_origins() -> Vec<String> {
    vec!["http://localhost:*".to_string()]
}

fn default_claude_code_match_paths() -> Vec<String> {
    vec!["/v1/messages".to_string()]
}

fn default_claude_code_target_api() -> String {
    "openai-chat".to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub request_journal: RequestJournalConfig,
    #[serde(default)]
    pub timing: TimingConfig,
    #[serde(default)]
    pub tracing: TracingConfig,
    #[serde(default)]
    pub log_level: String,
    #[serde(default = "default_conversion_log_level")]
    pub conversion_log_level: String,
    #[serde(default)]
    pub telemetry: crate::converter::telemetry::TelemetryConfig,
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
    #[serde(default)]
    pub capture_response_body: bool,
    #[serde(default = "default_max_response_body_bytes")]
    pub max_response_body_bytes: u64,
}

impl Default for RequestJournalConfig {
    fn default() -> Self {
        Self {
            enabled: true, // 默认启用，方便调试和问题排查
            storage_dir: default_request_journal_storage_dir(),
            retention_days: default_request_journal_retention_days(),
            redact_headers: default_redact_headers(),
            capture_response_body: true, // 默认启用，方便调试查看远程返回
            max_response_body_bytes: default_max_response_body_bytes(),
        }
    }
}

fn default_max_response_body_bytes() -> u64 {
    10240 // 10KB
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
    #[serde(default = "default_api_type")]
    pub api_type: String,
    #[serde(default = "default_true")]
    pub enable_conversion_fallback: bool,
}

impl ProviderConfig {
    /// Resolve the api_type string to an ApiType enum.
    /// Invalid values are treated as "auto" (which defaults to OpenAIChat for now).
    /// Logs a warning once for invalid values.
    pub fn resolved_api_type(&self) -> ApiType {
        static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        
        match self.api_type.as_str() {
            "anthropic" => ApiType::Anthropic,
            "openai-chat" => ApiType::OpenAIChat,
            "openai-responses" => ApiType::OpenAIResponses,
            "" | "auto" => {
                // First version: auto defaults to OpenAIChat
                ApiType::OpenAIChat
            }
            _ => {
                // Invalid value - warn once and treat as auto
                if !WARNED.load(std::sync::atomic::Ordering::Relaxed) {
                    tracing::warn!(
                        provider = %self.name,
                        api_type = %self.api_type,
                        "Invalid api_type, treating as 'auto' (OpenAIChat)"
                    );
                    WARNED.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                ApiType::OpenAIChat
            }
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_api_type() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}

fn default_conversion_log_level() -> String {
    "info".to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_old_config_loads_with_defaults() {
        let old_config_toml = r#"
[server]
listen = "127.0.0.1:9090"
request_timeout_secs = 300
log_level = "info"

[admin]
enabled = false
api_key = ""
allowed_origins = ["http://localhost:*"]

[routing]
strategy = "round-robin"
retry_on_failure = true
max_retries = 3

[health]
failure_threshold = 3
recovery_secs = 600
cooldown_secs = 60

[observability.timing]
enabled = true

[observability.request_journal]
enabled = true
storage_dir = "logs/request-journal"
retention_days = 7

[observability.tracing]
enabled = false

[[providers]]
name = "test-provider"
base_url = "https://api.example.com/v1"
api_key = "sk-test-key"
priority = 1
weight = 1
models = ["gpt-4"]
"#;

        let config = Config::load_from_str(old_config_toml).expect("Old config should load");
        
        assert_eq!(config.observability.conversion_log_level, "info");
        
        let provider = &config.provider_configs[0];
        assert_eq!(provider.api_type, "auto");
        assert_eq!(provider.enable_conversion_fallback, true);
    }

    #[test]
    fn test_api_type_anthropic() {
        let config_toml = r#"
[server]
listen = "127.0.0.1:9090"
log_level = "info"

[admin]
enabled = false
api_key = ""
allowed_origins = ["http://localhost:*"]

[routing]
strategy = "round-robin"
retry_on_failure = true
max_retries = 3

[health]
failure_threshold = 3
recovery_secs = 600
cooldown_secs = 60

[[providers]]
name = "anthropic-provider"
base_url = "https://api.anthropic.com/v1"
api_key = "sk-test"
priority = 1
weight = 1
models = ["claude-3-opus"]
api_type = "anthropic"
"#;

        let config = Config::load_from_str(config_toml).expect("Config should load");
        let provider = &config.provider_configs[0];
        
        assert_eq!(provider.api_type, "anthropic");
        assert_eq!(provider.resolved_api_type(), ApiType::Anthropic);
    }

    #[test]
    fn test_api_type_openai_chat() {
        let config_toml = r#"
[server]
listen = "127.0.0.1:9090"
log_level = "info"

[admin]
enabled = false
api_key = ""
allowed_origins = ["http://localhost:*"]

[routing]
strategy = "round-robin"
retry_on_failure = true
max_retries = 3

[health]
failure_threshold = 3
recovery_secs = 600
cooldown_secs = 60

[[providers]]
name = "openai-provider"
base_url = "https://api.openai.com/v1"
api_key = "sk-test"
priority = 1
weight = 1
models = ["gpt-4"]
api_type = "openai-chat"
"#;

        let config = Config::load_from_str(config_toml).expect("Config should load");
        let provider = &config.provider_configs[0];
        
        assert_eq!(provider.api_type, "openai-chat");
        assert_eq!(provider.resolved_api_type(), ApiType::OpenAIChat);
    }

    #[test]
    fn test_api_type_invalid() {
        let config_toml = r#"
[server]
listen = "127.0.0.1:9090"
log_level = "info"

[admin]
enabled = false
api_key = ""
allowed_origins = ["http://localhost:*"]

[routing]
strategy = "round-robin"
retry_on_failure = true
max_retries = 3

[health]
failure_threshold = 3
recovery_secs = 600
cooldown_secs = 60

[[providers]]
name = "invalid-provider"
base_url = "https://api.example.com/v1"
api_key = "sk-test"
priority = 1
weight = 1
models = ["gpt-4"]
api_type = "invalid-type"
"#;

        let config = Config::load_from_str(config_toml).expect("Config should load");
        let provider = &config.provider_configs[0];
        
        assert_eq!(provider.api_type, "invalid-type");
        assert_eq!(provider.resolved_api_type(), ApiType::OpenAIChat);
    }

    #[test]
    fn test_conversion_log_level() {
        let config_toml = r#"
[server]
listen = "127.0.0.1:9090"
log_level = "info"

[admin]
enabled = false
api_key = ""
allowed_origins = ["http://localhost:*"]

[routing]
strategy = "round-robin"
retry_on_failure = true
max_retries = 3

[health]
failure_threshold = 3
recovery_secs = 600
cooldown_secs = 60

[observability]
conversion_log_level = "debug"

[[providers]]
name = "test-provider"
base_url = "https://api.example.com/v1"
api_key = "sk-test"
priority = 1
weight = 1
models = ["gpt-4"]
"#;

        let config = Config::load_from_str(config_toml).expect("Config should load");
        assert_eq!(config.observability.conversion_log_level, "debug");
    }
}
