//! A service's wasm module, and the custom sections its manifest is read from.

use crate::workflow::config::Spec;
use anyhow::{Context, Result, bail, ensure};
use object::{Object, ObjectSection};
use std::collections::BTreeMap;
use wasm_encoder::{Module, RawSection};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef};

/// A wasm module's bytes.
pub struct Image<'a>(pub &'a [u8]);

impl<'a> Image<'a> {
    /// The payload of its custom section `name`, `None` when it has none.
    pub fn section(&self, name: &str) -> Result<Option<&'a [u8]>> {
        let file = object::File::parse(self.0).context("service is not a readable image")?;
        let Some(section) = file.section_by_name(name) else {
            return Ok(None);
        };
        let data = section
            .data()
            .with_context(|| format!("service's {name} section is unreadable"))?;
        Ok(Some(data))
    }

    /// The host calls it imports, by name, sorted.
    pub fn imports(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        for payload in Parser::new(0).parse_all(self.0) {
            let Payload::ImportSection(section) =
                payload.context("service is not a readable image")?
            else {
                continue;
            };
            for import in section.into_imports() {
                let import = import.context("service's imports are unreadable")?;
                if import.module == berm_abi::MODULE {
                    names.push(import.name.to_owned());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn check_workflow_exports(&self, workflows: &[Spec]) -> Result<()> {
        if workflows.is_empty() {
            return Ok(());
        }
        let mut types = Vec::new();
        let mut functions = Vec::new();
        let mut exports = BTreeMap::new();
        for payload in Parser::new(0).parse_all(self.0) {
            match payload? {
                Payload::TypeSection(section) => {
                    for ty in section.into_iter_err_on_gc_types() {
                        let ty = ty?;
                        types.push(ty.params().is_empty() && ty.results().is_empty());
                    }
                }
                Payload::ImportSection(section) => {
                    for import in section.into_imports() {
                        if let TypeRef::Func(index) = import?.ty {
                            functions.push(index);
                        }
                    }
                }
                Payload::FunctionSection(section) => {
                    for ty in section {
                        functions.push(ty?);
                    }
                }
                Payload::ExportSection(section) => {
                    for export in section {
                        let export = export?;
                        exports.insert(export.name, (export.kind, export.index));
                    }
                }
                _ => {}
            }
        }
        for spec in workflows {
            for node in &spec.entry.graph.nodes {
                let expected = format!(
                    "{}{}_{}",
                    berm_abi::WORKFLOW_PREFIX,
                    spec.entry.name,
                    node.id
                );
                ensure!(
                    node.export == expected,
                    "workflow {} node {} names an invalid control export",
                    spec.entry.name,
                    node.id
                );
                let (kind, index) = exports
                    .get(node.export.as_str())
                    .with_context(|| format!("missing workflow control export {}", node.export))?;
                ensure!(
                    *kind == ExternalKind::Func
                        && functions
                            .get(*index as usize)
                            .and_then(|index| types.get(*index as usize))
                            .copied()
                            == Some(true),
                    "workflow control {} must be a function with no parameters or results",
                    node.export
                );
            }
        }
        Ok(())
    }

    pub fn replace_section(&self, name: &str, data: &[u8]) -> Result<Vec<u8>> {
        let mut module = Module::new();
        for payload in Parser::new(0).parse_all(self.0) {
            let payload = payload?;
            if matches!(&payload,Payload::CustomSection(section) if section.name() == name) {
                continue;
            }
            if let Some((id, range)) = payload.as_section() {
                module.section(&RawSection {
                    id,
                    data: &self.0[range],
                });
            }
        }
        Image(&module.finish()).with_section(name, data)
    }

    /// Its bytes with `payload` appended as the custom section `name`.
    /// Refused when it holds a section of that name already.
    pub fn with_section(&self, name: &str, payload: &[u8]) -> Result<Vec<u8>> {
        if self.section(name)?.is_some() {
            bail!("service already holds a {name} section");
        }
        let mut body = Vec::with_capacity(name.len() + payload.len() + 5);
        leb128(&mut body, name.len());
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(payload);

        let mut image = Vec::with_capacity(self.0.len() + body.len() + 6);
        image.extend_from_slice(self.0);
        // Section id 0: a custom section.
        image.push(0);
        leb128(&mut image, body.len());
        image.extend_from_slice(&body);
        Ok(image)
    }
}

/// Append `value` as an unsigned LEB128.
fn leb128(into: &mut Vec<u8>, mut value: usize) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            into.push(byte);
            return;
        }
        into.push(byte | 0x80);
    }
}
