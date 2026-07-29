//! Offline dictionary search (FR-1..FR-5).
//!
//! Query routing:
//! - contains CJK        -> hanzi search: exact, then prefix, then substring
//! - otherwise           -> latin search: pinyin exact, pinyin prefix, then
//!                          English full-text (FTS5, prefix tokens)

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::error::AppError;

const COLS: &str = "id, traditional, simplified, pinyin_marks, definitions";

#[derive(Debug, Clone, Serialize)]
pub struct EntrySummary {
    pub id: i64,
    pub traditional: String,
    pub simplified: String,
    pub pinyin_marks: String,
    pub definitions: String,
}

#[derive(Debug, Serialize)]
pub struct Segment {
    pub surface: String,
    pub entry: Option<EntrySummary>,
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<EntrySummary> {
    Ok(EntrySummary {
        id: row.get(0)?,
        traditional: row.get(1)?,
        simplified: row.get(2)?,
        pinyin_marks: row.get(3)?,
        definitions: row.get(4)?,
    })
}

fn is_cjk(c: char) -> bool {
    ('\u{3400}'..='\u{4DBF}').contains(&c)
        || ('\u{4E00}'..='\u{9FFF}').contains(&c)
        || ('\u{F900}'..='\u{FAFF}').contains(&c)
}

fn contains_cjk(s: &str) -> bool {
    s.chars().any(is_cjk)
}

/// Normalize typed latin input to the toneless letters-only form stored in
/// `entries.pinyin_flat`: strips tone marks, tone digits, spaces and
/// apostrophes; 'ü' becomes 'v' (matching the importer).
fn flat_pinyin(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        let mapped = match c {
            'ā' | 'á' | 'ǎ' | 'à' => 'a',
            'ē' | 'é' | 'ě' | 'è' => 'e',
            'ī' | 'í' | 'ǐ' | 'ì' => 'i',
            'ō' | 'ó' | 'ǒ' | 'ò' => 'o',
            'ū' | 'ú' | 'ǔ' | 'ù' => 'u',
            'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' | 'ü' => 'v',
            '1'..='5' => continue,
            other => other.to_ascii_lowercase(),
        };
        if mapped.is_ascii_lowercase() {
            out.push(mapped);
        }
    }
    out
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn push_rows(
    stmt: &mut rusqlite::Statement<'_>,
    params: &[&dyn rusqlite::types::ToSql],
    limit: usize,
    seen: &mut HashSet<i64>,
    out: &mut Vec<EntrySummary>,
) -> Result<(), AppError> {
    let rows = stmt.query_map(params, row_to_entry)?;
    for row in rows {
        let entry = row?;
        if seen.insert(entry.id) {
            out.push(entry);
            if out.len() >= limit {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn search_hanzi(conn: &Connection, q: &str, limit: usize) -> Result<Vec<EntrySummary>, AppError> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let like_prefix = format!("{}%", escape_like(q));
    let like_contains = format!("%{}%", escape_like(q));

    {
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLS} FROM entries WHERE simplified = ?1 OR traditional = ?1 \
             ORDER BY char_len, id LIMIT ?2"
        ))?;
        push_rows(&mut stmt, rusqlite::params![q, limit as i64], limit, &mut seen, &mut out)?;
    }
    {
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLS} FROM entries WHERE \
             (simplified LIKE ?1 ESCAPE '\\' OR traditional LIKE ?1 ESCAPE '\\') \
             ORDER BY char_len, id LIMIT ?2"
        ))?;
        push_rows(&mut stmt, rusqlite::params![like_prefix, limit as i64], limit, &mut seen, &mut out)?;
    }
    {
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLS} FROM entries WHERE \
             (simplified LIKE ?1 ESCAPE '\\' OR traditional LIKE ?1 ESCAPE '\\') \
             ORDER BY char_len, id LIMIT ?2"
        ))?;
        push_rows(&mut stmt, rusqlite::params![like_contains, limit as i64], limit, &mut seen, &mut out)?;
    }
    Ok(out)
}

