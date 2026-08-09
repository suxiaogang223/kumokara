import type { ITheme } from '@xterm/xterm'

export type Appearance = 'light' | 'dark'
export type AppearanceMode = 'auto' | Appearance

export interface UiPalette {
  canvas: string
  surface: string
  surfaceRaised: string
  surfaceActive: string
  border: string
  borderStrong: string
  text: string
  textMuted: string
  accent: string
  success: string
  danger: string
}

export interface AppTheme {
  id: string
  name: string
  appearance: Appearance
  ui: UiPalette
  terminal: ITheme
}

type AnsiPalette = Pick<ITheme,
  'black' | 'red' | 'green' | 'yellow' | 'blue' | 'magenta' | 'cyan' | 'white' |
  'brightBlack' | 'brightRed' | 'brightGreen' | 'brightYellow' | 'brightBlue' |
  'brightMagenta' | 'brightCyan' | 'brightWhite'
>

function defineTheme(
  id: string,
  name: string,
  appearance: Appearance,
  ui: UiPalette,
  ansi: AnsiPalette,
): AppTheme {
  return {
    id,
    name,
    appearance,
    ui,
    terminal: {
      background: ui.canvas,
      foreground: ui.text,
      cursor: ui.accent,
      cursorAccent: ui.canvas,
      selectionBackground: `${ui.accent}40`,
      ...ansi,
    },
  }
}

const lightUi = (overrides: Partial<UiPalette>): UiPalette => ({
  canvas: '#fafafa', surface: '#ffffff', surfaceRaised: '#f3f4f6',
  surfaceActive: '#e8effa', border: '#e1e4e8', borderStrong: '#b8c5d8',
  text: '#30343b', textMuted: '#737983', accent: '#3578d4',
  success: '#3a8b55', danger: '#c84b55', ...overrides,
})

const darkUi = (overrides: Partial<UiPalette>): UiPalette => ({
  canvas: '#161320', surface: '#1e1c2e', surfaceRaised: '#26233a',
  surfaceActive: '#2a273f', border: '#2a2740', borderStrong: '#3a3650',
  text: '#e0def4', textMuted: '#817c9c', accent: '#c4a7e7',
  success: '#9ccfd8', danger: '#eb6f92', ...overrides,
})

