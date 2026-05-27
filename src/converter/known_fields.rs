//! Known field constants for API conversion
//!
//! Maintains whitelists of known fields for each API type and direction.
//! Used to detect unknown fields that should be reported via telemetry.

/// Known fields in Anthropic request format (/v1/messages)
pub const KNOWN_FIELDS_REQUEST_ANTHROPIC: &[&str] = &[
    "model",
    "max_tokens",
    "messages",
    "system",
    "tools",
    "tool_choice",
    "stream",
    "temperature",
    "top_p",
    "top_k",
    "metadata",
    "stop_sequences",
    "betas",
];

/// Known fields in OpenAI Chat request format (/v1/chat/completions)
pub const KNOWN_FIELDS_REQUEST_OPENAI_CHAT: &[&str] = &[
    "model",
    "messages",
    "functions",
    "function_call",
    "tools",
    "tool_choice",
    "stream",
    "temperature",
    "top_p",
    "n",
    "stop",
    "max_tokens",
    "presence_penalty",
    "frequency_penalty",
    "logit_bias",
    "user",
];

/// Known fields in Anthropic response format
pub const KNOWN_FIELDS_RESPONSE_ANTHROPIC: &[&str] = &[
    "id",
    "type",
    "role",
    "content",
    "model",
    "stop_reason",
    "stop_sequence",
    "usage",
];

/// Known fields in OpenAI Chat response format
pub const KNOWN_FIELDS_RESPONSE_OPENAI_CHAT: &[&str] = &[
    "id",
    "object",
    "created",
    "model",
    "choices",
    "usage",
    "system_fingerprint",
];

/// Known nested field paths for Anthropic messages
pub const KNOWN_NESTED_PATHS_ANTHROPIC: &[&str] = &[
    "messages[].content",
    "messages[].role",
    "system",
    "tools[].name",
    "tools[].description",
    "tools[].input_schema",
    "tool_choice.type",
    "tool_choice.name",
];

/// Known nested field paths for OpenAI Chat messages
pub const KNOWN_NESTED_PATHS_OPENAI_CHAT: &[&str] = &[
    "messages[].content",
    "messages[].role",
    "messages[].tool_calls",
    "messages[].function_call",
    "tools[].type",
    "tools[].function.name",
    "tools[].function.description",
    "tools[].function.parameters",
    "tool_choice.type",
    "tool_choice.function",
];

/// Known fields in OpenAI Responses request format (/v1/responses)
pub const KNOWN_FIELDS_REQUEST_OPENAI_RESPONSES: &[&str] = &[
    "model",
    "input",
    "instructions",
    "tools",
    "tool_choice",
    "stream",
    "temperature",
    "top_p",
    "max_output_tokens",
    "metadata",
    "store",
    "previous_response_id",
    "stop",
];

/// Known fields in OpenAI Responses response format
pub const KNOWN_FIELDS_RESPONSE_OPENAI_RESPONSES: &[&str] = &[
    "id",
    "object",
    "created",
    "model",
    "output",
    "usage",
    "status",
];
