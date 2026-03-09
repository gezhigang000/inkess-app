import { useState, useEffect, useRef, useCallback } from 'react'
import { type AiConfig, type ChatMessage, type AiStreamEvent, type MemoryEntry, type PythonSetupProgress, type SkillChangedEvent, aiLoadConfig, aiChat, aiSaveMemory, aiLoadMemories, aiSaveConfig } from '../lib/tauri'
import { AIModelConfig } from './AIModelConfig'
import { listen } from '@tauri-apps/api/event'
import { useI18n } from '../lib/i18n'
import { UpgradePrompt } from './UpgradePrompt'
import { AIChatHeader } from './AIChatHeader'
import { AIChatMessages, type UIMessage } from './AIChatMessages'
import { AIChatInput } from './AIChatInput'

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

// Auto-compact: summarize older messages when conversation gets long
const COMPACT_MSG_THRESHOLD = 30
const COMPACT_KEEP_RECENT = 10

async function compactHistory(
  config: AiConfig,
  msgs: UIMessage[],
): Promise<{ summary: string; kept: UIMessage[] } | null> {
  const userAssistant = msgs.filter(m => m.role === 'user' || m.role === 'assistant')
  if (userAssistant.length <= COMPACT_MSG_THRESHOLD) return null

  const oldMsgs = userAssistant.slice(0, -COMPACT_KEEP_RECENT)
  const recentMsgs = userAssistant.slice(-COMPACT_KEEP_RECENT)

  // Cap conversation string to ~8KB to avoid token overflow
  let totalLen = 0
  const cappedOld: string[] = []
  for (const m of oldMsgs) {
    const line = `[${m.role}]: ${m.content.slice(0, 500)}`
    if (totalLen + line.length > 8192) break
    cappedOld.push(line)
    totalLen += line.length
  }
  const conversation = cappedOld.join('\n\n')
  if (!conversation) return null

  const body = {
    model: config.model,
    messages: [
      { role: 'system', content: 'Summarize this conversation history concisely, preserving: 1) User goals and tasks 2) Key decisions made 3) Important code/file references 4) Problems encountered and solutions. Output a structured summary, max 500 words.' },
      { role: 'user', content: conversation },
    ],
    temperature: 0.3,
    max_tokens: 800,
  }

  const url = `${config.api_url.replace(/\/$/, '')}/chat/completions`
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), 10_000) // 10s timeout
  try {
    const resp = await fetch(url, {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${config.api_key}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(body),
      signal: controller.signal,
    })
    clearTimeout(timer)
    if (!resp.ok) return null
    const data = await resp.json()
    const summary = data.choices?.[0]?.message?.content
    if (!summary) return null
    return { summary, kept: recentMsgs }
  } catch {
    clearTimeout(timer)
    return null
  }
}

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
  const [searchQuery, setSearchQuery] = useState('')
  const [searchOpen, setSearchOpen] = useState(false)
  const [activeSkill, setActiveSkill] = useState('Default')
  const [activeSkillId, setActiveSkillId] = useState('default')
  const summarizedRef = useRef(false)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLTextAreaElement>(null)
  const assistantBufferRef = useRef('')
  const messagesRef = useRef(messages)
  messagesRef.current = messages
  const { t } = useI18n()

  // Sync busy state to parent ref
  useEffect(() => {
    if (busyRef) busyRef.current = streaming
  }, [streaming, busyRef])

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
      const dirName = currentDir.replace(/\\/g, '/').split('/').pop() || currentDir
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

  // Listen for skill-changed events
  useEffect(() => {
    let unlisten: (() => void) | undefined
    listen<SkillChangedEvent>('skill-changed', (event) => {
      const { session_id, skill_id, skill_name } = event.payload
      if (session_id !== sessionId) return

      setActiveSkillId(skill_id)
      setActiveSkill(skill_name)
      onToast(t('ai.skillChanged', { skill: skill_name }))
    }).then(fn => { unlisten = fn })
    return () => { unlisten?.() }
  }, [sessionId, onToast, t])

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
    // Auto-compact: summarize old messages when conversation is long
    const allMsgs = [...messagesRef.current, userMsg]
    const convMsgs = allMsgs.filter(m => m.role === 'user' || m.role === 'assistant')
    let contextSummary = ''

    if (convMsgs.length > COMPACT_MSG_THRESHOLD) {
      const compact = await compactHistory(config, allMsgs)
      if (compact) {
        contextSummary = `\n\n[Conversation History Summary]\n${compact.summary}\n[End Summary — Recent messages follow]`
      }
    }

    const apiMessages: ChatMessage[] = [
      { role: 'system', content: sysContent + contextSummary },
    ]
    // If compacted, keep last COMPACT_KEEP_RECENT user/assistant msgs; otherwise last 20
    const keepCount = contextSummary ? COMPACT_KEEP_RECENT : 20
    const recentUA = allMsgs
      .filter(m => m.role === 'user' || m.role === 'assistant')
      .slice(-keepCount)
    for (const m of recentUA) {
      apiMessages.push({ role: m.role, content: m.content })
    }

    try {
      await aiChat(sessionId, apiMessages, config, deepMode, currentDir || undefined, activeSkillId)
      // Auto-summarize after 20 user/assistant messages
      const sumMsgs = [...messagesRef.current, userMsg].filter(m => m.role === 'user' || m.role === 'assistant')
      if (sumMsgs.length >= 20 && !summarizedRef.current && currentDir) {
        summarizedRef.current = true
        triggerSummarize(config, sumMsgs, currentDir, sessionId).then(summary => {
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
    setActiveSkill('Default')
    setActiveSkillId('default')
    summarizedRef.current = false
    localStorage.removeItem(HISTORY_KEY)
  }

  const handleCopyChat = () => {
    const md = messages
      .filter(m => m.role === 'user' || m.role === 'assistant')
      .map(m => `**${m.role === 'user' ? 'You' : 'AI'}:**\n${m.content}`)
      .join('\n\n---\n\n')
    navigator.clipboard.writeText(md).then(() => onToast(t('ai.copiedChat'))).catch(() => onToast('Copy failed'))
  }

  return (
    <>
      <div className={`ai-panel ${visible ? 'ai-panel-open' : ''}`} style={{ width: panelWidth }} onKeyDown={e => {
        if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
          e.preventDefault()
          e.stopPropagation()
          setSearchOpen(v => !v)
        }
      }}>
        {/* Resize handle */}
        <div className="ai-panel-resize" onMouseDown={handleResizeStart} />

        <AIChatHeader
          config={config}
          showModelMenu={showModelMenu}
          setShowModelMenu={setShowModelMenu}
          setConfig={setConfig}
          activeSkill={activeSkill}
          memories={memories}
          messages={messages}
          streaming={streaming}
          onCopyChat={handleCopyChat}
          onClear={handleClear}
          onShowHistory={() => setShowHistory(true)}
          onShowConfig={() => setShowConfig(true)}
          onClose={onClose}
        />

        {!isPro ? (
          <UpgradePrompt feature="ai" onOpenLicense={() => onOpenLicense?.()} />
        ) : (
        <>
        <AIChatMessages
          messages={messages}
          streaming={streaming}
          config={config}
          searchOpen={searchOpen}
          searchQuery={searchQuery}
          setSearchOpen={setSearchOpen}
          setSearchQuery={setSearchQuery}
          onOpenFile={onOpenFile}
          onShowConfig={() => setShowConfig(true)}
          currentDir={currentDir}
          messagesEndRef={messagesEndRef}
        />

        <AIChatInput
          config={config}
          input={input}
          setInput={setInput}
          streaming={streaming}
          deepMode={deepMode}
          setDeepMode={setDeepMode}
          currentDir={currentDir}
          sessionId={sessionId}
          pythonSetup={pythonSetup}
          inputRef={inputRef}
          onSend={handleSend}
          onKeyDown={handleKeyDown}
          onToast={onToast}
        />
        </>
        )}
      </div>

      {showHistory && (
        <div className="shortcuts-backdrop" onClick={() => setShowHistory(false)}>
          <div className="shortcuts-modal" role="dialog" aria-modal="true" style={{ minWidth: 560, maxWidth: 640, maxHeight: '70vh', overflow: 'auto' }} onClick={e => e.stopPropagation()}>
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

// Auto-summarize conversation via LLM (non-streaming, fire-and-forget)
async function triggerSummarize(
  config: AiConfig,
  msgs: UIMessage[],
  dir: string,
  _sessionId: string,
): Promise<string | null> {
  try {
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
