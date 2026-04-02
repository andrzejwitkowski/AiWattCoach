# Model Suggestion Refresh Design

The AI settings form already allows any freeform model name, but its suggestion chips and provider autofill defaults are stale enough to steer users into invalid or outdated choices. The requested change is to refresh those suggestions using current provider model names while keeping custom input fully supported.

Chosen approach:
- update the curated static suggestion lists in `frontend/src/features/settings/components/AiAgentsCard.tsx`
- keep the freeform text input unchanged so users can always type any model they want
- use provider-native model names for direct OpenAI and Gemini calls
- use routed provider-prefixed model names for OpenRouter suggestions

Why this approach:
- smallest correct change with immediate UX value
- avoids backend/provider metadata work
- prevents suggestion chips from nudging users toward invalid provider/model combinations

Verification plan:
- update focused frontend tests for suggestion rendering and autofill defaults
- run the relevant frontend tests after the change
