use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

const DEFAULT_LOCALE: &str = "ja";
const DOMAIN: &str = "chatwork-cli";
const LOCALE_DIR_ENV_NAME: &str = "CHATWORK_LOCALE_DIR";
const LOCALE_ENV_NAMES: [&str; 4] = ["CHATWORK_LOCALE", "LC_ALL", "LC_MESSAGES", "LANG"];
const BUILTIN_JA_CATALOG: &str =
    include_str!("../locale/ja/LC_MESSAGES/chatwork-cli.po");

static CATALOG: OnceLock<BTreeMap<String, String>> = OnceLock::new();

pub fn gettext(msgid: &'static str) -> String {
    CATALOG
        .get_or_init(load_catalog)
        .get(msgid)
        .cloned()
        .unwrap_or_else(|| msgid.to_string())
}

pub fn gettextf(msgid: &'static str, vars: &[(&str, &str)]) -> String {
    let mut text = gettext(msgid);

    for (key, value) in vars {
        let placeholder = format!("{{{key}}}");
        text = text.replace(&placeholder, value);
    }

    text
}

fn load_catalog() -> BTreeMap<String, String> {
    let locale = selected_locale();

    if let Some(content) = load_external_catalog(&locale) {
        return parse_po_catalog(&content);
    }

    if locale == DEFAULT_LOCALE {
        return parse_po_catalog(BUILTIN_JA_CATALOG);
    }

    BTreeMap::new()
}

fn selected_locale() -> String {
    for env_name in LOCALE_ENV_NAMES {
        if let Ok(value) = env::var(env_name) {
            let normalized = normalize_locale(&value);
            if !normalized.is_empty() {
                return normalized;
            }
        }
    }

    DEFAULT_LOCALE.to_string()
}

fn normalize_locale(value: &str) -> String {
    let without_encoding = value.split('.').next().unwrap_or(value);
    let without_modifier = without_encoding.split('@').next().unwrap_or(without_encoding);
    let primary = without_modifier.split('_').next().unwrap_or(without_modifier);

    match primary {
        "" => DEFAULT_LOCALE.to_string(),
        "C" | "POSIX" => DEFAULT_LOCALE.to_string(),
        _ => primary.to_ascii_lowercase(),
    }
}

fn load_external_catalog(locale: &str) -> Option<String> {
    let base_dir = env::var(LOCALE_DIR_ENV_NAME)
        .map(PathBuf::from)
        .ok()
        .or_else(|| env::current_dir().ok().map(|dir| dir.join("locale")))?;
    let catalog_path = base_dir
        .join(locale)
        .join("LC_MESSAGES")
        .join(format!("{DOMAIN}.po"));

    fs::read_to_string(catalog_path).ok()
}

fn parse_po_catalog(content: &str) -> BTreeMap<String, String> {
    let mut messages = BTreeMap::new();
    let mut current_msgid = String::new();
    let mut current_msgstr = String::new();
    let mut active_field = ActiveField::None;
    let mut has_entry = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            flush_entry(&mut messages, &mut current_msgid, &mut current_msgstr, &mut has_entry);
            active_field = ActiveField::None;
            continue;
        }

        if trimmed.starts_with('#') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("msgid ") {
            flush_entry(&mut messages, &mut current_msgid, &mut current_msgstr, &mut has_entry);
            current_msgid = parse_po_string(rest);
            current_msgstr.clear();
            active_field = ActiveField::Msgid;
            has_entry = true;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("msgstr ") {
            current_msgstr = parse_po_string(rest);
            active_field = ActiveField::Msgstr;
            continue;
        }

        if trimmed.starts_with('"') {
            let fragment = parse_po_string(trimmed);
            match active_field {
                ActiveField::Msgid => current_msgid.push_str(&fragment),
                ActiveField::Msgstr => current_msgstr.push_str(&fragment),
                ActiveField::None => {}
            }
        }
    }

    flush_entry(&mut messages, &mut current_msgid, &mut current_msgstr, &mut has_entry);
    messages
}

fn flush_entry(
    messages: &mut BTreeMap<String, String>,
    current_msgid: &mut String,
    current_msgstr: &mut String,
    has_entry: &mut bool,
) {
    if *has_entry && !current_msgid.is_empty() && !current_msgstr.is_empty() {
        messages.insert(current_msgid.clone(), current_msgstr.clone());
    }

    current_msgid.clear();
    current_msgstr.clear();
    *has_entry = false;
}

fn parse_po_string(value: &str) -> String {
    let Some(inner) = value.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) else {
        return String::new();
    };

    let mut parsed = String::new();
    let mut chars = inner.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => parsed.push('\n'),
                Some('r') => parsed.push('\r'),
                Some('t') => parsed.push('\t'),
                Some('"') => parsed.push('"'),
                Some('\\') => parsed.push('\\'),
                Some(other) => {
                    parsed.push('\\');
                    parsed.push(other);
                }
                None => parsed.push('\\'),
            }
        } else {
            parsed.push(ch);
        }
    }

    parsed
}

#[derive(Clone, Copy)]
enum ActiveField {
    None,
    Msgid,
    Msgstr,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_po_catalog_reads_translations() {
        let catalog = parse_po_catalog(
            r#"
msgid ""
msgstr ""
"Language: ja\n"

msgid "Hello."
msgstr "こんにちは。"

msgid "Line 1\n"
"Line 2"
msgstr "1 行目\n"
"2 行目"
"#,
        );

        assert_eq!(catalog.get("Hello."), Some(&"こんにちは。".to_string()));
        assert_eq!(
            catalog.get("Line 1\nLine 2"),
            Some(&"1 行目\n2 行目".to_string())
        );
    }

    #[test]
    fn gettextf_replaces_named_placeholders() {
        let rendered = gettextf(
            "Sent the message. room_id={room_id} message_id={message_id}",
            &[("room_id", "123"), ("message_id", "456")],
        );

        assert!(rendered.contains("123"));
        assert!(rendered.contains("456"));
    }

    #[test]
    fn normalize_locale_treats_c_locale_as_default_locale() {
        assert_eq!(normalize_locale("C.UTF-8"), "ja");
        assert_eq!(normalize_locale("POSIX"), "ja");
    }
}
