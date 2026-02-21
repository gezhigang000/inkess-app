// Markdown formatting utilities for textarea manipulation

export interface Selection {
  start: number
  end: number
  text: string
}

export function getSelection(ta: HTMLTextAreaElement): Selection {
  return {
    start: ta.selectionStart,
    end: ta.selectionEnd,
    text: ta.value.substring(ta.selectionStart, ta.selectionEnd),
  }
}

/** Apply a change to the textarea, return new full text and set cursor */
function apply(ta: HTMLTextAreaElement, start: number, end: number, replacement: string, cursorOffset?: number): string {
  const before = ta.value.substring(0, start)
  const after = ta.value.substring(end)
  const newText = before + replacement + after
  // Schedule cursor positioning after React re-render
  const pos = cursorOffset !== undefined ? start + cursorOffset : start + replacement.length
  requestAnimationFrame(() => {
    if (ta && document.contains(ta)) {
      ta.focus()
      ta.setSelectionRange(pos, pos)
    }
  })
  return newText
}

/** Toggle inline wrap markers like ** for bold, * for italic, ` for code, ~~ for strikethrough */
export function toggleInlineWrap(ta: HTMLTextAreaElement, marker: string): string {
  const { start, end, text } = getSelection(ta)
  const len = marker.length

  // Check if selection is already wrapped
  if (text.startsWith(marker) && text.endsWith(marker) && text.length >= len * 2) {
    const unwrapped = text.slice(len, -len)
    const newText = apply(ta, start, end, unwrapped)
    requestAnimationFrame(() => {
      ta.focus()
      ta.setSelectionRange(start, start + unwrapped.length)
    })
    return newText
  }

  // Check if surrounding text has the markers
  const before = ta.value.substring(Math.max(0, start - len), start)
  const after = ta.value.substring(end, end + len)
  if (before === marker && after === marker) {
    const newText = ta.value.substring(0, start - len) + text + ta.value.substring(end + len)
    requestAnimationFrame(() => {
      ta.focus()
      ta.setSelectionRange(start - len, start - len + text.length)
    })
    return newText
  }

  // Wrap selection
  const wrapped = marker + (text || 'text') + marker
  const newText = apply(ta, start, end, wrapped)
  requestAnimationFrame(() => {
    ta.focus()
    if (text) {
      ta.setSelectionRange(start + len, start + len + text.length)
    } else {
      ta.setSelectionRange(start + len, start + len + 4) // select "text"
    }
  })
  return newText
}

/** Toggle line prefix like # , - , 1. , >  */
export function toggleLinePrefix(ta: HTMLTextAreaElement, prefix: string): string {
  const { start, end } = getSelection(ta)
  const value = ta.value

  // Find line boundaries
  const lineStart = value.lastIndexOf('\n', start - 1) + 1
  const lineEnd = value.indexOf('\n', end)
  const actualEnd = lineEnd === -1 ? value.length : lineEnd
  const line = value.substring(lineStart, actualEnd)

  // Check if line already has this prefix
  if (line.startsWith(prefix)) {
    const newLine = line.substring(prefix.length)
    const newText = value.substring(0, lineStart) + newLine + value.substring(actualEnd)
    requestAnimationFrame(() => {
      ta.focus()
      ta.setSelectionRange(lineStart, lineStart + newLine.length)
    })
    return newText
  }

  // For headings, remove existing heading prefix first
  let cleanLine = line
  if (prefix.startsWith('#')) {
    cleanLine = line.replace(/^#{1,6}\s*/, '')
  }

  const newLine = prefix + cleanLine
  const newText = value.substring(0, lineStart) + newLine + value.substring(actualEnd)
  requestAnimationFrame(() => {
    ta.focus()
    ta.setSelectionRange(lineStart + prefix.length, lineStart + newLine.length)
  })
  return newText
}

/** Insert a fenced code block */
export function insertCodeBlock(ta: HTMLTextAreaElement): string {
  const { start, end, text } = getSelection(ta)
  const block = '```\n' + (text || '') + '\n```'
  const newText = apply(ta, start, end, block, 4) // cursor after ```\n
  return newText
}

/** Insert horizontal rule */
export function insertHorizontalRule(ta: HTMLTextAreaElement): string {
  const { start, end } = getSelection(ta)
  const value = ta.value
  // Ensure blank line before
  const needNewline = start > 0 && value[start - 1] !== '\n'
  const hr = (needNewline ? '\n' : '') + '\n---\n\n'
  return apply(ta, start, end, hr)
}

/** Insert text at cursor position */
export function insertAtCursor(ta: HTMLTextAreaElement, text: string): string {
  const { start, end } = getSelection(ta)
  return apply(ta, start, end, text)
}

/** Generate a markdown table string */
export function generateTable(rows: number, cols: number): string {
  const header = '| ' + Array.from({ length: cols }, (_, i) => `Header ${i + 1}`).join(' | ') + ' |'
  const separator = '| ' + Array.from({ length: cols }, () => '---').join(' | ') + ' |'
  const bodyRows = Array.from({ length: rows - 1 }, () =>
    '| ' + Array.from({ length: cols }, () => '   ').join(' | ') + ' |'
  )
  return '\n' + [header, separator, ...bodyRows].join('\n') + '\n'
}
