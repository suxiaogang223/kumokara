import { useSessionStore } from '../store/sessionStore'
import type { SessionInfo } from '../protocol'
import { TabsPanelIcon } from './TabsPanelIcon'

interface Props {
  sessions: SessionInfo[]
  selectedSessionId: string | null
  connected: boolean
  onCreate: () => void
  onDestroy: (sessionId: string) => void
  onOpenSettings: () => void
  onHide: () => void
}

function shortPath(cwd: string) {
  const parts = cwd.split('/').filter(Boolean)
  return parts.length <= 2 ? cwd : `…/${parts.slice(-2).join('/')}`
}

export function SessionPanel({
  sessions,
  selectedSessionId,
  connected,
  onCreate,
  onDestroy,
  onOpenSettings,
  onHide,
}: Props) {
  const selectSession = useSessionStore((state) => state.selectSession)

  return (
    <aside className="session-panel">
      <div className="session-panel-header">
        <div className="session-panel-brand">Kumokara</div>
        <div className="session-panel-actions">
          <button className="panel-action" onClick={onCreate} title="New shell" aria-label="New shell">
            <span className="new-tab-icon" aria-hidden="true" />
          </button>
          <button
            className="panel-action tabs-panel-toggle"
            type="button"
            onClick={onHide}
            title="Hide tabs (⌘⇧L)"
            aria-label="Hide tabs"
          >
            <TabsPanelIcon />
          </button>
        </div>
      </div>
      <div className="session-toolbar">
        <span className="session-toolbar-label">Tabs</span>
        <span className="session-count" aria-label={`${sessions.length} sessions`}>
          {sessions.length}
        </span>
      </div>

      <div className="session-list">
        {sessions.length === 0 && (
          <p className="session-list-empty">
            No sessions yet.<br />Create a shell and start any agent inside it.
          </p>
        )}
        {sessions.map((session) => {
          const agent = session.agent
          const agentState = agent?.status ? ` is-${agent.status}` : ''
          const agentTooltip = agent
            ? [agent.display_name, agent.status, agent.detail].filter(Boolean).join(' · ')
            : 'Shell'
          return (
            <article
              className={`session-item${session.id === selectedSessionId ? ' is-selected' : ''}`}
              key={session.id}
            >
              <button className="session-select" onClick={() => selectSession(session.id)}>
                <span
                  className={agent ? `session-icon is-agent${agentState}` : 'session-icon'}
                  title={agentTooltip}
                >
                  {agent?.icon || '›_'}
                </span>
                <span className="session-labels">
                  <span className="session-name">{session.title}</span>
                  <span className="session-path" title={session.cwd}>{shortPath(session.cwd)}</span>
                </span>
              </button>
              <button
                className="session-close"
                onClick={(e) => { e.stopPropagation(); onDestroy(session.id) }}
                title="Close session"
                aria-label={`Close ${session.title}`}
              >
                ×
              </button>
            </article>
          )
        })}
      </div>

      <footer className="session-panel-footer">
        <div className="footer-left">
          <span className={`status-dot${connected ? ' is-online' : ''}`} title={connected ? 'Connected' : 'Reconnecting'} />
        </div>
        <button className="settings-button" onClick={onOpenSettings} title="Settings" aria-label="Settings">
          ⚙
        </button>
      </footer>
    </aside>
  )
}
