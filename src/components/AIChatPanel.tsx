import { useState, useEffect, useRef, useCallback } from 'react'
import { type AiConfig, type ChatMessage, type AiStreamEvent, type MemoryEntry, type PythonSetupProgress, aiLoadConfig, aiChat, aiSaveMemory, aiLoadMemories, aiSaveConfig, ragStats, type RagIndexStats, readFileLines, mcpToolLogs, type McpToolCallLog } from '../lib/tauri'
import { AIModelConfig, PRESETS } from './AIModelConfig'
import { listen } from '@tauri-apps/api/event'
import DOMPurify from 'dompurify'
import { useI18n } from '../lib/i18n'
import { UpgradePrompt } from './UpgradePrompt'

/** Default base prompt — user can customize in AI config */
export const DEFAULT_BASE_PROMPT = `You are Inkess AI assistant. Current working directory: {currentDir}. You can use tools to read files, list directories, search files, fetch web pages, write files, and open files for the user.

Research-first principle:
For any non-trivial question — analysis, technical topics, domain knowledge, methodology, best practices — ALWAYS use web_search first to find authoritative sources, official documentation, industry standards, or proven frameworks BEFORE answering or executing code. After web_search, use fetch_url to read the full content of the most relevant URLs for deeper understanding. Then combine the researched knowledge with the user's local files and data to produce well-grounded, high-quality results. Cite your sources when relevant.

Python execution runs in the current working directory with a 30-second timeout. You can:
- Write scripts/data files to the current directory and read them back in later calls
- Process large files in chunks: read a sample first, then process in batches across multiple run_python calls
- Save intermediate results to files (e.g. CSV, JSON) and aggregate in a final step
- If a script times out, break it into smaller steps and always print intermediate results
- Use all pre-installed libraries: numpy, matplotlib, pandas, scipy, sympy, Pillow, openpyxl, seaborn

File output tools:
- Use write_file to save reports, translations, analysis results, or generated content as files in the workspace
- Use open_file after write_file to show the created file to the user
- For research reports or analysis results with rich formatting, create a well-formatted HTML file with write_file and open it with open_file

Document conversion:
- For format conversion (e.g. Markdown↔HTML, CSV↔JSON↔Excel, text extraction from PDF/DOCX), use read_file to read the source, run_python to convert, and write_file to save the result
- Common conversions: Markdown→HTML (with styling), CSV→Excel (openpyxl), JSON→CSV (pandas), text→Markdown (formatting), batch file renaming/processing
- Always use open_file to show the converted file to the user after saving

Output format rules:
1. When your response contains substantial analysis or conclusions (more than a few paragraphs), use write_file to create a Markdown (.md) or HTML file in the working directory, then open_file to show it. Keep your chat reply brief and point the user to the file
2. If the analysis needs charts, data visualizations, or rich formatting that Markdown cannot express, write a self-contained single-page HTML file instead. Use inline CSS, HTML tables, and inline SVG for charts — do NOT generate image files with matplotlib/Pillow
3. Name output files descriptively (e.g. data_analysis_report.md, sales_dashboard.html)
4. After creating a file with write_file, use open_file to show it to the user
5. For simple questions or short answers, reply directly in chat — no need to create files`

/** Prompt presets for quick switching */
export const PROMPT_PRESETS = [
  {
    id: 'default',
    label: 'Default',
    label_zh: '默认',
    prompt: DEFAULT_BASE_PROMPT,
  },
  {
    id: 'analyst',
    label: 'Data Analyst',
    label_zh: '数据分析师',
    prompt: `You are a senior data analyst assistant. Current working directory: {currentDir}.

Workflow:
1. ALWAYS use web_search first to research analysis methodologies, industry benchmarks, and best practices for the specific domain
2. Read and explore data to understand structure, quality, and key dimensions
3. Apply researched methodology systematically with statistical rigor
4. Present findings with industry context, comparisons, and actionable recommendations

Python libraries available: numpy, matplotlib, pandas, scipy, sympy, Pillow, openpyxl, seaborn

Output: Write analysis reports as Markdown (.md) or self-contained HTML files. Use tables, structured sections, and clear data-backed conclusions. For visualizations, prefer HTML with inline SVG/CSS charts.`,
  },
  {
    id: 'researcher',
    label: 'Research Assistant',
    label_zh: '研究助手',
    prompt: `You are a research assistant. Current working directory: {currentDir}.

For every question:
1. Use web_search to find authoritative sources, academic papers, official documentation, and expert opinions
2. Cross-reference multiple sources for accuracy
3. Synthesize findings into well-structured, cited summaries
4. Clearly distinguish facts from opinions and note any conflicting information

Output: Write research summaries as Markdown (.md) files with source citations. Keep chat replies brief with key takeaways.`,
  },
  {
    id: 'coder',
    label: 'Code Assistant',
    label_zh: '编程助手',
    prompt: `You are a coding assistant. Current working directory: {currentDir}. You can read files, list directories, search files, and execute Python.

When helping with code:
1. Read relevant files first to understand the codebase context
2. Use web_search to check official docs for APIs, libraries, or frameworks involved
3. Provide concise, working code with clear explanations
4. Follow the project's existing coding style and conventions

Python libraries available: numpy, matplotlib, pandas, scipy, sympy, Pillow, openpyxl, seaborn
For simple answers, reply directly in chat.`,
  },
  {
    id: 'minimal',
    label: 'Minimal',
    label_zh: '极简',
    prompt: `You are Inkess AI assistant. Current working directory: {currentDir}. You can use tools to read files, list directories, search files, execute Python, and search the web. Be concise and direct.`,
  },
]

