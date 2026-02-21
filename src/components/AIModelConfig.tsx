import { useState } from 'react'
import { type AiConfig, aiSaveConfig, aiTestConnection, aiTestSearch } from '../lib/tauri'
import { useI18n } from '../lib/i18n'
import { DEFAULT_BASE_PROMPT, PROMPT_PRESETS } from './AIChatPanel'

interface AIModelConfigProps {
  config: AiConfig | null
  onSave: (config: AiConfig) => void
  onClose: () => void
  onToast: (msg: string) => void
}

export const PRESETS: { label: string; api_url: string; model: string }[] = [
  { label: 'OpenAI', api_url: 'https://api.openai.com/v1', model: 'gpt-4o' },
  { label: 'DeepSeek', api_url: 'https://api.deepseek.com/v1', model: 'deepseek-chat' },
  { label: 'Qwen', api_url: 'https://dashscope.aliyuncs.com/compatible-mode/v1', model: 'qwen-plus' },
  { label: 'Kimi', api_url: 'https://api.moonshot.cn/v1', model: 'moonshot-v1-8k' },
  { label: 'GLM', api_url: 'https://open.bigmodel.cn/api/paas/v4', model: 'glm-4-flash' },
  { label: 'Doubao', api_url: 'https://ark.cn-beijing.volces.com/api/v3', model: 'doubao-1.5-pro-32k' },
  { label: 'Custom', api_url: '', model: '' },
]

function maskKey(key: string): string {
  if (!key) return ''
  if (key.length <= 8) return '••••••••'
  return key.slice(0, 4) + '••••' + key.slice(-4)
}

function KeyInput({ value, onChange, placeholder }: { value: string; onChange: (v: string) => void; placeholder: string }) {
  const [showKey, setShowKey] = useState(false)
  return (
    <div style={{ position: 'relative', marginTop: 4 }}>
      <input
        className="new-file-input"
        type={showKey ? 'text' : 'password'}
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={placeholder}
        style={{ paddingRight: 32 }}
      />
      {value && (
        <button
          type="button"
          onClick={() => setShowKey(v => !v)}
          style={{ position: 'absolute', right: 6, top: '50%', transform: 'translateY(-50%)', background: 'none', border: 'none', cursor: 'pointer', padding: 2, color: 'var(--text-3)', fontSize: 11 }}
          aria-label={showKey ? 'Hide' : 'Show'}
        >
          {showKey ? (
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94M9.9 4.24A9.12 9.12 0 0112 4c7 0 11 8 11 8a18.5 18.5 0 01-2.16 3.19m-6.72-1.07a3 3 0 11-4.24-4.24" /><line x1="1" y1="1" x2="23" y2="23" />
            </svg>
          ) : (
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" /><circle cx="12" cy="12" r="3" />
            </svg>
          )}
        </button>
      )}
    </div>
  )
}

