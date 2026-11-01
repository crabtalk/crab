#![no_std]
extern crate alloc;
use berm_lang::http::Client;

const CLIENT: Client<'static> = Client {
    name: "TikHub",
    headers: &[("Authorization", "Bearer {secret:TIKHUB_API_KEY}")],
};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
