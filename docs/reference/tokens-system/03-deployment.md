# ZZ - Token Statistics Deployment & Operations Guide

## Version: 1.0.0

---

## 1. Deployment Checklist

### 1.1 Pre-Deployment

- [ ] **Database Path**: Ensure writable directory exists
- [ ] **Disk Space**: Estimate 500MB per 10k requests/day (90 days retention)
- [ ] **Config**: Add `[stats]` section to `config.toml`
- [ ] **Pricing**: Configure model pricing if cost calculation needed
- [ ] **Quotas**: Set up initial quotas (optional)
- [ ] **Migration**: Test schema migration on staging

### 1.2 Deployment Steps

```bash
# 1. Stop existing service
systemctl stop zz

# 2. Backup existing database (if upgrading)
cp /var/lib/zz/zz_stats.db /var/lib/zz/zz_stats.db.bak

# 3. Deploy new binary
cp zz /usr/local/bin/zz
chmod +x /usr/local/bin/zz

# 4. Update config
vim /etc/zz/config.toml
# Add [stats] section

# 5. Start service
systemctl start zz

# 6. Verify
curl http://127.0.0.1:9090/zz/api/tokens/summary
```

### 1.3 Post-Deployment Verification

```bash
# Check database was created
ls -la /var/lib/zz/zz_stats.db

# Check API responds
curl http://127.0.0.1:9090/zz/api/health

# Make test request
curl -X POST http://127.0.0.1:9090/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "gpt-4", "messages": [{"role": "user", "content": "test"}]}'

# Verify token was logged
curl http://127.0.0.1:9090/zz/api/logs?limit=1
```

---

## 2. Configuration

### 2.1 Minimal Configuration

```toml
# config.toml

[server]
listen = "127.0.0.1:9090"

[stats]
enabled = true
db_path = "/var/lib/zz/zz_stats.db"

[[providers]]
name = "default"
base_url = "https://api.openai.com"
api_key = "sk-xxx"
```

### 2.2 Full Configuration

```toml
# config.toml

[server]
listen = "127.0.0.1:9090"
request_timeout_secs = 300
log_level = "info"

[routing]
strategy = "failover"
max_retries = 3

[health]
cooldown_secs = 60
failure_threshold = 3
recovery_secs = 600

# Token Statistics Configuration
[stats]
enabled = true                              # Enable token tracking
db_path = "/var/lib/zz/zz_stats.db"        # SQLite database path
retention_days = 90                         # Keep 90 days of data
enable_cost_calculation = true              # Calculate costs
default_input_per_1k = 0.001               # Default $/1k input tokens
default_output_per_1k = 0.002              # Default $/1k output tokens

# Model-specific pricing
[stats.pricing.claude-3-opus]
input_per_1k = 0.015
output_per_1k = 0.075

[stats.pricing.claude-3-sonnet]
input_per_1k = 0.003
output_per_1k = 0.015

[stats.pricing.gpt-4]
input_per_1k = 0.03
output_per_1k = 0.06

[stats.pricing.gpt-4-turbo]
input_per_1k = 0.01
output_per_1k = 0.03

[stats.pricing.qwen-plus]
input_per_1k = 0.0004
output_per_1k = 0.0012

[stats.pricing.glm-4]
input_per_1k = 0.0014
output_per_1k = 0.0014

# Provider quotas
[[quotas]]
provider = "ali-account-1"
monthly_token_budget = 1000000
monthly_cost_budget_usd = 100.00
alert_threshold = 0.8
reset_day = 1

[[quotas]]
provider = "zhipu-account-1"
monthly_token_budget = 500000
monthly_cost_budget_usd = 50.00
alert_threshold = 0.9
reset_day = 1

# Providers
[[providers]]
name = "ali-account-1"
base_url = "https://dashscope.aliyuncs.com/compatible-mode"
api_key = "sk-xxx"
priority = 1
models = ["qwen-plus", "qwen-turbo"]

[[providers]]
name = "zhipu-account-1"
base_url = "https://open.bigmodel.cn/api/paas/v4"
api_key = "sk-yyy"
priority = 2
models = ["glm-4", "glm-4-flash"]
```

