# Mongo Log Noise Design

Local Docker development currently shows a large amount of MongoDB connection churn in the container logs. The request is not to hide Mongo problems entirely, only to reduce the repetitive `NETWORK` and `ACCESS` connection chatter while keeping warnings, errors, and useful startup information visible.

Chosen approach:
- update `docker-compose-dev.yml` and `docker-compose.yml`
- add a local `mongod` command override with reduced component verbosity for noisy connection categories
- keep Mongo running normally and keep warnings/errors visible

Why this approach:
- smallest change that addresses the actual source of noise
- keeps application logging untouched
- applies consistently across both local compose entrypoints in the repo

Verification plan:
- validate the compose files after the change
- restart Mongo and confirm that repetitive connection accepted/ended logs are reduced while the service still starts normally
