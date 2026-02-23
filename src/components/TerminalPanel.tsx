import { useState, useRef, useCallback, useEffect } from 'react'
import { TerminalTab, TERMINAL_SCHEMES } from './TerminalTab'
import { useI18n } from '../lib/i18n'
import { loadSettings, listTerminalLogs, readTerminalLog, deleteTerminalLog, type TerminalProvider, type TerminalLogEntry } from '../lib/tauri'

interface TerminalPanelProps {
  cwd: string
  visible: boolean
  onOpenSettings?: () => void
  onOpenLog?: (content: string) => void
}

interface TabInfo {
  id: string
  cwd: string
  providerName?: string
  envVars?: Array<{ key: string; value: string }>
  schemeId: string
  panes: string[]  // dynamic 1~4, each is sessionId
}

function genId() {
  return crypto.randomUUID?.() || Math.random().toString(36).slice(2)
}

const MIN_HEIGHT = 100
const MAX_HEIGHT_RATIO = 0.6
const DEFAULT_HEIGHT = 200

function loadHeight(): number {
  const saved = localStorage.getItem('inkess-terminal-height')
  return saved ? Math.max(MIN_HEIGHT, parseInt(saved, 10) || DEFAULT_HEIGHT) : DEFAULT_HEIGHT
}

export function TerminalPanel({ cwd, visible, onOpenSettings, onOpenLog }: TerminalPanelProps) {
  const { t } = useI18n()
  const [tabs, setTabs] = useState<TabInfo[]>(() => [{
    id: genId(),
    cwd,
    schemeId: localStorage.getItem('inkess-terminal-scheme') || 'default',
    panes: [genId()]
  }])
  const [activeTab, setActiveTab] = useState(0)
  const [height, setHeight] = useState(loadHeight)
  const [fullscreen, setFullscreen] = useState(false)
  const [providers, setProviders] = useState<TerminalProvider[]>([])
  const [selectedProvider, setSelectedProvider] = useState<string>('')
  const [showProviderMenu, setShowProviderMenu] = useState(false)
  const [showHistoryMenu, setShowHistoryMenu] = useState(false)
  const [showSchemeMenu, setShowSchemeMenu] = useState(false)
  const [historyLogs, setHistoryLogs] = useState<TerminalLogEntry[]>([])
  const [logViewerContent, setLogViewerContent] = useState<string | null>(null)
  const [logViewerTitle, setLogViewerTitle] = useState('')
  const [logCopied, setLogCopied] = useState(false)
  const [toast, setToast] = useState('')
  const draggingRef = useRef(false)
  const startYRef = useRef(0)
  const startHRef = useRef(0)
  const heightRef = useRef(height)
  const moveHandlerRef = useRef<((ev: MouseEvent) => void) | null>(null)
  const upHandlerRef = useRef<(() => void) | null>(null)

  // Keep heightRef in sync
  useEffect(() => { heightRef.current = height }, [height])

  // Store latest cwd for new tabs only (don't update existing tabs)
  const cwdRef = useRef(cwd)
  useEffect(() => { cwdRef.current = cwd }, [cwd])

  // Load providers from settings
  useEffect(() => {
    loadSettings().then(s => {
      const provs = s.terminal_providers || []
      setProviders(provs)
      const def = provs.find(p => p.isDefault)
      if (def) setSelectedProvider(def.id)
    }).catch(() => {})
  }, [])

  const addTab = useCallback(() => {
    if (tabs.length >= 5) return
    const prov = providers.find(p => p.id === selectedProvider)
    const newTab: TabInfo = {
      id: genId(),
      cwd: cwdRef.current,
      providerName: prov?.name,
      envVars: prov?.envVars,
      schemeId: localStorage.getItem('inkess-terminal-scheme') || 'default',
      panes: [genId()]
    }
    setTabs(prev => [...prev, newTab])
    setActiveTab(tabs.length)
  }, [tabs.length, selectedProvider, providers])

  const closeTab = useCallback((idx: number) => {
    setTabs(prev => {
      const next = prev.filter((_, i) => i !== idx)
      setActiveTab(a => Math.min(a, next.length - 1))
      return next
    })
  }, [])

  // Switch provider: open a new terminal tab with the selected provider's env vars
  const switchProvider = useCallback((providerId: string, providersList?: TerminalProvider[]) => {
    setSelectedProvider(providerId)
    if (!providerId) return // "No Provider" just updates the selection for future tabs
    const list = providersList || providers
    const prov = list.find(p => p.id === providerId)
    if (!prov) return
    // If already at max tabs, replace current tab; otherwise add a new one
    const newTab: TabInfo = {
      id: genId(),
      cwd: cwdRef.current,
      providerName: prov.name,
      envVars: prov.envVars,
      schemeId: localStorage.getItem('inkess-terminal-scheme') || 'default',
      panes: [genId()]
    }
    if (tabs.length >= 5) {
      setToast(t('terminal.maxTabs'))
      setTimeout(() => setToast(''), 2500)
      return
    }
    setTabs(prev => [...prev, newTab])
    setActiveTab(tabs.length)
  }, [providers, activeTab, tabs.length])

  const addPane = useCallback(() => {
    setTabs(prev => prev.map((tab, idx) => {
      if (idx !== activeTab || tab.panes.length >= 4) return tab
      return { ...tab, panes: [...tab.panes, genId()] }
    }))
  }, [activeTab])

  const removePane = useCallback((tabIdx: number, sessionId: string) => {
    setTabs(prev => prev.map((tab, idx) => {
      if (idx !== tabIdx || tab.panes.length <= 1) return tab
      return { ...tab, panes: tab.panes.filter(id => id !== sessionId) }
    }))
  }, [])

  // 配色方案切换
  const changeScheme = useCallback((newSchemeId: string) => {
    setTabs(prev => prev.map((tab, idx) =>
      idx === activeTab ? { ...tab, schemeId: newSchemeId } : tab
    ))
  }, [activeTab])

  // Drag resize
  const onDragStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    draggingRef.current = true
    startYRef.current = e.clientY
    startHRef.current = height
    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'ns-resize'

    const onMove = (ev: MouseEvent) => {
      ev.preventDefault()
      if (!draggingRef.current) return
      const maxH = window.innerHeight * MAX_HEIGHT_RATIO
      const newH = Math.min(maxH, Math.max(MIN_HEIGHT, startHRef.current - (ev.clientY - startYRef.current)))
      setHeight(newH)
    }
    const onUp = () => {
      draggingRef.current = false
      document.body.style.userSelect = ''
      document.body.style.cursor = ''
      localStorage.setItem('inkess-terminal-height', String(heightRef.current))
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)
      moveHandlerRef.current = null
      upHandlerRef.current = null
    }
    moveHandlerRef.current = onMove
    upHandlerRef.current = onUp
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  }, [height])

  // Cleanup drag listeners on unmount
  useEffect(() => {
    return () => {
      if (moveHandlerRef.current) document.removeEventListener('mousemove', moveHandlerRef.current)
      if (upHandlerRef.current) document.removeEventListener('mouseup', upHandlerRef.current)
      document.body.style.userSelect = ''
      document.body.style.cursor = ''
    }
  }, [])

  if (!visible) return null

  return (
    <div className="terminal-panel" style={{ height: fullscreen ? '100%' : height, ...(fullscreen ? { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, zIndex: 50 } : {}) }}>
      {!fullscreen && <div className="terminal-drag-handle" onMouseDown={onDragStart} />}
      <div className="terminal-tab-bar">
        {tabs.map((tab, i) => (
          <button
            key={tab.id}
            className={`terminal-tab ${i === activeTab ? 'terminal-tab-active' : ''}`}
            onClick={() => setActiveTab(i)}
          >
            <span className="truncate">
              {tab.providerName
                ? `${tab.cwd.split('/').pop() || 'Terminal'} (${tab.providerName})`
                : tab.cwd.split('/').pop() || 'Terminal'}
            </span>
            {tabs.length > 1 && (
              <span className="terminal-tab-close" onClick={(e) => { e.stopPropagation(); closeTab(i) }}>×</span>
            )}
          </button>
        ))}
        {tabs.length < 5 && (
          <button className="terminal-tab-add" onClick={addTab} title={t('terminal.newTab')}>+</button>
        )}
        <div className="flex-1" />
        {/* Right toolbar: grouped with gap */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 2, flexShrink: 0 }}>
          {/* Provider dropdown */}
          <div style={{ position: 'relative' }}>
            <button
              className="terminal-tab-action"
              onClick={() => {
                setShowProviderMenu(v => {
                  if (!v) {
                    loadSettings().then(s => {
                      const provs = s.terminal_providers || []
                      setProviders(provs)
                    }).catch(() => {})
                  }
                  return !v
                })
                setShowHistoryMenu(false)
                setShowSchemeMenu(false)
              }}
              title={t('terminal.provider')}
              style={{ fontSize: 10, padding: '0 6px', width: 'auto', maxWidth: 140, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
            >
              {providers.find(p => p.id === selectedProvider)?.name || t('terminal.noProvider')}
              <span style={{ marginLeft: 2, fontSize: 8 }}>▾</span>
            </button>
            {showProviderMenu && (
              <div style={{ position: 'absolute', top: '100%', right: 0, zIndex: 100, background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 6, minWidth: 150, boxShadow: '0 4px 12px rgba(0,0,0,0.15)', padding: '4px 0' }}>
                <button className="dropdown-item" style={{ display: 'block', width: '100%', textAlign: 'left', padding: '4px 10px', fontSize: 11, border: 'none', background: !selectedProvider ? 'var(--bg-2)' : 'transparent', color: 'var(--text)', cursor: 'pointer' }} onClick={() => { switchProvider('', providers); setShowProviderMenu(false) }}>
                  {t('providers.none')}
                </button>
                {providers.map(p => (
                  <button key={p.id} className="dropdown-item" style={{ display: 'block', width: '100%', textAlign: 'left', padding: '4px 10px', fontSize: 11, border: 'none', background: selectedProvider === p.id ? 'var(--bg-2)' : 'transparent', color: 'var(--text)', cursor: 'pointer' }} onClick={() => { switchProvider(p.id, providers); setShowProviderMenu(false) }}>
                    {p.name}
                  </button>
                ))}
                {onOpenSettings && (
                  <>
                    <div style={{ height: 1, background: 'var(--border-s)', margin: '4px 0' }} />
                    <button className="dropdown-item" style={{ display: 'block', width: '100%', textAlign: 'left', padding: '4px 10px', fontSize: 11, border: 'none', background: 'transparent', color: 'var(--color-accent)', cursor: 'pointer' }} onClick={() => { setShowProviderMenu(false); onOpenSettings() }}>
                      {t('providers.manage')}
                    </button>
                  </>
                )}
              </div>
            )}
          </div>
          <div style={{ width: 1, height: 14, background: 'var(--border-s)', margin: '0 2px' }} />
          {/* History button */}
          <div style={{ position: 'relative' }}>
            <button
              className="terminal-tab-action"
              onClick={() => {
                setShowHistoryMenu(v => !v); setShowProviderMenu(false); setShowSchemeMenu(false)
                if (!showHistoryMenu) listTerminalLogs().then(setHistoryLogs).catch(() => {})
              }}
              title={t('terminal.history')}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
                <circle cx="12" cy="12" r="9" /><polyline points="12 7 12 12 15 15" />
              </svg>
            </button>
            {showHistoryMenu && (
              <div style={{ position: 'absolute', top: '100%', right: 0, zIndex: 100, background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 6, minWidth: 240, maxHeight: 280, overflowY: 'auto', boxShadow: '0 4px 12px rgba(0,0,0,0.15)', padding: '4px 0' }}>
                {historyLogs.length === 0 && (
                  <div style={{ padding: '8px 10px', fontSize: 11, color: 'var(--text-3)' }}>{t('terminal.noLogs')}</div>
                )}
                {historyLogs.slice(0, 20).map(log => (
                  <div key={log.filename} className="dropdown-item" style={{ display: 'flex', alignItems: 'center', gap: 4, padding: '4px 10px', fontSize: 11 }}>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                        <span className="truncate" style={{ flex: 1 }}>{log.cwd.split('/').pop() || 'Terminal'}</span>
                        {log.recovered && <span style={{ fontSize: 9, color: 'var(--orange, #f59e0b)', background: 'var(--bg-2)', borderRadius: 3, padding: '0 3px' }}>{t('terminal.recovered')}</span>}
                      </div>
                      <div style={{ fontSize: 9, color: 'var(--text-3)' }}>
                        {log.started ? new Date(log.started).toLocaleString() : '—'}
                        {log.provider && ` · ${log.provider}`}
                      </div>
                    </div>
                    <button style={{ border: 'none', background: 'transparent', color: 'var(--text-3)', cursor: 'pointer', padding: '2px 4px', borderRadius: 3, fontSize: 11 }} title={t('terminal.viewLog')} onClick={() => {
                      setShowHistoryMenu(false)
                      readTerminalLog(log.filename).then(content => {
                        setLogViewerTitle(log.cwd.split('/').pop() || 'Terminal')
                        setLogViewerContent(content)
                      }).catch(() => {})
                    }}>
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 13, height: 13 }}><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" /><circle cx="12" cy="12" r="3" /></svg>
                    </button>
                    <button style={{ border: 'none', background: 'transparent', color: 'var(--text-3)', cursor: 'pointer', padding: '2px 4px', borderRadius: 3, fontSize: 11 }} title={t('terminal.deleteLog')} onClick={() => {
                      deleteTerminalLog(log.filename).then(() => {
                        setHistoryLogs(prev => prev.filter(l => l.filename !== log.filename))
                      }).catch(() => {})
                    }}>
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 13, height: 13 }}><polyline points="3 6 5 6 21 6" /><path d="M19 6l-1 14H6L5 6" /><path d="M10 11v6" /><path d="M14 11v6" /><path d="M9 6V4h6v2" /></svg>
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
          <div style={{ width: 1, height: 14, background: 'var(--border-s)', margin: '0 2px' }} />
          {/* Color scheme dropdown */}
          <div style={{ position: 'relative' }}>
            <button
              className="terminal-tab-action"
              onClick={() => { setShowSchemeMenu(v => !v); setShowProviderMenu(false); setShowHistoryMenu(false) }}
              title={t('terminal.colorScheme')}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
                <circle cx="12" cy="12" r="10" /><circle cx="12" cy="8" r="1.5" fill="currentColor" stroke="none" /><circle cx="8" cy="13" r="1.5" fill="currentColor" stroke="none" /><circle cx="16" cy="13" r="1.5" fill="currentColor" stroke="none" />
              </svg>
            </button>
            {showSchemeMenu && (
              <div style={{ position: 'absolute', top: '100%', right: 0, zIndex: 100, background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 6, minWidth: 150, boxShadow: '0 4px 12px rgba(0,0,0,0.15)', padding: '4px 0' }}>
                {TERMINAL_SCHEMES.map(s => (
                  <button key={s.id} className="dropdown-item" style={{ display: 'flex', alignItems: 'center', gap: 6, width: '100%', textAlign: 'left', padding: '5px 10px', fontSize: 11, border: 'none', background: tabs[activeTab]?.schemeId === s.id ? 'var(--bg-2)' : 'transparent', color: 'var(--text)', cursor: 'pointer' }} onClick={() => {
                    changeScheme(s.id)
                    localStorage.setItem('inkess-terminal-scheme', s.id)
                    setShowSchemeMenu(false)
                  }}>
                    <span style={{ width: 12, height: 12, borderRadius: 3, background: s.id === 'default' ? 'var(--bg)' : s.theme.background, border: '1px solid var(--border-s)', flexShrink: 0 }} />
                    {s.name}
                  </button>
                ))}
              </div>
            )}
          </div>
          <div style={{ width: 1, height: 14, background: 'var(--border-s)', margin: '0 2px' }} />
          <button
            className={`terminal-tab-action ${(tabs[activeTab]?.panes.length || 1) > 1 ? 'terminal-tab-action-active' : ''}`}
            onClick={addPane}
            disabled={(tabs[activeTab]?.panes.length || 1) >= 4}
            title={t('terminal.split')}
            style={{ opacity: (tabs[activeTab]?.panes.length || 1) >= 4 ? 0.3 : 1 }}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
              <rect x="3" y="3" width="18" height="18" rx="2" /><line x1="12" y1="3" x2="12" y2="21" />
            </svg>
          </button>
          <button
            className={`terminal-tab-action ${fullscreen ? 'terminal-tab-action-active' : ''}`}
            onClick={() => setFullscreen(v => !v)}
            title={t('terminal.fullscreen')}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3.5 h-3.5">
              {fullscreen ? (
                <><polyline points="4 14 10 14 10 20" /><polyline points="20 10 14 10 14 4" /><line x1="14" y1="10" x2="21" y2="3" /><line x1="3" y1="21" x2="10" y2="14" /></>
              ) : (
                <><polyline points="15 3 21 3 21 9" /><polyline points="9 21 3 21 3 15" /><line x1="21" y1="3" x2="14" y2="10" /><line x1="3" y1="21" x2="10" y2="14" /></>
              )}
            </svg>
          </button>
        </div>
      </div>
      <div className="terminal-body">
        {tabs.map((tab, tabIdx) => {
          const isActive = tabIdx === activeTab
          return (
            <div
              key={tab.id}
              style={{
                display: isActive ? 'grid' : 'none',
                gridTemplateColumns: '1fr 1fr',
                height: '100%',
                gap: tab.panes.length > 1 ? '1px' : 0,
                background: tab.panes.length > 1 ? 'var(--border)' : 'transparent',
              }}
            >
              {tab.panes.map(sessionId => (
                <div
                  key={sessionId}
                  className="terminal-pane-wrapper"
                  style={{ overflow: 'hidden', background: 'var(--bg)' }}
                >
                  <TerminalTab
                    sessionId={sessionId}
                    cwd={tab.cwd}
                    active={isActive}
                    envVars={tab.envVars}
                    schemeId={tab.schemeId}
                    onClose={tab.panes.length > 1 ? () => removePane(tabIdx, sessionId) : undefined}
                  />
                </div>
              ))}
            </div>
          )
        })}
      </div>
      {/* Toast */}
      {toast && (
        <div style={{ position: 'absolute', bottom: 48, left: '50%', transform: 'translateX(-50%)', zIndex: 200, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 6, padding: '6px 14px', fontSize: 11, color: 'var(--text)', boxShadow: '0 4px 12px rgba(0,0,0,0.15)', whiteSpace: 'nowrap' }}>{toast}</div>
      )}
      {/* Log viewer modal */}
      {logViewerContent !== null && (
        <div style={{ position: 'fixed', inset: 0, zIndex: 200, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(0,0,0,0.4)' }} onClick={() => setLogViewerContent(null)}>
          <div style={{ background: 'var(--bg)', border: '1px solid var(--border)', borderRadius: 10, width: '80%', maxWidth: 700, maxHeight: '70vh', display: 'flex', flexDirection: 'column', boxShadow: '0 8px 32px rgba(0,0,0,0.2)' }} onClick={e => e.stopPropagation()}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '10px 16px', borderBottom: '1px solid var(--border-s)' }}>
              <span style={{ fontSize: 13, fontWeight: 500 }}>{logViewerTitle}</span>
              <div style={{ display: 'flex', gap: 6 }}>
                <button style={{ border: 'none', background: 'var(--accent-subtle)', color: 'var(--color-accent)', borderRadius: 4, padding: '3px 10px', fontSize: 11, cursor: 'pointer' }} onClick={() => {
                  navigator.clipboard.writeText(logViewerContent).then(() => {
                    setLogCopied(true); setTimeout(() => setLogCopied(false), 1500)
                  }).catch(() => {})
                }}>{logCopied ? t('terminal.copied') : t('terminal.copy')}</button>
                <button style={{ border: 'none', background: 'transparent', color: 'var(--text-3)', cursor: 'pointer', fontSize: 16, lineHeight: 1, padding: '0 4px' }} onClick={() => setLogViewerContent(null)}>×</button>
              </div>
            </div>
            <pre style={{ flex: 1, overflow: 'auto', padding: '12px 16px', margin: 0, fontSize: 11, fontFamily: "'JetBrains Mono', 'Menlo', monospace", whiteSpace: 'pre-wrap', wordBreak: 'break-all', color: 'var(--text)', background: 'var(--surface)', borderRadius: '0 0 10px 10px' }}>{logViewerContent}</pre>
          </div>
        </div>
      )}
    </div>
  )
}
