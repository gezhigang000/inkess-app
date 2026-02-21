import { useState } from 'react'
import { useLicense } from '../lib/license'
import { useI18n } from '../lib/i18n'
import { invoke } from '@tauri-apps/api/core'

interface LicensePanelProps {
  visible: boolean
  onClose: () => void
  onToast: (msg: string) => void
}

const PURCHASE_URL = 'https://inkess-license.gezhigang.workers.dev/checkout'

const CHECK = '\u2713'
const CROSS = '\u2013'

export function LicensePanel({ visible, onClose, onToast }: LicensePanelProps) {
  const { t } = useI18n()
  const { isPro, licenseKey, activate, deactivate } = useLicense()
  const [key, setKey] = useState('')
  const [activating, setActivating] = useState(false)

  if (!visible) return null

  const handleActivate = async () => {
    if (!key.trim()) return
    setActivating(true)
    try {
      const ok = await activate(key.trim())
      if (ok) {
        onToast(t('license.activateSuccess'))
        setKey('')
        onClose()
      } else {
        onToast(t('license.activateFailed'))
      }
    } catch {
      onToast(t('license.activateFailed'))
    } finally {
      setActivating(false)
    }
  }

  const handleDeactivate = async () => {
    await deactivate()
    onToast(t('license.deactivated'))
  }

  // Feature comparison rows: [label_key, free_value, pro_value]
  const rows: [string, string, string][] = [
    ['license.compareMarkdown', CHECK, CHECK],
    ['license.compareThemes', CHECK, CHECK],
    ['license.compareEdit', CHECK, CHECK],
    ['license.compareSidebar', CHECK, CHECK],
    ['license.comparePreview', CHECK, CHECK],
    ['license.compareI18n', CHECK, CHECK],
    ['license.compareExport', CHECK, CHECK],
    ['license.compareWatermark', CROSS, CHECK],
    ['license.compareSnapshot', t('license.compareSnapshotFree'), t('license.compareSnapshotPro')],
    ['license.compareAI', CROSS, CHECK],
    ['license.compareTerminal', CROSS, CHECK],
    ['license.compareGit', CROSS, CHECK],
    ['license.compareFuture', CROSS, CHECK],
  ]

  const cellStyle = { padding: '5px 10px', fontSize: 12, borderBottom: '1px solid var(--border-s)' }
  const headerStyle = { ...cellStyle, fontWeight: 600 as const, fontSize: 11, textTransform: 'uppercase' as const, letterSpacing: '0.03em' }

  return (
    <div className="shortcuts-backdrop" onClick={onClose}>
      <div className="shortcuts-modal" onClick={e => e.stopPropagation()} style={{ minWidth: 480, maxWidth: 540 }}>
        <div className="flex items-center justify-between mb-1">
          <h3 style={{ margin: 0 }}>{t('license.title')}</h3>
          <button className="sidebar-action-btn" onClick={onClose} aria-label={t('ai.close')}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        {/* Status badge */}
        <div className="text-[12.5px] mb-4 px-3 py-2 rounded" style={{
          background: isPro ? 'rgba(34,197,94,0.1)' : 'var(--sidebar-bg)',
          color: isPro ? '#16a34a' : 'var(--text-2)',
        }}>
          {isPro ? `${t('license.pro')} — ${t('license.activated')}` : t('license.free')}
          {isPro && licenseKey && (
            <span className="ml-2 text-[11px]" style={{ opacity: 0.7 }}>
              {licenseKey.slice(0, 11)}...
            </span>
          )}
        </div>

        {/* Comparison table */}
        <div style={{ maxHeight: 320, overflowY: 'auto', marginBottom: 16, borderRadius: 8, border: '1px solid var(--border-s)' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ background: 'var(--sidebar-bg)' }}>
                <th style={{ ...headerStyle, textAlign: 'left', color: 'var(--text-3)' }}>{t('license.feature')}</th>
                <th style={{ ...headerStyle, textAlign: 'center', color: 'var(--text-3)', width: 72 }}>{t('license.free')}</th>
                <th style={{ ...headerStyle, textAlign: 'center', color: 'var(--color-accent)', width: 72 }}>
                  {t('license.pro')}
                </th>
              </tr>
            </thead>
            <tbody>
              {rows.map(([labelKey, free, pro]) => (
                <tr key={labelKey}>
                  <td style={{ ...cellStyle, color: 'var(--text-2)' }}>{t(labelKey)}</td>
                  <td style={{ ...cellStyle, textAlign: 'center', color: free === CHECK ? '#16a34a' : 'var(--text-3)' }}>{free}</td>
                  <td style={{ ...cellStyle, textAlign: 'center', color: '#16a34a' }}>{pro}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {isPro ? (
          <div className="flex justify-end">
            <button className="git-btn" style={{ fontSize: 12 }} onClick={handleDeactivate}>
              {t('license.deactivate')}
            </button>
          </div>
        ) : (
          <>
            <div className="text-[11px] font-semibold uppercase mb-3" style={{ color: 'var(--text-3)', letterSpacing: '0.05em' }}>
              {t('license.inputKey')}
            </div>
            <div className="flex gap-2 mb-4">
              <input
                className="new-file-input"
                style={{ flex: 1, margin: 0, fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
                placeholder={t('license.keyPlaceholder')}
                value={key}
                onChange={e => setKey(e.target.value.toUpperCase())}
                onKeyDown={e => e.key === 'Enter' && handleActivate()}
              />
              <button
                className="toolbar-btn toolbar-btn-accent"
                onClick={handleActivate}
                disabled={activating || !key.trim()}
              >
                {activating ? t('license.activating') : t('license.activate')}
              </button>
            </div>
            <div className="flex items-center justify-center gap-2">
              <button
                className="toolbar-btn toolbar-btn-accent"
                onClick={() => invoke('open_external_url', { url: PURCHASE_URL })}
              >
                {t('license.buyPro')} — $15 {t('license.oneTime')}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  )
}
