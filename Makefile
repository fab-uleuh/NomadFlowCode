include .env

IMAGE = nomadflow-docs

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
