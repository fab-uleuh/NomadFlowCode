include ../.env

VPS := $(VPS_HOST)
RELAY_DIR := /home/ubuntu/nomadflow-relay
RELAY_SRC := crates/nomadflow-relay

.PHONY: relay-deploy relay-logs relay-restart relay-status

## Deploy relay to VPS (rsync source + rebuild + restart)
relay-deploy:
	rsync -az --delete $(RELAY_SRC)/Cargo.toml $(RELAY_SRC)/Dockerfile $(VPS):$(RELAY_DIR)/
	rsync -az --delete $(RELAY_SRC)/src/ $(VPS):$(RELAY_DIR)/src/
	rsync -az --delete crates/nomadflow-ws/ $(VPS):$(RELAY_DIR)/nomadflow-ws/
	ssh $(VPS) "cd $(RELAY_DIR) && docker compose up -d --build"

## Show relay + bore logs
relay-logs:
	ssh $(VPS) "docker logs nomadflow-relay --tail 30 2>&1 && echo '---' && docker logs nomadflow-bore --tail 10 2>&1"

## Restart relay + bore without rebuilding
relay-restart:
	ssh $(VPS) "cd $(RELAY_DIR) && docker compose restart"

## Show relay + bore container status
relay-status:
	ssh $(VPS) "docker ps --filter 'name=nomadflow' --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'"
