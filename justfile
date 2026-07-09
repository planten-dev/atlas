set shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

backend_manifest := "backend/Cargo.toml"
sqlite_file := "data/atlas-dev.sqlite"

default: list

list:
    just --list

dev: run

run: run-sqlite

run-sqlite:
    cargo run --manifest-path {{backend_manifest}} -- --database-kind sqlite-file --sqlite-file {{sqlite_file}}

run-postgres:
    cargo run --manifest-path {{backend_manifest}}

fmt:
    cargo fmt --manifest-path {{backend_manifest}}

fmt-check:
    cargo fmt --manifest-path {{backend_manifest}} -- --check

check: fmt-check cargo-check lint test docs-check

cargo-check:
    cargo check --manifest-path {{backend_manifest}}

lint:
    cargo clippy --manifest-path {{backend_manifest}} --all-targets

lint-strict:
    cargo clippy --manifest-path {{backend_manifest}} --all-targets -- -D warnings

test:
    cargo test --manifest-path {{backend_manifest}}

build:
    cargo build --manifest-path {{backend_manifest}}

clean:
    cargo clean --manifest-path {{backend_manifest}}

docs-check:
    Get-Content docs/openapi.json -Raw | ConvertFrom-Json | Out-Null
    Write-Host "docs/openapi.json is valid JSON"

frontend-install:
    npm --prefix frontend install

frontend-dev:
    npm --prefix frontend run dev

frontend-gen-api:
    npm --prefix frontend run gen:api

frontend-typecheck:
    npm --prefix frontend run typecheck

frontend-lint:
    npm --prefix frontend run lint

frontend-test:
    npm --prefix frontend run test

frontend-build:
    npm --prefix frontend run build

frontend-check: frontend-typecheck frontend-lint frontend-test frontend-build

check-all: check frontend-check
