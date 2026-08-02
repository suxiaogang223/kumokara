export interface SessionInfo {
  id: string
  cwd: string
  agent: { provider: string } | null
  title: string
  created_at: string
  last_active_at: string
  cols: number
  rows: number
}

export type ClientMessage =
  | { type: 'auth'; token: string }
  | { type: 'session_create'; request_id: string; cwd?: string; cols: number; rows: number }
  | { type: 'session_list'; request_id: string }
  | { type: 'session_attach'; request_id: string; session_id: string; last_seq?: number }
  | { type: 'session_detach'; session_id: string }
  | { type: 'session_destroy'; request_id: string; session_id: string }
  | { type: 'terminal_input'; session_id: string; data_base64: string }
  | { type: 'terminal_resize'; session_id: string; cols: number; rows: number }

export type ServerMessage =
  | { type: 'auth_ok'; server_version: string }
  | { type: 'auth_error'; code: string; message: string }
  | { type: 'session_created'; request_id: string; session: SessionInfo }
  | { type: 'session_list'; request_id: string; sessions: SessionInfo[] }
  | { type: 'terminal_output'; session_id: string; seq: number; data_base64: string }
  | { type: 'session_destroyed'; session_id: string }
  | { type: 'server_notification'; message: string }
  | { type: 'error'; request_id?: string; code: string; message: string }
