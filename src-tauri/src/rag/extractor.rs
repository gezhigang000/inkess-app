use std::path::Path;

const MAX_INDEX_SIZE: u64 = 10 * 1024 * 1024; // 10MB

pub const SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", "target", "dist", "build", ".next",
    "__pycache__", ".venv", "venv", ".inkess", ".DS_Store",
    "vendor", "coverage", ".cache", ".parcel-cache",
];

const TEXT_EXTENSIONS: &[&str] = &[
    // Markdown / docs
    "md", "mdx", "txt", "rst", "adoc", "org",
    // Web
    "html", "htm", "css", "scss", "less", "js", "jsx", "ts", "tsx",
    "vue", "svelte",
    // Programming
    "rs", "py", "go", "java", "kt", "swift", "dart", "c", "cpp", "h", "hpp",
    "cs", "rb", "php", "lua", "r", "jl", "zig", "nim", "ex", "exs",
    "hs", "ml", "clj", "scala", "groovy",
    // Config / data
    "json", "yaml", "yml", "toml", "xml", "csv", "ini", "conf", "cfg",
    "env", "properties", "lock",
    // Shell / scripts
    "sh", "bash", "zsh", "fish", "ps1", "bat", "cmd",
    // Other
    "sql", "graphql", "gql", "proto", "dockerfile",
    "makefile", "cmake", "gradle",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    Markdown,
    Code,
    PlainText,
    Pdf,
    Docx,
    Xlsx,
    Unsupported,
}

pub fn detect_file_type(path: &Path) -> FileType {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "md" | "mdx" => FileType::Markdown,
        "txt" | "rst" | "adoc" | "org" => FileType::PlainText,
        "pdf" => FileType::Pdf,
        "docx" => FileType::Docx,
        "xlsx" | "xls" | "ods" => FileType::Xlsx,
        "" => {
            // No extension — check if filename suggests text
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            match name.as_str() {
                "readme" | "license" | "changelog" | "makefile"
                | "dockerfile" | "cmakelists.txt" | ".gitignore"
                | ".editorconfig" | ".env" => FileType::PlainText,
                _ => FileType::Unsupported,
            }
        }
        e if TEXT_EXTENSIONS.contains(&e) => {
            if matches!(e, "json" | "yaml" | "yml" | "toml" | "xml" | "csv"
                | "ini" | "conf" | "cfg" | "env" | "properties" | "lock") {
                FileType::PlainText
            } else {
                FileType::Code
            }
        }
        _ => FileType::Unsupported,
    }
}

/// Check if a file should be indexed.
pub fn should_index(path: &Path) -> bool {
    // Skip hidden files/dirs (except specific ones)
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();
            if SKIP_DIRS.iter().any(|d| name_str == *d) {
                return false;
            }
        }
    }

    // Check file size
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_INDEX_SIZE || !meta.is_file() {
            return false;
        }
    } else {
        return false;
    }

    detect_file_type(path) != FileType::Unsupported
}

/// Extract text content from a file.
pub fn extract_text(path: &Path) -> Result<(String, FileType), String> {
    let file_type = detect_file_type(path);
    if file_type == FileType::Unsupported {
        return Err("Unsupported file type".into());
    }

    // Binary document formats — dedicated extractors
    match file_type {
        FileType::Pdf => return extract_pdf(path).map(|t| (t, FileType::Pdf)),
        FileType::Docx => return extract_docx(path).map(|t| (t, FileType::Docx)),
        FileType::Xlsx => return extract_xlsx(path).map(|t| (t, FileType::Xlsx)),
        _ => {}
    }

    let bytes = std::fs::read(path)
        .map_err(|e| format!("Cannot read file: {}", e))?;

    // Check for UTF-8 BOM
    let data = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { &bytes[3..] } else { &bytes };

    // Reject likely binary files (high ratio of null bytes or control chars)
    let suspicious = data.iter().take(8192)
        .filter(|&&b| b == 0 || (b < 0x08 && b != 0x0A && b != 0x0D))
        .count();
    if suspicious > data.len().min(8192) / 20 {
        return Err("Binary file detected".into());
    }

    // Try UTF-8 first
    if let Ok(s) = std::str::from_utf8(data) {
        return Ok((s.to_string(), file_type));
    }

    // Try common CJK encodings: GBK, Shift-JIS, EUC-KR, Big5
    for encoding in &[
        encoding_rs::GBK,
        encoding_rs::SHIFT_JIS,
        encoding_rs::EUC_KR,
        encoding_rs::BIG5,
    ] {
        let (cow, _, had_errors) = encoding.decode(data);
        if !had_errors {
            return Ok((cow.into_owned(), file_type));
        }
    }

    // Fallback: lossy UTF-8
    Ok((String::from_utf8_lossy(&bytes).into_owned(), file_type))
}

fn extract_pdf(path: &Path) -> Result<String, String> {
    pdf_extract::extract_text(path)
        .map_err(|e| format!("PDF extraction failed: {}", e))
}

fn extract_docx(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open docx: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid docx: {}", e))?;
    let mut xml = String::new();
    {
        let mut doc = archive.by_name("word/document.xml")
            .map_err(|_| "Missing word/document.xml".to_string())?;
        doc.read_to_string(&mut xml).map_err(|e| format!("Read error: {}", e))?;
    }
    // Parse XML and extract text nodes
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut text = String::new();
    let mut in_t = false;
    let mut in_p = false;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == b"p" { in_p = true; }
                if local.as_ref() == b"t" { in_t = true; }
            }
            Ok(quick_xml::events::Event::End(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == b"t" { in_t = false; }
                if local.as_ref() == b"p" {
                    if in_p { text.push('\n'); }
                    in_p = false;
                }
            }
            Ok(quick_xml::events::Event::Text(ref e)) => {
                if in_t {
                    if let Ok(s) = e.unescape() {
                        text.push_str(&s);
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}

fn extract_xlsx(path: &Path) -> Result<String, String> {
    use calamine::{Reader, open_workbook_auto};
    let mut workbook: calamine::Sheets<std::io::BufReader<std::fs::File>> = open_workbook_auto(path)
        .map_err(|e| format!("Cannot open spreadsheet: {}", e))?;
    let mut text = String::new();
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    for name in sheet_names {
        if let Ok(range) = workbook.worksheet_range(&name) {
            text.push_str(&format!("## {}\n", name));
            for row in range.rows() {
                let cells: Vec<String> = row.iter().map(|c| c.to_string()).collect();
                text.push_str(&cells.join("\t"));
                text.push('\n');
            }
            text.push('\n');
        }
    }
    Ok(text)
}
