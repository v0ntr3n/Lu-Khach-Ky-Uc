//! Zero-dependency streaming MariaDB/phpMyAdmin `INSERT INTO` parser for `database team2026.sql`.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{DbError, DbResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlVal {
    Null,
    Str(String),
    Num(String),
}

impl SqlVal {
    #[must_use]
    pub fn as_str_or_empty(&self) -> &str {
        match self {
            Self::Str(s) | Self::Num(s) => s.as_str(),
            Self::Null => "",
        }
    }

    #[must_use]
    pub fn opt_string(&self) -> Option<String> {
        match self {
            Self::Null => None,
            Self::Str(s) | Self::Num(s) => Some(s.clone()),
        }
    }

    #[must_use]
    pub fn i64_or(&self, default: i64) -> i64 {
        match self {
            Self::Num(s) | Self::Str(s) => s.trim().parse().unwrap_or(default),
            Self::Null => default,
        }
    }

    #[must_use]
    pub fn i32_or(&self, default: i32) -> i32 {
        self.i64_or(i64::from(default)) as i32
    }

    #[must_use]
    pub fn i32_opt(&self) -> Option<i32> {
        match self {
            Self::Null => None,
            Self::Num(s) | Self::Str(s) => s.trim().parse().ok(),
        }
    }

    #[must_use]
    pub fn bool_or(&self, default: bool) -> bool {
        match self {
            Self::Null => default,
            Self::Num(s) | Self::Str(s) => match s.trim().to_ascii_lowercase().as_str() {
                "1" | "true" => true,
                "0" | "false" => false,
                _ => default,
            },
        }
    }

    #[must_use]
    pub fn bool_opt(&self) -> Option<bool> {
        match self {
            Self::Null => None,
            _ => Some(self.bool_or(false)),
        }
    }

    #[must_use]
    pub fn f64_opt(&self) -> Option<f64> {
        match self {
            Self::Null => None,
            Self::Num(s) | Self::Str(s) => s.trim().parse().ok(),
        }
    }

    #[must_use]
    pub fn opt_datetime_utc(&self) -> Option<DateTime<Utc>> {
        let raw = self.as_str_or_empty().trim();
        if raw.is_empty() || raw.starts_with("0000-00-00") {
            return None;
        }
        NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
            .ok()
            .map(|ndt| ndt.and_utc())
    }

    #[must_use]
    pub fn datetime_utc(&self, fallback: DateTime<Utc>) -> DateTime<Utc> {
        self.opt_datetime_utc().unwrap_or(fallback)
    }
}

pub struct SqlRowMap<'a> {
    cols: &'a HashMap<String, usize>,
    ordered_cols: &'a [String],
    vals: &'a [SqlVal],
}

impl<'a> SqlRowMap<'a> {
    #[must_use]
    pub fn get(&self, col: &str) -> &'a SqlVal {
        self.cols
            .get(&col.to_ascii_lowercase())
            .and_then(|&idx| self.vals.get(idx))
            .unwrap_or(&SqlVal::Null)
    }

    #[must_use]
    pub fn into_pairs(&self) -> Vec<(String, SqlVal)> {
        self.ordered_cols
            .iter()
            .cloned()
            .zip(self.vals.iter().cloned())
            .collect()
    }
}

