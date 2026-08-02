import { useEffect, useState } from 'react'
import { useAppearanceStore } from '../store/appearanceStore'
import { applyTheme, findTheme, type Appearance } from '../theme'

export function useAppearance() {
  const mode = useAppearanceStore((state) => state.mode)
  const lightThemeId = useAppearanceStore((state) => state.lightThemeId)
  const darkThemeId = useAppearanceStore((state) => state.darkThemeId)
  const [systemAppearance, setSystemAppearance] = useState<Appearance>(() =>
    window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  )

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const update = () => setSystemAppearance(media.matches ? 'dark' : 'light')
    media.addEventListener('change', update)
    return () => media.removeEventListener('change', update)
  }, [])

  const appearance = mode === 'auto' ? systemAppearance : mode
  const theme = findTheme(appearance === 'light' ? lightThemeId : darkThemeId, appearance)

  useEffect(() => applyTheme(theme), [theme])

  return { appearance, theme }
}
