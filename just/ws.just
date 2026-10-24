# ── Websocket ─────────────────────────────────

ws-protocol := "ws"
ws-endpoint := ws-protocol + "://" + address + ":" + port

ws-send-hello:
    echo 'hello' | websocat -1 {{ ws-endpoint }}/ws

ws-send-multi-msg:
    printf 'a\nb\nc\n' | websocat {{ ws-endpoint }}/ws

ws-listen:
    websocat -n -U {{ ws-endpoint }}/ws
