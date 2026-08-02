import { useSessionStore } from '../store/sessionStore'

interface Props {
  onCreate: () => void
  onDestroy: (sessionId: string) => void
}

function shortPath(cwd: string) {
  const parts = cwd.split('/').filter(Boolean)
  return parts.length <= 2 ? cwd : `…/${parts.slice(-2).join('/')}`
}

export function SessionPanel({ onCreate, onDestroy }: Props) {
  const sessions = useSessionStore((state) => state.sessions)
  const selectedSessionId = useSessionStore((state) => state.selectedSessionId)
  const selectSession = useSessionStore((state) => state.selectSession)

  return (
    <aside className="session-panel">
      <header className="session-panel-header">
        <span>Sessions</span>
        <button className="icon-button" onClick={onCreate} title="New shell" aria-label="New shell">+</button>
      </header>

      <div className="session-list">
        {sessions.length === 0 && (
          <p className="session-list-empty">No sessions yet.<br />Create a shell and start any agent inside it.</p>
        )}
        {sessions.map((session) => (
          <article
            className={`session-item${session.id === selectedSessionId ? ' is-selected' : ''}`}
            key={session.id}
          >
            <button className="session-select" onClick={() => selectSession(session.id)}>
              <span className={session.agent ? 'session-icon is-agent' : 'session-icon'}>
                {session.agent ? '◆' : '›_'}
              </span>
              <span className="session-labels">
                <span className="session-name">{session.agent?.provider ?? session.title}</span>
                <span className="session-path" title={session.cwd}>{shortPath(session.cwd)}</span>
              </span>
            </button>
            <button
              className="session-close"
              onClick={() => onDestroy(session.id)}
              title="Close session"
              aria-label={`Close ${session.title}`}
            >
              ×
            </button>
          </article>
        ))}
      </div>
    </aside>
  )
}