### 2.3 Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ZZ_CONFIG` | `./config.toml` | Config file path |
| `ZZ_DB_PATH` | From config | Override database path |
| `ZZ_LOG_LEVEL` | From config | Override log level |

---

## 3. Systemd Service

### 3.1 Service File

```ini
# /etc/systemd/system/zz.service
[Unit]
Description=ZZ LLM Proxy
After=network.target

[Service]
Type=simple
User=zz
Group=zz
ExecStart=/usr/local/bin/zz --config /etc/zz/config.toml
Restart=always
RestartSec=5
LimitNOFILE=65535

# Security
NoNewPrivileges=true
PrivateTmp=true

# Resource limits
MemoryMax=1G
CPUQuota=100%

[Install]
WantedBy=multi-user.target
```

### 3.2 User Setup

```bash
# Create user
sudo useradd -r -s /bin/false zz

# Create directories
sudo mkdir -p /var/lib/zz
sudo chown zz:zz /var/lib/zz
sudo chmod 700 /var/lib/zz

# Create config directory
sudo mkdir -p /etc/zz
sudo chown zz:zz /etc/zz
```

### 3.3 Service Management

```bash
# Enable service
sudo systemctl enable zz

# Start service
sudo systemctl start zz

# Check status
sudo systemctl status zz

# View logs
sudo journalctl -u zz -f

# Restart after config change
sudo systemctl restart zz
```

---

## 4. Database Management

### 4.1 Database Location

Default: Same directory as config file, or configured via `db_path`

```bash
# Check database file
ls -la /var/lib/zz/zz_stats.db

# Check database size
du -h /var/lib/zz/zz_stats.db
```

### 4.2 Backup & Restore

```bash
# Backup (online)
sqlite3 /var/lib/zz/zz_stats.db ".backup '/backup/zz_stats_$(date +%Y%m%d).db'"

# Backup (offline)
cp /var/lib/zz/zz_stats.db /backup/zz_stats_$(date +%Y%m%d).db

# Restore
systemctl stop zz
cp /backup/zz_stats_20260322.db /var/lib/zz/zz_stats.db
systemctl start zz
```

### 4.3 Retention Management

Automatic cleanup runs daily at midnight:

```sql
-- Delete logs older than retention_days
DELETE FROM request_logs 
WHERE date(timestamp) < date('now', '-90 days');

-- Delete empty hourly stats
DELETE FROM hourly_stats 
WHERE request_count = 0;
```

Manual cleanup:

```bash
# Vacuum database (reclaim space)
sqlite3 /var/lib/zz/zz_stats.db "VACUUM;"

# Reindex
sqlite3 /var/lib/zz/zz_stats.db "REINDEX;"
```

### 4.4 Database Queries

```bash
# Count total records
sqlite3 /var/lib/zz/zz_stats.db "SELECT COUNT(*) FROM request_logs;"

# Today's totals
sqlite3 /var/lib/zz/zz_stats.db "
  SELECT 
    SUM(total_tokens) as total,
    SUM(input_tokens) as input,
    SUM(output_tokens) as output,
    SUM(cost_usd) as cost
  FROM request_logs
  WHERE date(timestamp) = date('now');
"

# Top providers by tokens
sqlite3 /var/lib/zz/zz_stats.db "
  SELECT provider, SUM(total_tokens) as total
  FROM request_logs
  WHERE strftime('%Y-%m', timestamp) = strftime('%Y-%m', 'now')
  GROUP BY provider
  ORDER BY total DESC
  LIMIT 5;
"

# Database integrity check
sqlite3 /var/lib/zz/zz_stats.db "PRAGMA integrity_check;"
```

---

## 5. Monitoring

### 5.1 Health Check Endpoint

```bash
# Basic health
curl http://127.0.0.1:9090/zz/api/health

# Response
{
  "status": "ok",
  "uptime_secs": 86400,
  "providers": [
    {"name": "ali-account-1", "status": "healthy"},
    {"name": "zhipu-account-1", "status": "healthy"}
  ]
}
```

### 5.2 Metrics Endpoint (Future)

```bash
curl http://127.0.0.1:9090/zz/api/metrics
```

Prometheus format:

