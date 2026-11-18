fn main() {
    compile_catalogs();
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        resource.set("ProductName", "Easy Glucose");
        resource.set("FileDescription", "Easy Glucose · Dexcom Share companion");
        if let Err(error) = resource.compile() {
            panic!("Failed to compile Windows resources: {error}");
        }
    }
}

// Generate typed message IDs and static UTF-8 tables. Missing or empty translations
// are build errors, including for platforms whose UI is not exercised by local tests.
fn compile_catalogs() {
    use std::{collections::BTreeMap, env, fmt::Write, fs, path::PathBuf};
    println!("cargo:rerun-if-changed=locales");
    let languages: Vec<[String; 2]> =
        serde_json::from_slice(&fs::read("locales/languages.json").unwrap()).unwrap();
    assert_eq!(languages[0][0], "en", "English is the fallback catalog");
    let read = |code: &str| -> BTreeMap<String, String> {
        let path = format!("locales/{code}.json");
        serde_json::from_slice(&fs::read(&path).expect(&path)).expect(&path)
    };
    let reference = read("en");
    let mut out = String::from("#[derive(Clone, Copy)]\npub enum Text {\n");
    for key in reference.keys() {
        writeln!(out, "    {key},").unwrap();
    }
    out.push_str("}\npub const LANGUAGES: &[(&str, &str)] = &[\n");
    for [code, name] in &languages {
        writeln!(out, "({code:?},{name:?}),").unwrap();
    }
    out.push_str("];\nconst CATALOGS: &[&[&str]] = &[\n");
    for [code, _] in languages {
        let messages = read(&code);
        assert!(
            reference.keys().eq(messages.keys()),
            "Message IDs differ in {code}"
        );
        out.push_str("&[\n");
        for (key, value) in messages {
            assert!(!value.trim().is_empty(), "Empty {key} in {code}");
            writeln!(out, "{value:?},").unwrap();
        }
        out.push_str("],\n");
    }
    out.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("catalogs.rs"),
        out,
    )
    .unwrap();
}
