# third_party

## esp-idf-svc

Crates.io **0.52.1** plus a one-file change: `src/ws/client.rs` handles
`WEBSOCKET_EVENT_BEGIN` / `WEBSOCKET_EVENT_FINISH` (Espressif websocket component on IDF 5.4+).

Without this, WSS logs `ESP_ERR_INVALID_ARG` and disconnects before Hello.

Remove this directory and the root `[patch.crates-io]` entry when a newer **esp-idf-svc** includes the same match arms.
