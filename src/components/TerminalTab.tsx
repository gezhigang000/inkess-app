import { useEffect, useRef } from 'react'
import { Terminal, type ITheme } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import { listen } from '@tauri-apps/api/event'
import { ptySpawn, ptyWrite, ptyResize, ptyKill } from '../lib/tauri'
import '@xterm/xterm/css/xterm.css'

// Eye-friendly terminal color scheme presets
export interface TerminalScheme {
  id: string
  name: string
  theme: ITheme
}

const defaultLight: ITheme = {
  background: '#f8f8f7', foreground: '#1c1917', cursor: '#1c1917',
  selectionBackground: 'rgba(37,99,235,0.15)',
  black: '#1c1917', red: '#dc2626', green: '#16a34a', yellow: '#ca8a04',
  blue: '#2563eb', magenta: '#9333ea', cyan: '#0891b2', white: '#e7e5e4',
  brightBlack: '#78716c', brightRed: '#ef4444', brightGreen: '#22c55e', brightYellow: '#eab308',
  brightBlue: '#3b82f6', brightMagenta: '#a855f7', brightCyan: '#06b6d4', brightWhite: '#fafaf9',
}
const defaultDark: ITheme = {
  background: '#1a1918', foreground: '#e7e5e4', cursor: '#e7e5e4',
  selectionBackground: 'rgba(96,165,250,0.3)',
  black: '#1c1917', red: '#f87171', green: '#4ade80', yellow: '#facc15',
  blue: '#60a5fa', magenta: '#c084fc', cyan: '#22d3ee', white: '#e7e5e4',
  brightBlack: '#78716c', brightRed: '#fca5a5', brightGreen: '#86efac', brightYellow: '#fde047',
  brightBlue: '#93bbfd', brightMagenta: '#d8b4fe', brightCyan: '#67e8f9', brightWhite: '#fafaf9',
}

export const TERMINAL_SCHEMES: TerminalScheme[] = [
  { id: 'default', name: 'Default', theme: defaultDark }, // placeholder, resolved at runtime
  {
    id: 'solarized-dark', name: 'Solarized Dark',
    theme: {
      background: '#002b36', foreground: '#839496', cursor: '#93a1a1',
      selectionBackground: 'rgba(147,161,161,0.2)',
      black: '#073642', red: '#dc322f', green: '#859900', yellow: '#b58900',
      blue: '#268bd2', magenta: '#d33682', cyan: '#2aa198', white: '#eee8d5',
      brightBlack: '#586e75', brightRed: '#cb4b16', brightGreen: '#586e75', brightYellow: '#657b83',
      brightBlue: '#839496', brightMagenta: '#6c71c4', brightCyan: '#93a1a1', brightWhite: '#fdf6e3',
    },
  },
  {
    id: 'nord', name: 'Nord',
    theme: {
      background: '#2e3440', foreground: '#d8dee9', cursor: '#d8dee9',
      selectionBackground: 'rgba(136,192,208,0.2)',
      black: '#3b4252', red: '#bf616a', green: '#a3be8c', yellow: '#ebcb8b',
      blue: '#81a1c1', magenta: '#b48ead', cyan: '#88c0d0', white: '#e5e9f0',
      brightBlack: '#4c566a', brightRed: '#bf616a', brightGreen: '#a3be8c', brightYellow: '#ebcb8b',
      brightBlue: '#81a1c1', brightMagenta: '#b48ead', brightCyan: '#8fbcbb', brightWhite: '#eceff4',
    },
  },
  {
    id: 'catppuccin', name: 'Catppuccin',
    theme: {
      background: '#1e1e2e', foreground: '#cdd6f4', cursor: '#f5e0dc',
      selectionBackground: 'rgba(137,180,250,0.2)',
      black: '#45475a', red: '#f38ba8', green: '#a6e3a1', yellow: '#f9e2af',
      blue: '#89b4fa', magenta: '#f5c2e7', cyan: '#94e2d5', white: '#bac2de',
      brightBlack: '#585b70', brightRed: '#f38ba8', brightGreen: '#a6e3a1', brightYellow: '#f9e2af',
      brightBlue: '#89b4fa', brightMagenta: '#f5c2e7', brightCyan: '#94e2d5', brightWhite: '#a6adc8',
    },
  },
  {
    id: 'rose-pine', name: 'Rosé Pine',
    theme: {
      background: '#191724', foreground: '#e0def4', cursor: '#56526e',
      selectionBackground: 'rgba(110,106,134,0.3)',
      black: '#26233a', red: '#eb6f92', green: '#9ccfd8', yellow: '#f6c177',
      blue: '#31748f', magenta: '#c4a7e7', cyan: '#ebbcba', white: '#e0def4',
      brightBlack: '#6e6a86', brightRed: '#eb6f92', brightGreen: '#9ccfd8', brightYellow: '#f6c177',
      brightBlue: '#31748f', brightMagenta: '#c4a7e7', brightCyan: '#ebbcba', brightWhite: '#e0def4',
    },
  },
]

