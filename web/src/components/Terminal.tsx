import React, { useEffect, useRef } from 'react'
import { Terminal as XTerm } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { onTerminalOutput } from '../hooks/useWebSocket'
import { useWorkspaceStore } from '../store/workspaceStore'

interface Props {
  sessionId: string | null
}

export const Terminal: React.FC<Props> = ({ sessionId }) => {
  const containerRef = useRef<HTMLDivElement>(null)
  const xtermRef = useRef<XTerm | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const sessionIdRef = useRef<string | null>(sessionId)
  const ws = useWorkspaceStore((s) => s.ws)

  // Keep sessionIdRef in sync
  sessionIdRef.current = sessionId

  useEffect(() => {
    if (!containerRef.current) return

    const term = new XTerm({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      theme: {
        background: '#1a1a2e',
        foreground: '#e0e0e0',
        cursor: '#00E5FF',
        selectionBackground: '#00E5FF40',
        black: '#1a1a2e',
        red: '#FF5252',
        green: '#69F0AE',
        yellow: '#FFD600',
        blue: '#40C4FF',
        magenta: '#FF4081',
        cyan: '#00E5FF',
        white: '#e0e0e0',
        brightBlack: '#666',
        brightRed: '#FF8A80',
        brightGreen: '#B9F6CA',
        brightYellow: '#FFFF00',
        brightBlue: '#80D8FF',
        brightMagenta: '#FF80AB',
        brightCyan: '#84FFFF',
        brightWhite: '#ffffff',
      },
    })

    const fitAddon = new FitAddon()
    term.loadAddon(fitAddon)
    term.open(containerRef.current)
    fitAddon.fit()

    xtermRef.current = term
    fitAddonRef.current = fitAddon

    // Write welcome message
    term.writeln('\x1b[1;36m☁ Kumokara（雲殻）— Agents never sleep in Kumokara.\x1b[0m')
    term.writeln('')
    term.writeln('Select a workspace and create a session to start.')
    term.writeln('')

    const handleResize = () => fitAddon.fit()
    window.addEventListener('resize', handleResize)

    // Listen for terminal output from the server
    const unsub = onTerminalOutput((sid, data) => {
      if (sid === sessionIdRef.current && xtermRef.current) {
        xtermRef.current.write(data)
      }
    })

    // Send typed input to the server via WebSocket
    const handleData = (data: string) => {
      const currentWs = ws
      const currentSid = sessionIdRef.current
      if (currentWs && currentSid && currentWs.readyState === WebSocket.OPEN) {
        // Encode input as base64
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
          data: base64,
        }))
      }
    }

    term.onData(handleData)

    return () => {
      window.removeEventListener('resize', handleResize)
      unsub()
      term.dispose()
    }
  }, []) // mount once

  // Update terminal when session changes
  useEffect(() => {
    if (sessionId && xtermRef.current) {
      xtermRef.current.clear()
      xtermRef.current.writeln(`\x1b[1;36mConnected to session: ${sessionId.slice(0, 8)}...\x1b[0m`)
      xtermRef.current.writeln('')
    }
    // Resize on session change
    setTimeout(() => fitAddonRef.current?.fit(), 100)
  }, [sessionId])

  return (
    <div
      ref={containerRef}
      style={{
        flex: 1,
        overflow: 'hidden',
        padding: '4px',
      }}
    />
  )
}
