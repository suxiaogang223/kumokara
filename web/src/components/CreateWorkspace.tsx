import React, { useState } from 'react'
import { useWorkspaceStore } from '../store/workspaceStore'

interface Props {
  onCreated: (name: string) => void
}

export const CreateWorkspace: React.FC<Props> = ({ onCreated }) => {
  const [name, setName] = useState('')
  const [creating, setCreating] = useState(false)
  const connected = useWorkspaceStore((s) => s.connected)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim()) return
    setCreating(true)
    onCreated(name.trim())
  }

  if (!connected) {
    return (
      <div style={{ padding: '2rem', textAlign: 'center', color: '#888' }}>
        <p>Connecting to Kumokara server...</p>
        <p style={{ fontSize: '0.85rem', marginTop: '0.5rem' }}>
          Make sure the server is running on port 9876
        </p>
      </div>
    )
  }

  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      height: '100%',
      color: '#e0e0e0',
    }}>
      <div style={{ fontSize: '1.5rem', marginBottom: '0.5rem' }}>
        ☁ Kumokara（雲殻）
      </div>
      <p style={{ color: '#888', marginBottom: '2rem' }}>
        Create your first workspace to get started
      </p>
      <form onSubmit={handleSubmit} style={{ display: 'flex', gap: '0.5rem' }}>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Workspace name (e.g. my-project)"
          style={{
            padding: '0.75rem 1rem',
            fontSize: '1rem',
            borderRadius: '6px',
            border: '1px solid #444',
            background: '#2a2a3e',
            color: '#e0e0e0',
            width: '300px',
            outline: 'none',
          }}
          autoFocus
        />
        <button
          type="submit"
          disabled={creating || !name.trim()}
          style={{
            padding: '0.75rem 1.5rem',
            fontSize: '1rem',
            borderRadius: '6px',
            border: 'none',
            background: name.trim() ? '#00E5FF' : '#444',
            color: name.trim() ? '#1a1a2e' : '#888',
            cursor: name.trim() ? 'pointer' : 'not-allowed',
            fontWeight: 'bold',
          }}
        >
          {creating ? 'Creating...' : 'Create'}
        </button>
      </form>
    </div>
  )
}
