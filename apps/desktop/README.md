# Kumokara Desktop

The desktop app is a Tauri host for the same React UI and WebSocket protocol as
the browser build. It starts a token-protected Kumokara server on a random
loopback port and can switch to a remote TLS endpoint from Settings.

```bash
npm --prefix web ci
npm --prefix apps/desktop ci
npm --prefix apps/desktop run dev
```

The local token is generated for each app process and is passed to the WebView
through a Tauri command. It is not written to disk or included in process
arguments. Remote tokens are kept in frontend memory only.
