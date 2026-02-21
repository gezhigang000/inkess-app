export type FileType = 'markdown' | 'text' | 'code' | 'image' | 'pdf' | 'docx' | 'xlsx' | 'html' | 'unknown'

const EXT_MAP: Record<string, FileType> = {
  md: 'markdown', markdown: 'markdown', mdown: 'markdown', mkd: 'markdown',
  txt: 'text', log: 'text', csv: 'text',
  js: 'code', ts: 'code', tsx: 'code', jsx: 'code', py: 'code', rs: 'code',
  go: 'code', json: 'code', yaml: 'code', yml: 'code', toml: 'code',
  xml: 'code', css: 'code', html: 'html', sh: 'code', sql: 'code',
  c: 'code', cpp: 'code', h: 'code', java: 'code', rb: 'code',
  properties: 'code', ini: 'code', conf: 'code', cfg: 'code',
  env: 'code', gitignore: 'code', dockerignore: 'code',
  makefile: 'code', dockerfile: 'code',
  kt: 'code', swift: 'code', dart: 'code', lua: 'code', r: 'code',
  scala: 'code', groovy: 'code', gradle: 'code',
  vue: 'code', svelte: 'code', less: 'code', scss: 'code', sass: 'code',
  png: 'image', jpg: 'image', jpeg: 'image', gif: 'image',
  svg: 'image', webp: 'image', bmp: 'image', ico: 'image',
  pdf: 'pdf',
  docx: 'docx',
  xlsx: 'xlsx', xls: 'xlsx',
}

const LANG_MAP: Record<string, string> = {
  js: 'javascript', ts: 'typescript', tsx: 'typescript', jsx: 'javascript',
  py: 'python', rs: 'rust', go: 'go', json: 'json',
  yaml: 'yaml', yml: 'yaml', toml: 'toml', xml: 'xml',
  css: 'css', html: 'html', sh: 'bash', sql: 'sql',
  c: 'c', cpp: 'cpp', h: 'c', java: 'java', rb: 'ruby',
  properties: 'properties', ini: 'ini', conf: 'plaintext', cfg: 'plaintext',
  env: 'plaintext', gitignore: 'plaintext', dockerignore: 'plaintext',
  makefile: 'makefile', dockerfile: 'dockerfile',
  kt: 'kotlin', swift: 'swift', dart: 'dart', lua: 'lua', r: 'r',
  scala: 'scala', groovy: 'groovy', gradle: 'groovy',
  vue: 'html', svelte: 'html', less: 'less', scss: 'scss', sass: 'scss',
  txt: 'plaintext', log: 'plaintext', csv: 'plaintext',
}

function getExt(filename: string): string {
  const dot = filename.lastIndexOf('.')
  return dot >= 0 ? filename.slice(dot + 1).toLowerCase() : ''
}

export function getFileType(filename: string): FileType {
  return EXT_MAP[getExt(filename)] ?? 'unknown'
}

export function isEditable(type: FileType): boolean {
  return type === 'markdown' || type === 'text' || type === 'code' || type === 'html'
}

export function isTextBased(type: FileType): boolean {
  return type === 'markdown' || type === 'text' || type === 'code' || type === 'html'
}

export function isSupported(filename: string): boolean {
  return getFileType(filename) !== 'unknown'
}

export function getLanguage(filename: string): string | undefined {
  return LANG_MAP[getExt(filename)]
}

/** All supported file extensions for file dialog filters */
export const ALL_SUPPORTED_EXTENSIONS = Object.keys(EXT_MAP)
