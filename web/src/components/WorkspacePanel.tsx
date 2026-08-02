import React from 'react'
import { useWorkspaceStore } from '../store/workspaceStore'

const statusBadge = (status: string): string => {
  switch (status) {
    case 'agent_running': return '⠿'
    case 'agent_waiting': return '✋'
    case 'error': return '△'
    default: return '○'
  }
}

const statusColor = (status: string): string => {
  switch (status) {
    case 'agent_running': return '#00E5FF'
    case 'agent_waiting': return '#FFD600'
    case 'error': return '#FF5252'
    default: return '#888'
  }
}

export const WorkspacePanel: React.FC = () => {
  const workspaces = useWorkspaceStore((s) => s.workspaces)
  const selectedId = useWorkspaceStore((s) => s.selectedWorkspaceId)
  const selectWorkspace = useWorkspaceStore((s) => s.selectWorkspace)

  return (
    <div style={{
      width: '240px',
      background: '#16213e',
      borderRight: '1px solid #2a2a3e',
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      overflow: 'hidden',
    }}>
      {/* Header */}
      <div style={{
        padding: '1rem',
        fontSize: '0.75rem',
        fontWeight: 'bold',
        textTransform: 'uppercase',
        letterSpacing: '0.1em',
        color: '#888',
        borderBottom: '1px solid #2a2a3e',
      }}>
        Workspaces
      </div>

      {/* Workspace list */}
      <div style={{ flex: 1, overflow: 'auto', padding: '0.5rem' }}>
        {workspaces.length === 0 && (
          <div style={{ padding: '1rem', color: '#666', fontSize: '0.85rem', textAlign: 'center' }}>
            No workspaces yet
          </div>
        )}
        {workspaces.map((ws) => (
          <div
            key={ws.id}
            onClick={() => selectWorkspace(ws.id)}
            style={{
              padding: '0.75rem 1rem',
              borderRadius: '6px',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              gap: '0.5rem',
              background: selectedId === ws.id ? '#0f3460' : 'transparent',
              borderLeft: selectedId === ws.id ? '3px solid #00E5FF' : '3px solid transparent',
              marginBottom: '2px',
            }}
          >
            <span style={{ color: statusColor(ws.status), fontSize: '1.1rem' }}>
              {statusBadge(ws.status)}
            </span>
            <div>
              <div style={{ fontSize: '0.9rem', fontWeight: 500 }}>
                {ws.name}
              </div>
              <div style={{ fontSize: '0.7rem', color: '#888' }}>
                {ws.session_count} sessions
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
