import { useState, useEffect } from 'react'
import { type ThemeId, themes } from '../lib/themes'
import { useI18n, type Language } from '../lib/i18n'
import { useLicense } from '../lib/license'
import { useFocusTrap } from '../lib/useFocusTrap'
import { getSnapshotStats, cleanupSnapshots, loadSettings, saveSettings, ragStats, ragRebuild, mcpListServers, mcpAddServer, mcpRemoveServer, mcpRestartServer, getSystemEnvVars, getShellEnvVars, parseShellFunctions, type SnapshotStats, type RagIndexStats, type McpServerStatus, type TerminalProvider, type ShellFunction } from '../lib/tauri'

const MCP_TEMPLATES = [
  { name: 'Filesystem', command: 'npx', args: '-y @modelcontextprotocol/server-filesystem /path', transport: 'stdio' as const, env: '' },
  { name: 'Brave Search', command: 'npx', args: '-y @modelcontextprotocol/server-brave-search', transport: 'stdio' as const, env: 'BRAVE_API_KEY=your-key' },
  { name: 'GitHub', command: 'npx', args: '-y @modelcontextprotocol/server-github', transport: 'stdio' as const, env: 'GITHUB_PERSONAL_ACCESS_TOKEN=your-token' },
  { name: 'SQLite', command: 'npx', args: '-y @modelcontextprotocol/server-sqlite /path/to/db.sqlite', transport: 'stdio' as const, env: '' },
  { name: 'Memory', command: 'npx', args: '-y @modelcontextprotocol/server-memory', transport: 'stdio' as const, env: '' },
]

interface SettingsPanelProps {
  visible: boolean
  onClose: () => void
  themeId: ThemeId
  onSetTheme: (id: ThemeId) => void
  onToast: (msg: string) => void
  onOpenLicense?: () => void
  currentDir?: string
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB'
}

