import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
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

type SettingsSection = 'appearance' | 'terminal'
type SettingsIcon = 'appearance' | 'close' | 'dark' | 'light' | 'system' | 'terminal'

function Icon({ name, size = 18 }: { name: SettingsIcon; size?: number }) {
  const paths: Record<SettingsIcon, ReactNode> = {
    appearance: <><path d="M12 2.8v2M12 19.2v2M2.8 12h2M19.2 12h2M5.5 5.5l1.4 1.4M17.1 17.1l1.4 1.4M18.5 5.5l-1.4 1.4M6.9 17.1l-1.4 1.4" /><circle cx="12" cy="12" r="4" /></>,
    close: <><path d="m7 7 10 10M17 7 7 17" /></>,
    dark: <path d="M19.5 14.6A8 8 0 0 1 9.4 4.5 8.1 8.1 0 1 0 19.5 14.6Z" />,
    light: <><circle cx="12" cy="12" r="3.5" /><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.3 5.3l1.4 1.4M17.3 17.3l1.4 1.4M18.7 5.3l-1.4 1.4M6.7 17.3l-1.4 1.4" /></>,
    system: <><rect x="4" y="4.5" width="16" height="11.5" rx="2" /><path d="M9 20h6M12 16v4" /></>,
    terminal: <><rect x="3.5" y="4.5" width="17" height="15" rx="2" /><path d="m7 9 3 3-3 3M12.5 15h4" /></>,
  }
  return (
    <svg aria-hidden="true" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
      {paths[name]}
    </svg>
  )
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
  const [activeSection, setActiveSection] = useState<SettingsSection>('appearance')
  const [fontDraft, setFontDraft] = useState(fontFamily)
  const dialogRef = useRef<HTMLDialogElement>(null)

  useEffect(() => setFontDraft(fontFamily), [fontFamily])

  useEffect(() => {
    const dialog = dialogRef.current
    if (dialog && !dialog.open) {
      dialog.showModal()
      dialog.focus()
    }
  }, [])

  const closeDialog = () => dialogRef.current?.close()
  const commitFontFamily = () => setFontFamily(fontDraft)

  return (
    <dialog
      ref={dialogRef}
      className="settings-dialog"
      tabIndex={-1}
      aria-labelledby="settings-title"
      onClose={onClose}
      onClick={(event) => {
        if (event.target === event.currentTarget) closeDialog()
      }}
    >
      <nav className="settings-nav" aria-label="Settings sections">
        <h1 id="settings-title">Settings</h1>
        <div className="settings-nav-list">
          <button
            className={activeSection === 'appearance' ? 'is-active' : ''}
            type="button"
            onClick={() => setActiveSection('appearance')}
            aria-current={activeSection === 'appearance' ? 'page' : undefined}
          >
            <Icon name="appearance" />
            <span>Appearance</span>
          </button>
          <button
            className={activeSection === 'terminal' ? 'is-active' : ''}
            type="button"
            onClick={() => setActiveSection('terminal')}
            aria-current={activeSection === 'terminal' ? 'page' : undefined}
          >
            <Icon name="terminal" />
            <span>Terminal</span>
          </button>
        </div>
      </nav>

      <div className="settings-main">
        <header className="settings-topbar">
          <button className="settings-close" type="button" onClick={closeDialog} aria-label="Close settings">
            <Icon name="close" size={17} />
          </button>
        </header>

        <div className="settings-scroll">
          {activeSection === 'appearance' ? (
            <>
              <header className="settings-page-header">
                <h2>Appearance</h2>
                <p>Choose how Kumokara and every terminal session look.</p>
              </header>

              <section className="settings-section">
                <h3>Color mode</h3>
                <div className="appearance-mode-grid">
                  <ModeButton mode="light" selected={mode === 'light'} icon="light" label="Light" onSelect={setMode} />
                  <ModeButton mode="dark" selected={mode === 'dark'} icon="dark" label="Dark" onSelect={setMode} />
                  <ModeButton mode="auto" selected={mode === 'auto'} icon="system" label="System" onSelect={setMode} />
                </div>
                {mode === 'auto' && <p className="settings-hint">Following the system appearance, currently {activeAppearance}.</p>}
              </section>

              <ThemeSection title="Light theme" themes={LIGHT_THEMES} selectedId={lightThemeId} onSelect={(id) => setTheme('light', id)} />
              <ThemeSection title="Dark theme" themes={DARK_THEMES} selectedId={darkThemeId} onSelect={(id) => setTheme('dark', id)} />
            </>
          ) : (
            <>
              <header className="settings-page-header">
                <h2>Terminal</h2>
                <p>Configure typography for all current and future sessions.</p>
              </header>

              <section className="settings-section">
                <h3>Text</h3>
                <div className="settings-card">
                  <div className="setting-row">
                    <div>
                      <label htmlFor="terminal-font">Font family</label>
                      <p>Enter the exact name of a monospace font installed on this device.</p>
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
                      <button type="button" className="font-default-button" onClick={() => {
                        setFontDraft(DEFAULT_FONT_FAMILY)
                        setFontFamily(DEFAULT_FONT_FAMILY)
                      }}>Use system default</button>
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
                        if (Number.isFinite(event.currentTarget.valueAsNumber)) setFontSize(event.currentTarget.valueAsNumber)
                      }}
                    />
                  </div>
                </div>
              </section>
            </>
          )}

          <footer className="settings-footer">
            <button className="secondary-button" type="button" onClick={reset}>Restore defaults</button>
          </footer>
        </div>
      </div>
    </dialog>
  )
}

function ModeButton({
  mode,
  selected,
  icon,
  label,
  onSelect,
}: {
  mode: AppearanceMode
  selected: boolean
  icon: SettingsIcon
  label: string
  onSelect: (mode: AppearanceMode) => void
}) {
  return (
    <button className={selected ? 'is-selected' : ''} type="button" onClick={() => onSelect(mode)} aria-pressed={selected}>
      <Icon name={icon} size={21} />
      <span>{label}</span>
    </button>
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
      <h3>{title}</h3>
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
            <button className={`theme-option${selectedId === theme.id ? ' is-selected' : ''}`} key={theme.id} type="button" onClick={() => onSelect(theme.id)} aria-pressed={selectedId === theme.id}>
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
