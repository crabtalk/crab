use anyhow::Result;
use berm_api::{Image, Manifest};
use berm_build::fixture;
use berm_host::test::image;

#[test]
fn test_permissions_preserve_the_rest_of_the_image() -> Result<()> {
    let original = fixture::image("berm-fixture")?;
    let original = Image(&original).with_section(".keep", b"unchanged")?;
    let mut deps = Manifest::try_from(original.as_slice())?.deps;
    deps.http.fetch = vec!["bots.example".into()];
    let changed = image::with_deps(&original, deps)?;
    assert_eq!(
        Manifest::try_from(changed.as_slice())?.deps.http.fetch,
        ["bots.example"]
    );
    assert_eq!(Image(&changed).section(".keep")?, Some(&b"unchanged"[..]));
    assert_eq!(Image(&changed).imports()?, Image(&original).imports()?);
    assert!(image::with_deps(b"\0asm\x01\0\0\0", Default::default()).is_err());
    Ok(())
}
