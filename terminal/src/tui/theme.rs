use tui::style::{Color, Modifier, Style};
use tui::widgets::{BorderType, Borders};

pub struct BorderStyle {
    pub style: Style,
    pub borders: Borders,
    pub border_type: BorderType,
}

pub struct ListStyle {
    pub symbol: String,
}

pub struct Theme {
    pub border: BorderStyle,
    pub list: ListStyle,
    pub highlight: Style,
    pub highlight_dim: Style,
    pub highlight_invert: Style,
    pub primary: Style,
    pub primary_dim: Style,
    pub primary_invert: Style,
    pub secondary: Style,
    pub secondary_dim: Style,
    pub ternary: Style,
    pub ternary_dim: Style,
    pub bg_dark_secondary: Style,
    pub bg_bright_primary: Style,
    pub bg_bright_ternary: Style,
    pub open: Style,
    pub solved: Style,
    pub closed: Style,
}

impl Theme {
    pub fn default_dark() -> Self {
        Theme {
            border: BorderStyle {
                style: Style::default(),
                borders: Borders::NONE,
                border_type: BorderType::Plain,
            },
            list: ListStyle {
                symbol: String::from("│ "),
            },
            highlight: Style::default().fg(Color::Rgb(238, 111, 248)),
            highlight_dim: Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .add_modifier(Modifier::BOLD),
            highlight_invert: Style::default()
                .bg(Color::Rgb(238, 111, 248))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            primary: Style::default().fg(Color::Rgb(117, 113, 249)),
            primary_dim: Style::default().fg(Color::Rgb(79, 75, 187)),
            primary_invert: Style::default()
                .bg(Color::Rgb(79, 75, 187))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            secondary: Style::default().fg(Color::Rgb(66, 245, 161)),
            secondary_dim: Style::default().fg(Color::Rgb(30, 102, 68)),
            ternary: Style::default().fg(Color::Rgb(100, 100, 100)),
            ternary_dim: Style::default().fg(Color::Rgb(70, 70, 70)),
            bg_dark_secondary: Style::default()
                .fg(Color::Rgb(100, 100, 100))
                .bg(Color::Rgb(50, 50, 50)),
            bg_bright_primary: Style::default()
                .fg(Color::Rgb(117, 113, 249))
                .bg(Color::Rgb(40, 40, 40)),
            bg_bright_ternary: Style::default()
                .fg(Color::Rgb(100, 100, 100))
                .bg(Color::Rgb(40, 40, 40)),
            open: Style::default().fg(Color::Blue),
            solved: Style::default().fg(Color::Rgb(66, 245, 161)),
            closed: Style::default().fg(Color::Red),
        }
    }

    pub fn glow_dark() -> Self {
        Theme {
            border: BorderStyle {
                style: Style::default().fg(Color::Rgb(80, 80, 80)),
                borders: Borders::ALL,
                border_type: BorderType::Rounded,
            },
            list: ListStyle {
                symbol: String::from("│ "),
            },
            highlight: Style::default().fg(Color::Rgb(238, 111, 248)),
            highlight_dim: Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .add_modifier(Modifier::BOLD),
            highlight_invert: Style::default()
                .fg(Color::Rgb(238, 111, 248))
                .add_modifier(Modifier::BOLD),
            primary: Style::default().fg(Color::Rgb(117, 113, 249)),
            primary_dim: Style::default().fg(Color::Rgb(79, 75, 187)),
            primary_invert: Style::default()
                .fg(Color::Rgb(117, 113, 249))
                .add_modifier(Modifier::BOLD),
            secondary: Style::default().fg(Color::Rgb(66, 245, 161)),
            secondary_dim: Style::default().fg(Color::Rgb(30, 102, 68)),
            ternary: Style::default().fg(Color::Rgb(100, 100, 100)),
            ternary_dim: Style::default().fg(Color::Rgb(70, 70, 70)),
            bg_dark_secondary: Style::default().fg(Color::Rgb(30, 102, 68)),
            bg_bright_primary: Style::default().fg(Color::Rgb(117, 113, 249)),
            bg_bright_ternary: Style::default().fg(Color::Rgb(100, 100, 100)),
            open: Style::default().fg(Color::Blue),
            solved: Style::default().fg(Color::Rgb(66, 245, 161)),
            closed: Style::default().fg(Color::Red),
        }
    }
}
