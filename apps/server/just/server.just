# ── Server ─────────────────────────────────

protocol := "http"
address := "localhost"
port := "3000"
endpoint := protocol + "://" + address + ":" + port

server-check:
    @curl "{{ endpoint }}/health"
