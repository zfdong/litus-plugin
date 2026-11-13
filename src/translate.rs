//! The manifest's `i18n` table, for [`crate::Request::tr`].

use std::sync::OnceLock;

use serde_json::{Map, Value};

static I18N: OnceLock<Map<String, Value>> = OnceLock::new();

/// Keeps the `i18n` table of `manifest` (the text of `plugin.json`; empty for none).
pub fn set_manifest(manifest: &str) {
    I18N.get_or_init(|| table(manifest));
}

fn table(manifest: &str) -> Map<String, Value> {
    serde_json::from_str::<Value>(manifest).ok().and_then(|m| m.get("i18n")?.as_object().cloned()).unwrap_or_default()
}

/// `s` in `lang`, as Litus translates the manifest: the exact code, then the language without
/// its region, then any region of that language.
pub fn tr(lang: &str, s: &str) -> String {
    lookup(I18N.get().unwrap_or(&Map::new()), lang, s)
}

fn lookup(i18n: &Map<String, Value>, lang: &str, s: &str) -> String {
    let base_of = |l: &str| l.split(['-', '_']).next().unwrap_or(l).to_ascii_lowercase();
    let base = base_of(lang);
    let pick =
        |want: &dyn Fn(&str) -> bool| i18n.iter().filter(|(k, _)| want(k)).find_map(|(_, t)| t.get(s)?.as_str()).filter(|t| !t.is_empty());
    pick(&|k| k.eq_ignore_ascii_case(lang))
        .or_else(|| pick(&|k| k.eq_ignore_ascii_case(&base)))
        .or_else(|| pick(&|k| base_of(k) == base))
        .unwrap_or(s)
        .to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_as_litus_does() {
        let t = super::table(r#"{"i18n": {"de": {"Hello": "Hallo"}, "zh-CN": {"Hello": "你好"}, "fr": {"Hello": ""}}}"#);
        assert_eq!(super::lookup(&t, "de", "Hello"), "Hallo");
        assert_eq!(super::lookup(&t, "de-AT", "Hello"), "Hallo");
        assert_eq!(super::lookup(&t, "zh", "Hello"), "你好");
        // An empty translation, a missing language or string: the English.
        assert_eq!(super::lookup(&t, "fr", "Hello"), "Hello");
        assert_eq!(super::lookup(&t, "ja", "Hello"), "Hello");
        assert_eq!(super::lookup(&t, "de", "Bye"), "Bye");
        assert!(super::table("not json").is_empty());
    }
}
