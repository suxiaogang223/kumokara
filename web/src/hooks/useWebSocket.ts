import { useEffect, useRef } from 'react'
import { useWorkspaceStore } from '../store/workspaceStore'

// Callback type for terminal output
export type TerminalOutputHandler = (sessionId: string, data: string) => void

const terminalHandlers = new Set<TerminalOutputHandler>()

export function onTerminalOutput(handler: TerminalOutputHandler) {
  terminalHandlers.add(handler)
  return () => { terminalHandlers.delete(handler) }
}

export function useWebSocket() {
  const wsRef = useRef<WebSocket | null>(null)
  const authToken = useWorkspaceStore((s) => s.authToken)
  const setConnected = useWorkspaceStore((s) => s.setConnected)
  const setWorkspaces = useWorkspaceStore((s) => s.setWorkspaces)
  const setWs = useWorkspaceStore((s) => s.setWs)
  const setAuthState = useWorkspaceStore((s) => s.setAuthState)
  const setAuthError = useWorkspaceStore((s) => s.setAuthError)
  const addSession = useWorkspaceStore((s) => s.addSession)
  const removeSession = useWorkspaceStore((s) => s.removeSession)

  const connect = () => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const host = window.location.host
    const ws = new WebSocket(`${protocol}//${host}/api/ws`)

    ws.onopen = () => {
      console.log('WebSocket connected, sending auth...')
      if (authToken) {
        ws.send(JSON.stringify({ type: 'auth', token: authToken }))
      }
    }

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data)
        handleMessage(msg)
      } catch (_e) {
        // Binary frame — not used in Phase 0
      }
    }

    ws.onclose = (event) => {
      console.log('WebSocket disconnected, code:', event.code)
      setConnected(false)
      setWs(null)
      // Only auto-reconnect if already authenticated
      if (useWorkspaceStore.getState().authState === 'authenticated') {
        setTimeout(connect, 3000)
      }
    }

    ws.onerror = (_e) => {
      // onclose will fire after this
    }

    wsRef.current = ws
    setWs(ws)
  }

  const handleMessage = (msg: any) => {
    switch (msg.type) {
      case 'auth_ok':
        console.log('Auth succeeded')
        setConnected(true)
        setAuthState('authenticated')
        // Request workspace list after successful auth
        wsRef.current?.send(
          JSON.stringify({ type: 'list_workspaces', request_id: crypto.randomUUID() })
        )
        break

      case 'auth_error':
        console.error('Auth failed:', msg.message)
        setAuthError(msg.message || 'Authentication failed')
        setConnected(false)
        // Close the WebSocket — it's useless without auth
        wsRef.current?.close()
        wsRef.current = null
        setWs(null)
        break

      case 'workspace_list':
        setWorkspaces(msg.workspaces || [])
        break

      case 'workspace_created':
        useWorkspaceStore.getState().addWorkspace(msg.workspace)
        break

      case 'workspace_destroyed':
        wsRef.current?.send(
          JSON.stringify({ type: 'list_workspaces', request_id: crypto.randomUUID() })
        )
        break

      case 'session_created':
        addSession(msg.workspace_id, msg.session)
        break

      case 'session_destroyed':
        removeSession(msg.session_id)
        break

      case 'terminal_output': {
        const sessionId = msg.session_id
        const data = msg.data
        terminalHandlers.forEach((h) => {
          try { h(sessionId, data) } catch (_) { /* ignore */ }
        })
        break
      }

      case 'session_list':
        break

      default:
        break
    }
  }

  const send = (msg: object) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(msg))
      return true
    }
    return false
  }

  useEffect(() => {
    if (authToken) {
      connect()
    }
    return () => {
      wsRef.current?.close()
    }
  }, [authToken])

  return { send }
}
