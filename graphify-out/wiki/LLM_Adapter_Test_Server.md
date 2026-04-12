# LLM Adapter Test Server

> 41 nodes · cohesion 0.05

## Key Concepts

- **TestIntervalsServer** (24 connections) — `tests\intervals_adapters\support\server.rs`
- **server.rs** (8 connections) — `tests\llm_rest\support\server.rs`
- **TestLlmUpstreamServer** (6 connections) — `tests\llm_rest\support\server.rs`
- **capture_request()** (5 connections) — `tests\llm_rest\support\server.rs`
- **server.rs** (3 connections) — `tests\intervals_adapters\support\server.rs`
- **CapturedRequest** (2 connections) — `tests\llm_rest\support\server.rs`
- **gemini_cache_handler()** (2 connections) — `tests\llm_rest\support\server.rs`
- **gemini_generate_handler()** (2 connections) — `tests\llm_rest\support\server.rs`
- **openai_handler()** (2 connections) — `tests\llm_rest\support\server.rs`
- **openrouter_handler()** (2 connections) — `tests\llm_rest\support\server.rs`
- **ServerState** (2 connections) — `tests\intervals_adapters\support\server.rs`
- **.default()** (2 connections) — `tests\intervals_adapters\support\server.rs`
- **.start()** (2 connections) — `tests\intervals_adapters\support\server.rs`
- **MockServerState** (1 connections) — `tests\llm_rest\support\server.rs`
- **.base_url()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- **.push_activity()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- **.push_event()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- **.requests()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- **.set_activity()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- **.set_activity_intervals()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- **.set_activity_intervals_raw()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- **.set_activity_intervals_status()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- **.set_activity_with_intervals()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- **.set_created_event()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- **.set_created_event_failure()** (1 connections) — `tests\intervals_adapters\support\server.rs`
- *... and 16 more nodes in this community*

## Relationships

- No strong cross-community connections detected

## Source Files

- `tests\intervals_adapters\support\server.rs`
- `tests\llm_rest\support\server.rs`

## Audit Trail

- EXTRACTED: 90 (100%)
- INFERRED: 0 (0%)
- AMBIGUOUS: 0 (0%)

---

*Part of the graphify knowledge wiki. See [[index]] to navigate.*