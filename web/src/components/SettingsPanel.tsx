import { useEffect, useState, type CSSProperties } from 'react'
import { useAppearanceStore } from '../store/appearanceStore'
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

const FONT_SUGGESTIONS = [
  'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
  'MesloLGM Nerd Font Mono, monospace',
  'JetBrainsMono Nerd Font Mono, monospace',
  'FiraCode Nerd Font Mono, monospace',
  'Hack Nerd Font Mono, monospace',
  'JetBrains Mono, monospace',
  'Fira Code, monospace',
  'Cascadia Code, monospace',
]

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

  useEffect(() => setFontDraft(fontFamily), [fontFamily])

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', closeOnEscape)
    return () => window.removeEventListener('keydown', closeOnEscape)
  }, [onClose])

  const commitFontFamily = () => setFontFamily(fontDraft)

  return (
    <div className="settings-backdrop" onMouseDown={onClose}>
      <section
        className="settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <aside className="settings-sidebar">
          <div className="settings-brand">Settings</div>
          <button className="settings-nav-item is-active">◐ <span>Appearance</span></button>
        </aside>

        <div className="settings-content">
          <header className="settings-header">
            <div>
              <h1>Appearance</h1>
              <p>Terminal typography and color themes.</p>
            </div>
            <button className="settings-close" onClick={onClose} aria-label="Close settings">×</button>
          </header>

          <section className="settings-section">
            <h2>Color mode</h2>
            <div className="mode-selector" aria-label="Color mode">
              {(['auto', 'light', 'dark'] as AppearanceMode[]).map((value) => (
                <button
                  key={value}
                  className={mode === value ? 'is-selected' : ''}
                  onClick={() => setMode(value)}
                >
                  {value[0].toUpperCase() + value.slice(1)}
                </button>
              ))}
            </div>
            {mode === 'auto' && (
              <p className="settings-hint">Following system: currently {activeAppearance}.</p>
            )}
          </section>

          <section className="settings-section">
            <h2>Text</h2>
            <div className="setting-row">
              <div>
                <label htmlFor="terminal-font">Font family</label>
                <p>Enter the exact name of any font installed on this device.</p>
              </div>
              <div className="font-field">
                <input
                  id="terminal-font"
                  list="terminal-font-suggestions"
                  value={fontDraft}
                  onChange={(event) => setFontDraft(event.target.value)}
                  onBlur={commitFontFamily}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') event.currentTarget.blur()
                  }}
                  spellCheck={false}
                />
                <datalist id="terminal-font-suggestions">
                  {FONT_SUGGESTIONS.map((font) => <option value={font} key={font} />)}
                </datalist>
              </div>
            </div>
            <div className="setting-row">
              <div>
                <label>Font size</label>
                <p>Applied immediately to every terminal session.</p>
              </div>
              <div className="number-stepper">
                <button onClick={() => setFontSize(fontSize - 1)} aria-label="Decrease font size">−</button>
                <output>{fontSize}</output>
                <button onClick={() => setFontSize(fontSize + 1)} aria-label="Increase font size">+</button>
              </div>
            </div>
            <div className="font-note">
              Oh My Posh icons require a Nerd Font configured here, for example
              <code>MesloLGM Nerd Font Mono, monospace</code>. Kumokara does not force it for other users.
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
      </section>
    </div>
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
