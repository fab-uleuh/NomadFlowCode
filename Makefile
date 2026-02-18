include .env

IMAGE = nomadflow-docs

# ─── Version ──────────────────────────────────────────

.PHONY: bump

## Bump version everywhere (interactive prompt)
bump:
	@read -p "New version (e.g. 1.2.0): " VERSION && \
	if [ -z "$$VERSION" ]; then echo "Aborted."; exit 1; fi && \
	echo "Updating to $$VERSION..." && \
	sed -i '' 's/"version": "[0-9]*\.[0-9]*\.[0-9]*"/"version": "'$$VERSION'"/' nomadflowcode/app.json && \
	sed -i '' 's/"runtimeVersion": "[0-9]*\.[0-9]*\.[0-9]*"/"runtimeVersion": "'$$VERSION'"/' nomadflowcode/app.json && \
	sed -i '' 's/"version": "[0-9]*\.[0-9]*\.[0-9]*"/"version": "'$$VERSION'"/' nomadflowcode/package.json && \
	sed -i '' 's/"version": "[0-9]*\.[0-9]*\.[0-9]*"/"version": "'$$VERSION'"/' docs/package.json && \
	for f in nomadflow-rs/Cargo.toml \
		nomadflow-rs/crates/nomadflow-core/Cargo.toml \
		nomadflow-rs/crates/nomadflow-server/Cargo.toml \
		nomadflow-rs/crates/nomadflow-tui/Cargo.toml \
		nomadflow-rs/crates/nomadflow-relay/Cargo.toml \
		nomadflow-rs/crates/nomadflow-ws/Cargo.toml; do \
		sed -i '' 's/^version = "[0-9]*\.[0-9]*\.[0-9]*"/version = "'$$VERSION'"/' $$f; \
	done && \
	echo "Done! All files updated to $$VERSION"

# ─── Docs ──────────────────────────────────────────────

.PHONY: docs-sync docs-build docs-up docs-down docs-logs docs-deploy docs-caddy

## Sync docs source to VPS
docs-sync:
	rsync -avz --delete \
		--exclude node_modules --exclude .next --exclude out \
		docs/ $(VPS_HOST):$(VPS_DOCS_DIR)/

## Build Docker image on VPS
docs-build:
	ssh $(VPS_HOST) "cd $(VPS_DOCS_DIR) && docker build -t $(IMAGE) ."

## Start docs container on VPS
docs-up:
	ssh $(VPS_HOST) "docker rm -f $(IMAGE) 2>/dev/null; \
		docker run -d --name $(IMAGE) --restart unless-stopped \
		--network vps-network $(IMAGE)"

## Stop docs container
docs-down:
	ssh $(VPS_HOST) "docker rm -f $(IMAGE)"

## View docs container logs
docs-logs:
	ssh $(VPS_HOST) "docker logs -f $(IMAGE)"

## Update Caddy config with docs entry
docs-caddy:
	@ssh $(VPS_HOST) "grep -q '$(DOCS_DOMAIN)' /home/ubuntu/caddy/conf/Caddyfile && \
		echo 'Caddy already configured for $(DOCS_DOMAIN)' || \
		(printf '\n$(DOCS_DOMAIN) {\n    reverse_proxy $(IMAGE):3000\n}\n' | sudo tee -a /home/ubuntu/caddy/conf/Caddyfile > /dev/null && \
		docker exec caddy-caddy-1 caddy reload --config /etc/caddy/Caddyfile && \
		echo 'Caddy updated and reloaded')"

## Full deploy: sync → build → up → caddy
docs-deploy: docs-sync docs-build docs-up docs-caddy
	@echo "Docs deployed at https://$(DOCS_DOMAIN)"
