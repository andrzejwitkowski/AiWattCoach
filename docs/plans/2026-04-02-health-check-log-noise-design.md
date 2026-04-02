# Health Check Log Noise Design

The current HTTP trace layer logs successful `/health` and `/ready` requests at `INFO`, which overwhelms the useful application logs in local Docker development. The requested change is narrower than suppressing all successes: keep the health-check completion log entries, but downgrade successful health/readiness responses to `DEBUG` so they disappear from the normal `info` log level while failures remain visible.

Chosen approach:
- change only `src/adapters/rest/mod.rs`
- detect successful `/health` and `/ready` responses inside the shared response logger
- log those successful health/readiness completions at `DEBUG`
- keep warnings/errors for health/readiness unchanged
- keep all other route logging unchanged

Why this approach:
- smallest behavior change that reduces the noisy Docker health-check traffic
- preserves failure visibility for readiness issues
- keeps normal API success logging intact

Verification plan:
- add a focused unit test around the route/status log-level decision if practical
- run the relevant REST/unit tests and `cargo fmt --all --check`
