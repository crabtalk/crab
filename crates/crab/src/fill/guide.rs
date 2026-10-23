//! A credential's guide, Markdown, as terminal text: paragraphs, lists with
//! their numbers or bullets, bold, italics and code styled, and each link's
//! URL after its text, where the terminal makes it clickable.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const ITALIC: &str = "\x1b[3m";
const UNDERLINE: &str = "\x1b[4m";
const RESET: &str = "\x1b[0m";

/// `markdown` as lines for the terminal, each indented by `indent`.
pub fn render(markdown: &str, indent: &str) -> String {
    let mut out = String::new();
    let mut line = String::new();
    // Each open list's next number, or `None` for a bulleted one.
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut link: Option<(String, String)> = None;
    let end = |line: &mut String, out: &mut String| {
        if !line.is_empty() {
            out.push_str(indent);
            out.push_str(line);
            out.push_str(RESET);
            out.push('\n');
            line.clear();
        }
    };
    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Paragraph) if !out.is_empty() && lists.is_empty() => out.push('\n'),
            Event::End(TagEnd::Paragraph | TagEnd::Item) => end(&mut line, &mut out),
            Event::Start(Tag::List(first)) => {
                end(&mut line, &mut out);
                if !out.is_empty() && lists.is_empty() {
                    out.push('\n');
                }
                lists.push(first);
            }
            Event::End(TagEnd::List(_)) => {
                end(&mut line, &mut out);
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                end(&mut line, &mut out);
                line.push_str(&"  ".repeat(lists.len().saturating_sub(1)));
                match lists.last_mut() {
                    Some(Some(number)) => {
                        line.push_str(&format!("{DIM}{number}.{RESET} "));
                        *number += 1;
                    }
                    _ => line.push_str(&format!("{DIM}•{RESET} ")),
                }
            }
            Event::Start(Tag::Strong) => line.push_str(BOLD),
            Event::Start(Tag::Emphasis) => line.push_str(ITALIC),
            Event::End(TagEnd::Strong | TagEnd::Emphasis) => line.push_str(RESET),
            Event::Start(Tag::Link { dest_url, .. }) => {
                link = Some((dest_url.into_string(), String::new()));
                line.push_str(UNDERLINE);
            }
            Event::End(TagEnd::Link) => {
                line.push_str(RESET);
                if let Some((url, text)) = link.take()
                    && url != text
                {
                    line.push_str(&format!(" ({url})"));
                }
            }
            Event::Text(text) => {
                if let Some((_, said)) = &mut link {
                    said.push_str(&text);
                }
                line.push_str(&text);
            }
            Event::Code(code) => line.push_str(&format!("{BOLD}{code}{RESET}")),
            Event::SoftBreak => line.push(' '),
            Event::HardBreak => end(&mut line, &mut out),
            _ => {}
        }
    }
    end(&mut line, &mut out);
    out
}
