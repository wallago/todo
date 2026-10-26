# ── MCP ─────────────────────────────────

# Open the MCP Inspector; point it at the URL below (Streamable HTTP)
[group('mcp')]
mcp-inspect:
    @echo "MCP endpoint: {{ endpoint }}/mcp"
    npx @modelcontextprotocol/inspector
