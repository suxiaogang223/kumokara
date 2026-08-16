import { type FormEvent, type MouseEvent, type ReactNode, useEffect, useMemo, useState } from 'react'
import type { SessionInfo } from '../protocol'
import { useSessionStore } from '../store/sessionStore'
import { normalizeWorkspacePath, useWorkspaceStore } from '../store/workspaceStore'
import { TabsPanelIcon } from './TabsPanelIcon'

interface Props {
  sessions: SessionInfo[]
  selectedSessionId: string | null
  connected: boolean
  expanded: boolean
  onCreate: (cwd?: string) => void
  onDestroy: (sessionId: string) => void
  onOpenSettings: () => void
  onToggle: () => void
}

type IconName = 'add' | 'addFolder' | 'check' | 'chevron' | 'close' | 'folder' | 'more' | 'search' | 'settings' | 'terminal' | 'view'
type ViewMode = 'tree' | 'flat'
type SortMode = 'recent' | 'name'

const VIEW_STORAGE_KEY = 'kumokara.workspace-view.v1'

function Icon({ name, size = 16 }: { name: IconName; size?: number }) {
  const paths: Record<IconName, ReactNode> = {
    add: <><circle cx="12" cy="12" r="8.5" /><path d="M12 8v8M8 12h8" /></>,
    addFolder: <><path d="M3.5 7.5h6l2-2h3.7c1 0 1.8.8 1.8 1.8v.2h3.5v10.2c0 1-.8 1.8-1.8 1.8H5.3c-1 0-1.8-.8-1.8-1.8V7.5Z" /><path d="M16 11v6M13 14h6" /></>,
    check: <path d="m7 12 3.2 3.2L17.5 8" />,
    chevron: <path d="m9 6 6 6-6 6" />,
    close: <><path d="m7 7 10 10M17 7 7 17" /></>,
    folder: <path d="M3.5 7.5h6l2-2h3.7c1 0 1.8.8 1.8 1.8v.2h3.5v10.2c0 1-.8 1.8-1.8 1.8H5.3c-1 0-1.8-.8-1.8-1.8V7.5Z" />,
    more: <><circle cx="5" cy="12" r="1" fill="currentColor" stroke="none" /><circle cx="12" cy="12" r="1" fill="currentColor" stroke="none" /><circle cx="19" cy="12" r="1" fill="currentColor" stroke="none" /></>,
    search: <><circle cx="10.5" cy="10.5" r="6.5" /><path d="m15.5 15.5 4 4" /></>,
    settings: <><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3V2.8h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z" /></>,
    terminal: <><path d="m5 7 4 4-4 4M11 16h8" /></>,
    view: <><path d="M4 7h7M15 7h5M4 12h3M11 12h9M4 17h10M18 17h2" /><circle cx="13" cy="7" r="2" /><circle cx="9" cy="12" r="2" /><circle cx="16" cy="17" r="2" /></>,
  }
  return (
    <svg aria-hidden="true" className="sidebar-icon" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      {paths[name]}
    </svg>
  )
}

function folderName(cwd: string) {
  const normalized = cwd.replace(/\/+$/, '')
  const parts = normalized.split('/').filter(Boolean)
  return parts[parts.length - 1] || cwd
}

