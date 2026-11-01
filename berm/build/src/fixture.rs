//! Wasm images prepared by nextest setup scripts.
use anyhow::{Context, Result};
use std::{env, fs};

const IMAGE_PREFIX: &str = "BERM_TEST_IMAGE_";

/// Environment variable shared by setup scripts and image consumers.
pub fn variable(package: &str) -> String {
    format!(
        "{IMAGE_PREFIX}{}",
        package.replace('-', "_").to_ascii_uppercase()
    )
}

/// Read the image prepared for `package`; never compile during a behavior test.
pub fn image(package: &str) -> Result<Vec<u8>> {
    let path = env::var_os(variable(package)).with_context(|| {
        format!("no prepared image for {package}; configure its nextest setup script")
    })?;
    fs::read(&path).with_context(|| format!("reading prepared image {}", path.to_string_lossy()))
}
