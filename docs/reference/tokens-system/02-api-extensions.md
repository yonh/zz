# ZZ - Token Statistics API Reference

## Version: 1.0.0
## Base URL: `http://127.0.0.1:9090/zz/api`

---

## Authentication

None required (local-only proxy).

---

## Common Headers

```
Content-Type: application/json
Accept: application/json
```

---

## 1. Token Summary

### `GET /tokens/summary`

Get aggregated token statistics across different time periods.

#### Request

```http
GET /zz/api/tokens/summary HTTP/1.1
Host: 127.0.0.1:9090
Accept: application/json
```

#### Response `200 OK`

```json
{
  "today": {
    "totalTokens": 125000,
    "inputTokens": 85000,
    "outputTokens": 40000,
    "cachedTokens": 5000,
    "totalCostUsd": 12.50,
    "requestCount": 150,
    "successCount": 145,
    "errorCount": 5,
    "avgDurationMs": 2350
  },
  "yesterday": {
    "totalTokens": 98000,
    "inputTokens": 65000,
    "outputTokens": 33000,
    "cachedTokens": 3000,
    "totalCostUsd": 9.80,
    "requestCount": 120,
    "successCount": 118,
    "errorCount": 2,
    "avgDurationMs": 2100
  },
  "thisWeek": {
    "totalTokens": 450000,
    "inputTokens": 300000,
    "outputTokens": 150000,
    "cachedTokens": 20000,
    "totalCostUsd": 45.00,
    "requestCount": 550,
    "successCount": 540,
    "errorCount": 10,
    "avgDurationMs": 2200
  },
  "thisMonth": {
    "totalTokens": 1250000,
    "inputTokens": 850000,
    "outputTokens": 400000,
    "cachedTokens": 50000,
    "totalCostUsd": 125.00,
    "requestCount": 1500,
    "successCount": 1470,
    "errorCount": 30,
    "avgDurationMs": 2250
  },
  "lastMonth": {
    "totalTokens": 1100000,
    "inputTokens": 750000,
    "outputTokens": 350000,
    "cachedTokens": 40000,
    "totalCostUsd": 110.00,
    "requestCount": 1350,
    "successCount": 1320,
    "errorCount": 30,
    "avgDurationMs": 2180
  }
}
```

#### Response Fields

| Field | Type | Description |
|-------|------|-------------|
| totalTokens | number | Total tokens (input + output) |
| inputTokens | number | Prompt/input tokens |
| outputTokens | number | Completion/output tokens |
| cachedTokens | number | Tokens served from cache (if supported) |
| totalCostUsd | number | Estimated cost in USD |
| requestCount | number | Total requests in period |
| successCount | number | Successful requests (2xx) |
| errorCount | number | Failed requests (4xx, 5xx) |
| avgDurationMs | number | Average request duration in milliseconds |

#### Error Responses

None expected. Returns zeros if no data exists.

---

## 2. Time Series

### `GET /tokens/timeseries`

Get token consumption over time.

#### Query Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| start | string | Yes | - | Start timestamp (ISO 8601) |
| end | string | Yes | - | End timestamp (ISO 8601) |
| granularity | string | No | `hour` | `minute`, `hour`, `day`, `week`, `month` |
| provider | string | No | all | Filter by provider name |
| model | string | No | all | Filter by model name |

#### Request Examples

```http
GET /zz/api/tokens/timeseries?start=2026-03-01T00:00:00Z&end=2026-03-22T00:00:00Z&granularity=hour

GET /zz/api/tokens/timeseries?start=2026-03-01T00:00:00Z&end=2026-03-22T00:00:00Z&granularity=day&provider=ali-account-1

GET /zz/api/tokens/timeseries?start=2026-03-01T00:00:00Z&end=2026-03-22T00:00:00Z&granularity=hour&model=qwen-plus
```

#### Response `200 OK`

```json
{
  "start": "2026-03-01T00:00:00Z",
  "end": "2026-03-22T00:00:00Z",
  "granularity": "hour",
  "provider": null,
  "model": null,
  "data": [
    {
      "time": "2026-03-01T00:00:00Z",
      "inputTokens": 5000,
      "outputTokens": 2000,
      "totalTokens": 7000,
      "costUsd": 0.70,
      "requestCount": 10,
      "avgDurationMs": 2300
    },
    {
      "time": "2026-03-01T01:00:00Z",
      "inputTokens": 3500,
      "outputTokens": 1500,
      "totalTokens": 5000,
      "costUsd": 0.50,
      "requestCount": 8,
      "avgDurationMs": 2100
    }
  ],
  "meta": {
    "totalPoints": 504,
    "returnedPoints": 504,
    "maxPoints": 1000
  }
}
```

