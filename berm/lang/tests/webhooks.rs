use berm_lang::{Result, tool};

/// Receive standard event
#[tool(webhook)]
fn standard() -> Result<()> {
    Ok(())
}

/// Receive Stripe event
#[tool(webhook = "stripe")]
fn stripe() -> Result<()> {
    Ok(())
}

#[test]
fn both_schemes_generate_callable_tool_exports() {
    standard().unwrap();
    stripe().unwrap();
    let _: extern "C" fn() = berm_tool_standard;
    let _: extern "C" fn() = berm_tool_stripe;
}
