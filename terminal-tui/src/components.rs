use tuirealm::command::{Cmd, CmdResult};
use tuirealm::props::{AttrValue, Attribute, Color, Props, Style, TextModifiers};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::tui::text::{Span, Spans};
use tuirealm::tui::widgets::Tabs;
use tuirealm::{Frame, MockComponent, State, StateValue};

use crate::layout::{ComponentLayout, HorizontalLayout};

type BoxedComponent = Box<dyn MockComponent>;

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

/// A tab header that displays all labels horizontally aligned and separated
/// by a divider. Highlights the label defined by the current tab index.
#[derive(Clone)]
pub struct TabHeader {
    attributes: Props,
    tabs: Vec<Label>,
    divider: Label,
    state: TabState,
}

impl TabHeader {
    pub fn child(mut self, tab: Label) -> Self {
        self.tabs = [self.tabs, vec![tab]].concat();
        self.state.len = self.tabs.len() as u16;
        self
    }

    pub fn foreground(mut self, fg: Color) -> Self {
        self.attr(Attribute::Foreground, AttrValue::Color(fg));
        self
    }

    pub fn highlight(mut self, fg: Color) -> Self {
        self.attr(Attribute::HighlightedColor, AttrValue::Color(fg));
        self
    }

    fn height(mut self, h: u16) -> Self {
        self.attr(Attribute::Height, AttrValue::Size(h));
        self
    }
}

impl Default for TabHeader {
    fn default() -> Self {
        Self {
            attributes: Props::default(),
            tabs: vec![],
            divider: Label::new("|").foreground(Color::Rgb(70, 70, 70)),
            state: TabState::default(),
        }
        .height(1)
        .foreground(Color::Rgb(70, 70, 70))
        .highlight(Color::Rgb(100, 100, 100))
    }
}

impl MockComponent for TabHeader {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let selected = self.state().unwrap_one().unwrap_u16();
        let display = self
            .attributes
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let foreground = self
            .attributes
            .get_or(Attribute::Foreground, AttrValue::Color(Color::Reset))
            .unwrap_color();
        let highlight = self
            .attributes
            .get_or(Attribute::HighlightedColor, AttrValue::Color(Color::Reset))
            .unwrap_color();

        if display {
            let spans = self
                .tabs
                .iter()
                .map(|tab| Spans::from(vec![Span::from(tab)]))
                .collect::<Vec<_>>();

            let tabs = Tabs::new(spans)
                .style(Style::default().fg(foreground))
                .highlight_style(Style::default().fg(highlight))
                .divider(Span::from(&self.divider))
                .select(selected as usize);

            frame.render_widget(tabs, area);
        }
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.attributes.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.attributes.set(attr, value)
    }

    fn state(&self) -> State {
        State::One(StateValue::U16(self.state.selected))
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        use tuirealm::command::Direction;

        match cmd {
            Cmd::Move(Direction::Right) => {
                let prev = self.state.selected;
                self.state.incr_tab_index(true);
                if prev != self.state.selected {
                    CmdResult::Changed(self.state())
                } else {
                    CmdResult::None
                }
            }
            _ => CmdResult::None,
        }
    }
}

/// A container with a tab header. Displays the component selected by the index
/// held in the header state.
#[derive(Default)]
pub struct TabContainer {
    attributes: Props,
    header: TabHeader,
    children: Vec<BoxedComponent>,
}

impl TabContainer {
    pub fn height(mut self, h: u16) -> Self {
        self.attr(Attribute::Height, AttrValue::Size(h));
        self
    }

    pub fn child(mut self, title: String, component: impl MockComponent + 'static) -> Self {
        self.header = self
            .header
            .child(Label::new(&title).foreground(Color::Rgb(70, 70, 70)));
        self.children.push(Box::new(component));
        self
    }
}

impl MockComponent for TabContainer {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let display = self
            .attributes
            .get_or(Attribute::Display, AttrValue::Flag(true))
            .unwrap_flag();
        let tab_header_height = self
            .header
            .query(Attribute::Height)
            .unwrap_or(AttrValue::Size(1))
            .unwrap_size();
        let selected = self.header.state().unwrap_one().unwrap_u16();

        if display {
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .vertical_margin(1)
                .constraints(
                    [Constraint::Length(tab_header_height), Constraint::Length(0)].as_ref(),
                )
                .split(area);

            self.header.view(frame, layout[0]);

            if let Some(child) = self.children.get_mut(selected as usize) {
                child.view(frame, layout[1]);
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

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        CmdResult::Batch(
            [
                self.children
                    .iter_mut()
                    .map(|child| child.perform(cmd))
                    .collect(),
                vec![self.header.perform(cmd)],
            ]
            .concat(),
        )
    }
}

/// State that holds the index of a selected tab item and the count of all tab items.
/// The index can be increased and will start at 0, if length was reached.
#[derive(Clone, Default)]
pub struct TabState {
    pub selected: u16,
    pub len: u16,
}

impl TabState {
    pub fn incr_tab_index(&mut self, rewind: bool) {
        if self.selected + 1 < self.len {
            self.selected += 1;
        } else if rewind {
            self.selected = 0;
        }
    }
}
