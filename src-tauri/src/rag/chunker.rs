use crate::rag::extractor::FileType;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub content: String,
    pub start_line: u32,
    pub end_line: u32,
    pub heading: Option<String>,
}

const TARGET_TOKENS: usize = 300;
const MAX_TOKENS: usize = 500;
const OVERLAP_TOKENS: usize = 50;

/// Estimate token count. For CJK-heavy text, count characters / 2 as a rough
/// approximation (CJK characters typically map to 1-2 tokens each).
/// For Latin/whitespace-separated text, use word count.
fn estimate_tokens(text: &str) -> usize {
    let cjk_chars = text.chars().filter(|c| is_cjk(*c)).count();
    if cjk_chars > text.chars().count() / 3 {
        // CJK-dominant: ~1.5 chars per token on average
        (text.chars().count() * 2 + 2) / 3
    } else {
        // Latin-dominant: whitespace splitting
        text.split_whitespace().count().max(1)
    }
}

/// Check if a character is in a CJK Unicode range.
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility Ideographs
        | '\u{3000}'..='\u{303F}' // CJK Symbols and Punctuation
        | '\u{3040}'..='\u{309F}' // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{AC00}'..='\u{D7AF}' // Hangul Syllables
        | '\u{FF00}'..='\u{FFEF}' // Halfwidth and Fullwidth Forms
    )
}

/// Split text into chunks based on file type.
pub fn chunk_text(content: &str, file_type: FileType) -> Vec<Chunk> {
    match file_type {
        FileType::Markdown => chunk_markdown(content),
        FileType::Code => chunk_code(content),
        FileType::PlainText | FileType::Pdf | FileType::Docx | FileType::Xlsx => chunk_plain(content),
        FileType::Unsupported => vec![],
    }
}

/// Markdown: split by ## headings, then subdivide large sections.
fn chunk_markdown(content: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return vec![];
    }

    let mut sections: Vec<(Option<String>, u32, u32)> = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut section_start: u32 = 0;

    for (i, line) in lines.iter().enumerate() {
        if line.starts_with('#') {
            if i > 0 {
                sections.push((current_heading.clone(), section_start, i as u32 - 1));
            }
            current_heading = Some(line.trim_start_matches('#').trim().to_string());
            section_start = i as u32;
        }
    }
    sections.push((current_heading, section_start, lines.len() as u32 - 1));

    let mut chunks = Vec::new();
    for (heading, start, end) in sections {
        let section_text: String = lines[start as usize..=end as usize].join("\n");
        if estimate_tokens(&section_text) <= MAX_TOKENS {
            chunks.push(Chunk {
                content: section_text,
                start_line: start + 1,
                end_line: end + 1,
                heading: heading.clone(),
            });
        } else {
            // Subdivide large section
            let sub = subdivide_lines(&lines[start as usize..=end as usize], start, heading);
            chunks.extend(sub);
        }
    }
    chunks
}

/// Code: split by blank-line-separated blocks, then subdivide.
fn chunk_code(content: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return vec![];
    }

    let mut chunks = Vec::new();
    let mut block_start: usize = 0;
    let mut blank_count = 0;

    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            blank_count += 1;
        } else {
            if blank_count >= 2 && i > block_start {
                let block: String = lines[block_start..i].join("\n");
                if estimate_tokens(&block) > 0 {
                    let sub = subdivide_lines(&lines[block_start..i], block_start as u32, None);
                    chunks.extend(sub);
                }
                block_start = i;
            }
            blank_count = 0;
        }
    }

    // Last block
    if block_start < lines.len() {
        let sub = subdivide_lines(&lines[block_start..], block_start as u32, None);
        chunks.extend(sub);
    }

    chunks
}

/// Plain text: split by double newlines (paragraphs).
fn chunk_plain(content: &str) -> Vec<Chunk> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return vec![];
    }

    let mut chunks = Vec::new();
    let mut para_start: usize = 0;
    let mut prev_blank = false;

    for (i, line) in lines.iter().enumerate() {
        let is_blank = line.trim().is_empty();
        if is_blank && !prev_blank && i > para_start {
            let sub = subdivide_lines(&lines[para_start..i], para_start as u32, None);
            chunks.extend(sub);
            para_start = i + 1;
        }
        prev_blank = is_blank;
    }

    if para_start < lines.len() {
        let sub = subdivide_lines(&lines[para_start..], para_start as u32, None);
        chunks.extend(sub);
    }

    chunks
}

/// Subdivide a set of lines into chunks of ~TARGET_TOKENS with OVERLAP_TOKENS overlap.
fn subdivide_lines(lines: &[&str], base_line: u32, heading: Option<String>) -> Vec<Chunk> {
    if lines.is_empty() {
        return vec![];
    }

    let full_text: String = lines.join("\n");
    if estimate_tokens(&full_text) <= MAX_TOKENS {
        return vec![Chunk {
            content: full_text,
            start_line: base_line + 1,
            end_line: base_line + lines.len() as u32,
            heading,
        }];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < lines.len() {
        let mut end = start;
        let mut tokens = 0usize;

        while end < lines.len() && tokens < TARGET_TOKENS {
            tokens += estimate_tokens(lines[end]) + 1; // +1 for newline
            end += 1;
        }

        let chunk_text: String = lines[start..end].join("\n");
        chunks.push(Chunk {
            content: chunk_text,
            start_line: base_line + start as u32 + 1,
            end_line: base_line + end as u32,
            heading: heading.clone(),
        });

        if end >= lines.len() {
            break;
        }

        // Move start back by overlap
        let mut overlap_tokens = 0;
        let mut new_start = end;
        while new_start > start && overlap_tokens < OVERLAP_TOKENS {
            new_start -= 1;
            overlap_tokens += estimate_tokens(lines[new_start]) + 1;
        }
        start = if new_start > start { new_start } else { end };
    }

    chunks
}
