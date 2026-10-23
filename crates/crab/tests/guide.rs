//! A credential's guide as the terminal shows it: lists keep their numbers,
//! a link's URL follows its text, and styling is reset at each line's end.

use crab_agent::fill::guide;

/// `text` without its escape sequences.
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[test]
fn a_guide_keeps_its_numbers_and_shows_each_links_url() {
    let rendered = guide::render(
        "Make a **restricted** key.\n\n1. Open [the dashboard](https://dashboard.stripe.com/apikeys).\n2. Copy https://example.com as it is.\n",
        "  ",
    );
    assert_eq!(
        plain(&rendered),
        "  Make a restricted key.\n\n  1. Open the dashboard (https://dashboard.stripe.com/apikeys).\n  2. Copy https://example.com as it is.\n"
    );
    assert!(
        rendered
            .lines()
            .filter(|line| !line.is_empty())
            .all(|line| line.ends_with("\x1b[0m"))
    );
}
