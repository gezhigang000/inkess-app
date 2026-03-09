import { useState, useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '../lib/i18n'

interface ShellConfirmPayload {
  command: string
}

export function ShellConfirm() {
  const { t } = useI18n()
  const [pending, setPending] = useState<ShellConfirmPayload | null>(null)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    const unlisten = listen<ShellConfirmPayload>('shell-confirm', (event) => {
      setPending(event.payload)
      // Auto-deny after 60s
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => {
        invoke('shell_confirm_response', { approved: false }).catch(() => {})
        setPending(null)
      }, 60000)
    })
    return () => {
      unlisten.then(fn => fn())
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [])

  const respond = (approved: boolean) => {
    if (timerRef.current) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
    invoke('shell_confirm_response', { approved }).catch(() => {})
    setPending(null)
  }

  if (!pending) return null

  return (
    <div className="shortcuts-backdrop" onClick={() => respond(false)}>
      <div
        className="shortcuts-modal"
        role="dialog"
        aria-modal="true"
        onClick={e => e.stopPropagation()}
        style={{ minWidth: 400, maxWidth: 520 }}
      >
        <div className="flex items-center justify-between mb-1">
          <h3 style={{ margin: 0 }}>{t('shellConfirm.title')}</h3>
          <button className="sidebar-action-btn" onClick={() => respond(false)} aria-label={t('ai.close')}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
        <p className="text-[13px] mb-2" style={{ color: 'var(--text-2)', lineHeight: '1.5' }}>
          {t('shellConfirm.message')}
        </p>
        <pre style={{
          background: 'var(--ink-900, #1a1a2e)',
          color: 'var(--ink-100, #e0e0e0)',
          padding: '10px 14px',
          borderRadius: 6,
          fontSize: 13,
          lineHeight: '1.5',
          overflowX: 'auto',
          marginBottom: 16,
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-all',
        }}>
          {pending.command}
        </pre>
        <div className="flex gap-2.5 justify-end">
          <button
            className="toolbar-btn"
            onClick={() => respond(false)}
            style={{ minWidth: 70 }}
          >
            {t('shellConfirm.deny')}
          </button>
          <button
            className="toolbar-btn"
            onClick={() => respond(true)}
            style={{
              minWidth: 70,
              background: 'var(--accent)',
              color: '#fff',
              borderColor: 'var(--accent)',
            }}
          >
            {t('shellConfirm.allow')}
          </button>
        </div>
      </div>
    </div>
  )
}
