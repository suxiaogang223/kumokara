import { create } from 'zustand'

const STORAGE_KEY = 'kumokara.workspaces.v1'

export interface Workspace {
  path: string
  addedAt: number
}

interface PersistedWorkspaces {
  workspaces: Workspace[]
  activePath: string | null
}

interface WorkspaceState extends PersistedWorkspaces {
  addWorkspace: (path: string) => string
  removeWorkspace: (path: string) => void
  rememberWorkspaces: (paths: string[]) => void
  setActivePath: (path: string) => void
}

export function normalizeWorkspacePath(path: string) {
  const value = path.trim()
  if (value === '/') return value
  return value.replace(/\/+$/, '')
}

function loadWorkspaces(): PersistedWorkspaces {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}')
    const workspaces = Array.isArray(stored.workspaces)
      ? stored.workspaces
        .filter((item: unknown): item is Workspace => (
          typeof item === 'object' && item !== null
          && typeof (item as Workspace).path === 'string'
          && typeof (item as Workspace).addedAt === 'number'
        ))
        .map((item: Workspace) => ({ ...item, path: normalizeWorkspacePath(item.path) }))
        .filter((item: Workspace, index: number, items: Workspace[]) => (
          item.path.length > 0 && items.findIndex((candidate) => candidate.path === item.path) === index
        ))
      : []
    const activePath = typeof stored.activePath === 'string'
      ? normalizeWorkspacePath(stored.activePath)
      : null
    return { workspaces, activePath }
  } catch {
    return { workspaces: [], activePath: null }
  }
}

function persist(state: PersistedWorkspaces) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state))
  } catch {
    // Workspace navigation remains available when browser storage is unavailable.
  }
}

export const useWorkspaceStore = create<WorkspaceState>((set) => ({
  ...loadWorkspaces(),
  addWorkspace: (path) => {
    const normalized = normalizeWorkspacePath(path)
    set((state) => {
      const workspaces = state.workspaces.some((workspace) => workspace.path === normalized)
        ? state.workspaces
        : [...state.workspaces, { path: normalized, addedAt: Date.now() }]
      const next = { workspaces, activePath: normalized }
      persist(next)
      return next
    })
    return normalized
  },
  removeWorkspace: (path) => set((state) => {
    const normalized = normalizeWorkspacePath(path)
    const workspaces = state.workspaces.filter((workspace) => workspace.path !== normalized)
    const activePath = state.activePath === normalized
      ? workspaces[0]?.path ?? null
      : state.activePath
    const next = { workspaces, activePath }
    persist(next)
    return next
  }),
  rememberWorkspaces: (paths) => set((state) => {
    const known = new Set(state.workspaces.map((workspace) => workspace.path))
    const additions = [...new Set(paths.map(normalizeWorkspacePath))]
      .filter((path) => path && !known.has(path))
      .map((path, index) => ({ path, addedAt: Date.now() + index }))
    if (additions.length === 0) return state
    const workspaces = [...state.workspaces, ...additions]
    const activePath = state.activePath ?? additions[0]?.path ?? null
    const next = { workspaces, activePath }
    persist(next)
    return next
  }),
  setActivePath: (path) => set((state) => {
    const activePath = normalizeWorkspacePath(path)
    persist({ workspaces: state.workspaces, activePath })
    return { activePath }
  }),
}))
