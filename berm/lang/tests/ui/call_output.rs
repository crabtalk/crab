use berm_lang::{Result, tool, workflow::Call};
/// Read a number.
#[tool]
fn number() -> Result<u64> {
    Ok(1)
}
fn consume<C: Call<Output = String>>() {}
fn main() {
    consume::<NumberCall>();
}
