import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { renderMarkdown, escapeHtml } from './markdown'
import type { ThemeId } from './themes'

async function writeFile(path: string, data: Uint8Array): Promise<void> {
  await invoke('write_file', { path, contents: Array.from(data) })
}

function getThemeCSS(themeId: ThemeId): string {
  const rules: string[] = []
  const selector = '.theme-' + themeId
  for (const sheet of document.styleSheets) {
    try {
      for (const rule of sheet.cssRules) {
        const text = rule.cssText
        if (text.includes(selector) || text.includes('.hljs')) {
          rules.push(text)
        }
      }
    } catch {
      // Cross-origin stylesheets
    }
  }
  return rules.join('\n')
}

function buildHTMLDocument(markdown: string, themeId: ThemeId, title: string): string {
  const html = renderMarkdown(markdown)
  const css = getThemeCSS(themeId)
  const isDark = themeId === 'dark'

  const darkVars = isDark
    ? '[data-theme="dark"] { --bg:#1a1918;--surface:#242220;--text:#e7e5e4;--text-2:#a8a29e;--text-3:#78716c;--border:#3a3836;--border-s:#2e2c2a;--sidebar-bg:#1f1e1c;--color-accent:#60a5fa; }'
    : ''

  const parts = [
    '<!DOCTYPE html>',
    '<html lang="zh-CN"' + (isDark ? ' data-theme="dark"' : '') + '>',
    '<head>',
    '<meta charset="UTF-8">',
    '<meta name="viewport" content="width=device-width, initial-scale=1.0">',
    '<title>' + escapeHtml(title) + '</title>',
    '<style>',
    ':root{--bg:#f8f8f7;--surface:#fff;--text:#1c1917;--text-2:#78716c;--text-3:#a8a29e;--border:#e7e5e4;--border-s:#ececea;--sidebar-bg:#f2f1ef;--color-accent:#2563eb;}',
    darkVars,
    'body{margin:0;padding:40px;background:var(--bg);color:var(--text);font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;}',
    '.content{max-width:820px;margin:0 auto;}',
    css,
    '</style>',
    '</head>',
    '<body>',
    '<div class="content">',
    '<article class="md-body theme-' + themeId + '">' + html + '</article>',
    '</div>',
    '</body>',
    '</html>',
  ]
  return parts.join('\n')
}

/** Export as standalone HTML file */
async function exportHTML(markdown: string, themeId: ThemeId, title: string, defaultSavePath: string): Promise<void> {
  const path = await save({
    title: 'Export HTML',
    defaultPath: defaultSavePath,
    filters: [{ name: 'HTML', extensions: ['html'] }],
  })
  if (!path) return
  const doc = buildHTMLDocument(markdown, themeId, title)
  await writeFile(path, new TextEncoder().encode(doc))
}

/** Export as PDF using html2canvas + jsPDF */
async function exportPDF(markdown: string, themeId: ThemeId, title: string, defaultSavePath: string): Promise<void> {
  const path = await save({
    title: 'Export PDF',
    defaultPath: defaultSavePath,
    filters: [{ name: 'PDF', extensions: ['pdf'] }],
  })
  if (!path) return

  // Create an offscreen container with the rendered markdown
  const container = document.createElement('div')
  container.style.cssText = 'position:absolute;left:-9999px;top:0;width:820px;padding:40px;background:#fff;'
  const html = renderMarkdown(markdown)
  const article = document.createElement('article')
  article.className = 'md-body theme-' + themeId
  article.innerHTML = html
  container.appendChild(article)
  document.body.appendChild(container)

  try {
    const [{ default: html2canvas }, { jsPDF }] = await Promise.all([
      import('html2canvas'),
      import('jspdf'),
    ])
    const canvas = await html2canvas(container, {
      scale: 2,
      useCORS: true,
      backgroundColor: themeId === 'dark' ? '#1a1918' : '#ffffff',
    })

    const imgWidth = 210 // A4 width in mm
    const pageHeight = 297 // A4 height in mm
    const imgHeight = (canvas.height * imgWidth) / canvas.width
    const pdf = new jsPDF('p', 'mm', 'a4')

    let heightLeft = imgHeight
    let position = 0
    const imgData = canvas.toDataURL('image/png')

    pdf.addImage(imgData, 'PNG', 0, position, imgWidth, imgHeight)
    heightLeft -= pageHeight

    while (heightLeft > 0) {
      position -= pageHeight
      pdf.addPage()
      pdf.addImage(imgData, 'PNG', 0, position, imgWidth, imgHeight)
      heightLeft -= pageHeight
    }

    const pdfData = pdf.output('arraybuffer')
    await writeFile(path, new Uint8Array(pdfData))
  } finally {
    document.body.removeChild(container)
  }
}

