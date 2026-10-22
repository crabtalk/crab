//! What a manifest is read from: an image's `.berm.abi`, `.berm.program` and
//! `.berm.kits` sections, and what it refuses.

use berm_api::{
    ABI_SECTION, ABI_VERSION, Deps, Image, KITS_SECTION, Kit, Manifest, PROGRAM_SECTION,
    ProgramRecord,
};

/// A wasm module holding one custom section, `padding`, empty.
const EMPTY: &[u8] = b"\0asm\x01\0\0\0\0\x08\x07padding";

fn tool(name: &str, krate: &str) -> String {
    format!(
        r#"{{"tool":{{"name":"{name}","description":"d","parameters":{{"type":"object"}},"public":true}},"abi_version":{ABI_VERSION},"crate":"{krate}"}}"#
    )
}

fn program() -> Vec<u8> {
    serde_json::to_vec(&ProgramRecord {
        name: "demo".to_owned(),
        version: "0.1.0".to_owned(),
        description: String::new(),
        homepage: String::new(),
        repository: String::new(),
        readme: String::new(),
        buffer: 1024,
        deps: Deps {
            secrets: vec!["own".to_owned()],
            ..Deps::default()
        },
        database: None,
    })
    .unwrap()
}

fn kit(name: &str) -> Kit {
    Kit {
        name: name.to_owned(),
        version: "0.1.0".to_owned(),
        deps: Deps {
            secrets: vec![format!("{name}-secret")],
            ..Deps::default()
        },
        database: None,
    }
}

fn image(abi: &str, program: Option<&[u8]>, kits: &[Kit]) -> Vec<u8> {
    let mut image = Image(EMPTY)
        .with_section(ABI_SECTION, abi.as_bytes())
        .unwrap();
    if let Some(program) = program {
        image = Image(&image)
            .with_section(PROGRAM_SECTION, program)
            .unwrap();
    }
    Image(&image)
        .with_section(KITS_SECTION, &serde_json::to_vec(kits).unwrap())
        .unwrap()
}

#[test]
fn a_section_reads_back_as_appended() {
    let image = Image(EMPTY).with_section(".a", b"payload").unwrap();
    let image = Image(&image).with_section(".b", b"").unwrap();
    assert_eq!(Image(&image).section(".a").unwrap(), Some(&b"payload"[..]));
    assert_eq!(Image(&image).section(".b").unwrap(), Some(&b""[..]));
    assert_eq!(Image(&image).section(".c").unwrap(), None);
}

#[test]
fn a_section_is_appended_once() {
    let image = Image(EMPTY).with_section(".a", b"x").unwrap();
    assert!(Image(&image).with_section(".a", b"y").is_err());
}

#[test]
fn the_manifest_joins_the_three_sections() {
    let abi = tool("b", "demo") + &tool("a", "demo");
    let manifest =
        Manifest::from_image(&image(&abi, Some(&program()), &[kit("k1"), kit("k2")])).unwrap();
    assert_eq!(manifest.name, "demo");
    assert_eq!(manifest.buffer, 1024);
    let tools: Vec<&str> = manifest
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    assert_eq!(tools, ["a", "b"]);
    assert_eq!(manifest.deps.secrets, ["own", "k1-secret", "k2-secret"]);
    assert_eq!(manifest.kits.len(), 2);
}

#[test]
fn the_manifest_reads_back_from_its_json() {
    let manifest = Manifest::from_image(&image(&tool("a", "demo"), Some(&program()), &[])).unwrap();
    let json = serde_json::to_string(&manifest).unwrap();
    let read: Manifest = json.parse().unwrap();
    assert_eq!(read.tools.len(), 1);
    assert_eq!(read.buffer, 1024);
}

#[test]
fn an_image_without_its_program_section_is_refused() {
    let error = Manifest::from_image(&image(&tool("a", "demo"), None, &[])).unwrap_err();
    assert!(error.to_string().contains(PROGRAM_SECTION), "{error}");
}

#[test]
fn a_tool_compiled_in_another_crate_is_refused() {
    let abi = tool("a", "demo") + &tool("b", "some-kit");
    let error = Manifest::from_image(&image(&abi, Some(&program()), &[])).unwrap_err();
    assert!(error.to_string().contains("some-kit"), "{error}");
}

#[test]
fn a_kit_named_twice_is_refused() {
    let error = Manifest::from_image(&image(
        &tool("a", "demo"),
        Some(&program()),
        &[kit("k"), kit("k")],
    ))
    .unwrap_err();
    assert!(error.to_string().contains("kit k twice"), "{error}");
}

#[test]
fn an_image_of_another_abi_is_refused() {
    let old = r#"{"abi_version":4,"tools":[]}"#;
    let error = Manifest::from_image(&image(old, Some(&program()), &[])).unwrap_err();
    assert!(error.to_string().contains("ABI version 4"), "{error}");
}

#[test]
fn an_image_with_no_tools_is_refused() {
    let error = Manifest::from_image(&image("", Some(&program()), &[])).unwrap_err();
    assert!(error.to_string().contains("no tools"), "{error}");
}
