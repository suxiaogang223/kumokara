import { type FormEvent, type ReactNode, useCallback, useEffect, useRef, useState } from 'react'
import { onDirectoryBrowserMessage } from '../hooks/useWebSocket'
import type { ClientMessage, DirectoryEntry } from '../protocol'
import { useSessionStore } from '../store/sessionStore'
import { createRequestId } from '../utils/requestId'

interface Props {
  onCancel: () => void
  onOpen: (path: string) => void
}

type IconName = 'back' | 'chevron' | 'edit' | 'folder' | 'home' | 'plus'

function Icon({ name, size = 18 }: { name: IconName; size?: number }) {
  const paths: Record<IconName, ReactNode> = {
    back: <path d="m15 18-6-6 6-6" />,
    chevron: <path d="m9 6 6 6-6 6" />,
    edit: <><path d="m4 20 4.2-1 10.5-10.5a2.1 2.1 0 0 0-3-3L5.2 16 4 20Z" /><path d="m14.5 6.5 3 3" /></>,
    folder: <path d="M3.5 7.5h6l2-2h3.7c1 0 1.8.8 1.8 1.8v.2h3.5v10.2c0 1-.8 1.8-1.8 1.8H5.3c-1 0-1.8-.8-1.8-1.8V7.5Z" />,
    home: <><path d="m3 11 9-7 9 7" /><path d="M5.5 9.5V20h13V9.5M10 20v-6h4v6" /></>,
    plus: <path d="M12 5v14M5 12h14" />,
  }
  return (
    <svg aria-hidden="true" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      {paths[name]}
    </svg>
  )
}

function displayName(path: string) {
  const normalized = path.replace(/[\\/]+$/, '')
  const parts = normalized.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] || path
}

