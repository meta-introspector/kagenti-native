```
kagenti-native RabbitMQ Architecture
=====================================

                          ┌─────────────────────┐
                          │   RabbitMQ Broker    │
                          │  localhost:5672      │
                          │  vhost: /monster     │
                          │  user: kagenti       │
                          │                     │
                          │  ┌───────────────┐  │
                          │  │   Exchange     │  │
                          │  │ kagenti.events │  │
                          │  │  (fanout)      │  │
                          │  └───────┬───────┘  │
                          │          │ bind     │
                          │  ┌───────▼────────┐ │
                          │  │     Queue       │ │
                          │  │ kagenti.agent   │ │
                          │  │   .events       │ │
                          │  └───────┬────────┘ │
                          └──────────┼──────────┘
                                     │
              ┌──────────────────────┼──────────────────────┐
              │                      │                      │
     publish  │             subscribe│              publish  │
              │                      │                      │
   ┌──────────▼──┐          ┌────────▼────────┐    ┌────────▼──────┐
   │ API Server  │          │   Reconciler    │    │  CLI / TUI    │
   │ (axum)      │          │   (daemon)      │    │  (Go client)  │
   │ :8480       │          │                 │    │               │
   │             │          │ on AgentEvent:  │    │ POST /deploy  │
   │ POST /deploy│─publish─▶│  Created → nix  │    │ POST /start   │
   │ POST /start │          │    build +      │    │ POST /stop    │
   │ POST /stop  │          │    systemd      │    │ GET  /status  │
   │ GET /status │          │    install+start│    │               │
   │ GET /agents │          │  Updated →      │    │ GET /agents   │
   │             │          │    restart unit  │    │               │
   └─────────────┘          │  Deleted →      │    └───────────────┘
                            │    stop+remove  │
                            └────────┬────────┘
                                     │
                    ┌────────────────┼────────────────┐
                    │                │                │
           ┌───────▼──────┐ ┌───────▼──────┐ ┌──────▼───────┐
           │ systemd       │ │ nix build    │ │ systemd      │
           │ --user        │ │              │ │ --user       │
           │               │ │ flake.nix    │ │              │
           │ kagenti-agent │ │ → /nix/store │ │ kagenti-agent│
           │ -hello.service│ │   /...-agent │ │ -git.service │
           └───────────────┘ └──────────────┘ └──────────────┘

Message Flow:
─────────────
1. Client POSTs AgentRuntime to API server
2. API server publishes AgentEvent::Created to RabbitMQ
3. Reconciler receives event via subscribe (mpsc channel)
4. Reconciler calls kagenti-nix::build_agent() → nix build
5. Reconciler calls kagenti-daemon::systemd::install() + start()
6. Agent runs as systemd --user service

AgentEvent (JSON, serde tagged):
────────────────────────────────
  {"kind":"Created", "name":"...", "namespace":"...", ...}
  {"kind":"Updated", "name":"...", "namespace":"...", ...}
  {"kind":"Deleted", "name":"...", "namespace":"..."}

Crate Mapping:
──────────────
  kagenti-api   → types + axum routes (k8s-compatible REST)
  kagenti-rmq   → EventBus (lapin v2, fanout exchange)
  kagenti-nix   → generate_flake() + build_agent()
  kagenti-daemon→ systemd.rs + main.rs (API + reconciler)
```
