use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::fs;

use serde::Serialize;

// --- Tokenization ---

fn tokenize(text: &str) -> Vec<String> {
    let text_lower = text.to_lowercase();
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text_lower.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            if is_cjk(ch) {
                tokens.push(ch.to_string());
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}' |
        '\u{3400}'..='\u{4DBF}' |
        '\u{F900}'..='\u{FAFF}' |
        '\u{3000}'..='\u{303F}' |
        '\u{3040}'..='\u{309F}' |
        '\u{30A0}'..='\u{30FF}'
    )
}

// --- BM25 Index ---

const BM25_K1: f64 = 1.2;
const BM25_B: f64 = 0.75;

#[derive(Clone)]
struct Document {
    path: String,
    content: String,
    line_offset: usize,
    line_count: usize,
    token_count: usize,
}

#[derive(Serialize, Clone)]
pub struct SearchResult {
    pub path: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub score: f64,
}

pub struct Bm25Index {
    docs: Vec<Document>,
    inverted: HashMap<String, Vec<(usize, usize)>>,
    avg_dl: f64,
}

pub struct Bm25State {
    pub index: Mutex<Option<Bm25Index>>,
}

const INDEX_EXTENSIONS: &[&str] = &[
    "md", "txt", "rs", "ts", "tsx", "js", "jsx", "py", "java", "go", "rb", "c", "cpp", "h",
    "css", "scss", "html", "json", "toml", "yaml", "yml", "xml", "sh", "bash", "zsh",
    "sql", "graphql", "proto", "swift", "kt", "dart", "vue", "svelte",
    "properties", "ini", "conf", "cfg", "env",
];

const SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", "target", "dist", "build", ".next", "__pycache__",
    ".inkess", ".vscode", ".idea",
];

const MAX_FILE_SIZE: u64 = 512 * 1024;
const CHUNK_LINES: usize = 30;
const MAX_FILES: usize = 2000;
const MAX_DEPTH: usize = 8;

impl Bm25Index {
    pub fn build(dir: &Path) -> Result<Self, String> {
        let mut docs = Vec::new();
        let mut files_found: Vec<PathBuf> = Vec::new();
        collect_files(dir, 0, &mut files_found);
        files_found.truncate(MAX_FILES);

        for file_path in &files_found {
            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let rel_path = file_path.strip_prefix(dir)
                .unwrap_or(file_path)
                .to_string_lossy()
                .to_string();

            let lines: Vec<&str> = content.lines().collect();
            if lines.is_empty() { continue; }

            let mut offset = 0;
            while offset < lines.len() {
                let end = (offset + CHUNK_LINES).min(lines.len());
                let chunk_text = lines[offset..end].join("\n");
                let tokens = tokenize(&chunk_text);
                if tokens.is_empty() {
                    offset = end;
                    continue;
                }
                docs.push(Document {
                    path: rel_path.clone(),
                    content: chunk_text,
                    line_offset: offset,
                    line_count: end - offset,
                    token_count: tokens.len(),
                });
                offset = end;
            }
        }

        // Build inverted index
        let mut inverted: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        let total_tokens: usize = docs.iter().map(|d| d.token_count).sum();
        let avg_dl = if docs.is_empty() { 1.0 } else { total_tokens as f64 / docs.len() as f64 };

        for (doc_idx, doc) in docs.iter().enumerate() {
            let mut term_freq: HashMap<String, usize> = HashMap::new();
            for token in tokenize(&doc.content) {
                *term_freq.entry(token).or_insert(0) += 1;
            }
            for (term, freq) in term_freq {
                inverted.entry(term).or_default().push((doc_idx, freq));
            }
        }

        Ok(Bm25Index { docs, inverted, avg_dl })
    }

