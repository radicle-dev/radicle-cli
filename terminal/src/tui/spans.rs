use textwrap::Options;

use tui::style::{Color, Style};
use tui::text::{Span, Spans};

use super::strings;

pub fn lines(content: &str, width: u16, indent: u16) -> Vec<Spans<'_>> {
    let wrap = width.checked_sub(indent).unwrap_or(80);
    let whitespaces = strings::whitespaces(indent);

    let options = Options::new(wrap as usize)
        .initial_indent(&whitespaces)
        .subsequent_indent(&whitespaces);

    let lines = textwrap::wrap(content, options);
    lines
        .iter()
        .map(|line| {
            Spans::from(Span::styled(
                String::from(line.clone()),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            ))
        })
        .collect::<Vec<_>>()
}
