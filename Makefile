.PHONY: setup dev dev-ui build release release-gami test lint clean

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

# Rust unit tests
test:
	cargo test --manifest-path src-tauri/Cargo.toml

# Rust lint
lint:
	cargo clippy --manifest-path src-tauri/Cargo.toml

# Clean Rust + frontend build artifacts
clean:
	cargo clean --manifest-path src-tauri/Cargo.toml
	pnpm exec rimraf dist