interface RagStatusEvent {
  status: string
  message: string
}

interface ModelProgressEvent {
  stage: string
  progress: number // 0~1 determinate, -1 indeterminate
  downloaded_bytes: number
}

interface UIMessage {
  role: 'user' | 'assistant' | 'tool_call' | 'tool_result' | 'system'
  content: string
  toolName?: string
  toolId?: string
}

interface AIChatPanelProps {
  visible: boolean
  currentDir: string
  onClose: () => void
  onToast: (msg: string) => void
  isPro?: boolean
  onOpenLicense?: () => void
  onOpenFile?: (path: string, line?: number) => void
  busyRef?: React.MutableRefObject<boolean>
}

const HISTORY_KEY = 'inkess-ai-history'
const SESSIONS_KEY = 'inkess-ai-sessions'
const MAX_SESSIONS = 20

interface ChatSession {
  id: string
  title: string
  timestamp: number
  messages: UIMessage[]
}

function loadHistory(): UIMessage[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY)
    if (raw) return JSON.parse(raw)
  } catch { /* ignore */ }
  return []
}

function saveHistory(msgs: UIMessage[]) {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(msgs.slice(-100)))
  } catch { /* ignore */ }
}

function loadSessions(): ChatSession[] {
  try {
    const raw = localStorage.getItem(SESSIONS_KEY)
    if (raw) return JSON.parse(raw)
  } catch { /* ignore */ }
  return []
}

function saveSessions(sessions: ChatSession[]) {
  try {
    localStorage.setItem(SESSIONS_KEY, JSON.stringify(sessions.slice(0, MAX_SESSIONS)))
  } catch { /* ignore */ }
}

