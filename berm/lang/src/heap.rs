//! The guest's allocator, dlmalloc, which grows the guest's own memory the
//! first time something allocates. A service that never allocates never grows
//! it.
#![cfg(all(target_arch = "wasm32", not(feature = "std")))]

#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;
