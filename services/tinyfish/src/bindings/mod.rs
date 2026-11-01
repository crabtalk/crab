use berm_lang::http::Client;
pub(crate) mod agent;
pub(crate) mod fetch;
pub(crate) mod search;

const CLIENT: Client<'static> = Client {
    name: "TinyFish",
    headers: &[("X-API-Key", "{secret:TINYFISH_API_KEY}")],
};
