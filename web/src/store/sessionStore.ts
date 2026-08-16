import { create } from 'zustand'
import type { SessionInfo } from '../protocol'

export type AuthState = 'idle' | 'connecting' | 'authenticated' | 'error'

interface SessionState {
  sessions: SessionInfo[]
  selectedSessionId: string | null
  connected: boolean
  authToken: string
  authState: AuthState
  authErrorMessage: string
  sessionErrorMessage: string
  ws: WebSocket | null

  setSessions: (sessions: SessionInfo[]) => void
  addSession: (session: SessionInfo) => void
  removeSession: (sessionId: string) => void
  updateSessionTitle: (sessionId: string, title: string) => void
  selectSession: (id: string | null) => void
  setConnected: (connected: boolean) => void
  setAuthToken: (token: string) => void
  setAuthState: (state: AuthState) => void
  setAuthError: (message: string) => void
  setSessionError: (message: string) => void
  setWs: (ws: WebSocket | null) => void
}

export const useSessionStore = create<SessionState>((set) => ({
  sessions: [],
  selectedSessionId: null,
  connected: false,
  authToken: '',
  authState: 'idle',
  authErrorMessage: '',
  sessionErrorMessage: '',
  ws: null,

  setSessions: (sessions) =>
    set((state) => ({
      sessions,
      selectedSessionId:
        state.selectedSessionId && sessions.some((session) => session.id === state.selectedSessionId)
          ? state.selectedSessionId
          : sessions[0]?.id ?? null,
    })),
  addSession: (session) =>
    set((state) => ({
      sessions: [...state.sessions.filter((item) => item.id !== session.id), session],
      selectedSessionId: session.id,
      sessionErrorMessage: '',
    })),
  removeSession: (sessionId) =>
    set((state) => {
      const sessions = state.sessions.filter((session) => session.id !== sessionId)
      return {
        sessions,
        selectedSessionId:
          state.selectedSessionId === sessionId
            ? sessions[0]?.id ?? null
            : state.selectedSessionId,
      }
    }),
  updateSessionTitle: (sessionId, title) =>
    set((state) => ({
      sessions: state.sessions.map((session) => (
        session.id === sessionId ? { ...session, title } : session
      )),
    })),
  selectSession: (id) => set({ selectedSessionId: id }),
  setConnected: (connected) => set({ connected }),
  setAuthToken: (token) => set({ authToken: token }),
  setAuthState: (authState) => set({ authState }),
  setAuthError: (message) => set({ authErrorMessage: message, authState: 'error' }),
  setSessionError: (sessionErrorMessage) => set({ sessionErrorMessage }),
  setWs: (ws) => set({ ws }),
}))
