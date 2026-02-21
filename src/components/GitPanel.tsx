import { useState, useEffect, useCallback, useRef } from 'react'
import { gitStatus, gitInit, gitStage, gitUnstage, gitCommit, gitPush, gitPull, gitRemoteAdd, gitRemoteList, setupSshKey, type GitFileStatus, type GitRemoteInfo } from '../lib/tauri'
import { useI18n } from '../lib/i18n'
import { UpgradePrompt } from './UpgradePrompt'
import { GitStatusList } from './GitStatusList'
import { GitCommitForm } from './GitCommitForm'
import { GitRemoteSetup } from './GitRemoteSetup'

interface GitPanelProps {
  cwd: string
  visible: boolean
  onToast: (msg: string) => void
  onClose: () => void
  isPro?: boolean
  onOpenLicense?: () => void
}

export function GitPanel({ cwd, visible, onToast, onClose, isPro = true, onOpenLicense }: GitPanelProps) {
  const { t } = useI18n()
  const [isRepo, setIsRepo] = useState(false)
  const [branch, setBranch] = useState('')
  const [files, setFiles] = useState<GitFileStatus[]>([])
  const [remotes, setRemotes] = useState<GitRemoteInfo[]>([])
  const [loading, setLoading] = useState(false)
  const [showRemoteSetup, setShowRemoteSetup] = useState(false)
  const [sshKey, setSshKey] = useState<string | undefined>()
  const [selectedRemote, setSelectedRemote] = useState('')
  const panelRef = useRef<HTMLDivElement>(null)

  const refresh = useCallback(async () => {
    if (!cwd) return
    try {
      const status = await gitStatus(cwd)
      setIsRepo(status.is_repo)
      setBranch(status.branch)
      setFiles(status.files)
      if (status.is_repo) {
        const r = await gitRemoteList(cwd).catch(() => [])
        setRemotes(r)
        if (r.length > 0) {
          setSelectedRemote(prev => prev && r.some(rm => rm.name === prev) ? prev : r[0].name)
        }
      }
    } catch { /* silent */ }
  }, [cwd])

  useEffect(() => {
    if (visible && cwd) refresh()
  }, [visible, cwd, refresh])

  // Close on click outside
  useEffect(() => {
    if (!visible) return
    const handler = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) onClose()
    }
    const keyHandler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('mousedown', handler)
    document.addEventListener('keydown', keyHandler)
    return () => {
      document.removeEventListener('mousedown', handler)
      document.removeEventListener('keydown', keyHandler)
    }
  }, [visible, onClose])

  const handleInit = async () => {
    setLoading(true)
    try {
      await gitInit(cwd)
      onToast(t('git.repoInited'))
      await refresh()
      setShowRemoteSetup(true)
    } catch (e) { onToast(typeof e === 'string' ? e : t('git.initFailed')) }
    finally { setLoading(false) }
  }

  const handleToggleStage = async (path: string, isStaged: boolean) => {
    try {
      if (isStaged) await gitUnstage(cwd, [path])
      else await gitStage(cwd, [path])
      await refresh()
    } catch (e) { onToast(typeof e === 'string' ? e : t('git.opFailed')) }
  }

  const handleCommit = async (message: string) => {
    setLoading(true)
    try {
      await gitCommit(cwd, message)
      onToast(t('git.committed'))
      await refresh()
    } catch (e) { onToast(typeof e === 'string' ? e : t('git.commitFailed')) }
    finally { setLoading(false) }
  }

  const handlePush = async () => {
    setLoading(true)
    try {
      await gitPush(cwd, selectedRemote)
      onToast(t('git.pushed'))
    } catch (e) { onToast(typeof e === 'string' ? e : t('git.pushFailed')) }
    finally { setLoading(false) }
  }

  const handlePull = async () => {
    setLoading(true)
    try {
      await gitPull(cwd, selectedRemote)
      onToast(t('git.pulled'))
      await refresh()
    } catch (e) { onToast(typeof e === 'string' ? e : t('git.pullFailed')) }
    finally { setLoading(false) }
  }

  const handleAddRemote = async (name: string, url: string) => {
    try {
      await gitRemoteAdd(cwd, name, url)
      onToast(t('git.remoteAdded'))
      setShowRemoteSetup(false)
      await refresh()
    } catch (e) { onToast(typeof e === 'string' ? e : t('git.addFailed')) }
  }

  const handleSetupSsh = async (email: string) => {
    try {
      const key = await setupSshKey(email)
      setSshKey(key)
      onToast(t('git.sshGenerated'))
    } catch (e) { onToast(typeof e === 'string' ? e : t('git.genFailed')) }
  }

  if (!visible) return null
  const hasStagedFiles = files.some(f => f.staged)

  return (
    <div className="git-panel-backdrop">
      <div ref={panelRef} className="git-panel-popup">
        <div className="flex items-center justify-between px-4 py-3" style={{ borderBottom: '1px solid var(--border-s)' }}>
          <div className="flex items-center gap-2">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-4 h-4" style={{ color: 'var(--text-2)' }}>
              <circle cx="12" cy="12" r="3" /><line x1="12" y1="3" x2="12" y2="9" /><line x1="12" y1="15" x2="12" y2="21" />
            </svg>
            <span className="text-[13px] font-semibold">Git</span>
            {isRepo && branch && (
              <span className="text-[11px] px-1.5 py-0.5 rounded" style={{ background: 'var(--accent-subtle)', color: 'var(--color-accent)' }}>{branch}</span>
            )}
          </div>
          <button
            className="sidebar-action-btn"
            onClick={onClose}
            aria-label={t('ai.close')}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-4 h-4"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
          </button>
        </div>
        <div className="px-4 py-1.5 text-[11px] truncate" style={{ color: 'var(--text-3)', borderBottom: '1px solid var(--border-s)' }} title={cwd}>{cwd}</div>
        <div className="git-panel-body">
          {!isPro ? (
            <UpgradePrompt feature="git" onOpenLicense={() => onOpenLicense?.()} />
          ) : !isRepo ? (
            <div className="px-4 py-6 text-center">
              <p className="text-[12.5px] mb-3" style={{ color: 'var(--text-3)' }}>{t('git.notInit')}</p>
              <button className="git-btn git-btn-primary" onClick={handleInit} disabled={loading}>{t('git.initRepo')}</button>
            </div>
          ) : (
            <>
              {remotes.length === 0 && (
                <button className="text-[11px] px-4 py-1.5 cursor-pointer bg-transparent border-none" style={{ color: 'var(--color-accent)' }} onClick={() => setShowRemoteSetup(true)}>{t('git.configRemote')}</button>
              )}
              {remotes.length > 1 && (
                <div className="px-4 py-1.5 flex items-center gap-2" style={{ borderBottom: '1px solid var(--border-s)' }}>
                  <span className="text-[11px]" style={{ color: 'var(--text-3)' }}>{t('git.selectRemote')}</span>
                  <select
                    className="new-file-input"
                    style={{ fontSize: 11, padding: '2px 6px', flex: 1 }}
                    value={selectedRemote}
                    onChange={e => setSelectedRemote(e.target.value)}
                  >
                    {remotes.map(r => (
                      <option key={r.name} value={r.name}>{r.name} ({r.url})</option>
                    ))}
                  </select>
                </div>
              )}
              <GitStatusList files={files} onToggleStage={handleToggleStage} />
              <GitCommitForm onCommit={handleCommit} onPush={handlePush} onPull={handlePull} hasStagedFiles={hasStagedFiles} loading={loading} />
            </>
          )}
        </div>
        {showRemoteSetup && (
          <GitRemoteSetup onAdd={handleAddRemote} onSetupSsh={handleSetupSsh} onCancel={() => setShowRemoteSetup(false)} sshPublicKey={sshKey} />
        )}
      </div>
    </div>
  )
}