/** Export as Word (.docx) using docx library */
async function exportDOCX(markdown: string, _themeId: ThemeId, title: string, defaultSavePath: string): Promise<void> {
  const path = await save({
    title: 'Export Word',
    defaultPath: defaultSavePath,
    filters: [{ name: 'Word', extensions: ['docx'] }],
  })
  if (!path) return

  const { Document, Packer, Paragraph, TextRun, HeadingLevel, BorderStyle } = await import('docx')

  const html = renderMarkdown(markdown)
  const container = document.createElement('div')
  container.innerHTML = html

  const children: InstanceType<typeof Paragraph>[] = []

  type Fmt = { bold?: boolean; italics?: boolean; font?: string; size?: number; color?: string; underline?: object }

  function getTextRuns(el: Node, fmt: Fmt = {}): InstanceType<typeof TextRun>[] {
    const runs: InstanceType<typeof TextRun>[] = []
    for (const child of el.childNodes) {
      if (child.nodeType === Node.TEXT_NODE) {
        const text = child.textContent || ''
        if (text) runs.push(new TextRun({ text, ...fmt }))
      } else if (child.nodeType === Node.ELEMENT_NODE) {
        const tag = (child as Element).tagName.toLowerCase()
        if (tag === 'strong' || tag === 'b') {
          runs.push(...getTextRuns(child, { ...fmt, bold: true }))
        } else if (tag === 'em' || tag === 'i') {
          runs.push(...getTextRuns(child, { ...fmt, italics: true }))
        } else if (tag === 'code') {
          runs.push(...getTextRuns(child, { ...fmt, font: 'Courier New', size: 20 }))
        } else if (tag === 'a') {
          runs.push(...getTextRuns(child, { ...fmt, color: '2563EB', underline: {} } as Fmt))
        } else {
          runs.push(...getTextRuns(child, fmt))
        }
      }
    }
    return runs
  }

  for (const node of container.children) {
    const tag = node.tagName.toLowerCase()
    if (tag === 'h1') {
      children.push(new Paragraph({ children: getTextRuns(node), heading: HeadingLevel.HEADING_1 }))
    } else if (tag === 'h2') {
      children.push(new Paragraph({ children: getTextRuns(node), heading: HeadingLevel.HEADING_2 }))
    } else if (tag === 'h3') {
      children.push(new Paragraph({ children: getTextRuns(node), heading: HeadingLevel.HEADING_3 }))
    } else if (tag === 'p') {
      children.push(new Paragraph({ children: getTextRuns(node) }))
    } else if (tag === 'blockquote') {
      children.push(new Paragraph({
        children: getTextRuns(node),
        indent: { left: 720 },
        border: { left: { style: BorderStyle.SINGLE, size: 6, color: '2563EB' } },
      }))
    } else if (tag === 'ul' || tag === 'ol') {
      for (const li of node.querySelectorAll('li')) {
        const bullet = tag === 'ul' ? '• ' : ''
        const runs = getTextRuns(li)
        if (bullet) runs.unshift(new TextRun({ text: bullet }))
        children.push(new Paragraph({ children: runs, indent: { left: 720 } }))
      }
    } else if (tag === 'pre') {
      const code = node.textContent || ''
      for (const line of code.split('\n')) {
        children.push(new Paragraph({
          children: [new TextRun({ text: line || ' ', font: 'Courier New', size: 20 })],
          shading: { fill: 'F5F5F4' },
        }))
      }
    } else if (tag === 'hr') {
      children.push(new Paragraph({
        children: [],
        border: { bottom: { style: BorderStyle.SINGLE, size: 1, color: 'D6D3D1' } },
      }))
    } else if (tag === 'table') {
      // Flatten table to paragraphs (simplified)
      for (const row of node.querySelectorAll('tr')) {
        const cells = Array.from(row.querySelectorAll('th, td')).map(c => c.textContent || '')
        children.push(new Paragraph({
          children: [new TextRun({ text: cells.join('  |  ') })],
        }))
      }
    } else {
      children.push(new Paragraph({ children: getTextRuns(node) }))
    }
  }

  const doc = new Document({
    title: title.replace(/\.md$/, ''),
    sections: [{ properties: {}, children }],
  })

  const buffer = await Packer.toBuffer(doc)
  await writeFile(path, new Uint8Array(buffer))
}

