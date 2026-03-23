DAEMON := http://localhost:8480
NS := default
AGENTS_DIR := agents
AGENTS := fractran-vm moltboot moltis openclaw

.PHONY: build deploy-% deploy-all list status-% start-% stop-% undeploy-%

build:
	nix develop -c cargo build --release

# Deploy a single agent: make deploy-moltboot
deploy-%:
	curl -sf -X POST $(DAEMON)/apis/kagenti/v1/deploy \
	  -H 'Content-Type: application/json' \
	  -d @$(AGENTS_DIR)/$*.json | jq .

# Deploy all agents
deploy-all: $(addprefix deploy-,$(AGENTS))

# Register agent metadata (no systemd, just inventory)
register-%:
	@jq '{name: .name, namespace: .namespace, description: "kagenti-managed agent", status: "stopped", labels: {protocol: ["http"], type: "deployment"}}' \
	  $(AGENTS_DIR)/$*.json | \
	curl -sf -X POST $(DAEMON)/apis/kagenti/v1/namespaces/$(NS)/agents \
	  -H 'Content-Type: application/json' -d @- | jq .

register-all: $(addprefix register-,$(AGENTS))

# List all agents
list:
	curl -sf $(DAEMON)/apis/kagenti/v1/namespaces/$(NS)/agents | jq .

# Agent lifecycle
start-%:
	curl -sf -X POST $(DAEMON)/apis/kagenti/v1/agents/$*/start | jq .

stop-%:
	curl -sf -X POST $(DAEMON)/apis/kagenti/v1/agents/$*/stop | jq .

status-%:
	curl -sf $(DAEMON)/apis/kagenti/v1/agents/$*/status | jq .
