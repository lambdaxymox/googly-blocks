#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InputAction {
    Press,
    Repeat,
    Release,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InputKind {
    Left,
    Right,
    Down,
    Exit,
    Rotate,
    StartGame,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Input {
    pub kind: InputKind,
    pub action: InputAction,
}

impl Input {
    #[inline]
    pub fn new(kind: InputKind, action: InputAction) -> Input {
        Input {
            kind: kind,
            action: action,
        }
    }
}