export function WorkspacePicker({ onCancel, onOpen }: Props) {
  const ws = useSessionStore((state) => state.ws)
  const [homePath, setHomePath] = useState('')
  const [currentPath, setCurrentPath] = useState('')
  const [parentPath, setParentPath] = useState<string | null>(null)
  const [entries, setEntries] = useState<DirectoryEntry[]>([])
  const [selected, setSelected] = useState<DirectoryEntry | null>(null)
  const [showHidden, setShowHidden] = useState(false)
  const [editingPath, setEditingPath] = useState(false)
  const [pathInput, setPathInput] = useState('')
  const [newFolderOpen, setNewFolderOpen] = useState(false)
  const [newFolderName, setNewFolderName] = useState('')
  const [loading, setLoading] = useState(true)
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState('')
  const listRequestRef = useRef('')
  const createRequestRef = useRef('')
  const startedRef = useRef(false)

  const send = useCallback((message: ClientMessage) => {
    if (ws?.readyState !== WebSocket.OPEN) {
      setLoading(false)
      setCreating(false)
      setError('Kumokara is reconnecting. Close this dialog and try again when connected.')
      return false
    }
    ws.send(JSON.stringify(message))
    return true
  }, [ws])

  const requestDirectory = useCallback((path?: string, hidden = showHidden) => {
    const requestId = createRequestId()
    listRequestRef.current = requestId
    setLoading(true)
    setError('')
    send({
      type: 'directory_list',
      request_id: requestId,
      ...(path ? { path } : {}),
      show_hidden: hidden,
    })
  }, [send, showHidden])

  useEffect(() => {
    const unsubscribe = onDirectoryBrowserMessage((message) => {
      if (message.type === 'directory_listing' && message.request_id === listRequestRef.current) {
        setHomePath(message.home)
        setCurrentPath(message.path)
        setParentPath(message.parent)
        setPathInput(message.path)
        setEntries(message.entries)
        setSelected(null)
        setEditingPath(false)
        setLoading(false)
        return
      }
      if (message.type === 'directory_created' && message.request_id === createRequestRef.current) {
        setCreating(false)
        setNewFolderOpen(false)
        setNewFolderName('')
        requestDirectory(message.path)
        return
      }
      if (message.type === 'error'
        && message.request_id
        && (message.request_id === listRequestRef.current || message.request_id === createRequestRef.current)) {
        setLoading(false)
        setCreating(false)
        setError(message.message || 'Unable to open this directory.')
      }
    })
    return unsubscribe
  }, [requestDirectory])

  useEffect(() => {
    if (startedRef.current) return
    startedRef.current = true
    requestDirectory()
  }, [requestDirectory])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onCancel()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onCancel])

  const submitPath = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const path = pathInput.trim()
    if (path) requestDirectory(path)
  }

  const submitNewFolder = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    const name = newFolderName.trim()
    if (!currentPath || !name) return
    const requestId = createRequestId()
    createRequestRef.current = requestId
    setCreating(true)
    setError('')
    send({ type: 'directory_create', request_id: requestId, parent: currentPath, name })
  }

  const toggleHidden = () => {
    const next = !showHidden
    setShowHidden(next)
    requestDirectory(currentPath || undefined, next)
  }

  const selectedPath = selected?.path || currentPath
  const locationLabel = currentPath === homePath ? 'Home' : displayName(currentPath)

  return (
    <div className="workspace-picker-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onCancel() }}>
      <section className="workspace-picker" role="dialog" aria-modal="true" aria-labelledby="workspace-picker-title">
        <header className="workspace-picker-header">
          <h2 id="workspace-picker-title">Select Workspace Directory</h2>
        </header>

        <div className="workspace-picker-location">
          <button type="button" disabled={!parentPath || loading} onClick={() => parentPath && requestDirectory(parentPath)} title="Parent directory" aria-label="Open parent directory"><Icon name="back" /></button>
          {editingPath ? (
            <form onSubmit={submitPath}>
              <input autoFocus value={pathInput} onChange={(event) => setPathInput(event.target.value)} onBlur={() => { if (pathInput === currentPath) setEditingPath(false) }} aria-label="Server directory path" spellCheck={false} />
              <button type="submit" disabled={!pathInput.trim() || loading}>Go</button>
            </form>
          ) : (
            <button className="workspace-picker-current" type="button" onClick={() => homePath && requestDirectory(homePath)} disabled={!homePath || loading} title={homePath || currentPath}>
              <Icon name="home" size={17} />
              <span>{locationLabel || 'Home'}</span>
            </button>
          )}
          <button type="button" onClick={() => setEditingPath(true)} title="Edit server path" aria-label="Edit server directory path"><Icon name="edit" size={17} /></button>
        </div>

        <div className="workspace-picker-body" aria-busy={loading}>
          {newFolderOpen && (
            <form className="workspace-new-folder-form" onSubmit={submitNewFolder}>
              <Icon name="folder" />
              <input autoFocus value={newFolderName} onChange={(event) => setNewFolderName(event.target.value)} placeholder="New folder name" aria-label="New folder name" />
              <button className="secondary-button" type="button" onClick={() => { setNewFolderOpen(false); setNewFolderName('') }}>Cancel</button>
              <button className="primary-button" type="submit" disabled={!newFolderName.trim() || creating}>{creating ? 'Creating…' : 'Create'}</button>
            </form>
          )}

          {error && <div className="workspace-picker-error" role="alert">{error}</div>}
          {loading && <div className="workspace-picker-state">Loading directories…</div>}
          {!loading && !error && entries.length === 0 && <div className="workspace-picker-state">This directory has no subfolders. You can open the current directory.</div>}
          {!loading && entries.length > 0 && (
            <div className="workspace-directory-list" role="listbox" aria-label={`Folders in ${currentPath}`}>
              {entries.map((entry) => (
                <div className={`workspace-directory-row${selected?.path === entry.path ? ' is-selected' : ''}`} key={entry.path} role="option" aria-selected={selected?.path === entry.path} onDoubleClick={() => requestDirectory(entry.path)}>
                  <button className="workspace-directory-select" type="button" onClick={() => setSelected(entry)} title={entry.path}>
                    <Icon name="folder" size={19} />
                    <span>{entry.name}</span>
                  </button>
                  <button className="workspace-directory-enter" type="button" onClick={() => requestDirectory(entry.path)} title={`Open ${entry.name}`} aria-label={`Open folder ${entry.name}`}><Icon name="chevron" size={17} /></button>
                </div>
              ))}
            </div>
          )}
        </div>

        <footer className="workspace-picker-footer">
          <div className="workspace-picker-tools">
            <button className="workspace-picker-new-folder" type="button" disabled={!currentPath || loading} onClick={() => setNewFolderOpen(true)}><Icon name="plus" size={17} />New folder</button>
            <label><input type="checkbox" checked={showHidden} onChange={toggleHidden} />Show hidden folders</label>
          </div>
          <div className="workspace-picker-actions">
            <button className="secondary-button" type="button" onClick={onCancel}>Cancel</button>
            <button className="primary-button" type="button" disabled={!selectedPath || loading || creating} onClick={() => selectedPath && onOpen(selectedPath)}>Open</button>
          </div>
        </footer>
      </section>
    </div>
  )
}