export const THEMES: readonly AppTheme[] = [
  defineTheme('one-light', 'One Light', 'light', lightUi({}), {
    black: '#000000', red: '#e45649', green: '#50a14f', yellow: '#c18401',
    blue: '#4078f2', magenta: '#a626a4', cyan: '#0184bc', white: '#a0a1a7',
    brightBlack: '#696c77', brightRed: '#e45649', brightGreen: '#50a14f',
    brightYellow: '#c18401', brightBlue: '#4078f2', brightMagenta: '#a626a4',
    brightCyan: '#0184bc', brightWhite: '#ffffff',
  }),
  defineTheme('ayu-light', 'Ayu Light', 'light', lightUi({
    canvas: '#fafafa', surfaceRaised: '#f1f2f3', surfaceActive: '#edf5fb',
    text: '#5c6166', textMuted: '#8a9199', accent: '#399ee6', success: '#86b300',
  }), {
    black: '#000000', red: '#f07171', green: '#86b300', yellow: '#f2ae49',
    blue: '#399ee6', magenta: '#a37acc', cyan: '#4cbf99', white: '#e6e1cf',
    brightBlack: '#8a9199', brightRed: '#f07171', brightGreen: '#86b300',
    brightYellow: '#f2ae49', brightBlue: '#399ee6', brightMagenta: '#a37acc',
    brightCyan: '#4cbf99', brightWhite: '#ffffff',
  }),
  defineTheme('solarized-light', 'Solarized Light', 'light', lightUi({
    canvas: '#fdf6e3', surface: '#fffaf0', surfaceRaised: '#eee8d5',
    surfaceActive: '#e4e8d7', border: '#ded7c2', borderStrong: '#b9b49f',
    text: '#586e75', textMuted: '#839496', accent: '#268bd2', success: '#859900',
  }), {
    black: '#073642', red: '#dc322f', green: '#859900', yellow: '#b58900',
    blue: '#268bd2', magenta: '#d33682', cyan: '#2aa198', white: '#eee8d5',
    brightBlack: '#002b36', brightRed: '#cb4b16', brightGreen: '#586e75',
    brightYellow: '#657b83', brightBlue: '#839496', brightMagenta: '#6c71c4',
    brightCyan: '#93a1a1', brightWhite: '#fdf6e3',
  }),
  defineTheme('paper', 'Paper', 'light', lightUi({
    canvas: '#f7f7f2', surface: '#ffffff', surfaceRaised: '#eeeeea',
    surfaceActive: '#e7eee8', text: '#444744', textMuted: '#7c827d',
    accent: '#557b61', success: '#4f875e',
  }), {
    black: '#252525', red: '#af4448', green: '#4f875e', yellow: '#a2763c',
    blue: '#527a9b', magenta: '#8b5f8f', cyan: '#4b8585', white: '#d9d9d4',
    brightBlack: '#777b78', brightRed: '#c45b60', brightGreen: '#67a276',
    brightYellow: '#b88d4d', brightBlue: '#6891b1', brightMagenta: '#a477a8',
    brightCyan: '#62a0a0', brightWhite: '#ffffff',
  }),
  defineTheme('kumokara-dark', 'Kumokara', 'dark', darkUi({}), {
    black: '#1e1c2e', red: '#eb6f92', green: '#9ccfd8', yellow: '#f6c177',
    blue: '#9ccfd8', magenta: '#c4a7e7', cyan: '#9ccfd8', white: '#e0def4',
    brightBlack: '#403d52', brightRed: '#eb6f92', brightGreen: '#9ccfd8',
    brightYellow: '#f6c177', brightBlue: '#c4a7e7', brightMagenta: '#f6c177',
    brightCyan: '#9ccfd8', brightWhite: '#ffffff',
  }),
  defineTheme('one-dark', 'One Dark', 'dark', darkUi({
    canvas: '#282c34', surface: '#21252b', surfaceRaised: '#2c313a',
    surfaceActive: '#333a46', border: '#3e4451', borderStrong: '#4b5263',
    text: '#abb2bf', textMuted: '#7f848e', accent: '#61afef', success: '#98c379',
  }), {
    black: '#282c34', red: '#e06c75', green: '#98c379', yellow: '#e5c07b',
    blue: '#61afef', magenta: '#c678dd', cyan: '#56b6c2', white: '#abb2bf',
    brightBlack: '#5c6370', brightRed: '#e06c75', brightGreen: '#98c379',
    brightYellow: '#e5c07b', brightBlue: '#61afef', brightMagenta: '#c678dd',
    brightCyan: '#56b6c2', brightWhite: '#ffffff',
  }),
  defineTheme('dracula', 'Dracula', 'dark', darkUi({
    canvas: '#282a36', surface: '#21222c', surfaceRaised: '#343746',
    surfaceActive: '#3d4052', border: '#44475a', borderStrong: '#6272a4',
    text: '#f8f8f2', textMuted: '#9a9baa', accent: '#bd93f9', success: '#50fa7b',
  }), {
    black: '#21222c', red: '#ff5555', green: '#50fa7b', yellow: '#f1fa8c',
    blue: '#bd93f9', magenta: '#ff79c6', cyan: '#8be9fd', white: '#f8f8f2',
    brightBlack: '#6272a4', brightRed: '#ff6e6e', brightGreen: '#69ff94',
    brightYellow: '#ffffa5', brightBlue: '#d6acff', brightMagenta: '#ff92df',
    brightCyan: '#a4ffff', brightWhite: '#ffffff',
  }),
  defineTheme('nord', 'Nord', 'dark', darkUi({
    canvas: '#2e3440', surface: '#272c36', surfaceRaised: '#3b4252',
    surfaceActive: '#434c5e', border: '#4c566a', borderStrong: '#5e6b83',
    text: '#d8dee9', textMuted: '#8f9bae', accent: '#88c0d0', success: '#a3be8c',
  }), {
    black: '#3b4252', red: '#bf616a', green: '#a3be8c', yellow: '#ebcb8b',
    blue: '#81a1c1', magenta: '#b48ead', cyan: '#88c0d0', white: '#e5e9f0',
    brightBlack: '#4c566a', brightRed: '#bf616a', brightGreen: '#a3be8c',
    brightYellow: '#ebcb8b', brightBlue: '#81a1c1', brightMagenta: '#b48ead',
    brightCyan: '#8fbcbb', brightWhite: '#eceff4',
  }),
  defineTheme('gruvbox-dark', 'Gruvbox Dark', 'dark', darkUi({
    canvas: '#282828', surface: '#1d2021', surfaceRaised: '#32302f',
    surfaceActive: '#3c3836', border: '#504945', borderStrong: '#665c54',
    text: '#ebdbb2', textMuted: '#a89984', accent: '#d79921', success: '#b8bb26',
  }), {
    black: '#282828', red: '#cc241d', green: '#98971a', yellow: '#d79921',
    blue: '#458588', magenta: '#b16286', cyan: '#689d6a', white: '#a89984',
    brightBlack: '#928374', brightRed: '#fb4934', brightGreen: '#b8bb26',
    brightYellow: '#fabd2f', brightBlue: '#83a598', brightMagenta: '#d3869b',
    brightCyan: '#8ec07c', brightWhite: '#ebdbb2',
  }),
  defineTheme('solarized-dark', 'Solarized Dark', 'dark', darkUi({
    canvas: '#002b36', surface: '#073642', surfaceRaised: '#0b3f4a',
    surfaceActive: '#114b56', border: '#235762', borderStrong: '#3b6972',
    text: '#839496', textMuted: '#657b83', accent: '#268bd2', success: '#859900',
  }), {
    black: '#073642', red: '#dc322f', green: '#859900', yellow: '#b58900',
    blue: '#268bd2', magenta: '#d33682', cyan: '#2aa198', white: '#eee8d5',
    brightBlack: '#002b36', brightRed: '#cb4b16', brightGreen: '#586e75',
    brightYellow: '#657b83', brightBlue: '#839496', brightMagenta: '#6c71c4',
    brightCyan: '#93a1a1', brightWhite: '#fdf6e3',
  }),
  defineTheme('tokyo-night', 'Tokyo Night', 'dark', darkUi({
    canvas: '#1a1b26', surface: '#16161e', surfaceRaised: '#24283b',
    surfaceActive: '#2f354f', border: '#343b58', borderStrong: '#465075',
    text: '#c0caf5', textMuted: '#7982a9', accent: '#7aa2f7', success: '#9ece6a',
  }), {
    black: '#15161e', red: '#f7768e', green: '#9ece6a', yellow: '#e0af68',
    blue: '#7aa2f7', magenta: '#bb9af7', cyan: '#7dcfff', white: '#a9b1d6',
    brightBlack: '#414868', brightRed: '#f7768e', brightGreen: '#9ece6a',
    brightYellow: '#e0af68', brightBlue: '#7aa2f7', brightMagenta: '#bb9af7',
    brightCyan: '#7dcfff', brightWhite: '#c0caf5',
  }),
  defineTheme('rose-pine', 'Rosé Pine', 'dark', darkUi({
    canvas: '#191724', surface: '#1f1d2e', surfaceRaised: '#26233a',
    surfaceActive: '#2a273f', border: '#403d52', borderStrong: '#56526e',
    text: '#e0def4', textMuted: '#908caa', accent: '#c4a7e7', success: '#9ccfd8',
  }), {
    black: '#26233a', red: '#eb6f92', green: '#9ccfd8', yellow: '#f6c177',
    blue: '#31748f', magenta: '#c4a7e7', cyan: '#ebbcba', white: '#e0def4',
    brightBlack: '#6e6a86', brightRed: '#eb6f92', brightGreen: '#9ccfd8',
    brightYellow: '#f6c177', brightBlue: '#31748f', brightMagenta: '#c4a7e7',
    brightCyan: '#ebbcba', brightWhite: '#ffffff',
  }),
  defineTheme('catppuccin-mocha', 'Catppuccin Mocha', 'dark', darkUi({
    canvas: '#1e1e2e', surface: '#181825', surfaceRaised: '#313244',
    surfaceActive: '#36384d', border: '#45475a', borderStrong: '#585b70',
    text: '#cdd6f4', textMuted: '#9399b2', accent: '#89b4fa', success: '#a6e3a1',
  }), {
    black: '#45475a', red: '#f38ba8', green: '#a6e3a1', yellow: '#f9e2af',
    blue: '#89b4fa', magenta: '#cba6f7', cyan: '#94e2d5', white: '#bac2de',
    brightBlack: '#585b70', brightRed: '#f38ba8', brightGreen: '#a6e3a1',
    brightYellow: '#f9e2af', brightBlue: '#89b4fa', brightMagenta: '#cba6f7',
    brightCyan: '#94e2d5', brightWhite: '#a6adc8',
  }),
]