/** Export as PowerPoint (.pptx) using pptxgenjs */
async function exportPPTX(markdown: string, _themeId: ThemeId, title: string, defaultSavePath: string): Promise<void> {
  const path = await save({
    title: 'Export PPT',
    defaultPath: defaultSavePath,
    filters: [{ name: 'PowerPoint', extensions: ['pptx'] }],
  })
  if (!path) return

  const PptxGenJS = (await import('pptxgenjs')).default
  const pptx = new PptxGenJS()
  pptx.title = title.replace(/\.md$/, '')
  pptx.layout = 'LAYOUT_WIDE'

  const slides = parseMarkdownToSlides(markdown)

  for (const slide of slides) {
    const s = pptx.addSlide()
    // Background
    s.background = { color: 'FFFFFF' }

    // Title
    if (slide.title) {
      s.addText(slide.title, {
        x: 0.8, y: 0.5, w: 11.5, h: 0.8,
        fontSize: 28, fontFace: 'Arial', bold: true,
        color: '1c1917', valign: 'top',
      })
    }

    // Bullet points
    if (slide.bullets.length > 0) {
      const textItems = slide.bullets.map(b => ({
        text: b.text,
        options: {
          fontSize: 16 - (b.level * 2),
          fontFace: 'Arial',
          color: '44403c',
          bullet: { indent: 18 + b.level * 12 } as any,
          indentLevel: b.level,
          paraSpaceAfter: 6,
        },
      }))
      s.addText(textItems, {
        x: 0.8, y: 1.6, w: 11.5, h: 5.0,
        valign: 'top',
      })
    }

    // Code block
    if (slide.code) {
      s.addText(slide.code, {
        x: 0.8, y: slide.bullets.length > 0 ? 5.2 : 1.6,
        w: 11.5, h: 1.8,
        fontSize: 11, fontFace: 'Courier New',
        color: 'e7e5e4', fill: { color: '1c1917' },
        valign: 'top',
      })
    }
  }

  // If no slides were generated, create a single title slide
  if (slides.length === 0) {
    const s = pptx.addSlide()
    s.background = { color: 'FFFFFF' }
    s.addText(title.replace(/\.md$/, ''), {
      x: 0.8, y: 2.5, w: 11.5, h: 1.5,
      fontSize: 36, fontFace: 'Arial', bold: true,
      color: '1c1917', align: 'center', valign: 'middle',
    })
  }

  const data = await pptx.write({ outputType: 'arraybuffer' }) as ArrayBuffer
  await writeFile(path, new Uint8Array(data))
}

interface SlideData {
  title: string
  bullets: { text: string; level: number }[]
  code: string
}

