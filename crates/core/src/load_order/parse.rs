//! The pack-supplied rules file. Two columns are enough because every rule is relative to the pack
//! holding the file: `BEFORE` or `AFTER`, then the other pack's name. The game ignores the whole
//! `whmm\` folder, which is why this is plain text and not a DB table.
use super::{Rule, normalize_pack_name, pack_key};
use crate::localization::Message;

/// Packed-file path of the rules file inside a pack.
pub const RULES_FILE_PATH: &str = "whmm\\load_order.whmm";

/// Whether a packed-file path names the rules file, whatever its separators or case.
pub fn is_rules_file_path(path: &str) -> bool {
    path.trim().replace('/', "\\").to_lowercase() == RULES_FILE_PATH
}

/// Decodes the raw packed file like the original `decodePackedTextBuffer`: UTF-16 LE when it
/// starts with that BOM, UTF-8 otherwise.
pub fn parse_rules_bytes(pack_name: &str, bytes: &[u8]) -> (Vec<Rule>, Vec<Message>) {
    let mut warnings = Vec::new();
    let text = if let Some(utf16) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        // A trailing odd byte cannot be a UTF-16 unit and is ignored.
        let units: Vec<u16> = utf16
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_le_bytes(pair))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        match std::str::from_utf8(bytes) {
            Ok(text) => text.to_owned(),
            Err(_) => {
                warnings.push(crate::message!(
                    "{}: the load order rules file is not valid UTF-8; unreadable characters were replaced",
                    "{}: файл правил порядка загрузки не в кодировке UTF-8; нечитаемые символы заменены",
                    pack_name
                ));
                String::from_utf8_lossy(bytes).into_owned()
            }
        }
    };
    let (rules, mut line_warnings) = parse_rules_file(pack_name, &text);
    warnings.append(&mut line_warnings);
    (rules, warnings)
}

/// Tolerant like the original: any capitalisation, tabs or runs of spaces between the columns, CRLF
/// or LF, `#` comment lines and blank lines. Bad lines are skipped with a warning naming the line.
pub fn parse_rules_file(pack_name: &str, text: &str) -> (Vec<Rule>, Vec<Message>) {
    let own = normalize_pack_name(pack_name);
    let own_key = own.to_lowercase();
    let mut rules = Vec::new();
    let mut warnings = Vec::new();
    if own.is_empty() {
        return (rules, warnings);
    }
    // Rust's `trim` keeps U+FEFF, which JavaScript's `trim` in the original removed.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let number = index + 1;
        match parse_line(line) {
            Ok((_, other)) if pack_key(&other) == own_key => {
                // The original drops self-references silently; a warning tells the author why.
                warnings.push(crate::message!(
                    "{}: line {}: a pack cannot be ordered relative to itself",
                    "{}: строка {}: пак нельзя упорядочить относительно самого себя",
                    own,
                    number
                ));
            }
            Ok((true, other)) => rules.push(Rule::pack(&own, &own, &other)),
            Ok((false, other)) => rules.push(Rule::pack(&own, &other, &own)),
            Err(problem) => warnings.push(
                crate::message!("{}: line {}: ", "{}: строка {}: ", own, number).concat(problem),
            ),
        }
    }
    (rules, warnings)
}

/// One non-comment line as (is `BEFORE`, other pack), or why it is not a rule.
fn parse_line(line: &str) -> Result<(bool, String), Message> {
    let columns: Vec<&str> = line.split_whitespace().collect();
    let [relation, name] = columns.as_slice() else {
        return Err(if columns.len() < 2 {
            crate::message!(
                "expected two columns: BEFORE or AFTER, then a pack name",
                "ожидались два столбца: BEFORE или AFTER, затем имя пака"
            )
        } else {
            crate::message!(
                "expected two columns but found more; pack names cannot contain spaces",
                "ожидались два столбца, но найдено больше; имена паков не могут содержать пробелы"
            )
        });
    };
    let is_before = if relation.eq_ignore_ascii_case("BEFORE") {
        true
    } else if relation.eq_ignore_ascii_case("AFTER") {
        false
    } else {
        return Err(crate::message!(
            "unknown relation \"{}\"; expected BEFORE or AFTER",
            "неизвестное отношение «{}»; ожидалось BEFORE или AFTER",
            relation
        ));
    };
    let other = normalize_pack_name(name);
    if other.is_empty() {
        return Err(crate::message!("missing pack name", "не указано имя пака"));
    }
    Ok((is_before, other))
}