export const LIGHT_THEMES = THEMES.filter((theme) => theme.appearance === 'light')
export const DARK_THEMES = THEMES.filter((theme) => theme.appearance === 'dark')

export function findTheme(id: string, appearance: Appearance): AppTheme {
  return THEMES.find((theme) => theme.id === id && theme.appearance === appearance)
    ?? (appearance === 'dark' ? DARK_THEMES[0] : LIGHT_THEMES[0])
}

export function applyTheme(theme: AppTheme) {
  const root = document.documentElement
  const variables: Record<string, string> = {
    '--canvas': theme.ui.canvas,
    '--surface': theme.ui.surface,
    '--surface-raised': theme.ui.surfaceRaised,
    '--surface-active': theme.ui.surfaceActive,
    '--border': theme.ui.border,
    '--border-strong': theme.ui.borderStrong,
    '--text': theme.ui.text,
    '--text-muted': theme.ui.textMuted,
    '--accent': theme.ui.accent,
    '--accent-contrast': theme.appearance === 'dark' ? theme.ui.canvas : '#ffffff',
    '--success': theme.ui.success,
    '--danger': theme.ui.danger,
  }
  Object.entries(variables).forEach(([name, value]) => root.style.setProperty(name, value))
  root.style.colorScheme = theme.appearance
  root.dataset.appearance = theme.appearance
}
