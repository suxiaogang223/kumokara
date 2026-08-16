import { FormEvent, useCallback, useEffect, useState } from 'react'
import { SessionPanel } from './components/SessionPanel'
import { SettingsPanel } from './components/SettingsPanel'
import { Terminal } from './components/Terminal'
import { useAppearance } from './hooks/useAppearance'
import { useWebSocket } from './hooks/useWebSocket'
import { useAppearanceStore } from './store/appearanceStore'
import { useSessionStore } from './store/sessionStore'

const SIDEBAR_STATE_KEY = 'kumokara.sidebar-state.v3'
const NARROW_LAYOUT_QUERY = '(max-width: 1024px)'

function initialSidebarExpanded() {
  try {
    const stored = window.localStorage.getItem(SIDEBAR_STATE_KEY)
    if (stored === 'expanded') return true
    if (stored === 'collapsed') return false
  } catch {
    // Fall through to the responsive default when storage is unavailable.
  }
  return !window.matchMedia(NARROW_LAYOUT_QUERY).matches
}

export default function App() {
  const [tokenInput, setTokenInput] = useState('')
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [sidebarExpanded, setSidebarExpanded] = useState(initialSidebarExpanded)
  const { appearance, theme } = useAppearance()
  const fontFamily = useAppearanceStore((state) => state.fontFamily)
  const fontSize = useAppearanceStore((state) => state.fontSize)
  const authState = useSessionStore((state) => state.authState)
  const authError = useSessionStore((state) => state.authErrorMessage)
  const setAuthToken = useSessionStore((state) => state.setAuthToken)
  const setAuthState = useSessionStore((state) => state.setAuthState)
  const connected = useSessionStore((state) => state.connected)
  const sessions = useSessionStore((state) => state.sessions)
  const selectedSessionId = useSessionStore((state) => state.selectedSessionId)
  const { send } = useWebSocket()
  const selectedSession = sessions.find((session) => session.id === selectedSessionId)

  const toggleSidebar = useCallback(() => {
    setSidebarExpanded((expanded) => !expanded)
  }, [])

  useEffect(() => {
    try {
      window.localStorage.setItem(
        SIDEBAR_STATE_KEY,
        sidebarExpanded ? 'expanded' : 'collapsed',
      )
    } catch {
      // The layout still works when browser storage is unavailable.
    }
  }, [sidebarExpanded])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === 'l') {
        event.preventDefault()
        toggleSidebar()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [toggleSidebar])

  const submitToken = (event: FormEvent) => {
    event.preventDefault()
    const token = tokenInput.trim()
    if (!token) return
    setAuthState('connecting')
    setAuthToken(token)
  }

  const createSession = useCallback((cwd?: string) => {
    send({
      type: 'session_create',
      request_id: crypto.randomUUID(),
      ...(cwd ? { cwd } : {}),
      cols: 100,
      rows: 30,
    })
  }, [send])

  const destroySession = useCallback((sessionId: string) => {
    send({
      type: 'session_destroy',
      request_id: crypto.randomUUID(),
      session_id: sessionId,
    })
  }, [send])

  if (authState !== 'authenticated') {
    const connecting = authState === 'connecting'
    return (
      <main className="auth-screen">
        <form className="auth-card" onSubmit={submitToken}>
          <div className="brand">Kumokara</div>
          <p className="auth-intro">Persistent shells for whatever agent you use.</p>
          {authState === 'error' && (
            <div className="auth-error" role="alert">
              {authError || 'Authentication failed'}
            </div>
          )}
          <div className="auth-fields">
            <input
              className="text-input"
              type="password"
              value={tokenInput}
              onChange={(event) => setTokenInput(event.target.value)}
              placeholder="Server token"
              disabled={connecting}
              aria-label="Server token"
              autoFocus
            />
            <button className="primary-button" type="submit" disabled={!tokenInput.trim() || connecting}>
              {connecting ? '…' : 'Connect'}
            </button>
          </div>
        </form>
      </main>
    )
  }

  return (
    <main className={`app-shell${sidebarExpanded ? '' : ' is-sidebar-collapsed'}`}>
      <SessionPanel
        sessions={sessions}
        selectedSessionId={selectedSessionId}
        connected={connected}
        expanded={sidebarExpanded}
        onCreate={createSession}
        onDestroy={destroySession}
        onOpenSettings={() => setSettingsOpen(true)}
        onToggle={toggleSidebar}
      />
      <section className="terminal-pane">
        <header className="terminal-titlebar">
          <div className="terminal-titlebar-leading" aria-hidden="true" />
          <div className="terminal-title" title={selectedSession?.title}>
            {selectedSession?.title || 'Kumokara'}
          </div>
          <div className="terminal-titlebar-trailing" aria-hidden="true" />
        </header>
        <div className="terminal-content">
          {selectedSessionId ? (
            <Terminal
              sessionId={selectedSessionId}
              theme={theme.terminal}
              fontFamily={fontFamily}
              fontSize={fontSize}
            />
          ) : (
            <div className="empty-state">
              <div className="empty-state-icon">›_</div>
              <p>
                Open a shell, then run Claude Code, Codex, OpenCode, or any CLI inside it.
              </p>
              <button className="secondary-button" onClick={() => createSession()}>New shell</button>
            </div>
          )}
        </div>
      </section>
      {settingsOpen && (
        <SettingsPanel activeAppearance={appearance} onClose={() => setSettingsOpen(false)} />
      )}
    </main>
  )
}
