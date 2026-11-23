use runit_core::run::*;

#[test]
fn reads_streamed_text_thoughts_and_speed() {
    assert_eq!(parse_event(r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#), vec![Piece::Text("Hel".into())]);
    assert_eq!(parse_event(r#"data: {"choices":[{"delta":{"reasoning_content":"hmm"}}]}"#), vec![Piece::Thought("hmm".into())]);
    assert_eq!(parse_event(r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}],"timings":{"predicted_per_second":41.5,"prompt_per_second":300.0}}"#), vec![Piece::Speed { tokens_per_s: 41.5, prompt_tokens_per_s: 300.0 }]);
    assert_eq!(parse_event("data: [DONE]"), vec![Piece::End]);
    for quiet in ["", ": ping", "data: {broken", r#"data: {"choices":[{"delta":{"content":null,"role":"assistant"}}]}"#] { assert!(parse_event(quiet).is_empty(), "{quiet}"); }
}
