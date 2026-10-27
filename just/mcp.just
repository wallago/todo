# ── MCP ─────────────────────────────────

# Open the MCP Inspector; point it at the URL below (Streamable HTTP)
[group('mcp')]
mcp-inspect:
    @echo "MCP endpoint: {{ endpoint }}/mcp"
    @echo "With a token_file: add the header Authorization: Bearer <token>"
    npx @modelcontextprotocol/inspector
