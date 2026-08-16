import { useEffect, useRef } from 'react'
import { Terminal as XTerm, type ITheme } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { onTerminalOutput } from '../hooks/useWebSocket'
import type { AgentStatus } from '../protocol'
import { useSessionStore } from '../store/sessionStore'
import { createRequestId } from '../utils/requestId'

interface Props {
  sessionId: string
  theme: ITheme
  fontFamily: string
  fontSize: number
}

const AGENT_STATUSES = new Set<AgentStatus>([
  'idle',
  'running',
  'awaiting-approval',
  'awaiting-input',
  'error',
  'finished',
])

function decodeTapText(value: string, allowLiteral = false) {
  if (!value) return undefined
  try {
    const bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0))
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes)
  } catch {
    return allowLiteral ? value : undefined
  }
}

function sanitizeDisplayText(value: string, maxChars = 160) {
  const sanitized = [...value]
    .filter((character) => !/\p{Cc}/u.test(character))
    .slice(0, maxChars)
    .join('')
    .trim()
  return sanitized || undefined
}

export function Terminal({ sessionId, theme, fontFamily, fontSize }: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const xtermRef = useRef<XTerm | null>(null)
  const attachedSessionRef = useRef<string | null>(null)
  const scheduleFitRef = useRef<() => void>(() => {})
  const agentMetadataRef = useRef(new Map<string, string>())
  const terminalTitleRef = useRef<string | null>(null)
  const iconTitleRef = useRef<string | null>(null)
  const windowTitleRef = useRef<string | null>(null)
  const lastPtySizeRef = useRef<string | null>(null)
  const ws = useSessionStore((s) => s.ws)
  const ptyCols = useSessionStore((state) => (
    state.sessions.find((session) => session.id === sessionId)?.cols
  ))
  const ptyRows = useSessionStore((state) => (
    state.sessions.find((session) => session.id === sessionId)?.rows
  ))

  useEffect(() => {
    if (!containerRef.current) return

    const term = new XTerm({
      cursorBlink: true,
      fontSize,
      fontFamily,
      theme,
    })

    const fitAddon = new FitAddon()
    term.loadAddon(fitAddon)
    term.open(containerRef.current)
    fitAddon.fit()

    term.attachCustomWheelEventHandler(() => {
      // xterm converts wheel input to Up/Down when a normal buffer has no
      // scrollback, which unexpectedly recalls shell/agent command history.
      // Keep native scrollback and alternate-screen mouse handling unchanged.
      return term.modes.mouseTrackingMode !== 'none'
        || term.buffer.active.type === 'alternate'
        || term.buffer.active.baseY > 0
    })

    xtermRef.current = term

    const reportTerminalTitle = () => {
      const currentWs = useSessionStore.getState().ws
      const currentSid = attachedSessionRef.current
      if (!currentSid) return

      const selectedTitle = iconTitleRef.current ?? windowTitleRef.current
      terminalTitleRef.current = selectedTitle
      if (selectedTitle) {
        useSessionStore.getState().updateSessionTitle(currentSid, selectedTitle)
      }
      if (currentWs?.readyState === WebSocket.OPEN) {
        currentWs.send(JSON.stringify({
          type: 'terminal_title',
          session_id: currentSid,
          title: selectedTitle ?? '',
        }))
      }
    }

    // Match conventional terminal title semantics used by agent integrations:
    // OSC 1 owns the tab/icon title, OSC 2 owns the window
    // title, and OSC 0 sets both. A dedicated tab title wins over the window
    // title until it is explicitly cleared.
    const titleSubscriptions = [0, 1, 2].map((identifier) => (
      term.parser.registerOscHandler(identifier, (data) => {
        const title = sanitizeDisplayText(data) ?? null
        if (identifier === 0 || identifier === 1) iconTitleRef.current = title
        if (identifier === 0 || identifier === 2) windowTitleRef.current = title
        reportTerminalTitle()
        return true
      })
    ))

    const tapSubscription = term.parser.registerOscHandler(26, (data) => {
      if (data.length > 16_384) return true
      for (const token of data.split(';')) {
        const separator = token.indexOf('=')
        if (separator <= 0) continue
        const key = token.slice(0, separator)
        const value = token.slice(separator + 1)
        if (value) agentMetadataRef.current.set(key, value)
        else agentMetadataRef.current.delete(key)
      }

      const currentWs = useSessionStore.getState().ws
      const currentSid = attachedSessionRef.current
      const codeAgent = agentMetadataRef.current.get('CodeAgent')
      if (!currentSid || !codeAgent || currentWs?.readyState !== WebSocket.OPEN) return true

      const rawStatus = agentMetadataRef.current.get('Status')
      const status = rawStatus && AGENT_STATUSES.has(rawStatus as AgentStatus)
        ? rawStatus as AgentStatus
        : undefined
      const sessionTitle = agentMetadataRef.current.get('SessionTitle')
      const decodedTitle = sessionTitle
        ? sanitizeDisplayText(decodeTapText(sessionTitle) ?? '')
        : undefined
      if (decodedTitle && !terminalTitleRef.current) {
        useSessionStore.getState().updateSessionTitle(currentSid, decodedTitle)
      }
      const detail = agentMetadataRef.current.get('Detail')
      const mode = agentMetadataRef.current.get('Mode')

      currentWs.send(JSON.stringify({
        type: 'agent_update',
        session_id: currentSid,
        code_agent: codeAgent,
        ...(decodedTitle ? { session_title: decodedTitle } : {}),
        ...(status ? { status } : {}),
        ...(detail ? { detail: decodeTapText(detail, true) } : {}),
        ...(mode ? { mode: decodeTapText(mode) } : {}),
        ...(agentMetadataRef.current.has('TaskProgress')
          ? { task_progress: agentMetadataRef.current.get('TaskProgress') }
          : {}),
      }))
      return true
    })

    let resizeFrame: number | null = null
    let ptyResizeTimer: number | null = null
    const syncActivePtySize = () => {
      ptyResizeTimer = null
      if (!document.hasFocus() || document.visibilityState !== 'visible') return

      const currentWs = useSessionStore.getState().ws
      const currentSid = attachedSessionRef.current
      if (!currentSid || currentWs?.readyState !== WebSocket.OPEN) return

      const sizeKey = `${currentSid}:${term.cols}x${term.rows}`
      if (lastPtySizeRef.current === sizeKey) return
      currentWs.send(JSON.stringify({
        type: 'terminal_resize',
        session_id: currentSid,
        cols: term.cols,
        rows: term.rows,
        active: true,
      }))
      lastPtySizeRef.current = sizeKey
    }

    const schedulePtyResize = () => {
      if (ptyResizeTimer !== null) window.clearTimeout(ptyResizeTimer)
      // ResizeObserver fires throughout the sidebar transition and while the
      // user drags a window edge. Apply only the settled grid to the PTY.
      ptyResizeTimer = window.setTimeout(syncActivePtySize, 120)
    }

    const fitTerminal = () => {
      resizeFrame = null
      if (!containerRef.current?.clientWidth || !containerRef.current.clientHeight) return

      fitAddon.fit()
      schedulePtyResize()
    }

    const scheduleFit = () => {
      if (resizeFrame !== null) window.cancelAnimationFrame(resizeFrame)
      resizeFrame = window.requestAnimationFrame(fitTerminal)
    }
    scheduleFitRef.current = scheduleFit

    const resizeObserver = new ResizeObserver(scheduleFit)
    resizeObserver.observe(containerRef.current)
    const reclaimActiveViewport = () => {
      if (!document.hasFocus() || document.visibilityState !== 'visible') return
      // Another browser may have controlled the shared PTY since this page
      // last had focus, even when our own pixel dimensions did not change.
      lastPtySizeRef.current = null
      scheduleFit()
    }
    window.addEventListener('focus', reclaimActiveViewport)
    document.addEventListener('visibilitychange', reclaimActiveViewport)
    scheduleFit()

    const unsub = onTerminalOutput((sid, _seq, data) => {
      if (sid === attachedSessionRef.current && xtermRef.current) {
        xtermRef.current.write(data)
      }
    })

    const handleData = (data: string) => {
      // xterm can emit terminal-generated replies through onData. Only the
      // foreground browser may return them, otherwise every viewer would feed
      // duplicate control sequences into the shared PTY.
      if (!document.hasFocus()) return

      const currentWs = useSessionStore.getState().ws
      const currentSid = attachedSessionRef.current
      if (currentWs && currentSid && currentWs.readyState === WebSocket.OPEN) {
        const encoder = new TextEncoder()
        const bytes = encoder.encode(data)
        let binary = ''
        for (let i = 0; i < bytes.length; i++) {
          binary += String.fromCharCode(bytes[i])
        }
        const base64 = btoa(binary)
        const message = {
          type: 'terminal_input',
          session_id: currentSid,
          data_base64: base64,
          cols: term.cols,
          rows: term.rows,
        }
        currentWs.send(JSON.stringify(message))
        lastPtySizeRef.current = `${currentSid}:${term.cols}x${term.rows}`
      }
    }

    const inputSubscription = term.onData(handleData)

    return () => {
      resizeObserver.disconnect()
      window.removeEventListener('focus', reclaimActiveViewport)
      document.removeEventListener('visibilitychange', reclaimActiveViewport)
      if (resizeFrame !== null) window.cancelAnimationFrame(resizeFrame)
      if (ptyResizeTimer !== null) window.clearTimeout(ptyResizeTimer)
      scheduleFitRef.current = () => {}
      unsub()
      inputSubscription.dispose()
      titleSubscriptions.forEach((subscription) => subscription.dispose())
      tapSubscription.dispose()
      term.dispose()
    }
  }, [])

  useEffect(() => {
    if (!xtermRef.current) return
    xtermRef.current.options.theme = { ...theme }
  }, [theme])

  useEffect(() => {
    if (!xtermRef.current) return
    xtermRef.current.options.fontFamily = fontFamily
    xtermRef.current.options.fontSize = fontSize
    scheduleFitRef.current()
  }, [fontFamily, fontSize])

  useEffect(() => {
    const term = xtermRef.current
    if (!term || ptyCols === undefined || ptyRows === undefined) return
    if (!document.hasFocus() || document.visibilityState !== 'visible') return
    if (term.cols === ptyCols && term.rows === ptyRows) return

    // Session metadata is refreshed from the server. If it differs from this
    // focused page, another browser changed the shared PTY after our last
    // resize. Clear local deduplication and reclaim it with our fitted grid.
    lastPtySizeRef.current = null
    scheduleFitRef.current()
  }, [sessionId, ptyCols, ptyRows])

  useEffect(() => {
    if (sessionId && xtermRef.current && ws?.readyState === WebSocket.OPEN) {
      const term = xtermRef.current

      // Agent TUIs commonly enable xterm mouse reporting. Clearing only the
      // viewport preserves that mode and leaks mouse coordinates into the next
      // shell. Disconnect input first, then reset all terminal modes.
      attachedSessionRef.current = null
      term.reset()
      agentMetadataRef.current.clear()
      terminalTitleRef.current = null
      iconTitleRef.current = null
      windowTitleRef.current = null
      lastPtySizeRef.current = null
      attachedSessionRef.current = sessionId

      ws.send(JSON.stringify({
        type: 'session_attach',
        request_id: createRequestId(),
        session_id: sessionId,
      }))
      scheduleFitRef.current()
      return () => {
        if (attachedSessionRef.current === sessionId) {
          attachedSessionRef.current = null
        }
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({
            type: 'session_detach',
            session_id: sessionId,
          }))
        }
      }
    }
  }, [sessionId, ws])

  return (
    <div className="terminal-host" ref={containerRef} />
  )
}