function relativeTime(value: string, now: number) {
  const timestamp = Date.parse(value)
  if (!Number.isFinite(timestamp)) return ''
  const minutes = Math.floor(Math.max(0, now - timestamp) / 60_000)
  if (minutes < 1) return 'now'
  if (minutes < 60) return `${minutes}min`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h`
  const days = Math.floor(hours / 24)
  return days < 30 ? `${days}d` : `${Math.floor(days / 30)}mo`
}

function readViewPreference(): { viewMode: ViewMode; sortMode: SortMode } {
  try {
    const stored = JSON.parse(localStorage.getItem(VIEW_STORAGE_KEY) ?? '{}')
    return {
      viewMode: stored.viewMode === 'flat' ? 'flat' : 'tree',
      sortMode: stored.sortMode === 'name' ? 'name' : 'recent',
    }
  } catch {
    return { viewMode: 'tree', sortMode: 'recent' }
  }
}

export function SessionPanel({
  sessions,
  selectedSessionId,
  connected,
  expanded,
  onCreate,
  onDestroy,
  onOpenSettings,
  onToggle,
}: Props) {
  const selectSession = useSessionStore((state) => state.selectSession)
  const workspaces = useWorkspaceStore((state) => state.workspaces)
  const activePath = useWorkspaceStore((state) => state.activePath)
  const addWorkspace = useWorkspaceStore((state) => state.addWorkspace)
  const rememberWorkspaces = useWorkspaceStore((state) => state.rememberWorkspaces)
  const setActivePath = useWorkspaceStore((state) => state.setActivePath)
  const initialView = useMemo(readViewPreference, [])
  const [searchOpen, setSearchOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(() => new Set())
  const [viewMode, setViewMode] = useState<ViewMode>(initialView.viewMode)
  const [sortMode, setSortMode] = useState<SortMode>(initialView.sortMode)
  const [addWorkspaceOpen, setAddWorkspaceOpen] = useState(false)
  const [workspaceInput, setWorkspaceInput] = useState('')
  const [workspaceError, setWorkspaceError] = useState('')
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    rememberWorkspaces(sessions.map((session) => session.cwd))
  }, [rememberWorkspaces, sessions])

  useEffect(() => {
    try {
      localStorage.setItem(VIEW_STORAGE_KEY, JSON.stringify({ viewMode, sortMode }))
    } catch {
      // View preferences are optional.
    }
  }, [sortMode, viewMode])

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 30_000)
    return () => window.clearInterval(timer)
  }, [])

  const normalizedQuery = query.trim().toLowerCase()
  const sortedSessions = useMemo(() => [...sessions].sort((left, right) => {
    if (sortMode === 'name') return left.title.localeCompare(right.title)
    return Date.parse(right.last_active_at) - Date.parse(left.last_active_at)
  }), [sessions, sortMode])

  const groups = useMemo(() => {
    const known = new Map(workspaces.map((workspace) => [workspace.path, workspace]))
    for (const session of sessions) {
      const path = normalizeWorkspacePath(session.cwd)
      if (!known.has(path)) known.set(path, { path, addedAt: Date.now() })
    }

    return [...known.values()]
      .map((workspace) => {
        const label = folderName(workspace.path)
        const workspaceMatches = `${label} ${workspace.path}`.toLowerCase().includes(normalizedQuery)
        const items = sortedSessions.filter((session) => {
          if (normalizeWorkspacePath(session.cwd) !== workspace.path) return false
          if (!normalizedQuery || workspaceMatches) return true
          return [session.title, session.agent?.display_name, session.agent?.detail]
            .filter(Boolean).join(' ').toLowerCase().includes(normalizedQuery)
        })
        const latestActivity = items.reduce((latest, session) => Math.max(latest, Date.parse(session.last_active_at) || 0), workspace.addedAt)
        return { ...workspace, label, sessions: items, latestActivity, visible: workspaceMatches || items.length > 0 || !normalizedQuery }
      })
      .filter((workspace) => workspace.visible)
      .sort((left, right) => sortMode === 'name'
        ? left.label.localeCompare(right.label)
        : right.latestActivity - left.latestActivity)
  }, [normalizedQuery, sessions, sortMode, sortedSessions, workspaces])

  const flatSessions = useMemo(() => sortedSessions.filter((session) => {
    if (!normalizedQuery) return true
    return [session.title, session.cwd, session.agent?.display_name, session.agent?.detail]
      .filter(Boolean).join(' ').toLowerCase().includes(normalizedQuery)
  }), [normalizedQuery, sortedSessions])

  const startNewSession = (path = activePath) => {
    if (!path) {
      setAddWorkspaceOpen(true)
      return
    }
    setActivePath(path)
    onCreate(path)
  }

  const submitWorkspace = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const path = normalizeWorkspacePath(workspaceInput)
    if (!path.startsWith('/')) {
      setWorkspaceError('Enter an absolute path, for example /Users/me/code/project.')
      return
    }
    addWorkspace(path)
    setWorkspaceInput('')
    setWorkspaceError('')
    setAddWorkspaceOpen(false)
  }

  const select = (session: SessionInfo) => {
    setActivePath(session.cwd)
    selectSession(session.id)
  }

  const toggleGroup = (cwd: string) => {
    setActivePath(cwd)
    setCollapsedGroups((current) => {
      const next = new Set(current)
      if (next.has(cwd)) next.delete(cwd)
      else next.add(cwd)
      return next
    })
  }

  const chooseView = (event: MouseEvent<HTMLButtonElement>, mode: ViewMode) => {
    setViewMode(mode)
    const menu = event.currentTarget.closest('details')
    if (menu) menu.open = false
  }

  const chooseSort = (event: MouseEvent<HTMLButtonElement>, mode: SortMode) => {
    setSortMode(mode)
    const menu = event.currentTarget.closest('details')
    if (menu) menu.open = false
  }

  const renderSession = (session: SessionInfo, nested: boolean) => {
    const agent = session.agent
    const status = agent?.status || 'idle'
    const agentTooltip = agent
      ? [agent.display_name, agent.status, agent.detail].filter(Boolean).join(' · ')
      : 'Shell'
    return (
      <article className={`session-item${nested ? ' is-nested' : ''}${session.id === selectedSessionId ? ' is-selected' : ''}`} key={session.id}>
        <button className="session-select" type="button" onClick={() => select(session)} title={`${session.title}\n${session.cwd}`}>
          <span className={`session-status is-${status}`} title={agentTooltip} />
          <span className="session-name">{session.title}</span>
          {viewMode === 'flat' && <span className="session-workspace">{folderName(session.cwd)}</span>}
          <span className="session-time">{relativeTime(session.last_active_at, now)}</span>
        </button>
        <details className="session-menu" onClick={(event) => event.stopPropagation()}>
          <summary title={`Actions for ${session.title}`} aria-label={`Actions for ${session.title}`}><Icon name="more" /></summary>
          <div className="session-menu-popover">
            <button type="button" onClick={() => onDestroy(session.id)}><Icon name="close" size={15} />Close session</button>
          </div>
        </details>
      </article>
    )
  }

  return (
    <aside className={`session-panel${expanded ? ' is-expanded' : ' is-collapsed'}`}>
      <div className="session-panel-header">
        {expanded ? (
          <div className="session-panel-brand" aria-label="Kumokara"><span className="brand-mark">雲</span><span className="brand-wordmark">Kumokara</span></div>
        ) : (
          <button className="brand-mark-button" type="button" onClick={onToggle} title="Expand sidebar" aria-label="Expand sidebar"><span className="brand-mark">雲</span></button>
        )}
        {expanded && <button className="sidebar-control sidebar-toggle" type="button" onClick={onToggle} title="Collapse sidebar (⌘⇧L)" aria-label="Collapse sidebar"><TabsPanelIcon /></button>}
      </div>

      <button className="new-session-button" type="button" onClick={() => startNewSession()} title={expanded ? `New Session${activePath ? ` in ${activePath}` : ''}` : 'New Session'} aria-label="New Session">
        <Icon name="add" size={expanded ? 17 : 19} />
        {expanded && <span>New Session</span>}
      </button>

      {expanded ? (
        <div className="session-browser">
          <div className="session-browser-heading">
            <span>Workspaces</span>
            <div className="session-browser-actions">
              <button className={`sidebar-control${searchOpen ? ' is-active' : ''}`} type="button" onClick={() => { if (searchOpen) setQuery(''); setSearchOpen((open) => !open) }} title="Search" aria-label="Search workspaces and sessions"><Icon name={searchOpen ? 'close' : 'search'} size={18} /></button>
              <details className="workspace-view-menu">
                <summary className="sidebar-control" title="View options" aria-label="View options"><Icon name="view" size={18} /></summary>
                <div className="workspace-view-popover">
                  <span className="view-menu-label">View</span>
                  <button type="button" onClick={(event) => chooseView(event, 'tree')}><span>Tree</span>{viewMode === 'tree' && <Icon name="check" />}</button>
                  <button type="button" onClick={(event) => chooseView(event, 'flat')}><span>Flat list</span>{viewMode === 'flat' && <Icon name="check" />}</button>
                  <span className="view-menu-label">Sort by</span>
                  <button type="button" onClick={(event) => chooseSort(event, 'recent')}><span>Recent activity</span>{sortMode === 'recent' && <Icon name="check" />}</button>
                  <button type="button" onClick={(event) => chooseSort(event, 'name')}><span>Name</span>{sortMode === 'name' && <Icon name="check" />}</button>
                </div>
              </details>
              <button className="sidebar-control" type="button" onClick={() => setAddWorkspaceOpen(true)} title="Add workspace" aria-label="Add workspace"><Icon name="addFolder" size={19} /></button>
            </div>
          </div>

          {searchOpen && <label className="session-search"><Icon name="search" size={15} /><input autoFocus value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search workspaces" aria-label="Search workspaces and sessions" /></label>}

          <div className="session-list">
            {workspaces.length === 0 && sessions.length === 0 && <p className="session-list-empty">Add a workspace to start a session.</p>}
            {(workspaces.length > 0 || sessions.length > 0) && viewMode === 'tree' && groups.length === 0 && <p className="session-list-empty">No workspace or session matches “{query.trim()}”.</p>}
            {viewMode === 'tree' && groups.map((group) => {
              const collapsed = !normalizedQuery && collapsedGroups.has(group.path)
              const containsSelected = group.sessions.some((session) => session.id === selectedSessionId)
              return (
                <section className="session-group" key={group.path}>
                  <div className={`workspace-row${activePath === group.path ? ' is-active' : ''}`}>
                    <button className={`session-group-header${containsSelected ? ' contains-selected' : ''}`} type="button" onClick={() => toggleGroup(group.path)} title={group.path} aria-expanded={!collapsed}>
                      <span className="group-leading"><Icon name="folder" /><Icon name="chevron" size={13} /></span>
                      <span className="group-name">{group.label}</span>
                      <span className="group-count">{group.sessions.length}</span>
                    </button>
                    <button className="workspace-new-session" type="button" onClick={() => startNewSession(group.path)} title={`New session in ${group.path}`} aria-label={`New session in ${group.label}`}><Icon name="add" size={15} /></button>
                  </div>
                  {!collapsed && group.sessions.map((session) => renderSession(session, true))}
                </section>
              )
            })}
            {viewMode === 'flat' && flatSessions.length === 0 && (sessions.length > 0 || normalizedQuery) && <p className="session-list-empty">No session matches “{query.trim()}”.</p>}
            {viewMode === 'flat' && flatSessions.map((session) => renderSession(session, false))}
          </div>
        </div>
      ) : (
        <button className="sidebar-rail-button" type="button" onClick={onToggle} title={`${workspaces.length} workspaces`} aria-label={`Expand ${workspaces.length} workspaces`}><Icon name="folder" size={19} />{sessions.length > 0 && <span className="rail-count">{sessions.length}</span>}</button>
      )}

      <footer className="session-panel-footer">
        <div className="connection-status" title={connected ? 'Connected' : 'Reconnecting'}><span className={`status-dot${connected ? ' is-online' : ''}`} />{expanded && <span>{connected ? 'Connected' : 'Reconnecting'}</span>}</div>
        <button className="settings-button" type="button" onClick={onOpenSettings} title="Settings" aria-label="Settings"><Icon name="settings" size={18} />{expanded && <span>Settings</span>}</button>
      </footer>

      {addWorkspaceOpen && (
        <div className="workspace-dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setAddWorkspaceOpen(false) }}>
          <section className="workspace-dialog" role="dialog" aria-modal="true" aria-labelledby="add-workspace-title">
            <header><div><h2 id="add-workspace-title">Add workspace</h2><p>Sessions created here will start in this directory.</p></div><button type="button" onClick={() => setAddWorkspaceOpen(false)} aria-label="Close"><Icon name="close" /></button></header>
            <form onSubmit={submitWorkspace}>
              <label htmlFor="workspace-path">Directory path</label>
              <input id="workspace-path" autoFocus value={workspaceInput} onChange={(event) => { setWorkspaceInput(event.target.value); setWorkspaceError('') }} placeholder="/Users/me/code/project" spellCheck={false} />
              {workspaceError && <p className="workspace-dialog-error" role="alert">{workspaceError}</p>}
              <p className="workspace-dialog-hint">Use an absolute path on the Kumokara server.</p>
              <div className="workspace-dialog-actions"><button className="secondary-button" type="button" onClick={() => setAddWorkspaceOpen(false)}>Cancel</button><button className="primary-button" type="submit" disabled={!workspaceInput.trim()}>Add workspace</button></div>
            </form>
          </section>
        </div>
      )}
    </aside>
  )
}
