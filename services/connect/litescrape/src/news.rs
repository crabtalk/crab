use crate::{api, types};
use alloc::string::String;
use berm_lang::{Result, serde_json::Value};

const NEWS_VERTICAL: &str = "nws";

/// Search Google's news vertical with `tbm=nws`; `num` is from 1 to 10.
#[cfg_attr(feature = "tools", berm_lang::tool)]
pub fn search_news(q: String, num: Option<i64>, hl: Option<String>) -> Result<Value> {
    api::google_search(&types::GoogleSearchInput {
        query: Some(types::GoogleSearchInputQuery {
            q: Some(Value::from(q)),
            tbm: Some(Value::from(NEWS_VERTICAL)),
            num: num.map(Value::from),
            hl,
            ..Default::default()
        }),
    })
}
