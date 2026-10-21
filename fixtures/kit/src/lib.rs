//! The reference kit: a secret, a host under `berm.http.fetch`, and a table,
//! for each program that names it in `#[program(uses)]`.

#![no_std]

extern crate alloc;

use berm_lang::Result;

berm_lang::kit!(
    syscalls = { "berm.http.fetch" = ["kit.example.com"] },
    secrets = ["kit"],
    database = "migrations",
);

/// Adds `note` to `kit_notes`, and answers how many notes it holds.
pub fn note(note: &str) -> Result<u64> {
    berm_lang::rows::<()>("INSERT INTO kit_notes (note) VALUES (?)", &(note,))?;
    let counted = berm_lang::rows::<(u64,)>("SELECT COUNT(*) FROM kit_notes", &[(); 0])?;
    Ok(counted.first().map_or(0, |(count,)| *count))
}
