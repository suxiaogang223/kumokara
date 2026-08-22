import { create } from 'zustand'
import { normalizeServerUrl } from '../connection'
import { readDesktopConfig } from '../runtime/desktop'
import { useSessionStore } from './sessionStore'

export type ConnectionMode = 'browser' | 'local' | 'remote'

interface ConnectionPreference {
  mode: 'local' | 'remote'
  remoteServerUrl?: string
  allowInsecureRemote?: boolean
}

interface ConnectionState {
  initialized: boolean
  desktopRuntime: boolean
  appVersion: string | null
  mode: ConnectionMode
  serverUrl: string | null
  localServerUrl: string | null
  localServerToken: string
  remoteServerUrl: string
  allowInsecureRemote: boolean
  initialize: () => Promise<void>
  connectRemote: (serverUrl: string, token: string, allowInsecure: boolean) => void
  useLocalServer: () => void
}

const STORAGE_KEY = 'kumokara.desktop-connection.v1'
let initialization: Promise<void> | null = null

function readPreference(): ConnectionPreference {
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') as Partial<ConnectionPreference>
    if (value.mode === 'remote' && typeof value.remoteServerUrl === 'string') {
      return {
        mode: 'remote',
        remoteServerUrl: value.remoteServerUrl,
        allowInsecureRemote: value.allowInsecureRemote === true,
      }
    }
  } catch {
    // Invalid or unavailable storage falls back to the private local server.
  }
  return { mode: 'local' }
}

function storePreference(preference: ConnectionPreference) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(preference))
  } catch {
    // Connection remains active even when persistence is unavailable.
  }
}

export const useConnectionStore = create<ConnectionState>((set, get) => ({
  initialized: false,
  desktopRuntime: false,
  appVersion: null,
  mode: 'browser',
  serverUrl: null,
  localServerUrl: null,
  localServerToken: '',
  remoteServerUrl: '',
  allowInsecureRemote: false,

  initialize: async () => {
    if (get().initialized) return
    if (initialization) return initialization

    initialization = (async () => {
      const desktop = await readDesktopConfig()
      if (!desktop) {
        useSessionStore.getState().resetConnection('')
        set({
          initialized: true,
          mode: 'browser',
          serverUrl: window.location.origin,
        })
        return
      }

      const preference = readPreference()
      const useRemote = preference.mode === 'remote' && Boolean(preference.remoteServerUrl)
      let remoteServerUrl = ''
      if (useRemote) {
        try {
          remoteServerUrl = normalizeServerUrl(
            preference.remoteServerUrl!,
            preference.allowInsecureRemote,
          )
        } catch {
          storePreference({ mode: 'local' })
        }
      }

      const mode = remoteServerUrl ? 'remote' : 'local'
      const token = mode === 'local' ? desktop.localServerToken : ''
      useSessionStore.getState().resetConnection(token)
      set({
        initialized: true,
        desktopRuntime: true,
        appVersion: desktop.appVersion,
        mode,
        serverUrl: mode === 'local' ? desktop.localServerUrl : remoteServerUrl,
        localServerUrl: desktop.localServerUrl,
        localServerToken: desktop.localServerToken,
        remoteServerUrl,
        allowInsecureRemote: mode === 'remote' && preference.allowInsecureRemote === true,
      })
    })().finally(() => { initialization = null })

    return initialization
  },

  connectRemote: (serverUrl, token, allowInsecure) => {
    if (!get().desktopRuntime) throw new Error('Remote server switching is only available in the desktop app')
    const normalized = normalizeServerUrl(serverUrl, allowInsecure)
    storePreference({
      mode: 'remote',
      remoteServerUrl: normalized,
      allowInsecureRemote: allowInsecure,
    })
    useSessionStore.getState().resetConnection(token.trim())
    set({
      mode: 'remote',
      serverUrl: normalized,
      remoteServerUrl: normalized,
      allowInsecureRemote: allowInsecure,
    })
  },

  useLocalServer: () => {
    const state = get()
    if (!state.desktopRuntime || !state.localServerUrl) return
    storePreference({ mode: 'local' })
    useSessionStore.getState().resetConnection(state.localServerToken)
    set({
      mode: 'local',
      serverUrl: state.localServerUrl,
      allowInsecureRemote: false,
    })
  },
}))