```
# HELP token_requests_total Total token-tracked requests
# TYPE token_requests_total counter
token_requests_total 12500

# HELP token_input_total Total input tokens
# TYPE token_input_total counter
token_input_total 8500000

# HELP token_output_total Total output tokens
# TYPE token_output_total counter
token_output_total 4000000

# HELP token_cost_usd_total Total cost in USD
# TYPE token_cost_usd_total counter
token_cost_usd_total 1250.50

# HELP storage_errors_total Storage operation errors
# TYPE storage_errors_total counter
storage_errors_total 0
```

### 5.3 Log Monitoring

```bash
# Watch for errors
journalctl -u zz -f | grep -i error

# Watch for quota alerts
journalctl -u zz -f | grep -i "quota"

# Watch for token extraction failures
journalctl -u zz -f | grep -i "token"
```

### 5.4 Alerting Rules (Prometheus)

```yaml
groups:
  - name: zz_token_alerts
    rules:
      - alert: TokenStorageErrors
        expr: rate(storage_errors_total[5m]) > 0
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "ZZ token storage errors"
          description: "Token storage is experiencing errors"

      - alert: ProviderQuotaNearLimit
        expr: provider_quota_usage_percent > 80
        for: 1m
        labels:
          severity: warning
        annotations:
          summary: "Provider {{ $labels.provider }} quota at {{ $value }}%"

      - alert: ProviderQuotaExceeded
        expr: provider_quota_usage_percent >= 100
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Provider {{ $labels.provider }} quota exceeded"
```

---

## 6. Troubleshooting

### 6.1 Token Data Not Being Collected

**Symptoms**: API returns zeros, logs show null token values

**Check**:
```bash
# Check stats enabled in config
grep -A5 '\[stats\]' /etc/zz/config.toml

# Check for extraction errors in logs
journalctl -u zz | grep -i "extract"
```

**Solutions**:
1. Ensure `[stats]` section exists with `enabled = true`
2. Check provider response format matches expected
3. Verify response body contains `usage` field

### 6.2 Database Errors

**Symptoms**: "Failed to open database", "Database is locked"

**Check**:
```bash
# Check file permissions
ls -la /var/lib/zz/zz_stats.db

# Check disk space
df -h /var/lib/zz

# Check for database locks
lsof /var/lib/zz/zz_stats.db
```

**Solutions**:
1. Ensure user has write permissions
2. Check disk space is available
3. Restart service to clear locks
4. Run integrity check

### 6.3 Performance Issues

**Symptoms**: Slow API responses, high CPU usage

**Check**:
```bash
# Database size
du -h /var/lib/zz/zz_stats.db

# Query performance
time sqlite3 /var/lib/zz/zz_stats.db "
  SELECT COUNT(*) FROM request_logs WHERE timestamp > datetime('now', '-1 day');
"
```

**Solutions**:
1. Reduce retention days
2. Vacuum database
3. Add more indexes for common queries
4. Use aggregation tables for historical queries

### 6.4 Quota Reset Not Working

**Symptoms**: Quotas not resetting on configured day

**Check**:
```bash
# Check quota config
curl http://127.0.0.1:9090/zz/api/quotas

# Check current usage
curl http://127.0.0.1:9090/zz/api/tokens/by-provider?includeQuota=true
```

**Solutions**:
1. Verify reset_day is 1-28
2. Check service was running at midnight on reset day
3. Manually reset via API if needed

---

## 7. Upgrading

### 7.1 From Version Without Token Stats

```bash
# 1. Backup
cp /etc/zz/config.toml /etc/zz/config.toml.bak

# 2. Add stats section to config
cat >> /etc/zz/config.toml << EOF

[stats]
enabled = true
db_path = "/var/lib/zz/zz_stats.db"
retention_days = 90
enable_cost_calculation = true
default_input_per_1k = 0.001
default_output_per_1k = 0.002
EOF

# 3. Deploy new binary
cp zz /usr/local/bin/zz

# 4. Restart
systemctl restart zz

# 5. Verify database created
ls -la /var/lib/zz/zz_stats.db
```

### 7.2 Schema Migration

Migrations are automatic. On startup:
1. Check current schema version
2. Run pending migrations
3. Update version number

