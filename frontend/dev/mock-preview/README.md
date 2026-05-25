## Mock Preview

Local mock API presets for quickly reviewing frontend changes without the real backend.

### Presets

- `balanced`: populated default preview across dashboard, calendar, coach, settings, races, and admin.
- `mobile-focus`: denser data for mobile layout review.
- `empty`: mostly empty states.

### Run

Terminal 1:

```bash
PREVIEW_PRESET=mobile-focus bun run --cwd frontend mock:preview
```

Optional port override:

```bash
PREVIEW_PORT=4011 PREVIEW_PRESET=mobile-focus bun run --cwd frontend mock:preview
```

Terminal 2:

```bash
BACKEND_PROXY_TARGET=http://127.0.0.1:4010 bun run --cwd frontend dev
```

Then open `http://127.0.0.1:5173`.

### Notes

- The mock server keeps state in memory only.
- Workout coach and calendar coach websocket replies are demo responses.
- Settings save/test actions update only the in-memory preset state.
- Keep `VITE_API_BASE_URL` empty for preview so frontend requests stay same-origin and go through the Vite proxy.
