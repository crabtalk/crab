//! A credential's guide as the terminal shows it: lists keep their numbers,
//! and a link's URL follows its text.

use crab_agent::fill::guide;

#[test]
fn a_guide_keeps_its_numbers_and_shows_each_links_url() {
    let rendered = guide::render(
        "Make a **restricted** key.\n\n1. Open [the dashboard](https://dashboard.stripe.com/apikeys).\n2. Copy https://example.com as it is.\n",
    );
    let lines: Vec<String> = rendered
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect()
        })
        .collect();
    assert_eq!(
        lines,
        [
            "Make a restricted key.",
            "",
            "1. Open the dashboard (https://dashboard.stripe.com/apikeys).",
            "2. Copy https://example.com as it is.",
        ]
    );
}
