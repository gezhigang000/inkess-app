import { useI18n } from '../lib/i18n'
import { type AiConfig, type PythonSetupProgress, type McpToolCallLog, mcpToolLogs, aiCancelChat } from '../lib/tauri'
import { useState } from 'react'

export interface AIChatInputProps {
  config: AiConfig | null
  input: string
  setInput: (v: string) => void
  streaming: boolean
  deepMode: boolean
  setDeepMode: (v: boolean | ((prev: boolean) => boolean)) => void
  currentDir: string
  sessionId: string
  pythonSetup: PythonSetupProgress | null
  inputRef: React.RefObject<HTMLTextAreaElement | null>
  onSend: () => void
  onKeyDown: (e: React.KeyboardEvent) => void
  onToast: (msg: string) => void
}

export function AIChatInput({
  config, input, setInput, streaming, deepMode, setDeepMode,
  currentDir, sessionId, pythonSetup, inputRef,
  onSend, onKeyDown, onToast,
}: AIChatInputProps) {
  const { t } = useI18n()
  const [showToolLogs, setShowToolLogs] = useState(false)
  const [toolLogs, setToolLogs] = useState<McpToolCallLog[]>([])
  const [expandedLogIdx, setExpandedLogIdx] = useState<number | null>(null)

  return (
    <>
      {/* Python setup progress bar */}
      {pythonSetup && (
        <div style={{ padding: '6px 12px', borderTop: '1px solid var(--border-s)', background: 'var(--bg-s)' }}>
          <div style={{ fontSize: 11, color: 'var(--text-2)', marginBottom: 4 }}>
            {pythonSetup.message}
          </div>
          {pythonSetup.status !== 'error' && (
            <div style={{ width: '100%', height: 3, borderRadius: 2, background: 'var(--border-s)', overflow: 'hidden' }}>
              <div style={{
                width: `${Math.round(pythonSetup.progress * 100)}%`,
                height: '100%',
                borderRadius: 2,
                background: pythonSetup.status === 'done' ? 'var(--green, #22c55e)' : 'var(--accent, #3b82f6)',
                transition: 'width 0.3s ease',
              }} />
            </div>
          )}
          {pythonSetup.status === 'error' && (
            <div style={{ fontSize: 11, color: 'var(--red, #ef4444)', marginTop: 2 }}>
              {t('ai.pythonSetup.error')}
            </div>
          )}
        </div>
      )}

      {/* Workspace indicator */}
      {!currentDir && (
        <div style={{ padding: '6px 12px', fontSize: 11, color: 'var(--text-3)', background: 'var(--accent-subtle)', borderRadius: 4, margin: '0 12px 4px', display: 'flex', alignItems: 'center', gap: 6 }}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14, flexShrink: 0 }}>
            <circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          {t('ai.noWorkspace')}
        </div>
      )}
      {currentDir && (
        <div style={{ padding: '2px 12px', fontSize: 10, color: 'var(--text-3)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={currentDir}>
          {t('ai.currentWorkspace', { dir: currentDir.replace(/\\/g, '/').split('/').pop() || currentDir })}
        </div>
      )}

      {/* Input area */}
      <div className="ai-input-area">
        <button
          className={`ai-deep-toggle ${deepMode ? 'active' : ''}`}
          onClick={() => {
            setDeepMode(v => !v)
            onToast(deepMode ? t('ai.deepModeOff') : t('ai.deepModeOn'))
          }}
          title={t('ai.deepMode')}
          aria-label={t('ai.deepMode')}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 16, height: 16 }}>
            <circle cx="12" cy="12" r="10" /><path d="M12 6v6l4 2" />
          </svg>
        </button>
        <textarea
          ref={inputRef}
          className="ai-input"
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder={config ? t('ai.inputPlaceholder') : t('ai.configFirst')}
          rows={1}
          disabled={!config}
          aria-label={t('ai.title')}
        />
        {streaming ? (
        <button
          className="ai-send-btn ai-stop-btn"
          onClick={() => aiCancelChat(sessionId)}
          aria-label={t('ai.stopGeneration')}
        >
          <svg viewBox="0 0 24 24" fill="currentColor" style={{ width: 14, height: 14 }}>
            <rect x="6" y="6" width="12" height="12" rx="2" />
          </svg>
        </button>
        ) : (
        <button
          className="ai-send-btn"
          onClick={onSend}
          disabled={!input.trim() || !config}
          aria-label={t('ai.sendMsg')}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 16, height: 16 }}>
            <line x1="22" y1="2" x2="11" y2="13" /><polygon points="22 2 15 22 11 13 2 9 22 2" />
          </svg>
        </button>
        )}
      </div>

      {/* Tool logs button */}
      <div style={{ padding: '4px 12px', borderTop: '1px solid var(--border-s)', display: 'flex', justifyContent: 'flex-end' }}>
        <button className="git-btn" style={{ fontSize: 10 }} onClick={async () => {
          try {
            const logs = await mcpToolLogs()
            setToolLogs(logs)
            setShowToolLogs(true)
          } catch { /* skip */ }
        }}>{t('mcp.toolLogs')}</button>
      </div>

      {/* Tool logs panel */}
      {showToolLogs && (
        <div className="shortcuts-backdrop" onClick={() => setShowToolLogs(false)}>
          <div className="shortcuts-modal" role="dialog" aria-modal="true" style={{ minWidth: 560, maxWidth: 640, maxHeight: '70vh', overflow: 'auto' }} onClick={e => e.stopPropagation()}>
            <div className="flex items-center justify-between mb-1">
              <h3 style={{ margin: 0, fontSize: 14 }}>{t('mcp.toolLogs')}</h3>
              <button className="sidebar-action-btn" onClick={() => setShowToolLogs(false)} aria-label={t('ai.close')}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
                  <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </button>
            </div>
            {toolLogs.length === 0 ? (
              <div style={{ fontSize: 12, color: 'var(--text-3)', textAlign: 'center', padding: 20 }}>{t('mcp.toolLog.noLogs')}</div>
            ) : (
              toolLogs.slice().reverse().map((log, i) => (
                <div key={i} style={{ borderBottom: '1px solid var(--border-s)', padding: '8px 0' }}>
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 11 }}>
                    <span style={{ color: 'var(--text-3)' }}>{new Date(log.timestamp * 1000).toLocaleTimeString()}</span>
                    <span style={{ color: 'var(--text-2)', fontWeight: 500 }}>{log.tool_name}</span>
                    <span style={{ color: 'var(--text-3)' }}>{log.server_id}</span>
                    <span style={{ color: log.is_error ? 'var(--red)' : 'var(--text-3)', marginLeft: 'auto' }}>{log.duration_ms}ms</span>
                    <button className="git-btn" style={{ fontSize: 10, padding: '0 4px' }} onClick={() => setExpandedLogIdx(expandedLogIdx === i ? null : i)}>
                      {expandedLogIdx === i ? t('mcp.toolLog.collapse') : t('mcp.toolLog.expand')}
                    </button>
                  </div>
                  {expandedLogIdx === i && (
                    <div style={{ marginTop: 6 }}>
                      <div style={{ fontSize: 10, color: 'var(--text-3)', marginBottom: 2 }}>Arguments:</div>
                      <pre style={{ fontSize: 10, color: 'var(--text-2)', whiteSpace: 'pre-wrap', wordBreak: 'break-all', margin: '0 0 6px', maxHeight: 100, overflow: 'auto', background: 'var(--bg)', padding: 4, borderRadius: 4 }}>{log.arguments}</pre>
                      <div style={{ fontSize: 10, color: 'var(--text-3)', marginBottom: 2 }}>Result:</div>
                      <pre style={{ fontSize: 10, color: log.is_error ? 'var(--red)' : 'var(--text-2)', whiteSpace: 'pre-wrap', wordBreak: 'break-all', margin: 0, maxHeight: 150, overflow: 'auto', background: 'var(--bg)', padding: 4, borderRadius: 4 }}>{log.result}</pre>
                    </div>
                  )}
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </>
  )
}
