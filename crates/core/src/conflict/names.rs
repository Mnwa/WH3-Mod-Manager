//! Name rules shared by the compatibility checks, kept identical to the original manager.
use std::cmp::Ordering;

/// Packed paths are compared case-insensitively with Windows separators.
pub(super) fn normalize_path(name: &str) -> String {
    name.replace('/', "\\").to_lowercase()
}

/// Pack header dependencies may carry a folder; only the file name identifies a pack.
pub(super) fn normalize_pack_name(name: &str) -> String {
    let name = name.replace('/', "\\");
    name.rsplit('\\').next().unwrap_or_default().to_lowercase()
}

/// `normalized` must come from [`normalize_path`].
pub(super) fn is_startpos(normalized: &str) -> bool {
    normalized == "startpos.esf" || normalized.ends_with("\\startpos.esf")
}

pub(super) fn base_name(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or(path)
}

/// The original manager's DB file priority: UTF-16 code-unit order, except that a name which is
/// a prefix of another sorts after it. The first name wins a key collision.
pub(super) fn compare_names(first: &str, second: &str) -> Ordering {
    let mut first = first.encode_utf16();
    let mut second = second.encode_utf16();
    loop {
        match (first.next(), second.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Greater,
            (Some(_), None) => return Ordering::Less,
            (Some(a), Some(b)) if a != b => return a.cmp(&b),
            _ => {}
        }
    }
}
