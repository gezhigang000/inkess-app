import { useI18n } from '../lib/i18n'

interface UpgradePromptProps {
  feature: 'ai' | 'terminal' | 'git'
  onOpenLicense: () => void
}

const FEATURE_KEYS: Record<string, string> = {
  ai: 'license.featureAI',
  terminal: 'license.featureTerminal',
  git: 'license.featureGit',
}

export function UpgradePrompt({ feature, onOpenLicense }: UpgradePromptProps) {
  const { t } = useI18n()

  return (
    <div className="flex flex-col items-center justify-center gap-4 p-8" style={{ color: 'var(--text-2)' }}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ width: 40, height: 40, opacity: 0.4 }}>
        <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
      </svg>
      <div className="text-[13px] text-center" style={{ maxWidth: 280, lineHeight: '1.6' }}>
        {t('license.upgradeHint')}
      </div>
      <div className="text-[12px] text-center" style={{ color: 'var(--text-3)', maxWidth: 300 }}>
        {t(FEATURE_KEYS[feature] || 'license.upgradeHint')}
      </div>
      <button
        className="toolbar-btn toolbar-btn-accent"
        style={{ marginTop: 4 }}
        onClick={onOpenLicense}
      >
        {t('license.upgrade')}
      </button>
    </div>
  )
}
