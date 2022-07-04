use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::{AttrValue, Attribute, Color, Props, Style, TextModifiers};
use tuirealm::tui::layout::Rect;
use tuirealm::tui::text::Span;

use tuirealm::{Frame, MockComponent, State};

use crate::layout::{ComponentLayout, HorizontalLayout};

const WHITESPACE: char = ' ';

impl From<&Label> for Span<'_> {
    fn from(label: &Label) -> Self {
        Span::styled(label.text.clone(), Style::default())
    }
}

/// A label that can be styled using a foreground color and text modifiers.
/// Its height is fixed, its width depends on the length of the text it displays.
#[derive(Clone)]
pub struct Label {
    attributes: Props,
    text: String,
}

impl Label {
    pub fn new(text: &str) -> Self {
        Self {
            attributes: Props::default(),
            text: text.to_owned(),
        }
        .height(1)
        .width(text.chars().count() as u16)
    }

    pub fn foreground(mut self, fg: Color) -> Self {
        self.attr(Attribute::Foreground, AttrValue::Color(fg));
        self
    }

    pub fn modifiers(mut self, m: TextModifiers) -> Self {
        self.attr(Attribute::TextProps, AttrValue::TextModifiers(m));
        self
    }

    fn height(mut self, h: u16) -> Self {
        self.attr(Attribute::Height, AttrValue::Size(h));
        self
    }

    fn width(mut self, w: u16) -> Self {
        self.attr(Attribute::Width, AttrValue::Size(w));
        self
    }
}

impl MockComponent for Label {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use tui_realm_stdlib::Label;

        let display = self
            .attributes
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let foreground = self
            .attributes
            .get_or(Attribute::Foreground, AttrValue::Color(Color::Reset))
            .unwrap_color();

        if display {
            let mut label = match self.attributes.get(Attribute::TextProps) {
                Some(modifiers) => Label::default()
                    .foreground(foreground)
                    .modifiers(modifiers.unwrap_text_modifiers())
                    .text(self.text.clone()),
                None => Label::default()
                    .foreground(foreground)
                    .text(self.text.clone()),
            };

            label.view(frame, area);
        }
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.attributes.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.attributes.set(attr, value)
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A label with colored background that can be styled further by using a foreground
/// color and text modifiers. Its height is fixed, its width depends on the length of
/// the text it displays.
#[derive(Clone)]
pub struct HighlightedLabel {
    attributes: Props,
    text: String,
}

impl HighlightedLabel {
    pub fn new(text: &str) -> Self {
        let text = format!(" {} ", text);

        Self {
            attributes: Props::default(),
            text: text.clone(),
        }
        .height(1)
        .width(text.chars().count() as u16)
    }

    pub fn foreground(mut self, fg: Color) -> Self {
        self.attr(Attribute::Foreground, AttrValue::Color(fg));
        self
    }

    pub fn background(mut self, bg: Color) -> Self {
        self.attr(Attribute::Background, AttrValue::Color(bg));
        self
    }

    pub fn modifiers(mut self, m: TextModifiers) -> Self {
        self.attr(Attribute::TextProps, AttrValue::TextModifiers(m));
        self
    }

    fn height(mut self, h: u16) -> Self {
        self.attr(Attribute::Height, AttrValue::Size(h));
        self
    }

    fn width(mut self, w: u16) -> Self {
        self.attr(Attribute::Width, AttrValue::Size(w));
        self
    }
}

impl MockComponent for HighlightedLabel {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use tui_realm_stdlib::Label;

        let display = self
            .attributes
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let foreground = self
            .attributes
            .get_or(Attribute::Foreground, AttrValue::Color(Color::Reset))
            .unwrap_color();
        let background = self
            .attributes
            .get_or(Attribute::Background, AttrValue::Color(Color::Reset))
            .unwrap_color();

