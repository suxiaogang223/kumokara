import { useCallback, useEffect, useRef } from 'react'
import type { ClientMessage, ServerMessage } from '../protocol'
import { useSessionStore } from '../store/sessionStore'
import { decodeTerminalOutputFrame } from '../terminal/terminalOutput'
import { createRequestId } from '../utils/requestId'

export type TerminalOutputHandler = (sessionId: string, seq: number, data: Uint8Array) => void
export type DirectoryBrowserMessage = Extract<ServerMessage,
  { type: 'directory_listing' | 'directory_created' | 'error' }
>
export type DirectoryBrowserHandler = (message: DirectoryBrowserMessage) => void

const terminalHandlers = new Set<TerminalOutputHandler>()
const directoryBrowserHandlers = new Set<DirectoryBrowserHandler>()

export function onTerminalOutput(handler: TerminalOutputHandler) {
  terminalHandlers.add(handler)
  return () => { terminalHandlers.delete(handler) }
}

export function onDirectoryBrowserMessage(handler: DirectoryBrowserHandler) {
  directoryBrowserHandlers.add(handler)
  return () => { directoryBrowserHandlers.delete(handler) }
}

export function useWebSocket() {
  const socketRef = useRef<WebSocket | null>(null)
  const reconnectRef = useRef<number | null>(null)
  const refreshRef = useRef<number | null>(null)
  const authToken = useSessionStore((state) => state.authToken)

  useEffect(() => {
    let disposed = false

    const clearTimers = () => {
      if (reconnectRef.current !== null) window.clearTimeout(reconnectRef.current)
      if (refreshRef.current !== null) window.clearInterval(refreshRef.current)
      reconnectRef.current = null
      refreshRef.current = null
    }

    const requestSessions = () => {
      if (socketRef.current?.readyState === WebSocket.OPEN) {
        socketRef.current.send(JSON.stringify({
          type: 'session_list',
          request_id: createRequestId(),
        } satisfies ClientMessage))
      }
    }

    const handleMessage = (message: ServerMessage) => {
      const store = useSessionStore.getState()
      switch (message.type) {
        case 'auth_ok':
          store.setConnected(true)
          store.setAuthState('authenticated')
          store.setWs(socketRef.current)
          requestSessions()
          if (refreshRef.current !== null) window.clearInterval(refreshRef.current)
          refreshRef.current = window.setInterval(requestSessions, 2500)
          break
        case 'auth_error':
          store.setAuthError(message.message || 'Authentication failed')
          socketRef.current?.close()
          break
        case 'session_created':
          store.addSession(message.session)
          break
        case 'session_destroyed':
          store.removeSession(message.session_id)
          break
        case 'session_list':
          store.setSessions(message.sessions)
          break
        case 'directory_listing':
        case 'directory_created':
          directoryBrowserHandlers.forEach((handler) => handler(message))
          break
        case 'error':
          if (message.code === 'SESSION_CREATE_FAILED') {
            store.setSessionError(message.message || 'Failed to create session')
          }
          if (message.code.startsWith('DIRECTORY_')) {
            directoryBrowserHandlers.forEach((handler) => handler(message))
          }
          console.warn(`[${message.code}] ${message.message}`)
          break
        case 'server_notification':
          console.info(message.message)
          break
      }
    }

    const connect = () => {
      if (disposed) return
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
      const socket = new WebSocket(`${protocol}//${window.location.host}/api/ws`)
      socket.binaryType = 'arraybuffer'
      socketRef.current = socket

      socket.onopen = () => {
        if (authToken) {
          socket.send(JSON.stringify({ type: 'auth', token: authToken } satisfies ClientMessage))
        }
      }
      socket.onmessage = (event) => {
        if (typeof event.data === 'string') {
          try {
            handleMessage(JSON.parse(event.data) as ServerMessage)
          } catch (error) {
            console.warn('Invalid server message', error)
          }
          return
        }

        if (event.data instanceof ArrayBuffer) {
          try {
            const message = decodeTerminalOutputFrame(event.data)
            terminalHandlers.forEach((handler) => handler(message.sessionId, message.seq, message.data))
          } catch (error) {
            console.warn('Invalid binary terminal output', error)
          }
        }
      }
      socket.onclose = () => {
        const store = useSessionStore.getState()
        store.setConnected(false)
        store.setWs(null)
        if (refreshRef.current !== null) window.clearInterval(refreshRef.current)
        refreshRef.current = null
        if (!disposed && store.authState !== 'error') {
          reconnectRef.current = window.setTimeout(connect, 3000)
        }
      }
    }

    connect()
    return () => {
      disposed = true
      clearTimers()
      const socket = socketRef.current
      socketRef.current = null
      socket?.close()
    }
  }, [authToken])

  const send = useCallback((message: ClientMessage) => {
    if (socketRef.current?.readyState !== WebSocket.OPEN) return false
    socketRef.current.send(JSON.stringify(message))
    return true
  }, [])

  return { send }
}
