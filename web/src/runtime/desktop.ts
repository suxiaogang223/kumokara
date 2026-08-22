export interface DesktopConfig {
  localServerUrl: string
  localServerToken: string
  appVersion: string
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown
  }
}

export function isDesktopRuntime() {
  return window.__TAURI_INTERNALS__ !== undefined
}

export async function readDesktopConfig(): Promise<DesktopConfig | null> {
  if (!isDesktopRuntime()) return null
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<DesktopConfig>('desktop_config')
}