#### Error Responses

**400 Bad Request** - Invalid parameters

```json
{
  "error": "Invalid time range: start must be before end",
  "code": "E001"
}
```

```json
{
  "error": "Invalid granularity: must be one of minute, hour, day, week, month",
  "code": "E002"
}
```

---

## 3. By Provider

### `GET /tokens/by-provider`

Get token statistics grouped by provider.

#### Query Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| period | string | No | `month` | `today`, `week`, `month` |
| includeQuota | boolean | No | `true` | Include quota information |

#### Request

```http
GET /zz/api/tokens/by-provider?period=month&includeQuota=true HTTP/1.1
```

#### Response `200 OK`

```json
{
  "period": "month",
  "providers": [
    {
      "provider": "ali-account-1",
      "totalTokens": 500000,
      "inputTokens": 350000,
      "outputTokens": 150000,
      "cachedTokens": 10000,
      "totalCostUsd": 50.00,
      "requestCount": 600,
      "successCount": 590,
      "errorCount": 10,
      "avgDurationMs": 2100,
      "quota": {
        "provider": "ali-account-1",
        "monthlyTokenBudget": 1000000,
        "monthlyCostBudgetUsd": 100.00,
        "tokensUsed": 500000,
        "costUsedUsd": 50.00,
        "usagePercent": 50.0,
        "alertThreshold": 0.8,
        "resetDay": 1,
        "periodStart": "2026-03-01T00:00:00Z",
        "daysUntilReset": 10
      }
    },
    {
      "provider": "zhipu-account-1",
      "totalTokens": 300000,
      "inputTokens": 200000,
      "outputTokens": 100000,
      "cachedTokens": 0,
      "totalCostUsd": 30.00,
      "requestCount": 400,
      "successCount": 395,
      "errorCount": 5,
      "avgDurationMs": 1800,
      "quota": {
        "provider": "zhipu-account-1",
        "monthlyTokenBudget": 500000,
        "monthlyCostBudgetUsd": 50.00,
        "tokensUsed": 300000,
        "costUsedUsd": 30.00,
        "usagePercent": 60.0,
        "alertThreshold": 0.9,
        "resetDay": 1,
        "periodStart": "2026-03-01T00:00:00Z",
        "daysUntilReset": 10
      }
    },
    {
      "provider": "openai-account-1",
      "totalTokens": 100000,
      "inputTokens": 70000,
      "outputTokens": 30000,
      "cachedTokens": 0,
      "totalCostUsd": 15.00,
      "requestCount": 150,
      "successCount": 148,
      "errorCount": 2,
      "avgDurationMs": 2500,
      "quota": null
    }
  ]
}
```

#### Quota Field (when `includeQuota=true`)

| Field | Type | Nullable | Description |
|-------|------|----------|-------------|
| provider | string | No | Provider name |
| monthlyTokenBudget | number | Yes | Token budget per month (null = unlimited) |
| monthlyCostBudgetUsd | number | Yes | Cost budget per month (null = unlimited) |
| tokensUsed | number | No | Tokens used in current period |
| costUsedUsd | number | No | Cost used in current period |
| usagePercent | number | No | Percentage of budget used |
| alertThreshold | number | No | Alert threshold (0.0-1.0) |
| resetDay | number | No | Day of month quota resets (1-28) |
| periodStart | string | No | ISO 8601 timestamp of period start |
| daysUntilReset | number | No | Days until next reset |

---

## 4. By Model

### `GET /tokens/by-model`

Get token statistics grouped by model.

#### Query Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| period | string | No | `month` | `today`, `week`, `month` |
| limit | number | No | 20 | Max models to return |

#### Request

```http
GET /zz/api/tokens/by-model?period=month&limit=10 HTTP/1.1
```

#### Response `200 OK`