        if display {
            let mut label = match self.attributes.get(Attribute::TextProps) {
                Some(modifiers) => Label::default()
                    .foreground(foreground)
                    .background(background)
                    .modifiers(modifiers.unwrap_text_modifiers())
                    .text(self.text.clone()),
                None => Label::default()
                    .foreground(foreground)
                    .background(background)
                    .text(self.text.clone()),
            };

            label.view(frame, area);
        }
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.attributes.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.attributes.set(attr, value)
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// An application title that consists of a highlighted label and a empty label that
/// fills out any remaining space in a horizontal layout.
pub struct ApplicationTitle {
    attributes: Props,
    label: HighlightedLabel,
    spacer: Label,
}

impl ApplicationTitle {
    pub fn new(text: &str) -> Self {
        Self {
            attributes: Props::default(),
            label: HighlightedLabel::new(text)
                .foreground(Color::White)
                .background(Color::Rgb(238, 111, 248))
                .modifiers(TextModifiers::BOLD),
            spacer: Label::new(""),
        }
        .height(1)
    }

    fn height(mut self, h: u16) -> Self {
        self.attr(Attribute::Height, AttrValue::Size(h));
        self
    }
}

impl MockComponent for ApplicationTitle {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let display = self
            .attributes
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let layout = HorizontalLayout::new(
                vec![Box::new(self.label.clone()), Box::new(self.spacer.clone())],
                area,
            )
            .build();

            for (mut component, area) in layout {
                component.view(frame, area);
            }
        }
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.attributes.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.attributes.set(attr, value)
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A shortcut that consists of a label displaying the "hotkey", a label that displays
/// the action and a spacer between them.
#[derive(Clone)]
pub struct Shortcut {
    attributes: Props,
    short: Label,
    long: Label,
    spacer: Label,
}

impl Shortcut {
    pub fn new(short: &str, long: &str) -> Self {
        let short = Label::new(short).foreground(Color::Rgb(100, 100, 100));
        let long = Label::new(long).foreground(Color::Rgb(70, 70, 70));
        let spacer = Label::new(&format!("{}", WHITESPACE));

        Self {
            attributes: Props::default(),
            short: short.clone(),
            long: long.clone(),
            spacer: spacer.clone(),
        }
        .height(1)
        .width(
            short
                .attributes
                .get(Attribute::Width)
                .unwrap()
                .unwrap_size()
                + spacer
                    .attributes
                    .get(Attribute::Width)
                    .unwrap()
                    .unwrap_size()
                + long.attributes.get(Attribute::Width).unwrap().unwrap_size(),
        )
    }

    fn height(mut self, h: u16) -> Self {
        self.attr(Attribute::Height, AttrValue::Size(h));
        self
    }

    fn width(mut self, w: u16) -> Self {
        self.attr(Attribute::Width, AttrValue::Size(w));
        self
    }
}

impl MockComponent for Shortcut {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let display = self
            .attributes
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let layout = HorizontalLayout::new(
                vec![
                    Box::new(self.short.clone()),
                    Box::new(self.spacer.clone()),
                    Box::new(self.long.clone()),
                ],
                area,
            )
            .build();

            for (mut component, area) in layout {
                component.view(frame, area);
            }
        }
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.attributes.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.attributes.set(attr, value)
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}

/// A shortcut bar that displays multiple shortcuts and separates them with a
/// divider.
pub struct ShortcutBar {
    attributes: Props,
    shortcuts: Vec<Shortcut>,
    divider: Label,
}

impl ShortcutBar {
    pub fn child(mut self, shortcut: Shortcut) -> Self {
        self.shortcuts = [self.shortcuts, vec![shortcut]].concat();
        self
    }

    fn height(mut self, h: u16) -> Self {
        self.attr(Attribute::Height, AttrValue::Size(h));
        self
    }
}

impl Default for ShortcutBar {
    fn default() -> Self {
        Self {
            attributes: Props::default(),
            shortcuts: vec![],
            divider: Label::new(" ∙ ").foreground(Color::Rgb(70, 70, 70)),
        }
        .height(1)
    }
}

impl MockComponent for ShortcutBar {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let display = self
            .attributes
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();

        if display {
            let mut components: Vec<Box<dyn MockComponent>> = vec![];
            let mut shortcuts = self.shortcuts.iter_mut().peekable();

            while let Some(shortcut) = shortcuts.next() {
                if shortcuts.peek().is_some() {
                    components.push(Box::new(shortcut.clone()));
                    components.push(Box::new(self.divider.clone()))
                } else {
                    components.push(Box::new(shortcut.clone()));
                }
            }

            let layout = HorizontalLayout::new(components, area).build();
            for (mut component, area) in layout {
                component.view(frame, area);
            }
        }
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.attributes.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.attributes.set(attr, value)
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}
