use berm_lang::http::Response;
use url::Url;

pub(crate) const ACCEPT: (&str, &str) = ("Accept", "text/markdown");

/// The llms.txt filename convention uses index.html.md for a directory URL.
pub(crate) fn copy_url(url: &Url) -> Url {
    let mut copy = url.clone();
    copy.set_query(None);
    let path = if url.path().ends_with('/') {
        format!("{}index.html.md", url.path())
    } else {
        format!("{}.md", url.path())
    };
    copy.set_path(&path);
    copy
}

fn media_type(value: &str) -> &str {
    value.split(';').next().unwrap_or_default().trim()
}

pub(crate) fn is_markdown(value: &str) -> bool {
    media_type(value).eq_ignore_ascii_case(ACCEPT.1)
}

pub(crate) fn has_copy(response: &Response) -> bool {
    response.is_success()
        && !response.body.is_empty()
        && response.header("content-type").is_some_and(|kind| {
            let kind = media_type(kind);
            [ACCEPT.1, "text/x-markdown", "text/plain"]
                .iter()
                .any(|media_type| kind.eq_ignore_ascii_case(media_type))
        })
}

pub(crate) fn negotiated(response: &Response) -> bool {
    response.is_success()
        && !response.body.is_empty()
        && response.header("content-type").is_some_and(is_markdown)
}
