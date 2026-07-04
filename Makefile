.PHONY: setup dev dev-ui build release release-gami test test-unit test-frontend test-all lint clean

# Install frontend deps
setup:
	pnpm install

# Full app — Vite HMR + Rust backend
dev:
	pnpm tauri dev

# Full app with gamification feature
dev-gami:
	pnpm tauri dev -- --features gamification

# Frontend only (no Rust backend)
dev-ui:
	pnpm dev

# Type-check + prod frontend build
build:
	pnpm build

# Release bundle (all targets)
release:
	pnpm tauri build

# Release bundle with gamification
release-gami:
	pnpm tauri build -- --features gamification

# Rust unit tests only (inline #[cfg(test)] modules, no tests/ integration binaries)
test-unit:
	cargo test --manifest-path src-tauri/Cargo.toml --lib

# All Rust tests (unit + tests/ integration binaries)
test:
	cargo test --manifest-path src-tauri/Cargo.toml

# Frontend unit tests (vitest)
test-frontend:
	pnpm test

# Everything: Rust (unit + integration) + frontend
test-all: test test-frontend

# Rust lint
lint:
	cargo clippy --manifest-path src-tauri/Cargo.toml

# Clean Rust + frontend build artifacts
clean:
	cargo clean --manifest-path src-tauri/Cargo.toml
	pnpm exec rimraf dist
