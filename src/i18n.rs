use std::sync::{
    LazyLock,
    atomic::{AtomicUsize, Ordering},
};

include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));

static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static SYSTEM: LazyLock<usize> = LazyLock::new(|| resolve(sys_locale::get_locales()));

fn resolve(locales: impl IntoIterator<Item = impl AsRef<str>>) -> usize {
    locales
        .into_iter()
        .find_map(|tag| {
            let raw = tag.as_ref().split(['-', '_', '.']).next()?;
            let code = if raw.eq_ignore_ascii_case("no") {
                "nb"
            } else {
                raw
            };
            LANGUAGES
                .iter()
                .position(|(id, _)| id.eq_ignore_ascii_case(code))
        })
        .unwrap_or(0)
}

pub fn activate(preference: Option<&str>) {
    let index = preference.map_or_else(|| *SYSTEM, |code| resolve([code]));
    ACTIVE.store(index, Ordering::Relaxed);
}

impl Text {
    pub fn get(self) -> &'static str {
        CATALOGS[ACTIVE.load(Ordering::Relaxed)][self as usize]
    }

    /// Background services keep a language-independent message until it is displayed.
    pub fn english(self) -> &'static str {
        CATALOGS[0][self as usize]
    }
}

pub fn notice(message: &str) -> &str {
    CATALOGS[0]
        .iter()
        .position(|&source| source == message)
        .map_or(message, |key| CATALOGS[ACTIVE.load(Ordering::Relaxed)][key])
}

pub fn decimal(text: String) -> String {
    match LANGUAGES[ACTIVE.load(Ordering::Relaxed)].0 {
        "en" => text,
        _ => text.replace('.', ","),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_system_preferences_and_regional_variants() {
        assert_eq!(LANGUAGES[resolve(["xx", "fr-CH"])].0, "fr");
        assert_eq!(LANGUAGES[resolve(["pt_BR"])].0, "pt");
        assert_eq!(LANGUAGES[resolve(["no-NO"])].0, "nb");
        assert_eq!(LANGUAGES[resolve(["zz"])].0, "en");
        assert_eq!(LANGUAGES.len(), 20);
    }
}
