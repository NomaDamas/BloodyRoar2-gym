#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Noop,
    Up,
    Down,
    Left,
    Right,
    Punch,
    Kick,
    Beast,
    Guard,
    UpPunch,
    DownKick,
    LeftGuard,
    RightPunch,
    PunchKick,
    BeastPunch,
    BeastKick,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActionButtons {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub punch: bool,
    pub kick: bool,
    pub beast: bool,
    pub guard: bool,
}

pub const ACTION_SPACE: [Action; 16] = [
    Action::Noop,
    Action::Up,
    Action::Down,
    Action::Left,
    Action::Right,
    Action::Punch,
    Action::Kick,
    Action::Beast,
    Action::Guard,
    Action::UpPunch,
    Action::DownKick,
    Action::LeftGuard,
    Action::RightPunch,
    Action::PunchKick,
    Action::BeastPunch,
    Action::BeastKick,
];

impl Action {
    pub fn from_index(index: usize) -> Option<Self> {
        ACTION_SPACE.get(index).copied()
    }

    pub fn index(self) -> usize {
        ACTION_SPACE
            .iter()
            .position(|candidate| *candidate == self)
            .expect("action exists in ACTION_SPACE")
    }

    pub fn name(self) -> &'static str {
        match self {
            Action::Noop => "noop",
            Action::Up => "up",
            Action::Down => "down",
            Action::Left => "left",
            Action::Right => "right",
            Action::Punch => "punch",
            Action::Kick => "kick",
            Action::Beast => "beast",
            Action::Guard => "guard",
            Action::UpPunch => "up+punch",
            Action::DownKick => "down+kick",
            Action::LeftGuard => "left+guard",
            Action::RightPunch => "right+punch",
            Action::PunchKick => "punch+kick",
            Action::BeastPunch => "beast+punch",
            Action::BeastKick => "beast+kick",
        }
    }

    pub fn buttons(self) -> ActionButtons {
        match self {
            Action::Noop => ActionButtons::default(),
            Action::Up => ActionButtons {
                up: true,
                ..ActionButtons::default()
            },
            Action::Down => ActionButtons {
                down: true,
                ..ActionButtons::default()
            },
            Action::Left => ActionButtons {
                left: true,
                ..ActionButtons::default()
            },
            Action::Right => ActionButtons {
                right: true,
                ..ActionButtons::default()
            },
            Action::Punch => ActionButtons {
                punch: true,
                ..ActionButtons::default()
            },
            Action::Kick => ActionButtons {
                kick: true,
                ..ActionButtons::default()
            },
            Action::Beast => ActionButtons {
                beast: true,
                ..ActionButtons::default()
            },
            Action::Guard => ActionButtons {
                guard: true,
                ..ActionButtons::default()
            },
            Action::UpPunch => ActionButtons {
                up: true,
                punch: true,
                ..ActionButtons::default()
            },
            Action::DownKick => ActionButtons {
                down: true,
                kick: true,
                ..ActionButtons::default()
            },
            Action::LeftGuard => ActionButtons {
                left: true,
                guard: true,
                ..ActionButtons::default()
            },
            Action::RightPunch => ActionButtons {
                right: true,
                punch: true,
                ..ActionButtons::default()
            },
            Action::PunchKick => ActionButtons {
                punch: true,
                kick: true,
                ..ActionButtons::default()
            },
            Action::BeastPunch => ActionButtons {
                beast: true,
                punch: true,
                ..ActionButtons::default()
            },
            Action::BeastKick => ActionButtons {
                beast: true,
                kick: true,
                ..ActionButtons::default()
            },
        }
    }
}

impl ActionButtons {
    pub fn json(self) -> String {
        format!(
            "{{\"up\":{},\"down\":{},\"left\":{},\"right\":{},\"punch\":{},\"kick\":{},\"beast\":{},\"guard\":{}}}",
            self.up,
            self.down,
            self.left,
            self.right,
            self.punch,
            self.kick,
            self.beast,
            self.guard
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTION_SPACE, Action};

    #[test]
    fn action_indices_round_trip() {
        for (index, action) in ACTION_SPACE.iter().enumerate() {
            assert_eq!(Action::from_index(index), Some(*action));
            assert_eq!(action.index(), index);
        }
    }
}
