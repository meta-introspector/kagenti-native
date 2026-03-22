// Instantiate kagenti Go API types and dump to CBOR for alignment with Rust/Python.
// Uses the same field values as dump_types.rs and dump_types_py.py.
package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/fxamacker/cbor/v2"
)

// -- Types mirrored from kagenti/tui/internal/api/types.go --

type ResourceLabels struct {
	Protocol  []string `json:"protocol,omitempty" cbor:"protocol,omitempty"`
	Framework string   `json:"framework,omitempty" cbor:"framework,omitempty"`
	Type      string   `json:"type,omitempty" cbor:"type,omitempty"`
}

type AgentSummary struct {
	Name         string         `json:"name" cbor:"name"`
	Namespace    string         `json:"namespace" cbor:"namespace"`
	Description  string         `json:"description" cbor:"description"`
	Status       string         `json:"status" cbor:"status"`
	Labels       ResourceLabels `json:"labels" cbor:"labels"`
	WorkloadType string         `json:"workloadType,omitempty" cbor:"workloadType,omitempty"`
	CreatedAt    string         `json:"createdAt,omitempty" cbor:"createdAt,omitempty"`
}

type ToolSummary struct {
	Name         string         `json:"name" cbor:"name"`
	Namespace    string         `json:"namespace" cbor:"namespace"`
	Description  string         `json:"description" cbor:"description"`
	Status       string         `json:"status" cbor:"status"`
	Labels       ResourceLabels `json:"labels" cbor:"labels"`
	WorkloadType string         `json:"workloadType,omitempty" cbor:"workloadType,omitempty"`
	CreatedAt    string         `json:"createdAt,omitempty" cbor:"createdAt,omitempty"`
}

type MCPToolInfo struct {
	Name        string      `json:"name" cbor:"name"`
	Description string      `json:"description,omitempty" cbor:"description,omitempty"`
	InputSchema interface{} `json:"input_schema,omitempty" cbor:"input_schema,omitempty"`
}

type MCPToolsResponse struct {
	Tools []MCPToolInfo `json:"tools" cbor:"tools"`
}

type NamespaceListResponse struct {
	Namespaces []string `json:"namespaces" cbor:"namespaces"`
}

type DeleteResponse struct {
	Success bool   `json:"success" cbor:"success"`
	Message string `json:"message" cbor:"message"`
}

func dump(name string, v interface{}, dir string) {
	// JSON
	j, _ := json.MarshalIndent(v, "", "  ")
	os.WriteFile(fmt.Sprintf("%s/%s.json", dir, name), j, 0644)

	// CBOR
	c, _ := cbor.Marshal(v)
	os.WriteFile(fmt.Sprintf("%s/%s.cbor", dir, name), c, 0644)

	fmt.Printf("%s: json=%dB cbor=%dB\n", name, len(j), len(c))
}

func main() {
	dir := "dasl-dump-go"
	os.MkdirAll(dir, 0755)

	dump("agent_summary", AgentSummary{
		Name: "git-issue-agent", Namespace: "default",
		Description: "Resolves GitHub issues via LLM", Status: "running",
		Labels: ResourceLabels{
			Protocol: []string{"a2a", "mcp"}, Framework: "LangGraph", Type: "agent",
		},
		WorkloadType: "deployment", CreatedAt: "2026-03-18T13:00:00Z",
	}, dir)

	dump("tool_summary", ToolSummary{
		Name: "web-search-tool", Namespace: "default",
		Description: "Web search via MCP", Status: "running",
		Labels: ResourceLabels{
			Protocol: []string{"mcp"}, Type: "tool",
		},
		WorkloadType: "statefulset",
	}, dir)

	dump("mcp_tools", MCPToolsResponse{
		Tools: []MCPToolInfo{{
			Name: "search", Description: "Search the web",
			InputSchema: map[string]interface{}{
				"type": "object",
				"properties": map[string]interface{}{
					"query": map[string]interface{}{"type": "string"},
				},
				"required": []string{"query"},
			},
		}},
	}, dir)

	dump("namespace_list", NamespaceListResponse{
		Namespaces: []string{"default", "staging"},
	}, dir)

	dump("delete_response", DeleteResponse{
		Success: true, Message: "deleted agent git-issue-agent in default",
	}, dir)

	fmt.Printf("\n✅ Dumped to %s/\n", dir)
}
