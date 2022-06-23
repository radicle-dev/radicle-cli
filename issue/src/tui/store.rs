#[derive(Default)]
pub struct ToogleProperty {
    state: bool,
}

impl ToogleProperty {
    pub fn new(state: bool) -> Self {
        Self { state }
    }

    pub fn toggle(&mut self) {
        self.state = !self.state
    }

    pub fn is_on(&self) -> bool {
        self.state
    }
}
