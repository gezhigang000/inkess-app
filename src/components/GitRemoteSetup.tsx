import { useState } from 'react'
import { useI18n } from '../lib/i18n'

interface GitRemoteSetupProps {
  onAdd: (name: string, url: string) => void
  onSetupSsh: (email: string) => void
  onCancel: () => void
  sshPublicKey?: string
}

export function GitRemoteSetup({ onAdd, onSetupSsh, onCancel, sshPublicKey }: GitRemoteSetupProps) {
  const { t } = useI18n()
  const [platform, setPlatform] = useState<'github' | 'gitee' | 'custom'>('github')
  const [url, setUrl] = useState('')
  const [email, setEmail] = useState('')
  const [showSsh, setShowSsh] = useState(false)

  const handleAdd = () => {
    const trimmed = url.trim()
    if (!trimmed) return
    onAdd('origin', trimmed)
  }

  return (
    <div className="shortcuts-backdrop" onClick={onCancel}>
      <div className="shortcuts-modal" onClick={e => e.stopPropagation()} style={{ minWidth: 380 }}>
        <div className="flex items-center justify-between mb-1">
          <h3 style={{ margin: 0 }}>{t('gitRemote.title')}</h3>
          <button className="sidebar-action-btn" onClick={onCancel} aria-label={t('gitRemote.close')}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        <div className="flex gap-2 mb-4">
          {(['github', 'gitee', 'custom'] as const).map(p => (
            <button
              key={p}
              className="new-file-tpl-btn"
              style={{
                background: platform === p ? 'var(--color-accent)' : 'var(--sidebar-bg)',
                color: platform === p ? '#fff' : 'var(--text-2)',
              }}
              onClick={() => setPlatform(p)}
            >
              {p === 'github' ? 'GitHub' : p === 'gitee' ? 'Gitee' : t('gitRemote.custom')}
            </button>
          ))}
        </div>
        <input
          className="new-file-input mb-4"
          placeholder={platform === 'github' ? 'https://github.com/user/repo.git' : platform === 'gitee' ? 'https://gitee.com/user/repo.git' : t('gitRemote.repoUrl')}
          value={url}
          onChange={e => setUrl(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter') handleAdd() }}
        />

        <div className="mb-4">
          <button
            className="text-[12px] cursor-pointer bg-transparent border-none"
            style={{ color: 'var(--color-accent)' }}
            onClick={() => setShowSsh(v => !v)}
          >
            {showSsh ? t('gitRemote.hideSsh') : t('gitRemote.showSsh')}
          </button>
          {showSsh && (
            <div className="mt-2">
              <input
                className="new-file-input mb-2"
                placeholder={t('gitRemote.emailPlaceholder')}
                value={email}
                onChange={e => setEmail(e.target.value)}
              />
              <button
                className="git-btn git-btn-primary w-full mb-2"
                onClick={() => { if (email.trim()) onSetupSsh(email.trim()) }}
                disabled={!email.trim()}
              >
                {t('gitRemote.generateSsh')}
              </button>
              {sshPublicKey && (
                <div className="p-2 rounded text-[11px] break-all" style={{ background: 'var(--sidebar-bg)', fontFamily: 'monospace', color: 'var(--text-2)' }}>
                  {sshPublicKey}
                  <button
                    className="git-btn mt-1 w-full"
                    onClick={() => navigator.clipboard.writeText(sshPublicKey)}
                  >
                    {t('gitRemote.copyPublicKey')}
                  </button>
                </div>
              )}
            </div>
          )}
        </div>

        <div className="flex justify-end gap-3">
          <button className="toolbar-btn" onClick={onCancel}>{t('gitRemote.cancel')}</button>
          <button className="toolbar-btn toolbar-btn-accent" onClick={handleAdd} disabled={!url.trim()}>{t('gitRemote.addRemote')}</button>
        </div>
      </div>
    </div>
  )
}
