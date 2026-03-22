use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Token usage information from API response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    /// Optional detailed breakdown (cached, reasoning, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// A single log entry for a request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: String, // ISO 8601
    pub method: String,
    pub path: String,
    pub provider: String,
    pub status: u16,
    pub duration_ms: u64,
    pub ttfb_ms: u64,
    pub model: String,
    pub streaming: bool,
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub failover_chain: Option<Vec<String>>,
    /// Token usage from API response (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
}

/// Thread-safe ring buffer for request logs
pub struct RequestLogBuffer {
    buffer: Mutex<VecDeque<LogEntry>>,
    capacity: usize,
}

impl RequestLogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    pub fn push(&self, entry: LogEntry) {
        let mut buffer = self.buffer.lock().unwrap();
        if buffer.len() >= self.capacity {
            buffer.pop_front();
        }
        buffer.push_back(entry);
    }

    pub fn get_all(&self) -> Vec<LogEntry> {
        let buffer = self.buffer.lock().unwrap();
        buffer.iter().cloned().collect()
    }

    pub fn get_page(&self, offset: usize, limit: usize) -> Vec<LogEntry> {
        let buffer = self.buffer.lock().unwrap();
        buffer
            .iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn len(&self) -> usize {
        let buffer = self.buffer.lock().unwrap();
        buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for RequestLogBuffer {
    fn default() -> Self {
        Self::new(10000)
    }
}

/// Requests per minute counter with 60-second sliding window
pub struct RpmCounter {
    buckets: Mutex<[u64; 60]>,
    current_bucket: AtomicUsize,
    last_tick: Mutex<Instant>,
}

impl RpmCounter {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new([0; 60]),
            current_bucket: AtomicUsize::new(0),
            last_tick: Mutex::new(Instant::now()),
        }
    }

    fn tick(&self) {
        let mut last_tick = self.last_tick.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(*last_tick);
        let seconds_elapsed = elapsed.as_secs() as usize;

        if seconds_elapsed > 0 {
            let current = self.current_bucket.load(Ordering::Relaxed);

            // Clear buckets that have expired
            let mut buckets = self.buckets.lock().unwrap();
            for i in 1..=seconds_elapsed.min(60) {
                let bucket_to_clear = (current + i) % 60;
                buckets[bucket_to_clear] = 0;
            }

            // Update current bucket
            let new_current = (current + seconds_elapsed) % 60;
            self.current_bucket.store(new_current, Ordering::Relaxed);
            *last_tick = now;
        }
    }

    pub fn increment(&self) {
        self.tick();
        let current = self.current_bucket.load(Ordering::Relaxed);
        let mut buckets = self.buckets.lock().unwrap();
        buckets[current] += 1;
    }

    pub fn get_rpm(&self) -> f64 {
        self.tick();
        let buckets = self.buckets.lock().unwrap();
        let total: u64 = buckets.iter().sum();
        total as f64
    }
}

impl Default for RpmCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_log_buffer_push_and_get() {
        let buffer = RequestLogBuffer::new(5);

        for i in 0..3 {
            buffer.push(LogEntry {
                id: format!("req-{}", i),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                provider: "test".to_string(),
                status: 200,
                duration_ms: 100,
                ttfb_ms: 50,
                model: "gpt-4".to_string(),
                streaming: true,
                request_bytes: 100,
                response_bytes: 200,
                failover_chain: None,
                token_usage: None,
            });
        }

        assert_eq!(buffer.len(), 3);
        let all = buffer.get_all();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_request_log_buffer_capacity() {
        let buffer = RequestLogBuffer::new(3);

        for i in 0..5 {
            buffer.push(LogEntry {
                id: format!("req-{}", i),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                provider: "test".to_string(),
                status: 200,
                duration_ms: 100,
                ttfb_ms: 50,
                model: "gpt-4".to_string(),
                streaming: true,
                request_bytes: 100,
                response_bytes: 200,
                failover_chain: None,
                token_usage: None,
            });
        }

        assert_eq!(buffer.len(), 3);
        let all = buffer.get_all();
        assert_eq!(all[0].id, "req-2");
        assert_eq!(all[2].id, "req-4");
    }

    #[test]
    fn test_request_log_buffer_pagination() {
        let buffer = RequestLogBuffer::new(10);

        for i in 0..10 {
            buffer.push(LogEntry {
                id: format!("req-{}", i),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                provider: "test".to_string(),
                status: 200,
                duration_ms: 100,
                ttfb_ms: 50,
                model: "gpt-4".to_string(),
                streaming: true,
                request_bytes: 100,
                response_bytes: 200,
                failover_chain: None,
                token_usage: None,
            });
        }

        let page = buffer.get_page(5, 3);
        assert_eq!(page.len(), 3);
        assert_eq!(page[0].id, "req-5");
        assert_eq!(page[1].id, "req-6");
        assert_eq!(page[2].id, "req-7");
    }

    #[test]
    fn test_rpm_counter() {
        let counter = RpmCounter::new();

        for _ in 0..10 {
            counter.increment();
        }

        let rpm = counter.get_rpm();
        assert_eq!(rpm, 10.0);
    }
}
