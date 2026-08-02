import { FormEvent, useCallback, useState } from 'react'
import { SessionPanel } from './components/SessionPanel'
import { SettingsPanel } from './components/SettingsPanel'
import { Terminal } from './components/Terminal'
import { useAppearance } from './hooks/useAppearance'
import { useWebSocket } from './hooks/useWebSocket'
import { useAppearanceStore } from './store/appearanceStore'
import { useSessionStore } from './store/sessionStore'

export default function App() {
  const [tokenInput, setTokenInput] = useState('')
  const [settingsOpen, setSettingsOpen] = useState(false)
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
  const selectedSession = sessions.find(({ id }) => id === selectedSessionId)
  const { send } = useWebSocket()

  const submitToken = (event: FormEvent) => {
    event.preventDefault()
    const token = tokenInput.trim()
    if (!token) return
    setAuthState('connecting')
    setAuthToken(token)
  }

  const createSession = useCallback(() => {
    send({
      type: 'session_create',
      request_id: crypto.randomUUID(),
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
    <main className="app-shell">
      <SessionPanel onCreate={createSession} onDestroy={destroySession} />
      <section className="terminal-pane">
        <header className="terminal-header">
          <div className="terminal-context">
            <span className="terminal-title">{selectedSession?.title ?? 'Kumokara'}</span>
            {selectedSession && <span className="terminal-cwd">{selectedSession.cwd}</span>}
          </div>
          <div className="terminal-actions">
            <span className={connected ? 'connection-status is-online' : 'connection-status'}>
              {connected ? '● online' : '○ reconnecting'}
            </span>
            <button className="header-icon-button" onClick={() => setSettingsOpen(true)} title="Settings" aria-label="Settings">⚙</button>
          </div>
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
              <p>Open a shell, then run Claude Code, Codex, OpenCode, or any CLI inside it.</p>
              <button className="secondary-button" onClick={createSession}>New shell</button>
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