    pub fn search(&self, query: &str, top_k: usize) -> Vec<SearchResult> {
        if self.docs.is_empty() {
            return vec![];
        }

        let query_tokens = tokenize(query);
        if query_tokens.is_empty() {
            return vec![];
        }

        let n = self.docs.len() as f64;
        let mut scores: Vec<f64> = vec![0.0; self.docs.len()];

        for token in &query_tokens {
            if let Some(postings) = self.inverted.get(token) {
                let df = postings.len() as f64;
                let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                if idf <= 0.0 { continue; }

                for &(doc_idx, tf) in postings {
                    let dl = self.docs[doc_idx].token_count as f64;
                    let tf_norm = (tf as f64 * (BM25_K1 + 1.0))
                        / (tf as f64 + BM25_K1 * (1.0 - BM25_B + BM25_B * dl / self.avg_dl));
                    scores[doc_idx] += idf * tf_norm;
                }
            }
        }

        let mut ranked: Vec<(usize, f64)> = scores.iter()
            .enumerate()
            .filter(|(_, &s)| s > 0.0)
            .map(|(i, &s)| (i, s))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(top_k);

        ranked.iter().map(|&(idx, score)| {
            let doc = &self.docs[idx];
            SearchResult {
                path: doc.path.clone(),
                content: doc.content.clone(),
                start_line: doc.line_offset + 1,
                end_line: doc.line_offset + doc.line_count,
                score,
            }
        }).collect()
    }
}

fn collect_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES { return; }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut dirs_to_visit = Vec::new();

    for entry in entries.flatten() {
        if out.len() >= MAX_FILES { return; }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip symlinks to prevent traversal attacks and infinite loops
        if let Ok(ft) = entry.file_type() {
            if ft.is_symlink() { continue; }
        }

        if path.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                dirs_to_visit.push(path);
            }
        } else if path.is_file() {
            let ext = path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if !INDEX_EXTENSIONS.contains(&ext) { continue; }

            if let Ok(meta) = entry.metadata() {
                if meta.len() > MAX_FILE_SIZE { continue; }
            }

            out.push(path);
        }
    }

    for d in dirs_to_visit {
        collect_files(&d, depth + 1, out);
    }
}

// --- Tauri Commands ---

#[tauri::command]
pub fn bm25_init(state: tauri::State<'_, Bm25State>, dir: String) -> Result<(), String> {
    let dir_path = Path::new(&dir);
    if !dir_path.is_dir() {
        return Err("Not a directory".to_string());
    }
    let index = Bm25Index::build(dir_path)?;
    let mut guard = state.index.lock().map_err(|e| e.to_string())?;
    *guard = Some(index);
    Ok(())
}

