.PHONY: version tag help

help:
	@echo "Usage:"
	@echo "  make version VERSION=x.y.z  - Update version in package.json and Cargo.toml"
	@echo "  make tag VERSION=x.y.z      - Update version, commit, tag, and push"

version:
	@if [ -z "$(VERSION)" ]; then \
		echo "Error: VERSION not set. Usage: make version VERSION=x.y.z"; \
		exit 1; \
	fi
	@echo "Updating version to $(VERSION)..."
	@sed -i.bak 's/"version": ".*"/"version": "$(VERSION)"/' package.json && rm package.json.bak
	@sed -i.bak 's/^version = ".*"/version = "$(VERSION)"/' src-tauri/Cargo.toml && rm src-tauri/Cargo.toml.bak
	@sed -i.bak '4s/"version": ".*"/"version": "$(VERSION)"/' src-tauri/tauri.conf.json && rm src-tauri/tauri.conf.json.bak
	@echo "Version updated to $(VERSION)"

tag: version
	@if [ -z "$(VERSION)" ]; then \
		echo "Error: VERSION not set. Usage: make tag VERSION=x.y.z"; \
		exit 1; \
	fi
	@echo "Checking for changes..."
	@git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json src-tauri/Cargo.lock
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
