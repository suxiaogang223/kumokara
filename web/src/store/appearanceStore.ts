import { create } from 'zustand'
import type { AppearanceMode } from '../theme'

const STORAGE_KEY = 'kumokara.appearance.v1'
export const DEFAULT_FONT_FAMILY = 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace'

interface PersistedAppearance {
  mode: AppearanceMode
  lightThemeId: string
  darkThemeId: string
  fontFamily: string
  fontSize: number
}

interface AppearanceState extends PersistedAppearance {
  setMode: (mode: AppearanceMode) => void
  setTheme: (appearance: 'light' | 'dark', id: string) => void
  setFontFamily: (fontFamily: string) => void
  setFontSize: (fontSize: number) => void
  reset: () => void
}

const defaults: PersistedAppearance = {
  mode: 'auto',
  lightThemeId: 'one-light',
  darkThemeId: 'kumokara-dark',
  fontFamily: DEFAULT_FONT_FAMILY,
  fontSize: 14,
}

function loadSettings(): PersistedAppearance {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}')
    return {
      mode: ['auto', 'light', 'dark'].includes(stored.mode) ? stored.mode : defaults.mode,
      lightThemeId: typeof stored.lightThemeId === 'string' ? stored.lightThemeId : defaults.lightThemeId,
      darkThemeId: typeof stored.darkThemeId === 'string' ? stored.darkThemeId : defaults.darkThemeId,
      fontFamily: typeof stored.fontFamily === 'string' && stored.fontFamily.trim()
        ? stored.fontFamily
        : defaults.fontFamily,
      fontSize: Number.isFinite(stored.fontSize)
        ? Math.min(24, Math.max(10, stored.fontSize))
        : defaults.fontSize,
    }
  } catch {
    return defaults
  }
}

function persist(settings: PersistedAppearance) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
}

function persistedState(
  state: AppearanceState,
  update: Partial<PersistedAppearance>,
): PersistedAppearance {
  return {
    mode: update.mode ?? state.mode,
    lightThemeId: update.lightThemeId ?? state.lightThemeId,
    darkThemeId: update.darkThemeId ?? state.darkThemeId,
    fontFamily: update.fontFamily ?? state.fontFamily,
    fontSize: update.fontSize ?? state.fontSize,
  }
}

export const useAppearanceStore = create<AppearanceState>((set) => ({
  ...loadSettings(),
  setMode: (mode) => set((state) => {
    persist(persistedState(state, { mode }))
    return { mode }
  }),
  setTheme: (appearance, id) => set((state) => {
    const update = appearance === 'light' ? { lightThemeId: id } : { darkThemeId: id }
    persist(persistedState(state, update))
    return update
  }),
  setFontFamily: (fontFamily) => set((state) => {
    const value = fontFamily.trim() || DEFAULT_FONT_FAMILY
    persist(persistedState(state, { fontFamily: value }))
    return { fontFamily: value }
  }),
  setFontSize: (fontSize) => set((state) => {
    const value = Math.min(24, Math.max(10, fontSize))
    persist(persistedState(state, { fontSize: value }))
    return { fontSize: value }
  }),
  reset: () => set(() => {
    persist(defaults)
    return defaults
  }),
}))
