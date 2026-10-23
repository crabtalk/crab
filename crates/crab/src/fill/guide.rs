//! A credential's guide, Markdown, as terminal text: paragraphs, lists with
//! their numbers or bullets, bold, italics and code styled, and each link's
//! URL after its text, where the terminal makes it clickable.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span, Text},
};

/// `markdown` as lines of styled spans.
pub fn render(markdown: &str) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut line: Vec<Span<'static>> = Vec::new();
    let mut styles = vec![Style::default()];
    // Each open list's next number, or `None` for a bulleted one.
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut link: Option<(String, String)> = None;
    let dim = Style::default().add_modifier(Modifier::DIM);

    fn end(line: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>) {
        if !line.is_empty() {
            lines.push(Line::from(std::mem::take(line)));
        }
    }
    for event in Parser::new(markdown) {
        let style = *styles.last().unwrap_or(&Style::default());
        match event {
            Event::Start(Tag::Paragraph) if !lines.is_empty() && lists.is_empty() => {
                lines.push(Line::default())
            }
            Event::End(TagEnd::Paragraph | TagEnd::Item) => end(&mut line, &mut lines),
            Event::Start(Tag::List(first)) => {
                end(&mut line, &mut lines);
                if !lines.is_empty() && lists.is_empty() {
                    lines.push(Line::default());
                }
                lists.push(first);
            }
            Event::End(TagEnd::List(_)) => {
                end(&mut line, &mut lines);
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                end(&mut line, &mut lines);
                let indent = "  ".repeat(lists.len().saturating_sub(1));
                let marker = match lists.last_mut() {
                    Some(Some(number)) => {
                        let marker = format!("{indent}{number}. ");
                        *number += 1;
                        marker
                    }
                    _ => format!("{indent}• "),
                };
                line.push(Span::styled(marker, dim));
            }
            Event::Start(Tag::Strong) => styles.push(style.add_modifier(Modifier::BOLD)),
            Event::Start(Tag::Emphasis) => styles.push(style.add_modifier(Modifier::ITALIC)),
            Event::Start(Tag::Link { dest_url, .. }) => {
                link = Some((dest_url.into_string(), String::new()));
                styles.push(style.add_modifier(Modifier::UNDERLINED));
            }
            Event::End(TagEnd::Strong | TagEnd::Emphasis) => {
                styles.pop();
            }
            Event::End(TagEnd::Link) => {
                styles.pop();
                if let Some((url, text)) = link.take()
                    && url != text
                {
                    line.push(Span::raw(format!(" ({url})")));
                }
            }
            Event::Text(text) => {
                if let Some((_, said)) = &mut link {
                    said.push_str(&text);
                }
                line.push(Span::styled(text.into_string(), style));
            }
            Event::Code(code) => line.push(Span::styled(
                code.into_string(),
                style.add_modifier(Modifier::BOLD),
            )),
            Event::SoftBreak => line.push(Span::raw(" ")),
            Event::HardBreak => end(&mut line, &mut lines),
            _ => {}
        }
    }
    end(&mut line, &mut lines);
    Text::from(lines)
}