```bash
# Check schema version
sqlite3 /var/lib/zz/zz_stats.db "SELECT value FROM schema_meta WHERE key = 'version';"

# Manual migration (if needed)
sqlite3 /var/lib/zz/zz_stats.db < /usr/share/zz/migrations/v1_to_v2.sql
```

---

## 8. Performance Tuning

### 8.1 Database Optimization

```toml
# config.toml - increase batch size for high throughput
[stats]
batch_size = 200           # Default: 100
flush_interval_ms = 500    # Default: 1000
```

### 8.2 SQLite Pragmas

Applied automatically on startup:

```sql
PRAGMA journal_mode = WAL;      -- Better concurrency
PRAGMA synchronous = NORMAL;    -- Faster writes
PRAGMA busy_timeout = 5000;     -- Wait 5s for locks
PRAGMA cache_size = -64000;     -- 64MB cache
PRAGMA temp_store = MEMORY;     -- Temp tables in memory
```

### 8.3 Hardware Recommendations

| Scale | CPU | RAM | Disk |
|-------|-----|-----|------|
| < 1k req/day | 1 core | 512MB | 1GB |
| 1k-10k req/day | 2 cores | 1GB | 5GB |
| 10k-100k req/day | 4 cores | 2GB | 20GB |
| > 100k req/day | 4+ cores | 4GB | 50GB+ |

---

## 9. Security Considerations

### 9.1 Database Security

```bash
# Set restrictive permissions
chmod 600 /var/lib/zz/zz_stats.db
chown zz:zz /var/lib/zz/zz_stats.db

# Ensure parent directory is secure
chmod 700 /var/lib/zz
```

### 9.2 API Security

- API endpoints are local-only (127.0.0.1)
- No authentication required for local access
- For remote access, use reverse proxy with auth

### 9.3 Data Privacy

- Token counts are stored, not request/response content
- Model names are stored
- No API keys stored in database
- Logs can be purged via retention policy

---

## 10. Operational Runbooks

### 10.1 Daily Checks

```bash
#!/bin/bash
# /usr/local/bin/zz-daily-check.sh

echo "=== ZZ Daily Health Check ==="
echo ""

echo "1. Service Status:"
systemctl is-active zz

echo ""
echo "2. Database Size:"
du -h /var/lib/zz/zz_stats.db

echo ""
echo "3. Today's Token Usage:"
curl -s http://127.0.0.1:9090/zz/api/tokens/summary | jq '.today'

echo ""
echo "4. Provider Status:"
curl -s http://127.0.0.1:9090/zz/api/providers | jq '.providers[] | {name, status}'

echo ""
echo "5. Recent Errors:"
journalctl -u zz --since "1 day ago" | grep -i error | tail -5
```

### 10.2 Incident Response: Database Full

```bash
#!/bin/bash
# Run when disk usage > 90%

echo "Reducing retention..."
# Update config
sed -i 's/retention_days = 90/retention_days = 30/' /etc/zz/config.toml

# Restart to apply
systemctl restart zz

# Wait for retention cleanup
sleep 60

# Vacuum to reclaim space
sqlite3 /var/lib/zz/zz_stats.db "VACUUM;"

echo "Done. Check disk usage:"
df -h /var/lib/zz
```

### 10.3 Incident Response: Quota Alert

```bash
#!/bin/bash
# Run when quota alert fires

PROVIDER=$1

echo "Provider $PROVIDER quota alert triggered"

# Check current usage
curl -s "http://127.0.0.1:9090/zz/api/tokens/by-provider?provider=$PROVIDER" | jq '.providers[0].quota'

# Options:
# 1. Increase quota
# 2. Add failover provider
# 3. Monitor and wait

# Example: Increase quota
read -p "Increase quota? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    read -p "New token budget: " NEW_BUDGET
    curl -X PUT http://127.0.0.1:9090/zz/api/quotas \
        -H "Content-Type: application/json" \
        -d "{\"quotas\":[{\"provider\":\"$PROVIDER\",\"monthlyTokenBudget\":$NEW_BUDGET,\"alertThreshold\":0.8,\"resetDay\":1}]}"
fi
```

---

**Document Version**: 1.0
**Last Updated**: 2026-03-22