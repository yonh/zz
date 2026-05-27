//! Integration test for streaming SSE conversion
//! Tests true streaming conversion where chunks are converted on-the-fly
//! instead of collecting the entire response first.

use zz::converter::stream::StreamConverter;
use zz::converter::ApiType;

#[tokio::test]
async fn test_streaming_conversion_oa_to_an() {
    // Test OpenAI to Anthropic streaming conversion
    let mut converter = StreamConverter::new(ApiType::OpenAIChat, ApiType::Anthropic);
    
    // Simulate OpenAI SSE chunks
    let chunk1 = b"data: {\"id\":\"chatcmpl-123\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"}}]}\n\n";
    let chunk2 = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n";
    let chunk3 = b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"completion_tokens\":5}}\n\n";
    let chunk4 = b"data: [DONE]\n\n";
    
    let output1 = converter.push(chunk1).unwrap();
    let output2 = converter.push(chunk2).unwrap();
    let output3 = converter.push(chunk3).unwrap();
    let output4 = converter.push(chunk4).unwrap();
    
    // Should have output from first chunk (message_start)
    assert!(!output1.is_empty());
    let output1_str = std::str::from_utf8(&output1[0]).unwrap();
    assert!(output1_str.contains("message_start"));
    
    // Should have output from second chunk (content_block_start + content_block_delta)
    assert!(!output2.is_empty());
    
    // Should have output from third chunk (content_block_stop + message_delta + message_stop)
    assert!(!output3.is_empty());
    
    // Finalize should produce nothing (already finalized by [DONE])
    let final_chunks = converter.finalize().unwrap();
    assert!(final_chunks.is_empty());
}

#[tokio::test]
async fn test_streaming_conversion_an_to_oa() {
    // Test Anthropic to OpenAI streaming conversion
    let mut converter = StreamConverter::new(ApiType::Anthropic, ApiType::OpenAIChat);
    
    let chunk1 = b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg-123\",\"model\":\"claude-3\",\"role\":\"assistant\",\"content\":[],\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n";
    let chunk2 = b"event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
    let chunk3 = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n";
    let chunk4 = b"event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n";
    let chunk5 = b"event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":5}}\n\n";
    let chunk6 = b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    
    let output1 = converter.push(chunk1).unwrap();
    let output2 = converter.push(chunk2).unwrap();
    let output3 = converter.push(chunk3).unwrap();
    let _output4 = converter.push(chunk4).unwrap();
    let output5 = converter.push(chunk5).unwrap();
    let output6 = converter.push(chunk6).unwrap();
    
    // message_start and content_block_start are cached
    assert!(output1.is_empty());
    assert!(output2.is_empty());
    
    // content_block_delta produces OpenAI delta
    assert!(!output3.is_empty());
    let output3_str = std::str::from_utf8(&output3[0]).unwrap();
    assert!(output3_str.contains("\"content\":\"Hello\""));
    
    // message_delta produces finish_reason
    assert!(!output5.is_empty());
    let output5_str = std::str::from_utf8(&output5[0]).unwrap();
    assert!(output5_str.contains("\"finish_reason\":\"stop\""));
    
    // message_stop produces [DONE]
    assert!(!output6.is_empty());
    let output6_str = std::str::from_utf8(&output6[0]).unwrap();
    assert!(output6_str.contains("[DONE]"));
}

#[tokio::test]
async fn test_streaming_handles_partial_chunks() {
    // Test that the streaming converter handles chunks that don't end at event boundaries
    let mut converter = StreamConverter::new(ApiType::OpenAIChat, ApiType::Anthropic);
    
    // Split an event across two chunks
    let chunk1 = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content";
    let chunk2 = b":\"Hello\"}}]}\n\n";
    
    let output1 = converter.push(chunk1).unwrap();
    assert!(output1.is_empty()); // Buffering incomplete event
    
    let output2 = converter.push(chunk2).unwrap();
    assert!(!output2.is_empty()); // Event completed and converted
}

#[tokio::test]
async fn test_streaming_order_preservation() {
    // Test that chunks are converted and sent in the correct order
    let mut converter = StreamConverter::new(ApiType::OpenAIChat, ApiType::Anthropic);
    
    let chunks: Vec<Vec<u8>> = vec![
        b"data: {\"id\":\"test\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"}}]}\n\n".to_vec(),
        b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"A\"}}]}\n\n".to_vec(),
        b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"B\"}}]}\n\n".to_vec(),
        b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"C\"}}]}\n\n".to_vec(),
        b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n".to_vec(),
    ];
    
    let mut all_outputs = Vec::new();
    for chunk in chunks {
        let outputs = converter.push(&chunk).unwrap();
        all_outputs.extend(outputs);
    }
    
    // Verify we got outputs in order
    assert!(!all_outputs.is_empty());
    
    // Check that the message_start comes first
    let first_str = std::str::from_utf8(&all_outputs[0]).unwrap();
    assert!(first_str.contains("message_start"));
    
    // Check that content deltas appear
    let content_count = all_outputs.iter()
        .filter(|b| std::str::from_utf8(b).unwrap().contains("content_block_delta"))
        .count();
    assert!(content_count >= 3); // At least 3 content deltas for A, B, C
}
