import { useI18n } from '../lib/i18n'
import { type AiConfig, aiSaveConfig } from '../lib/tauri'
import { PRESETS } from './AIModelConfig'
import { SkillIndicator } from './SkillIndicator'
import { PROMPT_PRESETS, DEFAULT_BASE_PROMPT } from './AIChatPanel'
import type { MemoryEntry } from '../lib/tauri'

export interface AIChatHeaderProps {
  config: AiConfig | null
  showModelMenu: boolean
  setShowModelMenu: (v: boolean | ((prev: boolean) => boolean)) => void
  setConfig: (cfg: AiConfig) => void
  activeSkill: string
  memories: MemoryEntry[]
  messages: { role: string }[]
  streaming: boolean
  onCopyChat: () => void
  onClear: () => void
  onShowHistory: () => void
  onShowConfig: () => void
  onClose: () => void
}

export function AIChatHeader({
  config, showModelMenu, setShowModelMenu, setConfig,
  activeSkill, memories, messages,
  onCopyChat, onClear, onShowHistory, onShowConfig, onClose,
}: AIChatHeaderProps) {
  const { t, lang } = useI18n()

  return (
    <div className="ai-panel-header">
      <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--text)' }}>{t('ai.title')}</span>
      {config && (
        <div style={{ position: 'relative', marginLeft: 6 }}>
          <button
            className="git-btn"
            style={{ fontSize: 11, padding: '2px 8px', color: 'var(--text-2)' }}
            onClick={() => setShowModelMenu(v => !v)}
            title={t('ai.switchModel')}
          >
            {config.model} ▾
          </button>
          {showModelMenu && (
            <div
              style={{
                position: 'absolute', top: '100%', left: 0, marginTop: 4,
                background: 'var(--bg)', border: '1px solid var(--border-s)',
                borderRadius: 6, boxShadow: '0 4px 12px rgba(0,0,0,.12)',
                zIndex: 100, minWidth: 160, padding: '4px 0',
              }}
            >
              {PRESETS.filter(p => p.model).map(p => {
                const hasKey = !!(config.provider_keys?.[p.api_url] || (p.api_url === config.api_url && config.api_key))
                const isActive = config.model === p.model && config.api_url === p.api_url
                return (
                  <button
                    key={p.model}
                    className="ctx-menu-item"
                    disabled={!hasKey}
                    style={{
                      width: '100%', textAlign: 'left', fontSize: 12, padding: '6px 12px',
                      background: isActive ? 'var(--accent-subtle)' : 'transparent',
                      opacity: hasKey ? 1 : 0.4,
                      cursor: hasKey ? 'pointer' : 'not-allowed',
                    }}
                    title={hasKey ? '' : t('ai.keyNotConfigured')}
                    onClick={async () => {
                      if (!hasKey) return
                      const providerKey = config.provider_keys?.[p.api_url] || config.api_key
                      const updated = { ...config, model: p.model, api_url: p.api_url || config.api_url, api_key: providerKey }
                      setConfig(updated)
                      setShowModelMenu(false)
                      try { await aiSaveConfig(updated) } catch { /* silent */ }
                    }}
                  >
                    {p.label} <span style={{ color: 'var(--text-3)', marginLeft: 4 }}>{p.model}</span>
                    {!hasKey && <span style={{ fontSize: 10, color: 'var(--text-3)', marginLeft: 4 }}>🔒</span>}
                  </button>
                )
              })}
              {config.model && !PRESETS.some(p => p.model === config.model) && (
                <button
                  className="ctx-menu-item"
                  style={{ width: '100%', textAlign: 'left', fontSize: 12, padding: '6px 12px', background: 'var(--accent-subtle)' }}
                  onClick={() => setShowModelMenu(false)}
                >
                  {config.model}
                </button>
              )}
            </div>
          )}
        </div>
      )}
      {config && (() => {
        const bp = config.base_prompt || DEFAULT_BASE_PROMPT
        const preset = PROMPT_PRESETS.find(p => p.prompt === bp)
        const label = preset ? (lang === 'zh' ? preset.label_zh : preset.label) : t('aiConfig.custom')
        return (
          <span style={{ fontSize: 10, color: 'var(--accent)', background: 'var(--accent-subtle)', padding: '1px 6px', borderRadius: 4, marginLeft: 6 }}>
            {label}
          </span>
        )
      })()}
      <SkillIndicator skillName={activeSkill} />
      {memories.length > 0 && (
        <span style={{ fontSize: 10, color: 'var(--text-3)', marginLeft: 6 }} title={t('ai.memories', { n: memories.length })}>
          {t('ai.memories', { n: memories.length })}
        </span>
      )}
      <div style={{ flex: 1 }} />
      {messages.length > 0 && (
        <>
        <button className="sidebar-action-btn" onClick={onCopyChat} title={t('ai.copyChat')} aria-label={t('ai.copyChat')}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
          </svg>
        </button>
        <button className="sidebar-action-btn" onClick={onClear} title={t('ai.newChat')} aria-label={t('ai.newChat')}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
            <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /><line x1="12" y1="11" x2="12" y2="17" /><line x1="9" y1="14" x2="15" y2="14" />
          </svg>
        </button>
        </>
      )}
      <button className="sidebar-action-btn" onClick={onShowHistory} title={t('ai.chatHistory')} aria-label={t('ai.chatHistory')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
          <circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" />
        </svg>
      </button>
      <button className="sidebar-action-btn" onClick={onClear} title={t('ai.clearChat')} aria-label={t('ai.clearChat')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
          <polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" />
        </svg>
      </button>
      <button className="sidebar-action-btn" onClick={onShowConfig} title={t('ai.modelSettings')} aria-label={t('ai.modelSettings')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
          <circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
        </svg>
      </button>
      <button className="sidebar-action-btn" onClick={onClose} title={t('ai.close')} aria-label={t('ai.close')}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
          <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </div>
  )
}
