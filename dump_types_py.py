#!/usr/bin/env python3
"""Instantiate kagenti Python API types and dump to CBOR for alignment with Rust."""
import json, os, sys

sys.path.insert(0, os.path.expanduser("~/DOCS/services/kagenti/kagenti/backend"))

try:
    import cbor2
except ImportError:
    sys.exit("pip install cbor2")

from app.models.responses import (
    AgentSummary, ToolSummary, ResourceLabels, MCPToolInfo,
    MCPToolsResponse, AgentListResponse, NamespaceListResponse, DeleteResponse,
)

samples = {
    "agent_summary": AgentSummary(
        name="git-issue-agent", namespace="default",
        description="Resolves GitHub issues via LLM", status="running",
        labels=ResourceLabels(protocol=["a2a", "mcp"], framework="LangGraph", type="agent"),
        workloadType="deployment", createdAt="2026-03-18T13:00:00Z",
    ),
    "tool_summary": ToolSummary(
        name="web-search-tool", namespace="default",
        description="Web search via MCP", status="running",
        labels=ResourceLabels(protocol=["mcp"], type="tool"),
        workloadType="statefulset",
    ),
    "mcp_tools": MCPToolsResponse(tools=[
        MCPToolInfo(name="search", description="Search the web",
                    input_schema={"type": "object",
                                  "properties": {"query": {"type": "string"}},
                                  "required": ["query"]}),
    ]),
    "namespace_list": NamespaceListResponse(namespaces=["default", "staging"]),
    "delete_response": DeleteResponse(success=True, message="deleted agent git-issue-agent in default"),
}

out = "dasl-dump-py"
os.makedirs(out, exist_ok=True)

for name, obj in samples.items():
    d = obj.model_dump(exclude_none=True)
    j = json.dumps(d, indent=2, sort_keys=True)
    c = cbor2.dumps(d)
    with open(f"{out}/{name}.json", "w") as f:
        f.write(j)
    with open(f"{out}/{name}.cbor", "wb") as f:
        f.write(c)
    print(f"{name}: json={len(j)}B cbor={len(c)}B")

print(f"\n✅ Dumped to {out}/")
