import React, { useState, useCallback } from 'react'
import { Terminal } from './components/Terminal'
import { WorkspacePanel } from './components/WorkspacePanel'
import { CreateWorkspace } from './components/CreateWorkspace'
import { useWebSocket } from './hooks/useWebSocket'
import { useWorkspaceStore } from './store/workspaceStore'

const App: React.FC = () => {
  const [tokenInput, setTokenInput] = useState('')
  const authState = useWorkspaceStore((s) => s.authState)
  const authErrorMessage = useWorkspaceStore((s) => s.authErrorMessage)
  const setAuthToken = useWorkspaceStore((s) => s.setAuthToken)
  const setAuthState = useWorkspaceStore((s) => s.setAuthState)
  const connected = useWorkspaceStore((s) => s.connected)
  const workspaces = useWorkspaceStore((s) => s.workspaces)
  const selectedWorkspaceId = useWorkspaceStore((s) => s.selectedWorkspaceId)
  const selectedSessionId = useWorkspaceStore((s) => s.selectedSessionId)
  const sessions = useWorkspaceStore((s) => s.sessions)
  const selectSession = useWorkspaceStore((s) => s.selectSession)
  const { send } = useWebSocket()

  const handleTokenSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const token = tokenInput.trim()
    if (!token) return
    setAuthState('connecting')
    setAuthToken(token)
  }

  const handleRetry = () => {
    setAuthState('idle')
    useWorkspaceStore.getState().setAuthToken('')
  }

  const handleCreateWorkspace = useCallback(
    (name: string) => {
      send({
        type: 'create_workspace',
        request_id: crypto.randomUUID(),
        name,
      })
    },
    [send]
  )

  const handleCreateSession = useCallback(() => {
    if (!selectedWorkspaceId) return
    send({
      type: 'session_create',
      request_id: crypto.randomUUID(),
      workspace_id: selectedWorkspaceId,
      session_type: 'shell',
      cols: 80,
      rows: 24,
    })
  }, [send, selectedWorkspaceId])

  const workspaceSessions = selectedWorkspaceId ? (sessions[selectedWorkspaceId] || []) : []
  const selectedWorkspace = workspaces.find(w => w.id === selectedWorkspaceId)

  // ================================================================
  // Auth screen — shown until token is verified by the server
  // ================================================================
  if (authState !== 'authenticated') {
    const isLoading = authState === 'connecting'
    const hasError = authState === 'error'

    return (
      <div style={{
        display: 'flex', flexDirection: 'column', alignItems: 'center',
        justifyContent: 'center', height: '100vh', background: '#1a1a2e',
      }}>
        <div style={{ fontSize: '2rem', marginBottom: '0.5rem', color: '#00E5FF' }}>
          ☁ Kumokara（雲殻）
        </div>
        <p style={{ color: '#888', marginBottom: '1.5rem', fontSize: '0.9rem' }}>
          Agents never sleep in Kumokara.
        </p>

        {hasError ? (
          // Error state
          <div style={{ textAlign: 'center' }}>
            <div style={{
              background: '#3a1010', border: '1px solid #FF5252',
              borderRadius: '8px', padding: '1rem 1.5rem', marginBottom: '1rem',
              maxWidth: '400px',
            }}>
              <p style={{ color: '#FF5252', fontWeight: 'bold', margin: '0 0 0.5rem 0' }}>
                Authentication Failed
              </p>
              <p style={{ color: '#ff8a80', fontSize: '0.85rem', margin: 0 }}>
                {authErrorMessage || 'Invalid token. Please check and try again.'}
              </p>
            </div>
            <button
              onClick={handleRetry}
              style={{
                padding: '0.75rem 2rem', fontSize: '1rem', borderRadius: '6px',
                border: 'none', background: '#00E5FF', color: '#1a1a2e',
                cursor: 'pointer', fontWeight: 'bold',
              }}
            >
              Try Again
            </button>
          </div>
        ) : (
          // Idle or loading state
          <form onSubmit={handleTokenSubmit} style={{ display: 'flex', gap: '0.5rem' }}>
            <input
              type="password" value={tokenInput}
              onChange={(e) => setTokenInput(e.target.value)}
              placeholder="Enter server token"
              disabled={isLoading}
              autoFocus
              style={{
                padding: '0.75rem 1rem', fontSize: '1rem', borderRadius: '6px',
                border: '1px solid #444', background: '#2a2a3e', color: '#e0e0e0',
                width: '300px', outline: 'none',
                opacity: isLoading ? 0.6 : 1,
              }}
            />
            <button type="submit" disabled={!tokenInput.trim() || isLoading}
              style={{
                padding: '0.75rem 1.5rem', fontSize: '1rem', borderRadius: '6px',
                border: 'none',
                background: (tokenInput.trim() && !isLoading) ? '#00E5FF' : '#444',
                color: (tokenInput.trim() && !isLoading) ? '#1a1a2e' : '#888',
                cursor: (tokenInput.trim() && !isLoading) ? 'pointer' : 'not-allowed',
                fontWeight: 'bold',
              }}
            >
              {isLoading ? 'Connecting...' : 'Connect'}
            </button>
          </form>
        )}

        {isLoading && (
          <p style={{ color: '#888', marginTop: '1rem', fontSize: '0.85rem' }}>
            Verifying token with server...
          </p>
        )}
      </div>
    )
  }

  // ================================================================
  // Main layout — shown only after successful authentication
  // ================================================================
  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
      {/* Top Bar */}
      <div style={{
        height: '40px', background: '#0f3460', display: 'flex',
        alignItems: 'center', justifyContent: 'space-between',
        padding: '0 1rem', borderBottom: '1px solid #2a2a3e', fontSize: '0.85rem',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
          <span style={{ color: '#00E5FF', fontWeight: 'bold' }}>☁ Kumokara</span>
          <span style={{ color: '#666', fontSize: '0.75rem' }}>v0.1.0</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', color: '#888', fontSize: '0.75rem' }}>
          <span style={{
            width: '8px', height: '8px', borderRadius: '50%',
            background: connected ? '#69F0AE' : '#FF5252', display: 'inline-block',
          }} />
          {connected ? 'Connected' : 'Disconnected'}
        </div>
      </div>

      {/* Body */}
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        <WorkspacePanel />

        {/* Main content area */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          {/* Title bar */}
          <div style={{
            height: '36px', background: '#16213e', borderBottom: '1px solid #2a2a3e',
            display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            padding: '0 1rem', fontSize: '0.8rem', color: '#888',
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
              {selectedWorkspace ? (
                <>
                  <span>{selectedWorkspace.name}</span>
                  {workspaceSessions.map((s) => (
                    <button
                      key={s.id}
                      onClick={() => selectSession(s.id)}
                      style={{
                        background: selectedSessionId === s.id ? '#0f3460' : 'transparent',
                        border: '1px solid #444', borderRadius: '4px',
                        color: selectedSessionId === s.id ? '#00E5FF' : '#aaa',
                        padding: '2px 10px', cursor: 'pointer', fontSize: '0.75rem',
                      }}
                    >
                      $ {s.title} ({s.id.slice(0, 6)})
                    </button>
                  ))}
                </>
              ) : (
                <span>No workspace selected</span>
              )}
            </div>
            <div style={{ display: 'flex', gap: '0.5rem' }}>
              {selectedWorkspaceId && (
                <button onClick={handleCreateSession}
                  style={{
                    background: '#00E5FF', border: 'none', borderRadius: '4px',
                    color: '#1a1a2e', padding: '2px 12px', cursor: 'pointer',
                    fontSize: '0.75rem', fontWeight: 'bold',
                  }}
                >
                  + New Shell
                </button>
              )}
            </div>
          </div>

          {/* Terminal */}
          <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
            {workspaces.length === 0 ? (
              <CreateWorkspace onCreated={handleCreateWorkspace} />
            ) : (
              <Terminal sessionId={selectedSessionId} />
            )}
          </div>

          {/* Status Bar */}
          <div style={{
            height: '28px', background: '#0f3460', borderTop: '1px solid #2a2a3e',
            display: 'flex', alignItems: 'center', justifyContent: 'space-between',
            padding: '0 1rem', fontSize: '0.7rem', color: '#666',
          }}>
            <span>{connected ? '● Connected' : '○ Disconnected'}{selectedSessionId ? ` · Session: ${selectedSessionId.slice(0, 8)}...` : ''}</span>
            <span>Kumokara v0.1.0 — Phase 0</span>
          </div>
        </div>
      </div>
    </div>
  )
}

export default App
