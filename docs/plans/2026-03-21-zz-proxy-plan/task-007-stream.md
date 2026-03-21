# Task 007: Stream Module - SSE Support

## Goal
Implement SSE (Server-Sent Events) streaming with zero buffering and chunked transfer.

## BDD Scenarios

```gherkin
Scenario: Detect SSE request from Accept header
  Given request header Accept = "text/event-stream"
  When is_sse_request() is called
  Then returns true

Scenario: Detect SSE request from Content-Type
  Given request header Content-Type = "application/json"
  And request body contains "stream": true
  When is_sse_request() is called
  Then returns true (parse minimal JSON to check stream field)

Scenario: Pipe SSE chunks in real-time
  Given upstream sends SSE event chunks
  When stream_response() pipes data to client
  Then client receives each chunk immediately
  And no buffering occurs

Scenario: Handle chunked transfer encoding
  Given upstream response uses chunked encoding
  When stream_response() processes it
  Then chunks are forwarded with proper encoding
  And client can parse SSE events correctly

Scenario: Don't retry on mid-stream failure
  Given SSE streaming is in progress
  And upstream connection drops mid-stream
  When error occurs
  Then proxy returns error to client
  And does NOT retry on next provider (streaming already started)
```

## Files to Create/Edit

**Create**:
- `src/stream.rs` - Complete implementation

## Implementation Steps

1. Implement SSE detection:
   - `is_sse_request(req: &Request<Body>) -> bool`
   - Check `Accept: text/event-stream` header
   - Optionally parse request body JSON to check `stream: true` field

2. Implement streaming pipe:
   - Use `hyper_util::rt::TokioIo` to wrap streams
   - Use `tokio::io::copy_bidirectional` or manual chunk reading/writing
   - Forward chunks immediately without buffering
   - Preserve chunk boundaries

3. Implement chunked transfer handling:
   - Set `Transfer-Encoding: chunked` header on response
   - Forward upstream chunks directly to downstream
   - Handle SSE event format (data:, event:, id:, retry: fields)

4. Implement stream proxy function:
   - `proxy_stream(upstream: Response<Incoming>, downstream: &mut Response<Full<Bytes>>) -> Result<()>`
   - Handle both directions if needed (bidirectional streaming)

## Verification

Run:
```bash
cargo build
```

Manual test (requires running server):
```bash
# Test with curl
curl -N -H "Accept: text/event-stream" http://localhost:9090/v1/chat/completions -d '{
  "model": "qwen-plus",
  "messages": [{"role": "user", "content": "hi"}],
  "stream": true
}'
```

Expected: Stream events appear in real-time with no buffering.

## Dependencies
- Task 006 (Error handling for stream failures)
