export type ThemeId = 'github' | 'minimal' | 'dark'

export interface ThemeConfig {
  id: ThemeId
  name: string
  isDark: boolean
}

export const themes: ThemeConfig[] = [
  { id: 'github', name: 'GitHub', isDark: false },
  { id: 'minimal', name: 'Minimal', isDark: false },
  { id: 'dark', name: 'Dark', isDark: true },
]
