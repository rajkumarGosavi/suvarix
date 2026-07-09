.PHONY: setup dev dev-ui build release release-gami test test-unit test-frontend test-all lint clean status

# Windows-only: Git's bundled perl (cygwin-style paths) breaks openssl-sys build script.
# Set only here (not repo .cargo/config.toml) so it never leaks into mac/linux CI runners.
OPENSSL_SRC_PERL := C:/Strawberry/perl/bin/perl.exe

# make's default shell is cmd.exe on Windows, which doesn't understand `VAR=val cmd`
# recipe syntax — force Git Bash so the OPENSSL_SRC_PERL= prefix below actually works.
ifeq ($(OS),Windows_NT)
SHELL := C:/Program Files/Git/bin/bash.exe
endif

# Install frontend deps
setup:
	pnpm install

# Full app — Vite HMR + Rust backend
dev:
	OPENSSL_SRC_PERL=$(OPENSSL_SRC_PERL) pnpm tauri dev

# Full app with gamification feature
dev-gami:
	OPENSSL_SRC_PERL=$(OPENSSL_SRC_PERL) pnpm tauri dev -- --features gamification

# Frontend only (no Rust backend)
dev-ui:
	pnpm dev

# Type-check + prod frontend build
build:
	pnpm build

# Release bundle (all targets)
release:
	OPENSSL_SRC_PERL=$(OPENSSL_SRC_PERL) pnpm tauri build

# Release bundle with gamification
release-gami:
	OPENSSL_SRC_PERL=$(OPENSSL_SRC_PERL) pnpm tauri build -- --features gamification

# Rust unit tests only (inline #[cfg(test)] modules, no tests/ integration binaries)
test-unit:
	OPENSSL_SRC_PERL=$(OPENSSL_SRC_PERL) cargo test --manifest-path src-tauri/Cargo.toml --lib

# All Rust tests (unit + tests/ integration binaries)
test:
	OPENSSL_SRC_PERL=$(OPENSSL_SRC_PERL) cargo test --manifest-path src-tauri/Cargo.toml

# Frontend unit tests (vitest)
test-frontend:
	pnpm test

# Everything: Rust (unit + integration) + frontend
test-all: test test-frontend

# Rust lint
lint:
	OPENSSL_SRC_PERL=$(OPENSSL_SRC_PERL) cargo clippy --manifest-path src-tauri/Cargo.toml

# Clean Rust + frontend + generated mobile project build artifacts
clean:
	cargo clean --manifest-path src-tauri/Cargo.toml
	node -e "['dist','coverage','src-tauri/gen'].forEach(d=>require('fs').rmSync(d,{recursive:true,force:true}))"

# Check for leftover dev processes (Vite port 1420, suvarix.exe) that block clean/dev
status:
	powershell -NoProfile -File scripts/status.ps1