export function resolveSchemeTheme(schemeId: string): ITheme {
  if (schemeId === 'default') {
    const isDark = document.documentElement.getAttribute('data-theme') === 'dark'
    return isDark ? defaultDark : defaultLight
  }
  return TERMINAL_SCHEMES.find(s => s.id === schemeId)?.theme ?? defaultDark
}

interface TerminalTabProps {
  sessionId: string
  cwd: string
  active: boolean
  envVars?: Array<{ key: string; value: string }>
  schemeId?: string
  onClose?: () => void
}

export function TerminalTab({ sessionId, cwd, active, envVars, schemeId = 'default', onClose }: TerminalTabProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitRef = useRef<FitAddon | null>(null)

  useEffect(() => {
    if (!containerRef.current) return

    const theme = resolveSchemeTheme(schemeId)
    const term = new Terminal({
      fontSize: 13,
      fontFamily: "'JetBrains Mono', 'Menlo', 'PingFang SC', 'Microsoft YaHei', 'Noto Sans CJK SC', monospace",
      cursorBlink: true,
      theme,
      allowProposedApi: true,
    })

    const fitAddon = new FitAddon()
    term.loadAddon(fitAddon)
    term.loadAddon(new WebLinksAddon())
    term.open(containerRef.current)

    fitAddon.fit()

    termRef.current = term
    fitRef.current = fitAddon

    // Spawn PTY
    ptySpawn(cwd, sessionId, envVars).then(() => {
      // After shell starts and sources RC files, re-inject env vars via stdin
      // to override any top-level exports in .zshrc/.bashrc
      if (envVars && envVars.length > 0) {
        const encoder = new TextEncoder()
        const exports = envVars.map(v => `export ${v.key}="${v.value.replace(/"/g, '\\"')}"`).join(' && ')
        const cmd = ` ${exports} && clear\n` // leading space to avoid shell history
        setTimeout(() => {
          ptyWrite(sessionId, Array.from(encoder.encode(cmd))).catch(() => {})
        }, 300)
      }
    }).catch((e) => {
      term.writeln(`\r\n\x1b[31mTerminal start failed: ${e}\x1b[0m`)
    })

    // Listen for PTY data
    let unlistenData: (() => void) | undefined
    let unlistenExit: (() => void) | undefined

    listen<{ session_id: string; data: number[] }>('pty-data', (event) => {
      if (event.payload.session_id === sessionId) {
        term.write(new Uint8Array(event.payload.data))
      }
    }).then(fn => { unlistenData = fn })

    listen<{ session_id: string }>('pty-exit', (event) => {
      if (event.payload.session_id === sessionId) {
        term.writeln('\r\n\x1b[90m[Process exited]\x1b[0m')
      }
    }).then(fn => { unlistenExit = fn })

    // Send user input to PTY
    const onData = term.onData((data) => {
      const encoder = new TextEncoder()
      const bytes = Array.from(encoder.encode(data))
      ptyWrite(sessionId, bytes).catch(() => {})
    })

    // Resize handling
    const ro = new ResizeObserver(() => {
      if (!containerRef.current || containerRef.current.offsetWidth === 0) return
      fitAddon.fit()
      setTimeout(() => {
        term.scrollToBottom()
        ptyResize(sessionId, term.cols, term.rows).catch(() => {})
      }, 50)
    })
    ro.observe(containerRef.current)

    return () => {
      onData.dispose()
      ro.disconnect()
      unlistenData?.()
      unlistenExit?.()
      ptyKill(sessionId).catch(() => {})
      term.dispose()
    }
  }, [sessionId, cwd, envVars])

  // Re-fit and focus when tab becomes active
  useEffect(() => {
    if (active && fitRef.current) {
      setTimeout(() => {
        fitRef.current?.fit()
        termRef.current?.focus()
      }, 0)
    }
  }, [active])

  // Update theme dynamically without restarting PTY
  useEffect(() => {
    if (termRef.current) {
      termRef.current.options.theme = resolveSchemeTheme(schemeId)
    }
  }, [schemeId])

  // When scheme is 'default', react to app theme (light/dark) changes
  useEffect(() => {
    if (schemeId !== 'default') return
    const observer = new MutationObserver(() => {
      if (termRef.current) {
        termRef.current.options.theme = resolveSchemeTheme('default')
      }
    })
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
    return () => observer.disconnect()
  }, [schemeId])

  return (
    <div
      ref={containerRef}
      className="terminal-container"
      style={{ display: active ? 'block' : 'none', height: '100%', position: 'relative' }}
    >
      {onClose && (
        <button
          className="terminal-pane-close"
          onClick={(e) => { e.stopPropagation(); onClose() }}
          title="Close pane"
        >
          ×
        </button>
      )}
    </div>
  )
}
