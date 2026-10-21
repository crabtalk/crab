//! The guest's allocator, dlmalloc, which grows the guest's own memory the
//! first time something allocates. A program that never allocates never grows
//! it.
#![cfg(target_arch = "wasm32")]

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;