```json
{
  "period": "month",
  "models": [
    {
      "model": "qwen-plus",
      "totalTokens": 300000,
      "inputTokens": 200000,
      "outputTokens": 100000,
      "cachedTokens": 5000,
      "totalCostUsd": 30.00,
      "requestCount": 400,
      "successCount": 395,
      "errorCount": 5,
      "avgDurationMs": 2000,
      "avgInputTokens": 500,
      "avgOutputTokens": 250
    },
    {
      "model": "gpt-4",
      "totalTokens": 200000,
      "inputTokens": 140000,
      "outputTokens": 60000,
      "cachedTokens": 0,
      "totalCostUsd": 25.00,
      "requestCount": 250,
      "successCount": 248,
      "errorCount": 2,
      "avgDurationMs": 2800,
      "avgInputTokens": 560,
      "avgOutputTokens": 240
    },
    {
      "model": "claude-3-sonnet",
      "totalTokens": 150000,
      "inputTokens": 100000,
      "outputTokens": 50000,
      "cachedTokens": 3000,
      "totalCostUsd": 20.00,
      "requestCount": 200,
      "successCount": 198,
      "errorCount": 2,
      "avgDurationMs": 2200,
      "avgInputTokens": 500,
      "avgOutputTokens": 250
    }
  ],
  "meta": {
    "totalModels": 15,
    "returnedModels": 10,
    "period": "month"
  }
}
```

---

## 5. Quota Management

### `GET /quotas`

List all provider quota configurations.

#### Request

```http
GET /zz/api/quotas HTTP/1.1
```

#### Response `200 OK`

```json
{
  "quotas": [
    {
      "provider": "ali-account-1",
      "monthlyTokenBudget": 1000000,
      "monthlyCostBudgetUsd": 100.00,
      "alertThreshold": 0.8,
      "resetDay": 1,
      "currentUsage": {
        "tokensUsed": 500000,
        "costUsedUsd": 50.00,
        "requestCount": 600,
        "periodStart": "2026-03-01T00:00:00Z",
        "usagePercent": 50.0
      }
    },
    {
      "provider": "zhipu-account-1",
      "monthlyTokenBudget": 500000,
      "monthlyCostBudgetUsd": 50.00,
      "alertThreshold": 0.9,
      "resetDay": 1,
      "currentUsage": {
        "tokensUsed": 300000,
        "costUsedUsd": 30.00,
        "requestCount": 400,
        "periodStart": "2026-03-01T00:00:00Z",
        "usagePercent": 60.0
      }
    }
  ]
}
```

### `PUT /quotas`

Create or update provider quota configurations.

#### Request

```http
PUT /zz/api/quotas HTTP/1.1
Content-Type: application/json

{
  "quotas": [
    {
      "provider": "ali-account-1",
      "monthlyTokenBudget": 1000000,
      "monthlyCostBudgetUsd": 100.00,
      "alertThreshold": 0.8,
      "resetDay": 1
    },
    {
      "provider": "new-provider",
      "monthlyTokenBudget": 500000,
      "monthlyCostBudgetUsd": 50.00,
      "alertThreshold": 0.9,
      "resetDay": 15
    }
  ]
}
```

#### Request Fields

| Field | Type | Required | Constraints | Description |
|-------|------|----------|-------------|-------------|
| provider | string | Yes | Non-empty, existing provider | Provider name |
| monthlyTokenBudget | number | No | > 0, < 10^12 | Token budget per month (null = unlimited) |
| monthlyCostBudgetUsd | number | No | > 0, < 10^6 | Cost budget per month (null = unlimited) |
| alertThreshold | number | No | 0.5 - 1.0 | Alert when usage exceeds this threshold |
| resetDay | number | No | 1 - 28 | Day of month quota resets |

#### Response `200 OK`

```json
{
  "success": true,
  "updated": ["ali-account-1"],
  "created": ["new-provider"]
}
```

#### Error Responses

**400 Bad Request** - Validation errors

```json
{
  "error": "Invalid quota configuration",
  "details": [
    {
      "field": "alertThreshold",
      "message": "Threshold must be between 0.5 and 1.0",
      "value": 1.5
    },
    {
      "field": "resetDay",
      "message": "Reset day must be between 1 and 28",
      "value": 31
    }
  ],
  "code": "E004"
}
```

**404 Not Found** - Provider doesn't exist

```json
{
  "error": "Provider not found: unknown-provider",
  "code": "E003"
}
```

### `DELETE /quotas/{provider}`

Remove quota configuration for a provider.

#### Request

```http
DELETE /zz/api/quotas/ali-account-1 HTTP/1.1
```

