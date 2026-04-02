# OpenRouter Logging Design

The immediate problem is that `/api/settings/ai-agents/test` returns `503` when OpenRouter fails, but the current logs only show the outer REST failure and not the upstream OpenRouter response. The change should stay narrow: add diagnostics only inside the OpenRouter adapter so the rest of the request flow and error mapping remain unchanged.

Chosen approach:
- add structured logging in `src/adapters/llm/openrouter/client.rs`
- log request metadata only: provider, model, endpoint URL, message count, and whether system messages are present
- log transport failures with the reqwest error text
- log non-success upstream responses with status and a truncated response body
- avoid logging API keys or full prompt contents

Why this approach:
- smallest correct change for the current debugging need
- keeps the REST handler thin and unchanged
- improves root-cause visibility for live OpenRouter failures without increasing secret exposure

Verification plan:
- add a focused unit test for the body-truncation helper
- run focused adapter tests and the OpenRouter REST integration test after the change