fn search_latin(conn: &Connection, q: &str, limit: usize) -> Result<Vec<EntrySummary>, AppError> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let flat = flat_pinyin(q);

    if !flat.is_empty() {
        // 1) exact pinyin match (e.g. "kaixin" -> 开心)
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLS} FROM entries WHERE pinyin_flat = ?1 ORDER BY char_len, id LIMIT ?2"
        ))?;
        push_rows(&mut stmt, rusqlite::params![flat, limit as i64], limit, &mut seen, &mut out)?;
        // 2) pinyin prefix (skip 1-letter queries to avoid noise)
        if flat.len() >= 2 && out.len() < limit {
            let mut stmt = conn.prepare(&format!(
                "SELECT {COLS} FROM entries WHERE pinyin_flat LIKE ?1 || '%' \
                 ORDER BY char_len, id LIMIT ?2"
            ))?;
            push_rows(&mut stmt, rusqlite::params![flat, limit as i64], limit, &mut seen, &mut out)?;
        }
    }

    // 3) English full-text search, each token as a prefix term
    if out.len() < limit {
        let tokens: Vec<String> = q
            .split_whitespace()
            .map(|t| {
                t.chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '\'')
                    .collect::<String>()
            })
            .filter(|t| !t.is_empty())
            .map(|t| format!("\"{t}\"*"))
            .collect();
        if !tokens.is_empty() {
            let m = tokens.join(" ");
            let mut stmt = conn.prepare(&format!(
                "SELECT {COLS} FROM entries e JOIN fts_english f ON f.entry_id = e.id \
                 WHERE fts_english MATCH ?1 ORDER BY e.char_len, e.id LIMIT ?2"
            ))?;
            push_rows(&mut stmt, rusqlite::params![m, limit as i64], limit, &mut seen, &mut out)?;
        }
    }
    Ok(out)
}

pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<EntrySummary>, AppError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.clamp(1, 200);
    if contains_cjk(q) {
        search_hanzi(conn, q, limit)
    } else {
        search_latin(conn, q, limit)
    }
}

pub fn get_entry(conn: &Connection, id: i64) -> Result<EntrySummary, AppError> {
    conn.query_row(
        &format!("SELECT {COLS} FROM entries WHERE id = ?1"),
        rusqlite::params![id],
        row_to_entry,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("entry {id}")),
        other => AppError::Db(other),
    })
}

fn lookup_exact(conn: &Connection, word: &str) -> Result<Option<EntrySummary>, AppError> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLS} FROM entries WHERE simplified = ?1 ORDER BY char_len, id LIMIT 1"
            ),
            rusqlite::params![word],
            row_to_entry,
        )
        .optional()?)
}

/// FR-5: greedy longest-match segmentation of pasted Chinese text.
pub fn segment_lookup(
    conn: &Connection,
    text: &str,
    max_word_len: usize,
) -> Result<Vec<Segment>, AppError> {
    let chars: Vec<char> = text.trim().chars().collect();
    let mut segments = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !is_cjk(chars[i]) {
            // Group runs of non-CJK characters (punctuation, latin, digits).
            let start = i;
            while i < chars.len() && !is_cjk(chars[i]) {
                i += 1;
            }
            segments.push(Segment {
                surface: chars[start..i].iter().collect(),
                entry: None,
            });
            continue;
        }
        let max = max_word_len.min(chars.len() - i).max(1);
        let mut hit: Option<(usize, EntrySummary)> = None;
        for len in (1..=max).rev() {
            let cand: String = chars[i..i + len].iter().collect();
            if let Some(entry) = lookup_exact(conn, &cand)? {
                hit = Some((len, entry));
                break;
            }
        }
        match hit {
            Some((len, entry)) => {
                segments.push(Segment {
                    surface: chars[i..i + len].iter().collect(),
                    entry: Some(entry),
                });
                i += len;
            }
            None => {
                segments.push(Segment {
                    surface: chars[i].to_string(),
                    entry: None,
                });
                i += 1;
            }
        }
    }
    Ok(segments)
}
