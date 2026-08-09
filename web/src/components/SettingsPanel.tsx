import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { DEFAULT_FONT_FAMILY, useAppearanceStore } from '../store/appearanceStore'
import {
  DARK_THEMES,
  LIGHT_THEMES,
  type Appearance,
  type AppTheme,
  type AppearanceMode,
} from '../theme'

interface Props {
  activeAppearance: Appearance
  onClose: () => void
}

export function SettingsPanel({ activeAppearance, onClose }: Props) {
  const mode = useAppearanceStore((state) => state.mode)
  const lightThemeId = useAppearanceStore((state) => state.lightThemeId)
  const darkThemeId = useAppearanceStore((state) => state.darkThemeId)
  const fontFamily = useAppearanceStore((state) => state.fontFamily)
  const fontSize = useAppearanceStore((state) => state.fontSize)
  const setMode = useAppearanceStore((state) => state.setMode)
  const setTheme = useAppearanceStore((state) => state.setTheme)
  const setFontFamily = useAppearanceStore((state) => state.setFontFamily)
  const setFontSize = useAppearanceStore((state) => state.setFontSize)
  const reset = useAppearanceStore((state) => state.reset)
  const [fontDraft, setFontDraft] = useState(fontFamily)
  const dialogRef = useRef<HTMLDialogElement>(null)

  useEffect(() => setFontDraft(fontFamily), [fontFamily])

  useEffect(() => {
    const dialog = dialogRef.current
    if (dialog && !dialog.open) dialog.showModal()
  }, [])

  const commitFontFamily = () => setFontFamily(fontDraft)

  return (
    <dialog
      ref={dialogRef}
      className="settings-dialog"
      aria-labelledby="settings-title"
      onClose={onClose}
    >
      <div className="settings-content">
        <header className="settings-header">
          <div>
            <h1 id="settings-title">Appearance</h1>
            <p>Terminal typography and color themes.</p>
          </div>
          <button className="settings-close" onClick={onClose} aria-label="Close settings">×</button>
        </header>

        <section className="settings-section">
          <h2>Color mode</h2>
          <label className="select-field" htmlFor="appearance-mode">
            <span>Mode</span>
            <select
              id="appearance-mode"
              value={mode}
              onChange={(event) => setMode(event.target.value as AppearanceMode)}
            >
              <option value="auto">Auto</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </label>
          {mode === 'auto' && (
            <p className="settings-hint">Following system: currently {activeAppearance}.</p>
          )}
        </section>

        <section className="settings-section">
          <h2>Text</h2>
          <div className="setting-row">
            <div>
              <label htmlFor="terminal-font">Font family</label>
              <p>Use the system default or enter a font installed on this device.</p>
            </div>
            <div className="font-field">
              <input
                id="terminal-font"
                value={fontDraft}
                onChange={(event) => setFontDraft(event.target.value)}
                onBlur={commitFontFamily}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') event.currentTarget.blur()
                }}
                spellCheck={false}
              />
              <button
                className="font-default-button"
                onClick={() => {
                  setFontDraft(DEFAULT_FONT_FAMILY)
                  setFontFamily(DEFAULT_FONT_FAMILY)
                }}
              >
                Use system default
              </button>
            </div>
          </div>
          <div className="setting-row">
            <div>
              <label htmlFor="terminal-font-size">Font size</label>
              <p>Applied immediately to every terminal session.</p>
            </div>
            <input
              className="number-input"
              id="terminal-font-size"
              type="number"
              min="10"
              max="24"
              step="1"
              value={fontSize}
              onChange={(event) => {
                if (Number.isFinite(event.currentTarget.valueAsNumber)) {
                  setFontSize(event.currentTarget.valueAsNumber)
                }
              }}
            />
          </div>
        </section>

        <ThemeSection
          title="Light themes"
          themes={LIGHT_THEMES}
          selectedId={lightThemeId}
          onSelect={(id) => setTheme('light', id)}
        />
        <ThemeSection
          title="Dark themes"
          themes={DARK_THEMES}
          selectedId={darkThemeId}
          onSelect={(id) => setTheme('dark', id)}
        />

        <div className="settings-footer">
          <button className="secondary-button" onClick={reset}>Restore defaults</button>
        </div>
      </div>
    </dialog>
  )
}

function ThemeSection({
  title,
  themes,
  selectedId,
  onSelect,
}: {
  title: string
  themes: readonly AppTheme[]
  selectedId: string
  onSelect: (id: string) => void
}) {
  return (
    <section className="settings-section">
      <h2>{title}</h2>
      <div className="theme-grid">
        {themes.map((theme) => {
          const previewStyle = {
            '--preview-background': theme.terminal.background,
            '--preview-foreground': theme.terminal.foreground,
            '--preview-muted': theme.ui.textMuted,
            '--preview-accent': theme.ui.accent,
            '--preview-green': theme.terminal.green,
          } as CSSProperties
          return (
            <button
              className={`theme-option${selectedId === theme.id ? ' is-selected' : ''}`}
              key={theme.id}
              onClick={() => onSelect(theme.id)}
              aria-pressed={selectedId === theme.id}
            >
              <span className="theme-preview" style={previewStyle}>
                <span className="preview-dots"><i /><i /><i /></span>
                <span className="preview-command"><i /><i /><i /></span>
              </span>
              <span>{theme.name}</span>
            </button>
          )
        })}
      </div>
    </section>
  )
}
