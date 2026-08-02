import { FormEvent, useCallback, useState } from 'react'
import { SessionPanel } from './components/SessionPanel'
import { Terminal } from './components/Terminal'
import { useWebSocket } from './hooks/useWebSocket'
import { useSessionStore } from './store/sessionStore'

export default function App() {
  const [tokenInput, setTokenInput] = useState('')
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
          <span className={connected ? 'connection-status is-online' : 'connection-status'}>
            {connected ? '● online' : '○ reconnecting'}
          </span>
        </header>

        <div className="terminal-content">
          {selectedSessionId ? (
            <Terminal sessionId={selectedSessionId} />
          ) : (
            <div className="empty-state">
              <p>Open a shell, then run Claude Code, Codex, OpenCode, or any CLI inside it.</p>
              <button className="secondary-button" onClick={createSession}>New shell</button>
            </div>
          )}
        </div>
      </section>
    </main>
  )
}
