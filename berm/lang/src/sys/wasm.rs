//! WebAssembly, where a host call is an import.

macro_rules! imports {
    ($($(#[$doc:meta])* $call:ident = $name:literal;)*) => {
        #[allow(non_snake_case)]
        mod import {
            #[link(wasm_import_module = "berm")]
            unsafe extern "C" {
                $(#[link_name = $name] pub fn $call(ptr: usize, len: usize) -> u64;)*
            }
        }

        $(
            #[allow(non_snake_case)]
            #[inline]
            pub fn $call(ptr: usize, len: usize) -> u64 {
                unsafe { import::$call(ptr, len) }
            }
        )*
    };
}

crate::abi::host_calls!(imports);

#[inline]
pub fn abort() -> ! {
    core::arch::wasm32::unreachable()
}
