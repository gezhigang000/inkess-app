import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { ALL_SUPPORTED_EXTENSIONS } from './fileTypes'

export interface FileEntry {
  name: string
  is_dir: boolean
}

export interface SnapshotInfo {
  id: number
  created_at: string
}

export async function readFile(path: string): Promise<string> {
  return invoke<string>('read_file', { path })
}

export async function readFileBinary(path: string): Promise<number[]> {
  return invoke<number[]>('read_file_binary', { path })
}

export async function saveFile(path: string, content: string): Promise<void> {
  return invoke<void>('save_file', { path, content })
}

export interface DirectoryListing {
  entries: FileEntry[]
  truncated: boolean
  total: number
}

export async function listDirectory(path: string): Promise<DirectoryListing> {
  return invoke<DirectoryListing>('list_directory', { path })
}

export async function createSnapshot(filePath: string, content: string): Promise<boolean> {
  return invoke<boolean>('create_snapshot', { filePath, content })
}

export async function listSnapshots(filePath: string): Promise<SnapshotInfo[]> {
  return invoke<SnapshotInfo[]>('list_snapshots', { filePath })
}

export async function getSnapshotContent(snapshotId: number): Promise<string> {
  return invoke<string>('get_snapshot_content', { snapshotId })
}

export interface SnapshotStats {
  count: number
  size_bytes: number
}

export async function getSnapshotStats(): Promise<SnapshotStats> {
  return invoke<SnapshotStats>('get_snapshot_stats')
}

export async function cleanupSnapshots(retentionDays: number, retentionCount: number): Promise<number> {
  return invoke<number>('cleanup_snapshots', { retentionDays, retentionCount })
}

export async function getInitialFile(): Promise<string | null> {
  return invoke<string | null>('get_initial_file')
}

// --- File operations ---

export async function createFile(path: string, template: string = ''): Promise<void> {
  return invoke<void>('create_file', { path, template })
}

export async function createDirectory(path: string): Promise<void> {
  return invoke<void>('create_directory', { path })
}

export async function renameEntry(oldPath: string, newPath: string): Promise<void> {
  return invoke<void>('rename_entry', { oldPath, newPath })
}

export async function deleteToTrash(path: string): Promise<void> {
  return invoke<void>('delete_to_trash', { path })
}

export async function searchFiles(dir: string, query: string): Promise<string[]> {
  return invoke<string[]>('search_files', { dir, query })
}

export async function getFileSize(path: string): Promise<number> {
  return invoke<number>('get_file_size', { path })
}

export async function readFileLines(path: string, line: number, context?: number): Promise<string> {
  return invoke<string>('read_file_lines', { path, line, context })
}

// --- File watcher ---

export async function watchDirectory(path: string): Promise<void> {
  return invoke<void>('watch_directory', { path })
}

export async function unwatchDirectory(): Promise<void> {
  return invoke<void>('unwatch_directory')
}

// --- PTY terminal ---

export async function ptySpawn(cwd: string, sessionId: string, envVars?: Array<{ key: string; value: string }>): Promise<void> {
  return invoke<void>('pty_spawn', { cwd, sessionId, envVars: envVars || [] })
}

export async function ptyWrite(sessionId: string, data: number[]): Promise<void> {
  return invoke<void>('pty_write', { sessionId, data })
}

export async function ptyResize(sessionId: string, cols: number, rows: number): Promise<void> {
  return invoke<void>('pty_resize', { sessionId, cols, rows })
}

export async function ptyKill(sessionId: string): Promise<void> {
  return invoke<void>('pty_kill', { sessionId })
}

// --- Git ---

export interface GitFileStatus {
  path: string
  status: string
  staged: boolean
}

export interface GitStatusResult {
  is_repo: boolean
  branch: string
  files: GitFileStatus[]
}

export interface GitLogEntry {
  hash: string
  message: string
  author: string
  date: string
}

export interface GitRemoteInfo {
  name: string
  url: string
}

export async function gitStatus(cwd: string): Promise<GitStatusResult> {
  return invoke<GitStatusResult>('git_status', { cwd })
}

export async function gitInit(cwd: string): Promise<string> {
  return invoke<string>('git_init', { cwd })
}

export async function gitStage(cwd: string, files: string[]): Promise<void> {
  return invoke<void>('git_stage', { cwd, files })
}

export async function gitUnstage(cwd: string, files: string[]): Promise<void> {
  return invoke<void>('git_unstage', { cwd, files })
}

export async function gitCommit(cwd: string, message: string): Promise<string> {
  return invoke<string>('git_commit', { cwd, message })
}