function ProviderForm({ provider, t, onSave, onCancel }: {
  provider: TerminalProvider
  t: (key: string) => string
  onSave: (p: TerminalProvider) => void
  onCancel: () => void
}) {
  const [name, setName] = useState(provider.name)
  const [envVars, setEnvVars] = useState(provider.envVars.length > 0 ? provider.envVars : [{ key: '', value: '' }])
  const [isDefault, setIsDefault] = useState(provider.isDefault)
  const [sysVars, setSysVars] = useState<Array<[string, string]>>([])
  const [showSysVars, setShowSysVars] = useState(false)
  const [sysFilter, setSysFilter] = useState('')
  const [revealedIdx, setRevealedIdx] = useState<Set<number>>(new Set())

  const updateVar = (i: number, field: 'key' | 'value', val: string) => {
    setEnvVars(prev => prev.map((v, idx) => idx === i ? { ...v, [field]: val } : v))
  }
  const removeVar = (i: number) => setEnvVars(prev => prev.filter((_, idx) => idx !== i))
  const addVar = () => setEnvVars(prev => [...prev, { key: '', value: '' }])

  const loadSysVars = (source: 'system' | 'shell') => {
    const fetcher = source === 'system' ? getSystemEnvVars : getShellEnvVars
    fetcher().then(vars => { setSysVars(vars); setShowSysVars(true) }).catch(() => {})
  }

  const importSysVar = (key: string, value: string) => {
    const exists = envVars.findIndex(v => v.key === key)
    if (exists >= 0) {
      setEnvVars(prev => prev.map((v, i) => i === exists ? { ...v, value } : v))
    } else {
      // Replace first empty row or append
      const emptyIdx = envVars.findIndex(v => !v.key.trim())
      if (emptyIdx >= 0) {
        setEnvVars(prev => prev.map((v, i) => i === emptyIdx ? { key, value } : v))
      } else {
        setEnvVars(prev => [...prev, { key, value }])
      }
    }
  }

  const filteredSysVars = sysFilter
    ? sysVars.filter(([k]) => k.toLowerCase().includes(sysFilter.toLowerCase()))
    : sysVars

  return (
    <div style={{ marginTop: 8, padding: 10, border: '1px solid var(--border-s)', borderRadius: 6, background: 'var(--bg-2, var(--bg))' }}>
      <div style={{ marginBottom: 6 }}>
        <label style={{ fontSize: 11, color: 'var(--text-3)' }}>{t('providers.name')}</label>
        <input className="ai-config-input" style={{ width: '100%', fontSize: 12 }} value={name} onChange={e => setName(e.target.value)} placeholder="e.g. OpenAI" />
      </div>
      <div style={{ marginBottom: 6 }}>
        <label style={{ fontSize: 11, color: 'var(--text-3)' }}>{t('providers.envVars')}</label>
        {envVars.map((v, i) => (
          <div key={i} style={{ display: 'flex', gap: 4, marginTop: 4, alignItems: 'center' }}>
            <input className="ai-config-input" style={{ flex: 1, fontSize: 11 }} value={v.key} onChange={e => updateVar(i, 'key', e.target.value)} placeholder={t('providers.key')} />
            <input className="ai-config-input" style={{ flex: 1, fontSize: 11 }} type={revealedIdx.has(i) ? 'text' : 'password'} value={v.value} onChange={e => updateVar(i, 'value', e.target.value)} placeholder={t('providers.value')} />
            <button className="git-btn" style={{ fontSize: 9, padding: '1px 4px', minWidth: 20 }} title={revealedIdx.has(i) ? 'Hide' : 'Show'} onClick={() => setRevealedIdx(prev => { const next = new Set(prev); next.has(i) ? next.delete(i) : next.add(i); return next })}>{revealedIdx.has(i) ? '👁' : '···'}</button>
            <button className="git-btn" style={{ fontSize: 10, padding: '1px 4px', color: 'var(--red, #ef4444)' }} onClick={() => removeVar(i)}>✕</button>
          </div>
        ))}
        <button className="git-btn" style={{ fontSize: 10, marginTop: 4 }} onClick={addVar}>+ {t('providers.addVar')}</button>
        <button className="git-btn" style={{ fontSize: 10, marginTop: 4, marginLeft: 6 }} onClick={() => loadSysVars('system')}>{t('providers.importEnv')}</button>
        <button className="git-btn" style={{ fontSize: 10, marginTop: 4, marginLeft: 6 }} onClick={() => loadSysVars('shell')}>{t('providers.importShell')}</button>
        {showSysVars && (
          <div style={{ marginTop: 6, border: '1px solid var(--border-s)', borderRadius: 4, maxHeight: 160, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
            <input className="ai-config-input" style={{ fontSize: 10, margin: 4, width: 'calc(100% - 8px)' }} placeholder="Filter..." value={sysFilter} onChange={e => setSysFilter(e.target.value)} />
            <div style={{ overflowY: 'auto', flex: 1 }}>
              {filteredSysVars.slice(0, 50).map(([k, v]) => (
                <button key={k} style={{ display: 'block', width: '100%', textAlign: 'left', padding: '2px 8px', fontSize: 10, border: 'none', background: 'transparent', color: 'var(--text)', cursor: 'pointer' }} onClick={() => importSysVar(k, v)} onMouseOver={e => (e.currentTarget.style.background = 'var(--bg-2)')} onMouseOut={e => (e.currentTarget.style.background = 'transparent')}>
                  <span style={{ color: 'var(--color-accent)' }}>{k}</span>
                  <span style={{ color: 'var(--text-3)', marginLeft: 6 }}>{v.length > 40 ? v.slice(0, 40) + '…' : v}</span>
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
      <label style={{ fontSize: 11, color: 'var(--text-3)', display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8 }}>
        <input type="checkbox" checked={isDefault} onChange={e => setIsDefault(e.target.checked)} />
        {t('providers.default')}
      </label>
      <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
        <button className="git-btn" style={{ fontSize: 11 }} onClick={onCancel}>{t('providers.cancel')}</button>
        <button className="git-btn git-btn-primary" style={{ fontSize: 11 }} onClick={() => {
          if (!name.trim()) return
          const filtered = envVars.filter(v => v.key.trim())
          onSave({ ...provider, name: name.trim(), envVars: filtered, isDefault })
        }}>{t('providers.confirm')}</button>
      </div>
    </div>
  )
}

export function SettingsPanel({ visible, onClose, themeId, onSetTheme, onToast, onOpenLicense, currentDir }: SettingsPanelProps) {
  const { t, lang, setLang } = useI18n()
  const { isPro, licenseKey } = useLicense()
  const trapRef = useFocusTrap(visible)
  const [stats, setStats] = useState<SnapshotStats | null>(null)
  const [ragStat, setRagStat] = useState<RagIndexStats | null>(null)
  const [rebuilding, setRebuilding] = useState(false)
  const [retentionDays, setRetentionDays] = useState(() => {
    return parseInt(localStorage.getItem('inkess-retention-days') || '30') || 30
  })
  const [retentionCount, setRetentionCount] = useState(() => {
    return parseInt(localStorage.getItem('inkess-retention-count') || '100') || 100
  })
  const [cleaning, setCleaning] = useState(false)
  const [mcpServers, setMcpServers] = useState<McpServerStatus[]>([])
  const [showAddMcp, setShowAddMcp] = useState(false)
  const [mcpName, setMcpName] = useState('')
  const [mcpCommand, setMcpCommand] = useState('')
  const [mcpArgs, setMcpArgs] = useState('')
  const [mcpEnv, setMcpEnv] = useState('')
  const [mcpTransport, setMcpTransport] = useState<'stdio' | 'http'>('stdio')
  const [mcpUrl, setMcpUrl] = useState('')
  const [settingsTab, setSettingsTab] = useState<'appearance' | 'data' | 'mcp' | 'providers' | 'license'>('appearance')
  const [providers, setProviders] = useState<TerminalProvider[]>([])
  const [editingProvider, setEditingProvider] = useState<TerminalProvider | null>(null)
  const [isAddingProvider, setIsAddingProvider] = useState(false)
  const [shellFunctions, setShellFunctions] = useState<ShellFunction[]>([])
  const [showImportPicker, setShowImportPicker] = useState(false)

  useEffect(() => {
    if (visible) {
      getSnapshotStats().then(setStats).catch(() => {})
      ragStats().then(setRagStat).catch(() => setRagStat(null))
      mcpListServers().then(setMcpServers).catch(() => {})
      // Load retention settings from settings.json
      loadSettings().then(s => {
        if (s.retention_days != null) setRetentionDays(s.retention_days)
        if (s.retention_count != null) setRetentionCount(s.retention_count)
        if (s.terminal_providers) setProviders(s.terminal_providers)
      }).catch(() => {})
    }
  }, [visible])

  const handleCleanup = async () => {
    setCleaning(true)
    try {
      const deleted = await cleanupSnapshots(retentionDays, retentionCount)
      onToast(t('settings.cleanupDone', { n: deleted }))
      const newStats = await getSnapshotStats()
      setStats(newStats)
    } catch {
      onToast(t('toast.createFailed'))
    } finally {
      setCleaning(false)
    }
  }

  const handleSave = () => {
    localStorage.setItem('inkess-retention-days', String(retentionDays))
    localStorage.setItem('inkess-retention-count', String(retentionCount))
    // Persist to settings.json
    loadSettings().then(s => saveSettings({ ...s, retention_days: retentionDays, retention_count: retentionCount })).catch(() => {})
    onToast(t('settings.settingsSaved'))
    onClose()
  }

  if (!visible) return null

  const tabs = [
    { id: 'appearance' as const, label: t('settings.tab.appearance') },
    { id: 'data' as const, label: t('settings.tab.data') },
    { id: 'mcp' as const, label: t('settings.tab.mcp') },
    { id: 'providers' as const, label: t('settings.tab.providers') },
    { id: 'license' as const, label: t('settings.tab.license') },
  ]

  return (
    <div className="shortcuts-backdrop" onClick={onClose}>
      <div ref={trapRef} className="shortcuts-modal settings-tabbed" role="dialog" aria-modal="true" onClick={e => e.stopPropagation()}>
        <div className="flex items-center justify-between mb-1">
          <h3 style={{ margin: 0 }}>{t('settings.title')}</h3>
          <button className="sidebar-action-btn" onClick={onClose} aria-label={t('ai.close')}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
        <nav className="settings-tab-nav">
          {tabs.map(tab => (
            <button
              key={tab.id}
              className={`settings-tab-btn${settingsTab === tab.id ? ' active' : ''}`}
              onClick={() => setSettingsTab(tab.id)}
            >
              {tab.label}
            </button>
          ))}
        </nav>
        <div className="settings-tab-content">
          {/* Appearance tab */}
          {settingsTab === 'appearance' && (
            <div>
              <div className="flex items-center gap-3 mb-4">
                <span className="text-[12.5px]" style={{ color: 'var(--text-2)', minWidth: 48 }}>{t('settings.theme')}</span>
                <div className="flex gap-2">
                  {themes.map(th => (
                    <button
                      key={th.id}
                      className="text-[12px] px-3 py-1.5 rounded cursor-pointer border-none"
                      style={{
                        background: themeId === th.id ? 'var(--color-accent)' : 'var(--sidebar-bg)',
                        color: themeId === th.id ? '#fff' : 'var(--text-2)',
                        fontFamily: 'inherit',
                      }}
                      onClick={() => onSetTheme(th.id)}
                    >
                      {t(`theme.${th.id}`)}
                    </button>
                  ))}
                </div>
              </div>
              <div className="flex items-center gap-3">
                <span className="text-[12.5px]" style={{ color: 'var(--text-2)', minWidth: 48 }}>{t('settings.language')}</span>
                <div className="flex gap-2">
                  {([['zh', '中文'], ['en', 'English']] as const).map(([code, label]) => (
                    <button
                      key={code}
                      className="text-[12px] px-3 py-1.5 rounded cursor-pointer border-none"
                      style={{
                        background: lang === code ? 'var(--color-accent)' : 'var(--sidebar-bg)',
                        color: lang === code ? '#fff' : 'var(--text-2)',
                        fontFamily: 'inherit',
                      }}
                      onClick={() => setLang(code as Language)}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* Data tab */}
          {settingsTab === 'data' && (<div>
            <div className="text-[12.5px] mb-3" style={{ color: 'var(--text-2)' }}>
              {t('settings.snapshotStats')}:{' '}
              {stats ? t('settings.snapshotCount', { count: stats.count, size: formatBytes(stats.size_bytes) }) : t('settings.loading')}
            </div>
            <div className="flex items-center gap-3 mb-3">
              <label className="text-[12px] flex items-center gap-2" style={{ color: 'var(--text-2)' }}>
                {t('settings.retentionDays')}
                <input type="number" className="new-file-input" style={{ width: 70, margin: 0 }} value={retentionDays} min={1} onChange={e => setRetentionDays(parseInt(e.target.value) || 30)} />
              </label>
              <label className="text-[12px] flex items-center gap-2" style={{ color: 'var(--text-2)' }}>
                {t('settings.retentionCount')}
                <input type="number" className="new-file-input" style={{ width: 70, margin: 0 }} value={retentionCount} min={1} onChange={e => setRetentionCount(parseInt(e.target.value) || 100)} />
              </label>
            </div>
            <button className="git-btn" style={{ fontSize: 12, marginBottom: 20 }} onClick={handleCleanup} disabled={cleaning}>
              {cleaning ? t('settings.cleaning') : t('settings.cleanupNow')}
            </button>
            {/* Knowledge Base */}
            <h4 style={{ margin: '0 0 8px', fontSize: 13, color: 'var(--text)' }}>{t('rag.knowledgeBase')}</h4>
            {ragStat ? (
              <div style={{ fontSize: 12, color: 'var(--text-2)', marginBottom: 8 }}>
                {t('rag.stats', { files: ragStat.file_count, chunks: ragStat.chunk_count, size: formatBytes(ragStat.db_size_bytes) })}
              </div>
            ) : (
              <div style={{ fontSize: 12, color: 'var(--text-3)', marginBottom: 8 }}>{t('rag.status.disabled')}</div>
            )}
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="git-btn" style={{ fontSize: 12 }} onClick={async () => {
                if (!currentDir) return
                setRebuilding(true)
                try { await ragRebuild(currentDir); const s = await ragStats(); setRagStat(s); onToast(t('rag.rebuildDone')) }
                catch { onToast(t('rag.initFailed')) }
                finally { setRebuilding(false) }
              }} disabled={rebuilding || !currentDir}>
                {rebuilding ? t('rag.rebuilding') : t('rag.rebuild')}
              </button>
            </div>
            <div className="flex justify-end mt-4">
              <button className="toolbar-btn toolbar-btn-accent" onClick={handleSave}>{t('settings.save')}</button>
            </div>
          </div>)}

          {/* MCP tab */}
          {settingsTab === 'mcp' && (<div>
            {mcpServers.length === 0 && !showAddMcp && (
              <div style={{ fontSize: 12, color: 'var(--text-3)', marginBottom: 8 }}>No MCP servers configured</div>
            )}
            {mcpServers.map(s => {
              const lastSeenText = s.last_seen ? (() => {
                const diff = Math.floor(Date.now() / 1000) - s.last_seen!
                if (diff < 60) return t('mcp.secondsAgo', { n: diff })
                return t('mcp.minutesAgo', { n: Math.floor(diff / 60) })
              })() : null
              return (
              <div key={s.id} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 0', borderBottom: '1px solid var(--border-s)' }}>
                <span style={{ width: 6, height: 6, borderRadius: '50%', flexShrink: 0, background: s.connected ? '#22c55e' : s.error ? '#ef4444' : 'var(--text-3)' }} />
                <span style={{ fontSize: 12, color: 'var(--text)', flex: 1 }}>
                  {s.name}
                  <span style={{ fontSize: 10, color: 'var(--text-3)', marginLeft: 4 }}>{s.transport}</span>
                </span>
                <span style={{ fontSize: 11, color: 'var(--text-3)' }}>
                  {s.connected ? t('mcp.tools', { n: s.tool_count }) : s.error ? t('mcp.error') : t('mcp.disconnected')}
                  {lastSeenText && <span style={{ marginLeft: 4, fontSize: 10 }}>{lastSeenText}</span>}
                </span>
                <button className="git-btn" style={{ fontSize: 10, padding: '1px 6px' }} onClick={async () => {
                  try { await mcpRestartServer(s.id); const updated = await mcpListServers(); setMcpServers(updated) }
                  catch { onToast(t('mcp.restartFailed')) }
                }}>{t('mcp.restart')}</button>
                <button className="git-btn" style={{ fontSize: 10, padding: '1px 6px', color: 'var(--red, #ef4444)' }} onClick={async () => {
                  try { await mcpRemoveServer(s.id); const updated = await mcpListServers(); setMcpServers(updated) }
                  catch { onToast(t('mcp.removeFailed')) }
                }}>✕</button>
              </div>
              )
            })}
            {showAddMcp ? (
              <div style={{ marginTop: 8, padding: 10, border: '1px solid var(--border-s)', borderRadius: 6, background: 'var(--bg-2, var(--bg))' }}>
                <div style={{ marginBottom: 8 }}>
                  <label style={{ fontSize: 11, color: 'var(--text-3)' }}>{t('mcp.template')}</label>
                  <select className="ai-config-input" style={{ width: '100%', fontSize: 12 }} value="" onChange={e => {
                    const tpl = MCP_TEMPLATES.find(t => t.name === e.target.value)
                    if (tpl) { setMcpName(tpl.name); setMcpCommand(tpl.command); setMcpArgs(tpl.args); setMcpEnv(tpl.env); setMcpTransport(tpl.transport); setMcpUrl('') }
                  }}>
                    <option value="">{t('mcp.selectTemplate')}</option>
                    {MCP_TEMPLATES.map(tpl => <option key={tpl.name} value={tpl.name}>{tpl.name}</option>)}
                  </select>
                </div>
                <div style={{ marginBottom: 6 }}>
                  <label style={{ fontSize: 11, color: 'var(--text-3)' }}>{t('mcp.name')}</label>
                  <input className="ai-config-input" style={{ width: '100%', fontSize: 12 }} value={mcpName} onChange={e => setMcpName(e.target.value)} placeholder="e.g. filesystem" />
                </div>
                <div style={{ marginBottom: 6 }}>
                  <label style={{ fontSize: 11, color: 'var(--text-3)' }}>{t('mcp.transport')}</label>
                  <select className="ai-config-input" style={{ width: '100%', fontSize: 12 }} value={mcpTransport} onChange={e => setMcpTransport(e.target.value as 'stdio' | 'http')}>
                    <option value="stdio">Stdio</option>
                    <option value="http">HTTP</option>
                  </select>
                </div>
                {mcpTransport === 'http' ? (
                  <div style={{ marginBottom: 6 }}>
                    <label style={{ fontSize: 11, color: 'var(--text-3)' }}>{t('mcp.url')} <span style={{ color: 'var(--text-3)' }}>({t('mcp.urlHint')})</span></label>
                    <input className="ai-config-input" style={{ width: '100%', fontSize: 12 }} value={mcpUrl} onChange={e => setMcpUrl(e.target.value)} placeholder="http://localhost:3000" />
                  </div>
                ) : (<>
                  <div style={{ marginBottom: 6 }}>
                    <label style={{ fontSize: 11, color: 'var(--text-3)' }}>{t('mcp.command')}</label>
                    <input className="ai-config-input" style={{ width: '100%', fontSize: 12 }} value={mcpCommand} onChange={e => setMcpCommand(e.target.value)} placeholder="e.g. npx" />
                  </div>
                  <div style={{ marginBottom: 6 }}>
                    <label style={{ fontSize: 11, color: 'var(--text-3)' }}>{t('mcp.args')}</label>
                    <input className="ai-config-input" style={{ width: '100%', fontSize: 12 }} value={mcpArgs} onChange={e => setMcpArgs(e.target.value)} placeholder="e.g. @modelcontextprotocol/server-filesystem /tmp" />
                  </div>
                </>)}
                <div style={{ marginBottom: 8 }}>
                  <label style={{ fontSize: 11, color: 'var(--text-3)' }}>{t('mcp.env')} <span style={{ color: 'var(--text-3)' }}>({t('mcp.envHint')})</span></label>
                  <textarea className="ai-config-input" style={{ width: '100%', fontSize: 11, minHeight: 36, resize: 'vertical' }} value={mcpEnv} onChange={e => setMcpEnv(e.target.value)} placeholder="KEY=VALUE" />
                </div>
                <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                  <button className="git-btn" style={{ fontSize: 11 }} onClick={() => { setShowAddMcp(false); setMcpName(''); setMcpCommand(''); setMcpArgs(''); setMcpEnv(''); setMcpTransport('stdio'); setMcpUrl('') }}>{t('mcp.cancel')}</button>
                  <button className="git-btn git-btn-primary" style={{ fontSize: 11 }} onClick={async () => {
                    if (!mcpName.trim()) return
                    if (mcpTransport === 'stdio' && !mcpCommand.trim()) return
                    if (mcpTransport === 'http' && !mcpUrl.trim()) return
                    const envMap: Record<string, string> = {}
                    mcpEnv.split('\n').forEach(line => { const eq = line.indexOf('='); if (eq > 0) envMap[line.slice(0, eq).trim()] = line.slice(eq + 1).trim() })
                    try {
                      await mcpAddServer({ id: mcpName.trim().toLowerCase().replace(/\s+/g, '-'), name: mcpName.trim(), command: mcpTransport === 'stdio' ? mcpCommand.trim() : '', args: mcpTransport === 'stdio' && mcpArgs.trim() ? mcpArgs.trim().split(/\s+/) : [], env: envMap, enabled: true, transport: mcpTransport, url: mcpTransport === 'http' ? mcpUrl.trim() : undefined })
                      const updated = await mcpListServers(); setMcpServers(updated); setShowAddMcp(false)
                      setMcpName(''); setMcpCommand(''); setMcpArgs(''); setMcpEnv(''); setMcpTransport('stdio'); setMcpUrl('')
                      onToast(t('mcp.addSuccess'))
                    } catch { onToast(t('mcp.addFailed')) }
                  }}>{t('mcp.add')}</button>
                </div>
              </div>
            ) : (
              <button className="git-btn" style={{ fontSize: 11, marginTop: 8 }} onClick={() => setShowAddMcp(true)}>+ {t('mcp.addServer')}</button>
            )}
          </div>)}

          {/* Providers tab */}
          {settingsTab === 'providers' && (<div>
            <h4 style={{ margin: '0 0 8px', fontSize: 13, color: 'var(--text)' }}>{t('providers.title')}</h4>
            {providers.filter(p => !(editingProvider && editingProvider.id === p.id)).map(p => (
              <div key={p.id} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 0', borderBottom: '1px solid var(--border-s)' }}>
                <span style={{ fontSize: 12, color: 'var(--text)', flex: 1 }}>
                  {p.name}
                  {p.isDefault && <span style={{ fontSize: 10, color: 'var(--color-accent)', marginLeft: 6 }}>default</span>}
                  <span style={{ fontSize: 10, color: 'var(--text-3)', marginLeft: 6 }}>
                    {p.envVars.length > 0 ? p.envVars.map(v => v.key).join(', ') : '—'}
                  </span>
                </span>
                <button className="git-btn" style={{ fontSize: 10, padding: '1px 6px' }} onClick={() => { setEditingProvider({ ...p, envVars: p.envVars.map(v => ({ ...v })) }); setIsAddingProvider(false) }}>Edit</button>
                <button className="git-btn" style={{ fontSize: 10, padding: '1px 6px', color: 'var(--red, #ef4444)' }} onClick={() => {
                  if (!confirm(t('providers.deleteConfirm'))) return
                  const next = providers.filter(x => x.id !== p.id)
                  setProviders(next)
                  loadSettings().then(s => saveSettings({ ...s, terminal_providers: next })).catch(() => {})
                }}>✕</button>
              </div>
            ))}
            {(editingProvider || isAddingProvider) && <ProviderForm
              provider={editingProvider || { id: '', name: '', envVars: [{ key: '', value: '' }], isDefault: false }}
              t={t}
              onSave={(p) => {
                let next: TerminalProvider[]
                if (isAddingProvider) {
                  const newP = { ...p, id: p.id || Date.now().toString(36) }
                  next = [...providers, newP]
                } else {
                  next = providers.map(x => x.id === p.id ? p : x)
                }
                if (p.isDefault) next = next.map(x => ({ ...x, isDefault: x.id === p.id }))
                setProviders(next)
                loadSettings().then(s => saveSettings({ ...s, terminal_providers: next })).catch(() => {})
                setEditingProvider(null); setIsAddingProvider(false)
                onToast(t('providers.saved'))
              }}
              onCancel={() => { setEditingProvider(null); setIsAddingProvider(false) }}
            />}
            {!editingProvider && !isAddingProvider && (
              <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
                <button className="git-btn" style={{ fontSize: 11 }} onClick={() => setIsAddingProvider(true)}>+ {t('providers.add')}</button>
                <button className="git-btn" style={{ fontSize: 11 }} onClick={() => {
                  parseShellFunctions().then(fns => {
                    // Filter out functions already imported (by name match)
                    const existingNames = new Set(providers.map(p => p.name))
                    setShellFunctions(fns.filter(f => !existingNames.has(f.name)))
                    setShowImportPicker(true)
                  }).catch(() => {})
                }}>{t('providers.importRC')}</button>
              </div>
            )}
            {showImportPicker && (
              <div style={{ marginTop: 8, padding: 10, border: '1px solid var(--border-s)', borderRadius: 6, background: 'var(--bg-2, var(--bg))' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 6 }}>
                  <span style={{ fontSize: 12, fontWeight: 500 }}>{t('providers.importRCTitle')}</span>
                  <button style={{ border: 'none', background: 'transparent', color: 'var(--text-3)', cursor: 'pointer', fontSize: 14 }} onClick={() => setShowImportPicker(false)}>×</button>
                </div>
                {shellFunctions.length === 0 && (
                  <div style={{ fontSize: 11, color: 'var(--text-3)', padding: '4px 0' }}>{t('providers.noFunctions')}</div>
                )}
                {shellFunctions.map(fn => (
                  <div key={fn.name} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '5px 0', borderBottom: '1px solid var(--border-s)' }}>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: 12, color: 'var(--text)' }}>{fn.name}</div>
                      <div style={{ fontSize: 10, color: 'var(--text-3)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {fn.env_vars.map(([k]) => k).join(', ')}
                      </div>
                    </div>
                    <button className="git-btn" style={{ fontSize: 10, flexShrink: 0 }} onClick={() => {
                      const newP: TerminalProvider = {
                        id: Date.now().toString(36) + Math.random().toString(36).slice(2, 5),
                        name: fn.name,
                        envVars: fn.env_vars.map(([key, value]) => ({ key, value })),
                        isDefault: false,
                      }
                      const next = [...providers, newP]
                      setProviders(next)
                      loadSettings().then(s => saveSettings({ ...s, terminal_providers: next })).catch(() => {})
                      setShellFunctions(prev => prev.filter(f => f.name !== fn.name))
                      onToast(`${fn.name} imported`)
                    }}>{t('providers.import')}</button>
                  </div>
                ))}
              </div>
            )}
          </div>)}

          {/* License tab */}
          {settingsTab === 'license' && (
            <div>
              <div className="flex items-center gap-3 mb-4">
                <span className="text-[12.5px]" style={{ color: isPro ? '#16a34a' : 'var(--text-2)' }}>
                  {isPro ? `${t('license.pro')} — ${t('license.activated')}` : t('license.free')}
                  {isPro && licenseKey && (
                    <span className="ml-2 text-[11px]" style={{ opacity: 0.6 }}>{licenseKey.slice(0, 11)}...</span>
                  )}
                </span>
                <div style={{ flex: 1 }} />
                {onOpenLicense && (
                  <button className="git-btn" style={{ fontSize: 11 }} onClick={() => { onClose(); onOpenLicense() }}>
                    {isPro ? t('license.title') : t('license.upgrade')}
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