export function AIModelConfig({ config, onSave, onClose, onToast }: AIModelConfigProps) {
  const { t, lang } = useI18n()
  const [apiUrl, setApiUrl] = useState(config?.api_url || PRESETS[0].api_url)
  const [apiKey, setApiKey] = useState(config?.api_key || '')
  const [model, setModel] = useState(config?.model || PRESETS[0].model)
  const [temperature, setTemperature] = useState(config?.temperature ?? 0.7)
  const [maxTokens, setMaxTokens] = useState(config?.max_tokens ?? 4096)
  const [systemPrompt, setSystemPrompt] = useState(config?.system_prompt || '')
  const [searchApiKey, setSearchApiKey] = useState(config?.search_api_key || '')
  const [basePrompt, setBasePrompt] = useState(config?.base_prompt || '')
  const [providerKeys, setProviderKeys] = useState<Record<string, string>>(config?.provider_keys || {})
  const [testing, setTesting] = useState(false)
  const [activeTab, setActiveTab] = useState<'model' | 'shared' | 'search'>('model')
  const [searchProvider, setSearchProvider] = useState(config?.search_provider || 'duckduckgo')

  const SEARCH_PROVIDERS = [
    { id: 'duckduckgo', label: 'DuckDuckGo', desc: lang === 'zh' ? '免费，无需 API Key' : 'Free, no API key needed', placeholder: '' },
    { id: 'tavily', label: 'Tavily', desc: lang === 'zh' ? 'AI 优化搜索，适合深度分析' : 'AI-optimized search for deep analysis', placeholder: 'tvly-...' },
    { id: 'brave', label: 'Brave Search', desc: lang === 'zh' ? '隐私优先，结果质量高' : 'Privacy-first, high quality results', placeholder: 'BSA...' },
    { id: 'serpapi', label: 'SerpAPI (Google)', desc: lang === 'zh' ? 'Google 搜索结果，覆盖最全' : 'Google results, most comprehensive', placeholder: '' },
  ]

  const handlePreset = (idx: number) => {
    const p = PRESETS[idx]
    // Save current key to provider_keys before switching
    if (apiUrl && apiKey) {
      setProviderKeys(prev => ({ ...prev, [apiUrl]: apiKey }))
    }
    setApiUrl(p.api_url)
    setModel(p.model)
    // Restore key for the new provider
    const savedKey = providerKeys[p.api_url] || (config?.provider_keys?.[p.api_url]) || ''
    setApiKey(savedKey)
  }

  const handleTestModel = async () => {
    if (!apiUrl || !apiKey) { onToast(t('aiConfig.fillRequired')); return }
    setTesting(true)
    try {
      const msg = await aiTestConnection({ api_url: apiUrl, api_key: apiKey, model, temperature, max_tokens: maxTokens, system_prompt: systemPrompt, base_prompt: basePrompt, search_api_key: searchApiKey, search_provider: searchProvider, provider_keys: providerKeys })
      onToast(msg)
    } catch (e) {
      onToast(typeof e === 'string' ? e : t('aiConfig.connFailed'))
    } finally { setTesting(false) }
  }

  const handleTestSearch = async () => {
    if (searchProvider !== 'duckduckgo' && !searchApiKey) { onToast(t('aiConfig.fillRequired')); return }
    setTesting(true)
    try {
      const msg = await aiTestSearch(searchProvider, searchProvider === 'duckduckgo' ? '' : searchApiKey)
      onToast(msg)
    } catch (e) {
      onToast(typeof e === 'string' ? e : t('aiConfig.connFailed'))
    } finally { setTesting(false) }
  }

  const handleSave = async () => {
    if (!apiUrl || !model) { onToast(t('aiConfig.fillAll')); return }
    // Merge current key into provider_keys (remove entry if key is empty)
    const keys = { ...providerKeys }
    if (apiKey) {
      keys[apiUrl] = apiKey
    } else {
      delete keys[apiUrl]
    }
    const cfg: AiConfig = { api_url: apiUrl, api_key: apiKey, model, temperature, max_tokens: maxTokens, system_prompt: systemPrompt, base_prompt: basePrompt, search_api_key: searchApiKey, search_provider: searchProvider, provider_keys: keys }
    try {
      await aiSaveConfig(cfg)
      onSave(cfg)
      onToast(t('aiConfig.saved'))
      onClose()
    } catch (e) {
      onToast(typeof e === 'string' ? e : t('aiConfig.saveFailed'))
    }
  }

  return (
    <div className="shortcuts-backdrop" onClick={onClose}>
      <div className="shortcuts-modal" onClick={e => e.stopPropagation()} style={{ minWidth: 460, maxWidth: 520 }}>
        <div className="flex items-center justify-between mb-1">
          <h3 style={{ margin: 0 }}>{t('aiConfig.title')}</h3>
          <button className="sidebar-action-btn" onClick={onClose} aria-label={t('ai.close')}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        <div style={{ display: 'flex', gap: 0, borderBottom: '1px solid var(--border)', marginBottom: 16 }}>
          <button
            style={{ flex: 1, padding: '8px 0', fontSize: 12, fontWeight: activeTab === 'model' ? 600 : 400, color: activeTab === 'model' ? 'var(--color-accent)' : 'var(--text-3)', background: 'none', border: 'none', borderBottom: activeTab === 'model' ? '2px solid var(--color-accent)' : '2px solid transparent', cursor: 'pointer' }}
            onClick={() => setActiveTab('model')}
          >
            {t('aiConfig.modelSection')}
          </button>
          <button
            style={{ flex: 1, padding: '8px 0', fontSize: 12, fontWeight: activeTab === 'shared' ? 600 : 400, color: activeTab === 'shared' ? 'var(--color-accent)' : 'var(--text-3)', background: 'none', border: 'none', borderBottom: activeTab === 'shared' ? '2px solid var(--color-accent)' : '2px solid transparent', cursor: 'pointer' }}
            onClick={() => setActiveTab('shared')}
          >
            {t('aiConfig.sharedSection')}
          </button>
          <button
            style={{ flex: 1, padding: '8px 0', fontSize: 12, fontWeight: activeTab === 'search' ? 600 : 400, color: activeTab === 'search' ? 'var(--color-accent)' : 'var(--text-3)', background: 'none', border: 'none', borderBottom: activeTab === 'search' ? '2px solid var(--color-accent)' : '2px solid transparent', cursor: 'pointer' }}
            onClick={() => setActiveTab('search')}
          >
            {t('aiConfig.searchSection')}
          </button>
        </div>

        {activeTab === 'model' && (
          <>
            <div style={{ display: 'flex', gap: 6, marginBottom: 16, flexWrap: 'wrap' }}>
              {PRESETS.map((p, i) => {
                const isCurrentProvider = p.api_url === apiUrl
                const hasKey = !!(providerKeys[p.api_url] || config?.provider_keys?.[p.api_url] || (isCurrentProvider && apiKey))
                return (
                  <button
                    key={p.label}
                    className="git-btn"
                    style={{
                      fontSize: 11, padding: '4px 10px',
                      background: isCurrentProvider ? 'var(--accent-subtle)' : undefined,
                      color: isCurrentProvider ? 'var(--color-accent)' : undefined,
                    }}
                    onClick={() => handlePreset(i)}
                  >
                    {p.label === 'Custom' ? t('aiConfig.custom') : p.label}
                    {hasKey && p.api_url && <span style={{ marginLeft: 3, fontSize: 9, color: 'var(--color-accent)' }}>●</span>}
                  </button>
                )
              })}
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              <label style={{ fontSize: 12, color: 'var(--text-2)' }}>
                {t('aiConfig.apiUrl')}
                <input
                  className="new-file-input"
                  value={apiUrl}
                  onChange={e => setApiUrl(e.target.value)}
                  placeholder="https://api.openai.com/v1"
                  style={{ marginTop: 4 }}
                />
              </label>
              <label style={{ fontSize: 12, color: 'var(--text-2)' }}>
                {t('aiConfig.apiKey')}
                <KeyInput value={apiKey} onChange={setApiKey} placeholder="sk-..." />
              </label>
              <label style={{ fontSize: 12, color: 'var(--text-2)' }}>
                {t('aiConfig.modelName')}
                <input
                  className="new-file-input"
                  value={model}
                  onChange={e => setModel(e.target.value)}
                  placeholder="gpt-4o"
                  style={{ marginTop: 4 }}
                />
              </label>
              <div style={{ display: 'flex', gap: 12 }}>
                <label style={{ fontSize: 12, color: 'var(--text-2)', flex: 1 }}>
                  Temperature
                  <input
                    className="new-file-input"
                    type="number"
                    step="0.1"
                    min="0"
                    max="2"
                    value={temperature}
                    onChange={e => setTemperature(parseFloat(e.target.value) || 0)}
                    style={{ marginTop: 4 }}
                  />
                </label>
                <label style={{ fontSize: 12, color: 'var(--text-2)', flex: 1 }}>
                  {t('aiConfig.maxTokens')}
                  <input
                    className="new-file-input"
                    type="number"
                    step="256"
                    min="256"
                    max="128000"
                    value={maxTokens}
                    onChange={e => setMaxTokens(parseInt(e.target.value) || 4096)}
                    style={{ marginTop: 4 }}
                  />
                </label>
              </div>
            </div>
          </>
        )}

        {activeTab === 'shared' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <label style={{ fontSize: 12, color: 'var(--text-2)' }}>
              {t('aiConfig.basePrompt')}
              <span style={{ fontSize: 11, color: 'var(--text-3)', marginLeft: 6 }}>{t('aiConfig.basePromptHint')}</span>
              <div style={{ display: 'flex', gap: 4, marginTop: 6, flexWrap: 'wrap' }}>
                {PROMPT_PRESETS.map(p => (
                  <button
                    key={p.id}
                    className="git-btn"
                    style={{
                      fontSize: 10, padding: '2px 8px',
                      background: (basePrompt || DEFAULT_BASE_PROMPT) === p.prompt ? 'var(--accent-subtle)' : undefined,
                      color: (basePrompt || DEFAULT_BASE_PROMPT) === p.prompt ? 'var(--color-accent)' : undefined,
                    }}
                    onClick={() => setBasePrompt(p.prompt === DEFAULT_BASE_PROMPT ? '' : p.prompt)}
                  >
                    {lang === 'zh' ? p.label_zh : p.label}
                  </button>
                ))}
              </div>
              <textarea
                className="new-file-input"
                value={basePrompt || DEFAULT_BASE_PROMPT}
                onChange={e => setBasePrompt(e.target.value === DEFAULT_BASE_PROMPT ? '' : e.target.value)}
                rows={8}
                style={{ marginTop: 6, resize: 'vertical', minHeight: 120, fontSize: 11, fontFamily: "'JetBrains Mono', monospace", lineHeight: '1.5' }}
              />
            </label>
            <label style={{ fontSize: 12, color: 'var(--text-2)' }}>
              {t('aiConfig.systemPrompt')}
              <span style={{ fontSize: 11, color: 'var(--text-3)', marginLeft: 6 }}>{t('aiConfig.systemPromptHint')}</span>
              <textarea
                className="new-file-input"
                value={systemPrompt}
                onChange={e => setSystemPrompt(e.target.value)}
                placeholder={t('aiConfig.systemPromptPlaceholder')}
                rows={3}
                style={{ marginTop: 4, resize: 'vertical', minHeight: 60 }}
              />
            </label>
          </div>
        )}

        {activeTab === 'search' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <div style={{ fontSize: 12, color: 'var(--text-2)', marginBottom: 4 }}>
              {t('aiConfig.searchProvider')}
            </div>
            {SEARCH_PROVIDERS.map(p => (
              <label
                key={p.id}
                style={{
                  display: 'flex', alignItems: 'flex-start', gap: 10, padding: '8px 10px',
                  borderRadius: 6, cursor: 'pointer',
                  border: searchProvider === p.id ? '1px solid var(--color-accent)' : '1px solid var(--border-s)',
                  background: searchProvider === p.id ? 'var(--accent-subtle)' : 'transparent',
                }}
                onClick={() => setSearchProvider(p.id)}
              >
                <input
                  type="radio" name="search-provider" checked={searchProvider === p.id}
                  onChange={() => setSearchProvider(p.id)}
                  style={{ marginTop: 2 }}
                />
                <div style={{ flex: 1 }}>
                  <div style={{ fontSize: 12, fontWeight: 500, color: 'var(--text)' }}>{p.label}</div>
                  <div style={{ fontSize: 11, color: 'var(--text-3)', marginTop: 2 }}>{p.desc}</div>
                </div>
              </label>
            ))}
            {searchProvider !== 'duckduckgo' && (
              <label style={{ fontSize: 12, color: 'var(--text-2)', marginTop: 4 }}>
                API Key
                <KeyInput
                  value={searchApiKey}
                  onChange={setSearchApiKey}
                  placeholder={SEARCH_PROVIDERS.find(p => p.id === searchProvider)?.placeholder || 'API Key'}
                />
              </label>
            )}
          </div>
        )}

        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 20 }}>
          {activeTab === 'model' && (
            <button className="git-btn" onClick={handleTestModel} disabled={testing}>
              {testing ? t('aiConfig.testing') : t('aiConfig.test')}
            </button>
          )}
          {activeTab === 'search' && (
            <button className="git-btn" onClick={handleTestSearch} disabled={testing}>
              {testing ? t('aiConfig.testing') : t('aiConfig.test')}
            </button>
          )}
          <button className="git-btn git-btn-primary" onClick={handleSave}>{t('aiConfig.save')}</button>
        </div>
      </div>
    </div>
  )
}