export async function gitPush(cwd: string, remote: string = ''): Promise<string> {
  return invoke<string>('git_push', { cwd, remote })
}

export async function gitPull(cwd: string, remote: string = ''): Promise<string> {
  return invoke<string>('git_pull', { cwd, remote })
}

export async function gitRemoteAdd(cwd: string, name: string, url: string): Promise<void> {
  return invoke<void>('git_remote_add', { cwd, name, url })
}

export async function gitRemoteList(cwd: string): Promise<GitRemoteInfo[]> {
  return invoke<GitRemoteInfo[]>('git_remote_list', { cwd })
}

export async function gitLog(cwd: string, count: number): Promise<GitLogEntry[]> {
  return invoke<GitLogEntry[]>('git_log', { cwd, count })
}

export async function gitConfigUser(cwd: string, username: string, email: string): Promise<void> {
  return invoke<void>('git_config_user', { cwd, username, email })
}

export async function setupSshKey(email: string): Promise<string> {
  return invoke<string>('setup_ssh_key', { email })
}

export async function openFileOrDirDialog(): Promise<string | null> {
  return invoke<string | null>('open_file_or_dir')
}

export async function openFileDialog(): Promise<string | null> {
  const selected = await open({
    title: 'Open File',
    filters: [
      { name: 'All Supported Formats', extensions: [...ALL_SUPPORTED_EXTENSIONS] },
      { name: 'Markdown', extensions: ['md', 'markdown', 'mdown', 'mkd'] },
      { name: 'Text/Code', extensions: ['txt', 'log', 'csv', 'js', 'ts', 'py', 'rs', 'go', 'json', 'yaml', 'toml', 'xml', 'css', 'html', 'sh'] },
      { name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico'] },
      { name: 'PDF', extensions: ['pdf'] },
      { name: 'Word', extensions: ['docx'] },
    ],
    multiple: false,
  })
  return typeof selected === 'string' ? selected : null
}

export async function openDirectoryDialog(): Promise<string | null> {
  const selected = await open({
    title: 'Open Folder',
    directory: true,
    multiple: false,
  })
  return typeof selected === 'string' ? selected : null
}

// --- AI ---

export interface AiConfig {
  api_url: string
  api_key: string
  model: string
  temperature: number
  max_tokens: number
  system_prompt: string
  base_prompt: string
  search_api_key: string
  search_provider: string
  provider_keys: Record<string, string>
}

export interface ToolCall {
  id: string
  type: string
  function: { name: string; arguments: string }
}

export interface ChatMessage {
  role: string
  content: string | null
  tool_calls?: ToolCall[]
  tool_call_id?: string
}

export interface AiStreamEvent {
  session_id: string
  event_type: string
  content: string
}

export async function aiSaveConfig(config: AiConfig): Promise<void> {
  return invoke<void>('ai_save_config', { config })
}

export async function aiLoadConfig(): Promise<AiConfig | null> {
  return invoke<AiConfig | null>('ai_load_config')
}

export async function aiTestConnection(config: AiConfig): Promise<string> {
  return invoke<string>('ai_test_connection', { config })
}

export async function aiTestSearch(provider: string, apiKey: string): Promise<string> {
  return invoke<string>('ai_test_search', { provider, apiKey })
}

export async function aiChat(sessionId: string, messages: ChatMessage[], config: AiConfig, deepMode?: boolean, cwd?: string): Promise<void> {
  return invoke<void>('ai_chat', { sessionId, messages, config, deepMode: deepMode || false, cwd: cwd || '' })
}

export interface MemoryEntry {
  content: string
  created_at: string
}

export async function aiSaveMemory(dir: string, content: string): Promise<void> {
  return invoke<void>('ai_save_memory', { dir, content })
}

export async function aiLoadMemories(dir: string): Promise<MemoryEntry[]> {
  return invoke<MemoryEntry[]>('ai_load_memories', { dir })
}

// --- Python Environment ---

export interface PythonEnvStatus {
  installed: boolean
  path: string | null
}

export async function checkPythonEnv(): Promise<PythonEnvStatus> {
  return invoke<PythonEnvStatus>('check_python_env')
}

export async function preloadPythonEnv(): Promise<void> {
  return invoke<void>('preload_python_env')
}

export interface PythonSetupProgress {
  status: string
  progress: number
  message: string
}

// --- App Settings ---

export interface TerminalProvider {
  id: string
  name: string
  envVars: Array<{ key: string; value: string }>
  isDefault: boolean
}

export interface AppSettings {
  theme?: string
  language?: string
  retention_days?: number
  retention_count?: number
  terminal_providers?: TerminalProvider[]
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  return invoke<void>('save_settings', { settings })
}

export async function loadSettings(): Promise<AppSettings> {
  return invoke<AppSettings>('load_settings')
}

// --- License ---

export interface LicenseInfo {
  key: string
  activated_at: string
}

export async function licenseLoad(): Promise<LicenseInfo | null> {
  return invoke<LicenseInfo | null>('license_load')
}

export async function licenseActivate(key: string): Promise<LicenseInfo> {
  return invoke<LicenseInfo>('license_activate', { key })
}

export async function licenseDeactivate(): Promise<void> {
  return invoke<void>('license_deactivate')
}

// --- RAG Knowledge Base ---

export interface RagSearchResult {
  path: string
  content: string
  start_line: number
  end_line: number
  heading: string | null
  distance: number
}

export interface RagIndexStats {
  file_count: number
  chunk_count: number
  db_size_bytes: number
}

export async function ragInit(dir: string): Promise<void> {
  return invoke<void>('rag_init', { dir })
}

export async function ragSearch(query: string, topK?: number): Promise<RagSearchResult[]> {
  return invoke<RagSearchResult[]>('rag_search', { query, topK })
}

export async function ragStats(): Promise<RagIndexStats> {
  return invoke<RagIndexStats>('rag_stats')
}

export async function ragRebuild(dir: string): Promise<void> {
  return invoke<void>('rag_rebuild', { dir })
}

// --- MCP Servers ---

export interface McpServerConfig {
  id: string
  name: string
  command: string
  args: string[]
  env: Record<string, string>
  enabled: boolean
  transport: 'stdio' | 'http'
  url?: string
}

export interface McpServerStatus {
  id: string
  name: string
  connected: boolean
  tool_count: number
  error: string | null
  transport: string
  last_seen: number | null
}

export interface McpToolInfo {
  server_id: string
  server_name: string
  name: string
  description: string
  input_schema: unknown
}

export async function mcpAddServer(config: McpServerConfig): Promise<void> {
  return invoke<void>('mcp_add_server', { config })
}

export async function mcpRemoveServer(id: string): Promise<void> {
  return invoke<void>('mcp_remove_server', { id })
}

export async function mcpRestartServer(id: string): Promise<void> {
  return invoke<void>('mcp_restart_server', { id })
}

export async function mcpListServers(): Promise<McpServerStatus[]> {
  return invoke<McpServerStatus[]>('mcp_list_servers')
}

export async function mcpListTools(): Promise<McpToolInfo[]> {
  return invoke<McpToolInfo[]>('mcp_list_tools')
}

export interface McpToolCallLog {
  timestamp: number
  server_id: string
  tool_name: string
  arguments: string
  result: string
  duration_ms: number
  is_error: boolean
}

export async function mcpToolLogs(): Promise<McpToolCallLog[]> {
  return invoke<McpToolCallLog[]>('mcp_tool_logs')
}

// --- Debug logs ---

export interface DebugLogEntry {
  timestamp: string
  level: string
  module: string
  message: string
}

export async function getDebugLogs(): Promise<DebugLogEntry[]> {
  return invoke<DebugLogEntry[]>('get_debug_logs')
}

export async function clearDebugLogs(): Promise<void> {
  return invoke<void>('clear_debug_logs')
}

// --- Terminal Logs ---

export interface TerminalLogEntry {
  filename: string
  session_id: string
  started: string
  provider: string
  cwd: string
  size_bytes: number
  recovered: boolean
}

export async function listTerminalLogs(): Promise<TerminalLogEntry[]> {
  return invoke<TerminalLogEntry[]>('list_terminal_logs')
}

export async function readTerminalLog(filename: string): Promise<string> {
  return invoke<string>('read_terminal_log', { filename })
}

export async function deleteTerminalLog(filename: string): Promise<void> {
  return invoke<void>('delete_terminal_log', { filename })
}

export async function getSystemEnvVars(): Promise<Array<[string, string]>> {
  return invoke<Array<[string, string]>>('get_system_env_vars')
}

export async function getShellEnvVars(): Promise<Array<[string, string]>> {
  return invoke<Array<[string, string]>>('get_shell_env_vars')
}

export interface ShellFunction {
  name: string
  env_vars: Array<[string, string]>
}

export async function parseShellFunctions(): Promise<ShellFunction[]> {
  return invoke<ShellFunction[]>('parse_shell_functions')
}
