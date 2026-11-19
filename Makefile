.PHONY: version tag help run test app

help:
	@echo "Usage:"
	@echo "  make version VERSION=x.y.z  - Update the workspace version in Cargo.toml"
	@echo "  make tag VERSION=x.y.z      - Update version, commit, tag, and push"
	@echo "  make run                    - Run the GPUI app (debug)"
	@echo "  make test                   - Run the Rust tests"
	@echo "  make app                    - Build target/release/Seer.app"

run:
	cargo run -p seer

test:
	cargo test --workspace

app:
	scripts/bundle.sh

version:
	@if [ -z "$(VERSION)" ]; then \
		echo "Error: VERSION not set. Usage: make version VERSION=x.y.z"; \
		exit 1; \
	fi
	@echo "Updating version to $(VERSION)..."
	@sed -i.bak 's/^version = ".*"/version = "$(VERSION)"/' Cargo.toml && rm Cargo.toml.bak
	@cargo update --workspace --offline >/dev/null 2>&1 || cargo update --workspace
	@echo "Version updated to $(VERSION)"

tag: version
	@if [ -z "$(VERSION)" ]; then \
		echo "Error: VERSION not set. Usage: make tag VERSION=x.y.z"; \
		exit 1; \
	fi
	@echo "Checking for changes..."
	@git add Cargo.toml Cargo.lock
	@if git diff --cached --quiet; then \
		echo "No version changes detected - version already $(VERSION)"; \
	else \
		echo "Committing version $(VERSION)..."; \
		git commit -m "Bump version to $(VERSION)"; \
	fi
	@echo "Creating and pushing tag v$(VERSION)..."
	@git tag v$(VERSION)
	@git push origin main
	@git push origin v$(VERSION)
	@echo "✓ Version $(VERSION) tagged and pushed"
