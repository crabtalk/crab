//! Drawing the form: the request beside the step on screen when the terminal
//! is wide, one column with the steps on a line above it when it is not.

use crate::fill::{Field, Form, Step};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Wrap},
};

/// The narrowest terminal the two panes are drawn side by side in.
const WIDE: u16 = 100;
/// The request's pane, when they are.
const ASIDE: u16 = 36;
/// What marks the current step and a warning: crabtalk's coral.
const ACCENT: Color = Color::Rgb(0xFF, 0x6A, 0x30);

pub fn draw(form: &Form, frame: &mut Frame) {
    let [body, keys] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
    if body.width >= WIDE {
        let [aside, main] = Layout::horizontal([Constraint::Length(ASIDE), Constraint::Min(0)])
            .spacing(1)
            .areas(body);
        frame.render_widget(request(form), aside);
        step(form, frame, main);
    } else {
        let [rail, main] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(body);
        frame.render_widget(Paragraph::new(self::rail(form)), rail);
        step(form, frame, main);
    }
    frame.render_widget(
        Paragraph::new("tab next field · ←/→ step · enter save · c copy · esc leave").style(dim()),
        keys,
    );
}

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

/// What a step stands at.
fn status(step: &Step) -> &'static str {
    match (step.saved, step.fields.iter().all(|field| field.set)) {
        (true, _) => "SAVED",
        (false, true) => "SET",
        (false, false) => "NOT SET",
    }
}

/// The left pane: where values go, the service and the steps.
fn request<'a>(form: &'a Form) -> Paragraph<'a> {
    let mut lines = vec![
        Line::styled("SAVING TO", dim()),
        Line::from(form.target.as_str()),
        Line::default(),
        Line::styled("SERVICE", dim()),
        Line::from(form.service.as_str()),
        Line::default(),
        Line::styled("STEPS", dim()),
    ];
    for (at, step) in form.steps.iter().enumerate() {
        let current = at == form.at;
        lines.push(Line::from(vec![
            Span::styled(
                if current { "▌ " } else { "  " },
                Style::default().fg(ACCENT),
            ),
            Span::styled(format!("{:02} ", at + 1), dim()),
            Span::styled(
                step.title.as_str(),
                match current {
                    true => Style::default().add_modifier(Modifier::BOLD),
                    false => dim(),
                },
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("     "),
            Span::styled(status(step), dim()),
        ]));
    }
    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::bordered().title(" crab secret fill "))
}

/// The steps on one line, for a narrow terminal.
fn rail<'a>(form: &'a Form) -> Line<'a> {
    let mut spans = Vec::new();
    for (at, step) in form.steps.iter().enumerate() {
        if at > 0 {
            spans.push(Span::raw("  "));
        }
        let style = match at == form.at {
            true => Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            false => dim(),
        };
        spans.push(Span::styled(format!("{:02} {}", at + 1, step.title), style));
    }
    Line::from(spans)
}

/// The right pane: the step on screen.
fn step(form: &Form, frame: &mut Frame, area: Rect) {
    let step = &form.steps[form.at];
    let block = Block::bordered()
        .title(Line::styled(
            format!(" {} ", step.title),
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .title(
            Line::from(format!(" Step {} of {} ", form.at + 1, form.steps.len())).right_aligned(),
        );
    let inner = block.inner(area).inner(Margin::new(1, 0));
    frame.render_widget(block, area);

    let copies = step.copy.len();
    let mut rows = vec![Constraint::Min(0)];
    rows.extend((0..copies).map(|_| Constraint::Length(2)));
    rows.extend(
        step.fields
            .iter()
            .map(|field| Constraint::Length(if field.off() { 3 } else { 2 })),
    );
    rows.push(Constraint::Length(2));
    rows.push(Constraint::Length(1));
    let areas = Layout::vertical(rows).spacing(1).split(inner);

    frame.render_widget(
        Paragraph::new(step.guide.clone()).wrap(Wrap { trim: false }),
        areas[0],
    );
    for (row, (label, value)) in step.copy.iter().enumerate() {
        let focused = form.focus == row;
        let text = Text::from(vec![
            Line::styled(label.to_uppercase(), dim()),
            Line::from(vec![
                Span::styled(value.as_str(), focus(focused)),
                Span::styled(if focused { "  c to copy" } else { "" }, dim()),
            ]),
        ]);
        frame.render_widget(Paragraph::new(text), areas[1 + row]);
    }
    for (row, field) in step.fields.iter().enumerate() {
        let area = areas[1 + copies + row];
        let focused = form.focus == copies + row;
        frame.render_widget(Paragraph::new(self::field(field, focused)), area);
        if focused {
            let x = area.x + 2 + shown(field).chars().count() as u16;
            frame.set_cursor_position((x.min(area.right().saturating_sub(1)), area.y + 1));
        }
    }
    let area = areas[1 + copies + step.fields.len()];
    let focused = form.focus == copies + step.fields.len();
    frame.render_widget(Paragraph::new(hosts(step, focused)), area);
    if focused {
        let x = area.x + 2 + step.hosts.chars().count() as u16;
        frame.set_cursor_position((x.min(area.right().saturating_sub(1)), area.y + 1));
    }
    if let Some((said, wrong)) = &form.said {
        let style = match wrong {
            true => Style::default().fg(ACCENT),
            false => dim(),
        };
        frame.render_widget(
            Paragraph::new(Line::styled(said.as_str(), style)),
            areas[areas.len() - 1],
        );
    }
}

/// The style of a copy value, focused or not.
fn focus(focused: bool) -> Style {
    match focused {
        true => Style::default().add_modifier(Modifier::REVERSED),
        false => Style::default(),
    }
}

/// What a field shows of what is typed.
fn shown(field: &Field) -> String {
    match field.secret.masked {
        true => "•".repeat(field.typed.chars().count()),
        false => field.typed.clone(),
    }
}

/// The hosts the step's secrets are sent to, as typed.
fn hosts(step: &Step, focused: bool) -> Text<'_> {
    Text::from(vec![
        Line::from(vec![
            Span::styled("Hosts", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("  sent to these alone, apart by commas", dim()),
        ]),
        Line::from(vec![
            Span::styled(
                "› ",
                if focused {
                    Style::default().fg(ACCENT)
                } else {
                    dim()
                },
            ),
            Span::raw(step.hosts.as_str()),
            Span::styled(
                if step.hosts.is_empty() {
                    "none: such as api.example.com"
                } else {
                    ""
                },
                dim(),
            ),
        ]),
    ])
}

/// A field: its label, the secret's name beside it when they differ, and
/// what is typed under them.
fn field<'a>(field: &'a Field, focused: bool) -> Text<'a> {
    let mut label = vec![Span::styled(
        field.label(),
        Style::default().add_modifier(Modifier::BOLD),
    )];
    if field.label() != field.secret.name {
        label.push(Span::styled(format!("  {}", field.secret.name), dim()));
    }
    let hint = match (field.typed.is_empty(), field.set) {
        (true, true) => "set · type to replace",
        (true, false) => "not set",
        _ => "",
    };
    let mut lines = vec![
        Line::from(label),
        Line::from(vec![
            Span::styled(
                "› ",
                if focused {
                    Style::default().fg(ACCENT)
                } else {
                    dim()
                },
            ),
            Span::raw(shown(field)),
            Span::styled(hint, dim()),
        ]),
    ];
    if field.off() {
        lines.push(Line::styled(
            format!("Doesn't look like a {}; it can't be saved.", field.label()),
            Style::default().fg(ACCENT),
        ));
    }
    Text::from(lines)
}
