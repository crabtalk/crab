use anyhow::{Context, Result, bail};
use berm_abi::SERVICE_SECTION;
use berm_api::{Deps, Image, ServiceRecord};
use wasmparser::{Parser, Payload};

pub fn with_deps(image: &[u8], deps: Deps) -> Result<Vec<u8>> {
    let mut record: ServiceRecord = serde_json::from_slice(
        Image(image)
            .section(SERVICE_SECTION)?
            .context("service has no metadata")?,
    )?;
    record.deps = deps;
    let mut start = 8;
    for section in Parser::new(0).parse_all(image) {
        let section = section?;
        if let Payload::CustomSection(custom) = &section
            && custom.name() == SERVICE_SECTION
        {
            let mut changed = image[..start].to_vec();
            changed.extend_from_slice(&image[custom.range().end..]);
            return Image(&changed).with_section(SERVICE_SECTION, &serde_json::to_vec(&record)?);
        }
        if let Some((_, range)) = section.as_section() {
            start = range.end;
        }
    }
    bail!("service has no metadata")
}
