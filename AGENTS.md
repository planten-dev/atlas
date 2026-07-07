# AGENTS.md

## Project

Atlas is a company internal management system.

Current repository structure:

- `backend/`: Rust backend service
- `frontend/`: React frontend application
- `docs/`: project documentation

## Backend Stack

- Rust
- Axum
- Tokio
- SeaORM
- PostgreSQL
- tracing
- serde

## Frontend Stack

- React
- TypeScript
- Vite

## Development Rules

- Make small, reviewable changes.
- Do not introduce unrelated features.
- Do not modify files outside the task scope.
- Do not add new dependencies unless the task explicitly requires them.
- Do not create commits unless explicitly requested.
- Prefer simple and explicit code over clever abstractions.
- Keep business logic out of HTTP handlers when possible.
- Prefer clear module boundaries:
  - `handlers/` for HTTP request/response handling
  - `services/` for business logic
  - `repositories/` for database access
  - `entities/` for SeaORM entities
  - `dto/` for request/response types
  - `middleware/` for auth, audit, tracing, etc.

## Agent Rules

Before editing code, the agent should:

1. Restate the task.
2. List files it expects to modify.
3. Mention whether new dependencies are required.
4. Avoid changing files outside the task scope.

After editing code, the agent should:

1. Summarize changed files.
2. List added dependencies, if any.
3. Explain how to verify the change.
4. Mention whether tests or checks were run.
5. If checks were not run, explain why.
