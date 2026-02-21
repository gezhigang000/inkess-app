import { createContext, useContext, useState, useCallback, useEffect, type ReactNode } from 'react'
import { licenseLoad, licenseActivate, licenseDeactivate, type LicenseInfo } from './tauri'

interface LicenseContextValue {
  isPro: boolean
  licenseKey: string | null
  loading: boolean
  activate: (key: string) => Promise<boolean>
  deactivate: () => Promise<void>
}

const LicenseContext = createContext<LicenseContextValue>({
  isPro: false,
  licenseKey: null,
  loading: true,
  activate: async () => false,
  deactivate: async () => {},
})

export function LicenseProvider({ children }: { children: ReactNode }) {
  const [info, setInfo] = useState<LicenseInfo | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    licenseLoad()
      .then(result => setInfo(result))
      .catch(() => {})
      .finally(() => setLoading(false))
  }, [])

  const activate = useCallback(async (key: string): Promise<boolean> => {
    try {
      const result = await licenseActivate(key)
      setInfo(result)
      return true
    } catch {
      return false
    }
  }, [])

  const deactivate = useCallback(async () => {
    try {
      await licenseDeactivate()
      setInfo(null)
    } catch { /* silent */ }
  }, [])

  return (
    <LicenseContext.Provider value={{
      isPro: info !== null,
      licenseKey: info?.key ?? null,
      loading,
      activate,
      deactivate,
    }}>
      {children}
    </LicenseContext.Provider>
  )
}

export function useLicense() {
  return useContext(LicenseContext)
}