pub fn stream_mysqldump_inserts<F>(sql: &str, mut visitor: F) -> DbResult<u64>
where
    F: FnMut(&str, SqlRowMap<'_>) -> DbResult<()>,
{
    let bytes = sql.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    let mut total_rows = 0_u64;

    while i < n {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if sql[i..].starts_with("--") {
            i = sql[i..].find('\n').map_or(n, |off| i + off + 1);
            continue;
        }
        if sql[i..].starts_with("/*") {
            i = sql[i + 2..].find("*/").map_or(n, |off| i + 4 + off);
            continue;
        }
        if sql[i..].starts_with("INSERT INTO `") {
            i += "INSERT INTO `".len();
            let end_tbl = sql[i..].find('`').ok_or_else(|| parse_err("unterminated table name"))? + i;
            let table = &sql[i..end_tbl];
            i = end_tbl + 1;

            let open_p = sql[i..].find('(').ok_or_else(|| parse_err("missing '('"))? + i;
            let close_p = sql[open_p..].find(')').ok_or_else(|| parse_err("missing ')'"))? + open_p;
            let ordered_cols: Vec<String> = sql[open_p + 1..close_p]
                .split(',')
                .map(|c| c.trim().trim_matches('`').to_owned())
                .collect();
            let col_map: HashMap<String, usize> = ordered_cols
                .iter()
                .enumerate()
                .map(|(idx, c)| (c.to_ascii_lowercase(), idx))
                .collect();

            let kw = sql[close_p..].find("VALUES").ok_or_else(|| parse_err("missing VALUES"))? + close_p;
            i = kw + "VALUES".len();

            while i < n {
                while i < n && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
                    i += 1;
                }
                if i < n && bytes[i] == b';' {
                    i += 1;
                    break;
                }
                if bytes[i] != b'(' {
                    return Err(parse_err("expected '(' starting VALUES tuple"));
                }
                i += 1;
                let mut vals = Vec::with_capacity(ordered_cols.len());
                while i < n {
                    while i < n && bytes[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    if bytes[i] == b'\'' {
                        i += 1;
                        let mut buf = String::new();
                        let mut chunk_start = i;
                        while i < n {
                            match bytes[i] {
                                b'\\' => {
                                    buf.push_str(&sql[chunk_start..i]);
                                    i += 1;
                                    if i >= n {
                                        return Err(parse_err("dangling backslash escape"));
                                    }
                                    let esc = match bytes[i] {
                                        b'n' => '\n',
                                        b'r' => '\r',
                                        b't' => '\t',
                                        b'0' => '\0',
                                        other => char::from(other),
                                    };
                                    buf.push(esc);
                                    i += 1;
                                    chunk_start = i;
                                }
                                b'\'' if i + 1 < n && bytes[i + 1] == b'\'' => {
                                    buf.push_str(&sql[chunk_start..=i]);
                                    i += 2;
                                    chunk_start = i;
                                }
                                b'\'' => {
                                    buf.push_str(&sql[chunk_start..i]);
                                    i += 1;
                                    break;
                                }
                                _ => i += 1,
                            }
                        }
                        vals.push(SqlVal::Str(buf));
                    } else {
                        let start = i;
                        while i < n && bytes[i] != b',' && bytes[i] != b')' {
                            i += 1;
                        }
                        let raw = sql[start..i].trim();
                        if raw.eq_ignore_ascii_case("NULL") {
                            vals.push(SqlVal::Null);
                        } else {
                            vals.push(SqlVal::Num(raw.to_owned()));
                        }
                    }
                    while i < n && bytes[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    if bytes[i] == b',' {
                        i += 1;
                    } else if bytes[i] == b')' {
                        i += 1;
                        break;
                    }
                }
                if vals.len() != ordered_cols.len() {
                    return Err(parse_err("column count mismatch in VALUES tuple"));
                }
                visitor(
                    table,
                    SqlRowMap {
                        cols: &col_map,
                        ordered_cols: &ordered_cols,
                        vals: &vals,
                    },
                )?;
                total_rows += 1;
            }
        } else {
            let mut in_quote = false;
            while i < n {
                match bytes[i] {
                    b'\\' if in_quote => i = (i + 2).min(n),
                    b'\'' => {
                        in_quote = !in_quote;
                        i += 1;
                    }
                    b';' if !in_quote => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
        }
    }
    Ok(total_rows)
}

fn parse_err(reason: &str) -> DbError {
    DbError::InvalidColumnFormat {
        column: "mysqldump",
        reason: reason.to_owned(),
    }
}