#### Response `200 OK`

```json
{
  "removed": "ali-account-1"
}
```

---

## 6. Model Pricing

### `GET /pricing`

List all model pricing configurations.

#### Request

```http
GET /zz/api/pricing HTTP/1.1
```

#### Response `200 OK`

```json
{
  "defaultPricing": {
    "inputPricePer1k": 0.001,
    "outputPricePer1k": 0.002
  },
  "modelPricing": [
    {
      "modelPattern": "claude-3-opus",
      "inputPricePer1k": 0.015,
      "outputPricePer1k": 0.075,
      "effectiveFrom": "2026-01-01T00:00:00Z",
      "effectiveUntil": null
    },
    {
      "modelPattern": "gpt-4*",
      "inputPricePer1k": 0.03,
      "outputPricePer1k": 0.06,
      "effectiveFrom": "2026-01-01T00:00:00Z",
      "effectiveUntil": null
    },
    {
      "modelPattern": "qwen-*",
      "inputPricePer1k": 0.0004,
      "outputPricePer1k": 0.0012,
      "effectiveFrom": "2026-01-01T00:00:00Z",
      "effectiveUntil": null
    }
  ]
}
```

### `PUT /pricing`

Update model pricing configurations.

#### Request

```http
PUT /zz/api/pricing HTTP/1.1
Content-Type: application/json

{
  "defaultPricing": {
    "inputPricePer1k": 0.001,
    "outputPricePer1k": 0.002
  },
  "modelPricing": [
    {
      "modelPattern": "claude-3-opus",
      "inputPricePer1k": 0.015,
      "outputPricePer1k": 0.075
    },
    {
      "modelPattern": "claude-3-sonnet",
      "inputPricePer1k": 0.003,
      "outputPricePer1k": 0.015
    },
    {
      "modelPattern": "gpt-4*",
      "inputPricePer1k": 0.03,
      "outputPricePer1k": 0.06
    }
  ]
}
```

#### Request Fields

| Field | Type | Required | Constraints | Description |
|-------|------|----------|-------------|-------------|
| modelPattern | string | Yes | Valid glob pattern | Model name or pattern |
| inputPricePer1k | number | Yes | >= 0 | USD per 1000 input tokens |
| outputPricePer1k | number | Yes | >= 0 | USD per 1000 output tokens |

#### Response `200 OK`

```json
{
  "success": true,
  "updated": 3
}
```

#### Error Responses

**400 Bad Request** - Invalid pattern

```json
{
  "error": "Invalid pricing pattern",
  "details": [
    {
      "field": "modelPattern",
      "message": "Pattern cannot be empty"
    }
  ],
  "code": "E005"
}
```

---

## 7. Export

### `GET /tokens/export`

Export token usage data.

#### Query Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| format | string | Yes | - | `csv` or `json` |
| start | string | Yes | - | Start timestamp (ISO 8601) |
| end | string | Yes | - | End timestamp (ISO 8601) |
| provider | string | No | all | Filter by provider |
| model | string | No | all | Filter by model |
| limit | number | No | 10000 | Max records (max 100000) |

#### CSV Export

```http
GET /zz/api/tokens/export?format=csv&start=2026-03-01T00:00:00Z&end=2026-03-22T00:00:00Z HTTP/1.1
```

**Response** `200 OK`

```
Content-Type: text/csv
Content-Disposition: attachment; filename="token_export_2026-03-22.csv"

timestamp,provider,model,input_tokens,output_tokens,total_tokens,cost_usd,status,duration_ms
2026-03-01T00:05:23Z,ali-account-1,qwen-plus,500,200,700,0.07,200,2300
2026-03-01T00:10:45Z,zhipu-account-1,glm-4,300,150,450,0.045,200,1800
...
```

#### JSON Export

```http
GET /zz/api/tokens/export?format=json&start=2026-03-01T00:00:00Z&end=2026-03-22T00:00:00Z HTTP/1.1
```

**Response** `200 OK`

```json
{
  "exportedAt": "2026-03-22T10:30:00Z",
  "start": "2026-03-01T00:00:00Z",
  "end": "2026-03-22T00:00:00Z",
  "recordCount": 1500,
  "records": [
    {
      "timestamp": "2026-03-01T00:05:23Z",
      "provider": "ali-account-1",
      "model": "qwen-plus",
      "inputTokens": 500,
      "outputTokens": 200,
      "totalTokens": 700,
      "costUsd": 0.07,
      "status": 200,
      "durationMs": 2300
    }
  ]
}
```