#[tauri::command]
pub fn bm25_search(state: tauri::State<'_, Bm25State>, query: String, top_k: Option<usize>) -> Result<Vec<SearchResult>, String> {
    let guard = state.index.lock().map_err(|e| e.to_string())?;
    let index = guard.as_ref().ok_or("Search index not initialized")?;
    Ok(index.search(&query, top_k.unwrap_or(5)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a Bm25Index from in-memory documents (bypasses filesystem).
    fn build_index_from_docs(entries: Vec<(&str, &str)>) -> Bm25Index {
        let mut docs = Vec::new();
        for (path, content) in &entries {
            let lines: Vec<&str> = content.lines().collect();
            if lines.is_empty() {
                continue;
            }
            let mut offset = 0;
            while offset < lines.len() {
                let end = (offset + CHUNK_LINES).min(lines.len());
                let chunk_text = lines[offset..end].join("\n");
                let tokens = tokenize(&chunk_text);
                if tokens.is_empty() {
                    offset = end;
                    continue;
                }
                docs.push(Document {
                    path: path.to_string(),
                    content: chunk_text,
                    line_offset: offset,
                    line_count: end - offset,
                    token_count: tokens.len(),
                });
                offset = end;
            }
        }

        let mut inverted: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        let total_tokens: usize = docs.iter().map(|d| d.token_count).sum();
        let avg_dl = if docs.is_empty() { 1.0 } else { total_tokens as f64 / docs.len() as f64 };

        for (doc_idx, doc) in docs.iter().enumerate() {
            let mut term_freq: HashMap<String, usize> = HashMap::new();
            for token in tokenize(&doc.content) {
                *term_freq.entry(token).or_insert(0) += 1;
            }
            for (term, freq) in term_freq {
                inverted.entry(term).or_default().push((doc_idx, freq));
            }
        }

        Bm25Index { docs, inverted, avg_dl }
    }

    #[test]
    fn test_empty_index() {
        let index = build_index_from_docs(vec![]);
        assert!(index.docs.is_empty());
        assert!(index.inverted.is_empty());
        let results = index.search("anything", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_add_document_and_search() {
        let index = build_index_from_docs(vec![
            ("test.md", "hello world this is a test document"),
        ]);
        assert!(!index.docs.is_empty());
        let results = index.search("hello", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, "test.md");
    }

    #[test]
    fn test_search_returns_correct_document() {
        let index = build_index_from_docs(vec![
            ("alpha.rs", "rust programming language systems"),
            ("beta.py", "python scripting language dynamic"),
        ]);
        let results = index.search("rust", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, "alpha.rs");

        let results = index.search("python", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, "beta.py");
    }

    #[test]
    fn test_ranking_more_occurrences_scores_higher() {
        let index = build_index_from_docs(vec![
            ("few.txt", "rust is nice"),
            ("many.txt", "rust rust rust rust rust programming in rust"),
        ]);
        let results = index.search("rust", 10);
        assert!(results.len() >= 2);
        // Document with more "rust" occurrences should rank first
        assert_eq!(results[0].path, "many.txt");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_cjk_text_indexing_and_search() {
        // CJK chars are alphanumeric in Rust, so they concatenate into whole-word tokens
        // "你好世界 欢迎使用" tokenizes to ["你好世界", "欢迎使用"]
        let index = build_index_from_docs(vec![
            ("chinese.md", "你好世界 欢迎使用"),
            ("english.md", "hello world welcome"),
        ]);
        // Search for exact token
        let results = index.search("你好世界", 10);
        assert!(!results.is_empty(), "CJK search should return results");
        assert_eq!(results[0].path, "chinese.md");

        let results = index.search("欢迎使用", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, "chinese.md");
    }

    #[test]
    fn test_empty_query_returns_empty() {
        let index = build_index_from_docs(vec![
            ("doc.txt", "some content here"),
        ]);
        let results = index.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_limit_respected() {
        let index = build_index_from_docs(vec![
            ("a.txt", "common term here"),
            ("b.txt", "common term there"),
            ("c.txt", "common term everywhere"),
        ]);
        let results = index.search("common", 2);
        assert!(results.len() <= 2);

        let results = index.search("common", 1);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_multiple_documents_ranked_results() {
        let index = build_index_from_docs(vec![
            ("irrelevant.txt", "apples oranges bananas fruit salad"),
            ("relevant.txt", "database query optimization indexing search"),
            ("most_relevant.txt", "search search search engine search ranking search"),
        ]);
        let results = index.search("search", 10);
        assert!(!results.is_empty());
        // most_relevant.txt has the most "search" occurrences
        assert_eq!(results[0].path, "most_relevant.txt");
        // irrelevant.txt should not appear (no "search" term)
        for r in &results {
            assert_ne!(r.path, "irrelevant.txt");
        }
    }

    #[test]
    fn test_tokenize_basic() {
        let tokens = tokenize("Hello World");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_cjk_characters() {
        // CJK chars are alphanumeric in Rust, so they concatenate into one token
        let tokens = tokenize("你好世界");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0], "你好世界");

        // Space-separated CJK produces separate tokens
        let tokens = tokenize("你好 世界");
        assert_eq!(tokens.len(), 2);
        assert!(tokens.contains(&"你好".to_string()));
        assert!(tokens.contains(&"世界".to_string()));
    }

    #[test]
    fn test_build_from_directory() {
        let dir = std::env::temp_dir().join("bm25_test_build");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("test.md"), "hello world bm25 test").unwrap();

        let index = Bm25Index::build(&dir).unwrap();
        let results = index.search("bm25", 5);
        assert!(!results.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }
}
