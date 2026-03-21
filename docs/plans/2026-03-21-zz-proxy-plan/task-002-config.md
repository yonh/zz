# Task 002: Config Module - TOML Parsing and Validation

## Goal
Implement configuration parsing from TOML with validation and defaults.

## BDD Scenarios

```gherkin
Scenario: Parse valid config with multiple providers
  Given a config.toml file with server, routing, health sections
  And three [[providers]] entries with name, base_url, api_key
  When the config is loaded via Config::load()
  Then parsing succeeds without errors
  And all three providers are available in config.providers
  And default values are applied (request_timeout_secs=300, log_level=info)

Scenario: Validate required fields
  Given a config.toml file missing provider.api_key
  When Config::load() is called
  Then it returns an error
  And the error message indicates missing api_key field

Scenario: Apply default routing strategy
  Given a config.toml with no [routing.strategy] field
  When config is loaded
  Then routing.strategy equals "failover"

Scenario: Parse provider priority and weight
  Given a config.toml with providers having priority=1 and weight=5
  When config is loaded
  Then provider.priority equals 1
  And provider.weight equals 5
```

## Files to Create/Edit

**Create**:
- `src/config.rs` - Complete implementation

**Create**:
- `config.toml.example` - Example configuration file

## Implementation Steps

1. Define config structs with Serde derive:
   - `Config` (root)
   - `ServerConfig`
   - `RoutingConfig`
   - `HealthConfig`
   - `ProviderConfig`

2. Add validation logic in `Config::load()`:
   - Check required fields (name, base_url, api_key)
   - Parse URL validation for base_url
   - Set default values for optional fields

3. Implement `Config::load(path: &str)` that:
   - Reads TOML file
   - Deserializes to Config struct
   - Validates and returns Result<Config, Error>

4. Create example config file with comments

## Verification

Run:
```bash
cargo build
```

Test manually:
```rust
// In a test or temporary main
let config = Config::load("config.toml.example").unwrap();
assert_eq!(config.server.listen, "127.0.0.1:9090");
assert_eq!(config.providers.len(), 3);
```

## Dependencies
- Task 001 (project structure and serde dependency)