#### Error Responses

**400 Bad Request** - Invalid format

```json
{
  "error": "Invalid export format: must be 'csv' or 'json'",
  "code": "E002"
}
```

**400 Bad Request** - Too many records

```json
{
  "error": "Export limit exceeded: max 100000 records, requested 150000",
  "code": "E002"
}
```

---

## 8. Request Logs with Tokens

### `GET /logs` (Extended)

Existing logs endpoint extended with token fields.

#### Query Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| limit | number | No | 100 | Max entries (max 1000) |
| offset | number | No | 0 | Pagination offset |
| status | string | No | all | `2xx`, `4xx`, `5xx`, `error` |
| provider | string | No | all | Filter by provider |
| search | string | No | - | Search in path/model |
| includeTokens | boolean | No | true | Include token fields |

#### Response `200 OK`

```json
{
  "logs": [
    {
      "id": "req_abc123",
      "timestamp": "2026-03-22T10:30:00Z",
      "method": "POST",
      "path": "/v1/chat/completions",
      "provider": "ali-account-1",
      "status": 200,
      "durationMs": 2300,
      "ttfbMs": 800,
      "model": "qwen-plus",
      "streaming": true,
      "requestBytes": 1200,
      "responseBytes": 3400,
      "failoverChain": null,
      "inputTokens": 500,
      "outputTokens": 200,
      "totalTokens": 700,
      "cachedTokens": null,
      "costUsd": 0.07
    }
  ],
  "total": 1500,
  "offset": 0,
  "limit": 100
}
```

---

## 9. WebSocket Events

### Token Update Event

Sent after each successful request with token data.

```json
{
  "type": "token_update",
  "data": {
    "provider": "ali-account-1",
    "model": "qwen-plus",
    "inputTokens": 500,
    "outputTokens": 200,
    "totalTokens": 700,
    "costUsd": 0.07,
    "timestamp": "2026-03-22T10:30:00Z"
  }
}
```

### Quota Alert Event

Sent when provider usage exceeds alert threshold.

```json
{
  "type": "quota_alert",
  "data": {
    "provider": "ali-account-1",
    "usagePercent": 85.5,
    "threshold": 80.0,
    "tokensUsed": 855000,
    "tokenBudget": 1000000,
    "costUsedUsd": 85.50,
    "costBudgetUsd": 100.00,
    "message": "Provider ali-account-1 has exceeded 80% quota usage",
    "daysUntilReset": 10
  }
}
```

### Quota Exceeded Event

Sent when provider quota is exhausted.

```json
{
  "type": "quota_exceeded",
  "data": {
    "provider": "ali-account-1",
    "usagePercent": 100.0,
    "tokensUsed": 1000000,
    "tokenBudget": 1000000,
    "message": "Provider ali-account-1 quota exhausted",
    "action": "Provider will be skipped for routing"
  }
}
```

---

## 10. Error Code Reference

| Code | HTTP Status | Description | User Action |
|------|-------------|-------------|-------------|
| E001 | 400 | Invalid time range | Check start/end format (ISO 8601) |
| E002 | 400 | Invalid parameter value | Check granularity, format, limit values |
| E003 | 404 | Resource not found | Check provider/model name |
| E004 | 400 | Invalid quota configuration | Check budget > 0, threshold 0.5-1.0, reset day 1-28 |
| E005 | 400 | Invalid pricing configuration | Check pattern syntax, prices >= 0 |
| E006 | 500 | Storage unavailable | Check database file, permissions |
| E007 | 500 | Migration failed | Check logs, may need manual intervention |
| E008 | 503 | Write queue full | Reduce request rate or wait |

---

## 11. Rate Limiting

Token API endpoints are not rate-limited separately from the proxy itself.

---

## 12. Caching

| Endpoint | Cache Duration | Cache Key |
|----------|----------------|-----------|
| `/tokens/summary` | 5 seconds | None |
| `/tokens/timeseries` | 1 minute | Query params hash |
| `/tokens/by-provider` | 5 seconds | Period + includeQuota |
| `/tokens/by-model` | 5 seconds | Period + limit |
| `/quotas` | No cache | - |
| `/pricing` | No cache | - |

---

**Document Version**: 1.0
**Last Updated**: 2026-03-22