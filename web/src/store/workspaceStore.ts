import { create } from 'zustand'

export interface WorkspaceInfo {
  id: string
  name: string
  status: string
  work_dir: string
  created_at: string
  updated_at: string
  session_count: number
}

export interface SessionInfo {
  id: string
  workspace_id: string
  session_type: string
  agent: {
    provider: string
    cli_session_id: string | null
    model: string | null
  } | null
  title: string
  state: string
  created_at: string
  last_active_at: string
  cols: number
  rows: number
}

export type AuthState = 'idle' | 'connecting' | 'authenticated' | 'error'

interface WorkspaceState {
  workspaces: WorkspaceInfo[]
  selectedWorkspaceId: string | null
  sessions: Record<string, SessionInfo[]>  // workspace_id → sessions
  selectedSessionId: string | null
  connected: boolean
  authToken: string
  authState: AuthState
  authErrorMessage: string
  ws: WebSocket | null

  setWorkspaces: (workspaces: WorkspaceInfo[]) => void
  addWorkspace: (workspace: WorkspaceInfo) => void
  selectWorkspace: (id: string | null) => void
  setSessions: (workspaceId: string, sessions: SessionInfo[]) => void
  addSession: (workspaceId: string, session: SessionInfo) => void
  removeSession: (sessionId: string) => void
  selectSession: (id: string | null) => void
  setConnected: (connected: boolean) => void
  setAuthToken: (token: string) => void
  setAuthState: (state: AuthState) => void
  setAuthError: (message: string) => void
  setWs: (ws: WebSocket | null) => void
}

export const useWorkspaceStore = create<WorkspaceState>((set) => ({
  workspaces: [],
  selectedWorkspaceId: null,
  sessions: {},
  selectedSessionId: null,
  connected: false,
  authToken: '',
  authState: 'idle',
  authErrorMessage: '',
  ws: null,

  setWorkspaces: (workspaces) => set({ workspaces }),
  addWorkspace: (workspace) =>
    set((state) => ({ workspaces: [...state.workspaces, workspace] })),
  selectWorkspace: (id) => set({ selectedWorkspaceId: id }),
  setSessions: (workspaceId, sessions) =>
    set((state) => ({
      sessions: { ...state.sessions, [workspaceId]: sessions },
    })),
  addSession: (workspaceId, session) =>
    set((state) => ({
      sessions: {
        ...state.sessions,
        [workspaceId]: [...(state.sessions[workspaceId] || []), session],
      },
      selectedSessionId: session.id,
    })),
  removeSession: (sessionId) =>
    set((state) => {
      const newSessions: Record<string, SessionInfo[]> = {}
      for (const [wid, sessions] of Object.entries(state.sessions)) {
        newSessions[wid] = sessions.filter((s) => s.id !== sessionId)
      }
      return {
        sessions: newSessions,
        selectedSessionId: state.selectedSessionId === sessionId ? null : state.selectedSessionId,
      }
    }),
  selectSession: (id) => set({ selectedSessionId: id }),
  setConnected: (connected) => set({ connected }),
  setAuthToken: (token) => set({ authToken: token }),
  setAuthState: (authState) => set({ authState }),
  setAuthError: (message) => set({ authErrorMessage: message, authState: 'error' }),
  setWs: (ws) => set({ ws }),
}))