export function AIChatPanel({ visible, currentDir, onClose, onToast, isPro = true, onOpenLicense, onOpenFile, busyRef }: AIChatPanelProps) {
  const [messages, setMessages] = useState<UIMessage[]>(loadHistory)
  const [input, setInput] = useState('')
  const [streaming, setStreaming] = useState(false)
  const [config, setConfig] = useState<AiConfig | null>(null)
  const [showConfig, setShowConfig] = useState(false)
  const [sessionId] = useState(() => crypto.randomUUID())
  const [memories, setMemories] = useState<MemoryEntry[]>([])
  const [showModelMenu, setShowModelMenu] = useState(false)
  const [showHistory, setShowHistory] = useState(false)
  const [sessions, setSessions] = useState<ChatSession[]>(loadSessions)
  const [pythonSetup, setPythonSetup] = useState<PythonSetupProgress | null>(null)
  const [deepMode, setDeepMode] = useState(false)
  const [ragStatus, setRagStatus] = useState<RagStatusEvent | null>(null)
  const [ragStatData, setRagStatData] = useState<RagIndexStats | null>(null)
  const [modelProgress, setModelProgress] = useState<ModelProgressEvent | null>(null)
  const [hoverPreview, setHoverPreview] = useState<{ x: number; y: number; content: string; path: string; line: number } | null>(null)
  const [showToolLogs, setShowToolLogs] = useState(false)
  const [toolLogs, setToolLogs] = useState<McpToolCallLog[]>([])
  const [expandedLogIdx, setExpandedLogIdx] = useState<number | null>(null)
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const summarizedRef = useRef(false)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const assistantBufferRef = useRef('')
  const messagesRef = useRef(messages)
  messagesRef.current = messages
  const { t, lang } = useI18n()

  // Sync busy state to parent ref
  useEffect(() => {
    if (busyRef) busyRef.current = streaming
  }, [streaming, busyRef])

  // Cleanup hover timer on unmount
  useEffect(() => {
    return () => {
      if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
    }
  }, [])

  // Resize drag state
  const [panelWidth, setPanelWidth] = useState(() => {
    const saved = localStorage.getItem('inkess-ai-panel-width')
    return saved ? parseInt(saved, 10) : 380
  })
  const draggingRef = useRef(false)
  const rafRef = useRef(0)

  useEffect(() => {
    const onMouseMove = (e: MouseEvent) => {
      if (!draggingRef.current) return
      cancelAnimationFrame(rafRef.current)
      rafRef.current = requestAnimationFrame(() => {
        const w = Math.max(300, Math.min(window.innerWidth * 0.8, window.innerWidth - e.clientX))
        setPanelWidth(w)
      })
    }
    const onMouseUp = () => {
      if (draggingRef.current) {
        draggingRef.current = false
        cancelAnimationFrame(rafRef.current)
        document.body.style.cursor = ''
        document.body.style.userSelect = ''
        localStorage.setItem('inkess-ai-panel-width', String(panelWidth))
      }
    }
    window.addEventListener('mousemove', onMouseMove)
    window.addEventListener('mouseup', onMouseUp)
    return () => {
      window.removeEventListener('mousemove', onMouseMove)
      window.removeEventListener('mouseup', onMouseUp)
      cancelAnimationFrame(rafRef.current)
    }
  }, [panelWidth])

  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    draggingRef.current = true
    document.body.style.cursor = 'col-resize'
    document.body.style.userSelect = 'none'
  }, [])

  // Load config on mount
  useEffect(() => {
    aiLoadConfig().then(cfg => {
      if (cfg) setConfig(cfg)
    }).catch(() => {})
  }, [])

  // Load memories when directory changes
  useEffect(() => {
    if (!currentDir) return
    aiLoadMemories(currentDir).then(setMemories).catch(() => {})
  }, [currentDir])

  // Notify when workspace directory changes while panel is visible
  const prevDirRef = useRef(currentDir)
  useEffect(() => {
    if (prevDirRef.current && currentDir && currentDir !== prevDirRef.current && visible) {
      const dirName = currentDir.split('/').pop() || currentDir
      setMessages(prev => [...prev, {
        role: 'system',
        content: t('ai.workspaceSwitched', { dir: dirName }),
      }])
    }
    prevDirRef.current = currentDir
  }, [currentDir, visible, t])

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // Save history when messages change
  useEffect(() => {
    saveHistory(messages)
  }, [messages])

  // Listen for AI stream events
  useEffect(() => {
    let unlisten: (() => void) | undefined
    let cancelled = false
    listen<AiStreamEvent>('ai-stream', (event) => {
      if (cancelled) return
      const { session_id, event_type, content } = event.payload
      if (session_id !== sessionId) return

      switch (event_type) {
        case 'delta': {
          assistantBufferRef.current += content
          const text = assistantBufferRef.current
          setMessages(prev => {
            const last = prev[prev.length - 1]
            if (last && last.role === 'assistant') {
              return [...prev.slice(0, -1), { ...last, content: text }]
            }
            return [...prev, { role: 'assistant', content: text }]
          })
          break
        }
        case 'tool_call': {
          try {
            const info = JSON.parse(content)
            setMessages(prev => [...prev, {
              role: 'tool_call',
              content: info.arguments,
              toolName: info.name,
              toolId: info.id,
            }])
          } catch { /* ignore */ }
          break
        }
        case 'tool_result': {
          try {
            const info = JSON.parse(content)
            setMessages(prev => [...prev, {
              role: 'tool_result',
              content: info.result,
              toolName: info.name,
              toolId: info.id,
            }])
          } catch { /* ignore */ }
          // Reset buffer for next assistant response
          assistantBufferRef.current = ''
          break
        }
        case 'done': {
          setStreaming(false)
          assistantBufferRef.current = ''
          break
        }
        case 'error': {
          setStreaming(false)
          assistantBufferRef.current = ''
          setMessages(prev => [...prev, { role: 'assistant', content: `⚠ ${content}` }])
          break
        }
      }
    }).then(fn => { unlisten = fn })
    return () => { cancelled = true; unlisten?.() }
  }, [sessionId])

  // Listen for Python setup progress events
  useEffect(() => {
    let unlisten: (() => void) | undefined
    let cancelled = false
    let timer: ReturnType<typeof setTimeout> | undefined
    listen<PythonSetupProgress>('python-setup-progress', (event) => {
      if (cancelled) return
      const { status, progress, message } = event.payload
      if (status === 'done' || status === 'error') {
        setPythonSetup({ status, progress, message })
        timer = setTimeout(() => setPythonSetup(null), status === 'done' ? 2000 : 5000)
      } else {
        setPythonSetup({ status, progress, message })
      }
    }).then(fn => { unlisten = fn })
    return () => { cancelled = true; unlisten?.(); if (timer) clearTimeout(timer) }
  }, [])

  // Listen for RAG status events
  useEffect(() => {
    let unlisten: (() => void) | undefined
    let cancelled = false
    listen<RagStatusEvent>('rag-status', (event) => {
      if (cancelled) return
      setRagStatus(event.payload)
      if (event.payload.status === 'ready') {
        ragStats().then(setRagStatData).catch(() => {})
      }
    }).then(fn => { unlisten = fn })
    return () => { cancelled = true; unlisten?.() }
  }, [])

  // Listen for RAG model download progress
  useEffect(() => {
    let unlisten: (() => void) | undefined
    let cancelled = false
    listen<ModelProgressEvent>('rag-model-progress', (event) => {
      if (cancelled) return
      const { stage, progress, downloaded_bytes } = event.payload
      if (stage === 'ready') {
        setModelProgress(null)
      } else {
        setModelProgress({ stage, progress, downloaded_bytes })
      }
    }).then(fn => { unlisten = fn })
    return () => { cancelled = true; unlisten?.() }
  }, [])

  // Listen for open-file-request from AI tools (open_file tool)
  useEffect(() => {
    let unlisten: (() => void) | undefined
    listen<{ path: string }>('open-file-request', (event) => {
      const { path } = event.payload
      if (path && onOpenFile) onOpenFile(path)
    }).then(fn => { unlisten = fn })
    return () => { unlisten?.() }
  }, [onOpenFile])

  const handleSend = useCallback(async () => {
    const text = input.trim()
    if (!text || streaming) return
    if (!config) {
      setShowConfig(true)
      return
    }

    const userMsg: UIMessage = { role: 'user', content: text }
    setMessages(prev => [...prev, userMsg])
    setInput('')
    setStreaming(true)
    assistantBufferRef.current = ''

    // Build ChatMessage array for the API (only user/assistant messages)
    const basePromptTemplate = config.base_prompt || DEFAULT_BASE_PROMPT
    const baseSysPrompt = basePromptTemplate.replace('{currentDir}', currentDir || 'not set')
    const deepPrompt = deepMode
      ? '\n\n[Deep Analysis Mode]\nYou are now in deep analysis mode. Work like a senior analyst:\n1. Plan your analysis steps first and share the plan with the user\n2. Execute step by step: read data → exploratory analysis → deep computation → structured output\n3. Summarize findings after each step and decide the next direction\n4. If you find anomalies or interesting patterns, proactively dig deeper\n5. Use web_search to find industry benchmarks, methodologies, and reference data\n6. Output a structured report with key findings, data support, and recommendations\nDo not rush to conclusions. Run multiple rounds of analysis to ensure conclusions are data-backed.'
      : ''
    const memoryBlock = memories.length > 0
      ? `\n\n## Memories\n${memories.map(m => m.content).join('\n---\n')}`
      : ''
    const sysContent = [
      config.system_prompt,
      baseSysPrompt,
      deepPrompt,
      memoryBlock,
    ].filter(Boolean).join('\n\n')
    const apiMessages: ChatMessage[] = [
      { role: 'system', content: sysContent },
    ]
    // Include recent conversation context
    const recent = [...messagesRef.current, userMsg]
    for (const m of recent.slice(-20)) {
      if (m.role === 'user' || m.role === 'assistant') {
        apiMessages.push({ role: m.role, content: m.content })
      }
    }

    try {
      await aiChat(sessionId, apiMessages, config, deepMode, currentDir || undefined)
      // Auto-summarize after 20 user/assistant messages
      const convMsgs = [...messagesRef.current, userMsg].filter(m => m.role === 'user' || m.role === 'assistant')
      if (convMsgs.length >= 20 && !summarizedRef.current && currentDir) {
        summarizedRef.current = true
        triggerSummarize(config, convMsgs, currentDir, sessionId).then(summary => {
          if (summary) {
            aiSaveMemory(currentDir, summary).catch(() => {})
            aiLoadMemories(currentDir).then(setMemories).catch(() => {})
          }
        })
      }
    } catch (e) {
      setStreaming(false)
      onToast(typeof e === 'string' ? e : t('ai.requestFailed'))
    }
  }, [input, streaming, config, currentDir, sessionId, onToast, memories, deepMode])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSend()
    }
  }

  const handleClear = () => {
    // Archive current conversation if it has user messages
    if (messages.some(m => m.role === 'user')) {
      const firstUserMsg = messages.find(m => m.role === 'user')
      const title = firstUserMsg ? firstUserMsg.content.slice(0, 50) : 'Untitled'
      const session: ChatSession = {
        id: crypto.randomUUID(),
        title,
        timestamp: Date.now(),
        messages: [...messages],
      }
      setSessions(prev => {
        const updated = [session, ...prev].slice(0, MAX_SESSIONS)
        saveSessions(updated)
        return updated
      })
    }
    setMessages([])
    summarizedRef.current = false
    localStorage.removeItem(HISTORY_KEY)
  }

  return (
    <>
      <div className={`ai-panel ${visible ? 'ai-panel-open' : ''}`} style={{ width: panelWidth }}>
        {/* Resize handle */}
        <div className="ai-panel-resize" onMouseDown={handleResizeStart} />
        {/* Header */}
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
          {memories.length > 0 && (
            <span style={{ fontSize: 10, color: 'var(--text-3)', marginLeft: 6 }} title={t('ai.memories', { n: memories.length })}>
              {t('ai.memories', { n: memories.length })}
            </span>
          )}
          {ragStatus && (
            <span
              style={{ fontSize: 10, marginLeft: 6, display: 'inline-flex', alignItems: 'center', gap: 3, cursor: 'default' }}
              title={
                ragStatus.status === 'ready' && ragStatData
                  ? `${t('rag.tooltip.files', { n: ragStatData.file_count })}\n${t('rag.tooltip.chunks', { n: ragStatData.chunk_count })}\n${t('rag.tooltip.size', { size: ragStatData.db_size_bytes < 1024 * 1024 ? (ragStatData.db_size_bytes / 1024).toFixed(1) + ' KB' : (ragStatData.db_size_bytes / (1024 * 1024)).toFixed(1) + ' MB' })}`
                  : ragStatus.status === 'indexing'
                    ? t('rag.tooltip.indexing')
                    : ragStatus.message
              }
            >
              <span style={{
                width: 6, height: 6, borderRadius: '50%', display: 'inline-block',
                background: ragStatus.status === 'ready' ? '#22c55e' : ragStatus.status === 'indexing' ? '#f59e0b' : 'var(--text-3)',
                animation: ragStatus.status === 'indexing' ? 'pulse 1.5s infinite' : 'none',
              }} />
              <span style={{ color: 'var(--text-3)' }}>
                {ragStatus.status === 'ready' ? 'KB' : ragStatus.status === 'indexing' ? t('rag.status.indexing') : ''}
              </span>
            </span>
          )}
          <div style={{ flex: 1 }} />
          {messages.length > 0 && (
            <>
            <button className="sidebar-action-btn" onClick={() => {
              const md = messages
                .filter(m => m.role === 'user' || m.role === 'assistant')
                .map(m => `**${m.role === 'user' ? 'You' : 'AI'}:**\n${m.content}`)
                .join('\n\n---\n\n')
              navigator.clipboard.writeText(md).then(() => onToast(t('ai.copiedChat'))).catch(() => onToast('Copy failed'))
            }} title={t('ai.copyChat')} aria-label={t('ai.copyChat')}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
              </svg>
            </button>
            <button className="sidebar-action-btn" onClick={handleClear} title={t('ai.newChat')} aria-label={t('ai.newChat')}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
                <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /><line x1="12" y1="11" x2="12" y2="17" /><line x1="9" y1="14" x2="15" y2="14" />
              </svg>
            </button>
            </>
          )}
          <button className="sidebar-action-btn" onClick={() => setShowHistory(true)} title={t('ai.chatHistory')} aria-label={t('ai.chatHistory')}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" />
            </svg>
          </button>
          <button className="sidebar-action-btn" onClick={handleClear} title={t('ai.clearChat')} aria-label={t('ai.clearChat')}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
              <polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" />
            </svg>
          </button>
          <button className="sidebar-action-btn" onClick={() => setShowConfig(true)} title={t('ai.modelSettings')} aria-label={t('ai.modelSettings')}>
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

        {/* Model download progress bar */}
        {modelProgress && (modelProgress.stage === 'downloading_model' || modelProgress.stage === 'downloading_tokenizer' || modelProgress.stage === 'loading') && (
          <div style={{ padding: '4px 12px' }}>
            <div style={{ fontSize: 10, color: 'var(--text-3)', marginBottom: 2 }}>
              {modelProgress.stage === 'loading'
                ? t('ai.loadingModel')
                : modelProgress.stage === 'downloading_model' ? t('ai.downloadingModel') : t('ai.downloadingTokenizer')}
              {modelProgress.progress >= 0
                ? ` ${Math.round(modelProgress.progress * 100)}%`
                : ` ${(modelProgress.downloaded_bytes / 1024 / 1024).toFixed(1)} MB`}
            </div>
            <div style={{ width: '100%', height: 3, borderRadius: 2, background: 'var(--border-s)', overflow: 'hidden' }}>
              {modelProgress.progress >= 0 ? (
                <div style={{
                  width: `${Math.round(modelProgress.progress * 100)}%`,
                  height: '100%',
                  borderRadius: 2,
                  background: 'var(--accent, #3b82f6)',
                  transition: 'width 0.3s ease',
                }} />
              ) : (
                <div style={{
                  width: '30%',
                  height: '100%',
                  borderRadius: 2,
                  background: 'var(--accent, #3b82f6)',
                  animation: 'indeterminate-bar 1.5s ease-in-out infinite',
                }} />
              )}
            </div>
          </div>
        )}

        {!isPro ? (
          <UpgradePrompt feature="ai" onOpenLicense={() => onOpenLicense?.()} />
        ) : (
        <>
        {/* Messages */}
        <div className="ai-messages">
          {messages.length === 0 && (
            <div style={{ textAlign: 'center', padding: '40px 20px', color: 'var(--text-3)', fontSize: 13 }}>
              {config ? t('ai.startChat') : (
                <button className="git-btn git-btn-primary" style={{ fontSize: 12 }} onClick={() => setShowConfig(true)}>
                  {t('ai.configModel')}
                </button>
              )}
            </div>
          )}
          {messages.map((msg, i) => {
            // Format MCP tool names: mcp__{serverid}__{toolname} → serverid / toolname
            const displayToolName = msg.toolName?.startsWith('mcp__')
              ? msg.toolName.slice(5).replace('__', ' / ')
              : msg.toolName
            return (
            <div key={i} className={`ai-msg ai-msg-${msg.role}`}>
              {msg.role === 'tool_call' ? (
                <div className="ai-msg-tool">
                  <div style={{ fontSize: 11, color: 'var(--text-3)', marginBottom: 2 }}>
                    {t('ai.toolCall')} <span style={{ fontWeight: 600 }}>{displayToolName}</span>
                  </div>
                  {msg.toolName === 'run_python' ? (
                    <PythonCodeBlock content={msg.content} />
                  ) : (
                    <CollapsibleBlock text={msg.content} maxLines={3} />
                  )}
                </div>
              ) : msg.role === 'tool_result' ? (
                <div className="ai-msg-tool">
                  <div style={{ fontSize: 11, color: 'var(--text-3)', marginBottom: 2 }}>
                    {t('ai.toolResult', { name: displayToolName || '' })}
                  </div>
                  <CollapsibleBlock text={msg.content.length > 2000 ? msg.content.slice(0, 2000) + '...' : msg.content} maxLines={5} />
                  {msg.toolName === 'write_file' && msg.content.startsWith('File written:') && (() => {
                    const match = msg.content.match(/^File written: (.+?) \(/)
                    const filePath = match?.[1]
                    if (!filePath) return null
                    const ext = filePath.split('.').pop()?.toLowerCase()
                    const viewable = ['html', 'htm', 'md', 'txt', 'json', 'csv'].includes(ext || '')
                    if (!viewable) return null
                    return (
                      <button
                        className="git-btn"
                        style={{ fontSize: 10, marginTop: 4, padding: '2px 8px' }}
                        onClick={() => onOpenFile?.(filePath)}
                      >
                        {t('ai.openPreview')}
                      </button>
                    )
                  })()}
                </div>
              ) : msg.role === 'system' ? (
                <div style={{ textAlign: 'center', fontSize: 11, color: 'var(--text-3)', padding: '4px 0' }}>
                  {msg.content}
                </div>
              ) : (
                <div className={msg.role === 'user' ? 'ai-bubble ai-bubble-user' : 'ai-bubble ai-bubble-assistant'}>
                  <MessageContent text={msg.content} onOpenFile={onOpenFile} currentDir={currentDir} hoverTimerRef={hoverTimerRef} setHoverPreview={setHoverPreview} />
                </div>
              )}
            </div>
            )
          })}
          {streaming && messages[messages.length - 1]?.role !== 'assistant' && (
            <div className="ai-msg ai-msg-assistant">
              <div className="ai-bubble ai-bubble-assistant">
                <span className="ai-typing-indicator" />
              </div>
            </div>
          )}
          <div ref={messagesEndRef} />
        </div>

        {/* Python setup progress bar — fixed between messages and input */}
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

        {/* Input */}
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
            {t('ai.currentWorkspace', { dir: currentDir.split('/').pop() || currentDir })}
          </div>
        )}
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
            onKeyDown={handleKeyDown}
            placeholder={config ? t('ai.inputPlaceholder') : t('ai.configFirst')}
            rows={1}
            disabled={!config}
            aria-label={t('ai.title')}
          />
          <button
            className="ai-send-btn"
            onClick={handleSend}
            disabled={streaming || !input.trim() || !config}
            aria-label={t('ai.sendMsg')}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 16, height: 16 }}>
              <line x1="22" y1="2" x2="11" y2="13" /><polygon points="22 2 15 22 11 13 2 9 22 2" />
            </svg>
          </button>
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
        </>
        )}
      </div>

      {/* Hover preview tooltip */}
      {hoverPreview && (
        <div
          className="ai-code-tooltip"
          style={{
            position: 'fixed',
            left: Math.min(hoverPreview.x, window.innerWidth - 420),
            top: Math.min(hoverPreview.y, window.innerHeight - 220),
          }}
          onMouseEnter={() => { /* keep visible */ }}
          onMouseLeave={() => setHoverPreview(null)}
        >
          <div style={{ fontSize: 10, color: 'var(--text-3)', marginBottom: 4 }}>{hoverPreview.path}:{hoverPreview.line}</div>
          <pre style={{ margin: 0, fontSize: 11, lineHeight: 1.5, whiteSpace: 'pre', overflow: 'auto' }}>{hoverPreview.content}</pre>
        </div>
      )}

      {/* Tool logs panel */}
      {showToolLogs && (
        <div className="shortcuts-backdrop" onClick={() => setShowToolLogs(false)}>
          <div className="shortcuts-modal" style={{ minWidth: 560, maxWidth: 640, maxHeight: '70vh', overflow: 'auto' }} onClick={e => e.stopPropagation()}>
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

      {showHistory && (
        <div className="shortcuts-backdrop" onClick={() => setShowHistory(false)}>
          <div className="shortcuts-modal" style={{ minWidth: 560, maxWidth: 640, maxHeight: '70vh', overflow: 'auto' }} onClick={e => e.stopPropagation()}>
            <div className="flex items-center justify-between mb-1">
              <h3 style={{ margin: 0, fontSize: 14 }}>{t('ai.chatHistory')}</h3>
              <button className="sidebar-action-btn" onClick={() => setShowHistory(false)} aria-label={t('ai.close')}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 14, height: 14 }}>
                  <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </button>
            </div>
            {sessions.length === 0 ? (
              <div style={{ fontSize: 12, color: 'var(--text-3)', textAlign: 'center', padding: 20 }}>{t('ai.noHistory')}</div>
            ) : (
              sessions.map((s) => (
                <div key={s.id} style={{ borderBottom: '1px solid var(--border-s)', padding: '8px 0', display: 'flex', alignItems: 'center', gap: 8 }}>
                  <div
                    style={{ flex: 1, cursor: 'pointer', minWidth: 0 }}
                    onClick={() => {
                      setMessages(s.messages)
                      saveHistory(s.messages)
                      summarizedRef.current = false
                      setShowHistory(false)
                    }}
                  >
                    <div style={{ fontSize: 12, color: 'var(--text-1)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{s.title}</div>
                    <div style={{ fontSize: 10, color: 'var(--text-3)', marginTop: 2 }}>
                      {new Date(s.timestamp).toLocaleString()} · {s.messages.filter(m => m.role === 'user').length} {t('ai.historyMessages')}
                    </div>
                  </div>
                  <button
                    className="sidebar-action-btn"
                    title={t('ai.deleteSession')}
                    onClick={(e) => {
                      e.stopPropagation()
                      const updated = sessions.filter(x => x.id !== s.id)
                      setSessions(updated)
                      saveSessions(updated)
                    }}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ width: 12, height: 12 }}>
                      <polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" />
                    </svg>
                  </button>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {showConfig && (
        <AIModelConfig
          config={config}
          onSave={setConfig}
          onClose={() => setShowConfig(false)}
          onToast={onToast}
        />
      )}
    </>
  )
}

/** Collapsible text block for tool call/result — collapses long content */
function CollapsibleBlock({ text, maxLines = 3, label }: { text: string; maxLines?: number; label?: string }) {
  const [expanded, setExpanded] = useState(false)
  const lines = text.split('\n')
  const needsCollapse = lines.length > maxLines + 2
  const displayText = needsCollapse && !expanded ? lines.slice(0, maxLines).join('\n') : text

  return (
    <div>
      {label && <div style={{ fontSize: 10, color: 'var(--text-3)', marginBottom: 2 }}>{label}</div>}
      <pre style={{ fontSize: 11, color: 'var(--text-2)', whiteSpace: 'pre-wrap', wordBreak: 'break-all', margin: 0 }}>
        {displayText}
        {needsCollapse && !expanded && '\n...'}
      </pre>
      {needsCollapse && (
        <button
          onClick={() => setExpanded(v => !v)}
          style={{ fontSize: 10, color: 'var(--accent)', background: 'none', border: 'none', cursor: 'pointer', padding: '2px 0', marginTop: 2 }}
        >
          {expanded ? '▲ Collapse' : `▼ Show all (${lines.length} lines)`}
        </button>
      )}
    </div>
  )
}

/** Specialized block for run_python tool calls — collapsed by default with code styling */
function PythonCodeBlock({ content }: { content: string }) {
  const [expanded, setExpanded] = useState(false)

  let code = content
  try {
    const parsed = JSON.parse(content)
    if (parsed.code) code = parsed.code
  } catch { /* use raw content */ }

  const lines = code.split('\n')
  // Extract summary from first comment line
  const commentLine = lines.find(l => /^\s*#[^!]/.test(l))
  const summary = commentLine
    ? commentLine.trim().replace(/^#+\s*/, '')
    : lines.find(l => l.trim() && !l.trim().startsWith('import') && !l.trim().startsWith('from') && !l.trim().startsWith('#'))?.trim().slice(0, 60)
      || 'Python code'

  return (
    <div>
      <button
        onClick={() => setExpanded(v => !v)}
        style={{
          display: 'flex', alignItems: 'center', gap: 6, width: '100%',
          fontSize: 11, color: 'var(--text-2)', background: 'none', border: 'none',
          cursor: 'pointer', padding: '2px 0', textAlign: 'left',
        }}
      >
        <span style={{ flexShrink: 0 }}>{expanded ? '▼' : '▶'}</span>
        <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {summary}
        </span>
        <span style={{ flexShrink: 0, fontSize: 10, color: 'var(--text-3)' }}>
          {lines.length} lines
        </span>
      </button>
      {expanded && (
        <pre className="ai-code-block" style={{ marginTop: 4 }}>
          <code>{code}</code>
        </pre>
      )}
    </div>
  )
}

// Simple markdown-like rendering for AI messages
function MessageContent({ text, onOpenFile, currentDir, hoverTimerRef, setHoverPreview }: {
  text: string
  onOpenFile?: (path: string, line?: number) => void
  currentDir?: string
  hoverTimerRef: React.MutableRefObject<ReturnType<typeof setTimeout> | null>
  setHoverPreview: (v: { x: number; y: number; content: string; path: string; line: number } | null) => void
}) {
  // Split by code blocks
  const parts = text.split(/(```[\s\S]*?```)/g)
  return (
    <>
      {parts.map((part, i) => {
        if (part.startsWith('```') && part.endsWith('```')) {
          const inner = part.slice(3, -3)
          const newline = inner.indexOf('\n')
          const code = newline >= 0 ? inner.slice(newline + 1) : inner
          return (
            <pre key={i} className="ai-code-block">
              <code>{code}</code>
            </pre>
          )
        }
        // Bold + inline code
        const html = part
          .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
          .replace(/`([^`]+)`/g, '<code class="ai-inline-code">$1</code>')
          // File references: path/to/file.ext:42 or C:\path\file.ext:42-50
          .replace(
            /(?<![`<\w])(?:[a-zA-Z]:[/\\])?([a-zA-Z0-9_./\\-]+\.[a-zA-Z0-9]+):(\d+)(?:-(\d+))?(?![`>\w])/g,
            (_match, filePath, lineStart, _lineEnd) => {
              const line = parseInt(lineStart, 10)
              return `<a class="ai-file-link" data-path="${DOMPurify.sanitize(filePath)}" data-line="${line}" title="${filePath}:${lineStart}${_lineEnd ? '-' + _lineEnd : ''}">${filePath}:${lineStart}${_lineEnd ? '-' + _lineEnd : ''}</a>`
            }
          )
        return (
          <span
            key={i}
            dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(html, { ADD_ATTR: ['data-path', 'data-line'] }) }}
            onClick={(e) => {
              const target = e.target as HTMLElement
              if (target.classList.contains('ai-file-link')) {
                const path = target.getAttribute('data-path')
                const line = parseInt(target.getAttribute('data-line') || '0', 10)
                if (path && onOpenFile) onOpenFile(path, line || undefined)
              }
            }}
            onMouseOver={(e) => {
              const target = e.target as HTMLElement
              if (target.classList.contains('ai-file-link')) {
                const path = target.getAttribute('data-path')
                const line = parseInt(target.getAttribute('data-line') || '0', 10)
                if (path && line > 0) {
                  if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
                  hoverTimerRef.current = setTimeout(async () => {
                    try {
                      const fullPath = currentDir ? `${currentDir}/${path}` : path
                      const content = await readFileLines(fullPath, line, 3)
                      const rect = target.getBoundingClientRect()
                      setHoverPreview({ x: rect.left, y: rect.bottom + 4, content, path, line })
                    } catch { /* skip */ }
                  }, 200)
                }
              }
            }}
            onMouseOut={(e) => {
              const target = e.target as HTMLElement
              if (target.classList.contains('ai-file-link')) {
                if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current)
                setHoverPreview(null)
              }
            }}
          />
        )
      })}
    </>
  )
}

// Auto-summarize conversation via LLM (non-streaming, fire-and-forget)
async function triggerSummarize(
  config: AiConfig,
  msgs: UIMessage[],
  dir: string,
  _sessionId: string,
): Promise<string | null> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const conversation = msgs.slice(-20).map(m => `${m.role}: ${m.content}`).join('\n')
    const body = {
      model: config.model,
      messages: [
        { role: 'system', content: 'You are a conversation summarizer. Summarize the following conversation into concise bullet points, preserving key information (project structure, user preferences, important decisions), no more than 300 words.' },
        { role: 'user', content: conversation },
      ],
      temperature: 0.3,
      max_tokens: 512,
    }
    const url = `${config.api_url.replace(/\/$/, '')}/chat/completions`
    // Use a direct fetch since we don't need streaming for summarization
    const resp = await fetch(url, {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${config.api_key}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(body),
    })
    if (!resp.ok) return null
    const data = await resp.json()
    return data.choices?.[0]?.message?.content || null
  } catch {
    return null
  }
}
