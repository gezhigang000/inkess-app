import { useState } from 'react'
import { useI18n } from '../lib/i18n'

interface GitCommitFormProps {
  onCommit: (message: string) => void
  onPush: () => void
  onPull: () => void
  hasStagedFiles: boolean
  loading: boolean
}

export function GitCommitForm({ onCommit, onPush, onPull, hasStagedFiles, loading }: GitCommitFormProps) {
  const { t } = useI18n()
  const [message, setMessage] = useState('')

  const handleCommit = () => {
    const msg = message.trim()
    if (!msg) return
    onCommit(msg)
    setMessage('')
  }

  return (
    <div className="git-commit-form">
      <input
        className="git-commit-input"
        placeholder={t('git.commitPlaceholder')}
        value={message}
        onChange={e => setMessage(e.target.value)}
        onKeyDown={e => { if (e.key === 'Enter') handleCommit() }}
        disabled={loading}
      />
      <div className="flex gap-2 mt-2.5">
        <button
          className="git-btn git-btn-primary flex-1"
          onClick={handleCommit}
          disabled={!hasStagedFiles || !message.trim() || loading}
        >
          Commit
        </button>
        <button className="git-btn" onClick={onPush} disabled={loading}>Push</button>
        <button className="git-btn" onClick={onPull} disabled={loading}>Pull</button>
      </div>
    </div>
  )
}
