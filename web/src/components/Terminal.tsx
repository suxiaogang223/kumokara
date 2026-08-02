import { useEffect, useRef } from 'react'
import { Terminal as XTerm } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { onTerminalOutput } from '../hooks/useWebSocket'
import { useSessionStore } from '../store/sessionStore'

interface Props {
  sessionId: string
}

export function Terminal({ sessionId }: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const xtermRef = useRef<XTerm | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const attachedSessionRef = useRef<string | null>(null)
  const ws = useSessionStore((s) => s.ws)

  useEffect(() => {
    if (!containerRef.current) return

    const term = new XTerm({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      theme: {
        background: '#0b101b', foreground: '#dce4f5', cursor: '#86eaff',
        selectionBackground: '#86eaff33', black: '#111827', red: '#ff7d86',
        green: '#75d99f', yellow: '#e8c66a', blue: '#77bdfb', magenta: '#c49aff',
        cyan: '#86eaff', white: '#dce4f5', brightBlack: '#65718a',
        brightRed: '#ff9ba2', brightGreen: '#9ee8ba', brightYellow: '#f2d98e',
        brightBlue: '#9bd0ff', brightMagenta: '#d7b7ff', brightCyan: '#b5f3ff',
        brightWhite: '#ffffff',
      },
    })

    const fitAddon = new FitAddon()
    term.loadAddon(fitAddon)
    term.open(containerRef.current)
    fitAddon.fit()

    xtermRef.current = term
    fitAddonRef.current = fitAddon

    const sendResize = () => {
      fitAddon.fit()
      const currentWs = useSessionStore.getState().ws
      const currentSid = attachedSessionRef.current
      if (currentWs?.readyState === WebSocket.OPEN && currentSid) {
        currentWs.send(JSON.stringify({
          type: 'terminal_resize',
          session_id: currentSid,
          cols: term.cols,
          rows: term.rows,
        }))
      }
    }
    const resizeObserver = new ResizeObserver(sendResize)
    resizeObserver.observe(containerRef.current)

    const unsub = onTerminalOutput((sid, _seq, data) => {
      if (sid === attachedSessionRef.current && xtermRef.current) {
        xtermRef.current.write(data)
      }
    })

    const handleData = (data: string) => {
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
        currentWs.send(JSON.stringify({
          type: 'terminal_input',
          session_id: currentSid,
          data_base64: base64,
        }))
      }
    }

    const inputSubscription = term.onData(handleData)

    return () => {
      resizeObserver.disconnect()
      unsub()
      inputSubscription.dispose()
      term.dispose()
    }
  }, [])

  useEffect(() => {
    if (sessionId && xtermRef.current && ws?.readyState === WebSocket.OPEN) {
      const term = xtermRef.current

      // Agent TUIs commonly enable xterm mouse reporting. Clearing only the
      // viewport preserves that mode and leaks mouse coordinates into the next
      // shell. Disconnect input first, then reset all terminal modes.
      attachedSessionRef.current = null
      term.reset()
      attachedSessionRef.current = sessionId

      ws.send(JSON.stringify({
        type: 'session_attach',
        request_id: crypto.randomUUID(),
        session_id: sessionId,
      }))
      fitAddonRef.current?.fit()
      ws.send(JSON.stringify({
        type: 'terminal_resize',
        session_id: sessionId,
        cols: term.cols,
        rows: term.rows,
      }))
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
    <div className="terminal" ref={containerRef} />
  )
}