function parseMarkdownToSlides(markdown: string): SlideData[] {
  const lines = markdown.split('\n')
  const slides: SlideData[] = []
  let current: SlideData | null = null

  const flush = () => { if (current) slides.push(current) }

  for (const line of lines) {
    // H1 or H2 starts a new slide
    const headingMatch = line.match(/^(#{1,2})\s+(.+)/)
    if (headingMatch) {
      flush()
      current = { title: headingMatch[2].trim(), bullets: [], code: '' }
      continue
    }

    if (!current) {
      // Content before first heading → create untitled slide
      if (line.trim()) {
        current = { title: '', bullets: [], code: '' }
      } else {
        continue
      }
    }

    // Code block
    if (line.startsWith('```')) {
      // Accumulate until closing ```
      const codeLines: string[] = []
      const idx = lines.indexOf(line)
      let j = idx + 1
      while (j < lines.length && !lines[j].startsWith('```')) {
        codeLines.push(lines[j])
        j++
      }
      if (codeLines.length > 0) {
        current.code = (current.code ? current.code + '\n' : '') + codeLines.join('\n')
      }
      continue
    }

    // H3/H4 → bold bullet
    const subHeading = line.match(/^#{3,4}\s+(.+)/)
    if (subHeading) {
      current.bullets.push({ text: subHeading[1].trim(), level: 0 })
      continue
    }

    // List items
    const listMatch = line.match(/^(\s*)([-*+]|\d+\.)\s+(.+)/)
    if (listMatch) {
      const indent = listMatch[1].length
      const level = Math.min(Math.floor(indent / 2), 2)
      current.bullets.push({ text: listMatch[3].replace(/\*\*(.+?)\*\*/g, '$1').replace(/`(.+?)`/g, '$1'), level })
      continue
    }

    // Regular paragraph text → bullet level 0
    const trimmed = line.trim()
    if (trimmed && !trimmed.startsWith('```') && !trimmed.startsWith('---') && !trimmed.startsWith('|')) {
      current.bullets.push({ text: trimmed.replace(/\*\*(.+?)\*\*/g, '$1').replace(/`(.+?)`/g, '$1'), level: 0 })
    }
  }

  flush()
  return slides
}

/** Main export dispatcher */
export async function exportFile(
  format: string,
  markdown: string,
  themeId: ThemeId,
  filePath: string,
  isPro: boolean = true,
): Promise<string> {
  if (!markdown || !markdown.trim()) {
    throw new Error('Cannot export empty document')
  }
  const raw = filePath || 'document'
  // Extract basename for display, keep full path for defaultPath
  const baseName = raw.includes('/') ? raw.split('/').pop()! : raw
  const defaultDir = raw.includes('/') ? raw.substring(0, raw.lastIndexOf('/') + 1) : ''
  const stem = baseName.replace(/\.\w+$/, '')
  // Free users get a watermark appended
  const content = isPro ? markdown : markdown + '\n\n---\n\n<p style="text-align:center;font-size:11px;color:#aaa;">Made with Inkess</p>\n'
  try {
    switch (format) {
      case 'HTML':
        await exportHTML(content, themeId, baseName, defaultDir + stem + '.html')
        return 'Exported as HTML'
      case 'PDF':
        await exportPDF(content, themeId, baseName, defaultDir + stem + '.pdf')
        return 'Exported as PDF'
      case 'DOCX':
        await exportDOCX(content, themeId, baseName, defaultDir + stem + '.docx')
        return 'Exported as Word'
      case 'PPTX':
        await exportPPTX(content, themeId, baseName, defaultDir + stem + '.pptx')
        return 'Exported as PPT'
      default:
        throw new Error('Unsupported format: ' + format)
    }
  } catch (err) {
    if (err instanceof Error && err.message.includes('Unsupported')) throw err
    throw new Error('Export failed, please retry')
  }
}
