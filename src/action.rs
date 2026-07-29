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
    Start,
    Coin,
    Service,
    CoinStart,
    P2Up,
    P2Down,
    P2Left,
    P2Right,
    P2Punch,
    P2Kick,
    P2Beast,
    P2Guard,
    P2UpPunch,
    P2DownKick,
    P2LeftGuard,
    P2RightPunch,
    P2PunchKick,
    P2BeastPunch,
    P2BeastKick,
    P2Start,
    P2Coin,
    P2CoinStart,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActionButtons {
    pub start: bool,
    pub coin: bool,
    pub service: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub punch: bool,
    pub kick: bool,
    pub beast: bool,
    pub guard: bool,
    pub p2_start: bool,
    pub p2_coin: bool,
    pub p2_up: bool,
    pub p2_down: bool,
    pub p2_left: bool,
    pub p2_right: bool,
    pub p2_punch: bool,
    pub p2_kick: bool,
    pub p2_beast: bool,
    pub p2_guard: bool,
}

pub const ACTION_SPACE: [Action; 38] = [
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
    Action::Start,
    Action::Coin,
    Action::Service,
    Action::CoinStart,
    Action::P2Up,
    Action::P2Down,
    Action::P2Left,
    Action::P2Right,
    Action::P2Punch,
    Action::P2Kick,
    Action::P2Beast,
    Action::P2Guard,
    Action::P2UpPunch,
    Action::P2DownKick,
    Action::P2LeftGuard,
    Action::P2RightPunch,
    Action::P2PunchKick,
    Action::P2BeastPunch,
    Action::P2BeastKick,
    Action::P2Start,
    Action::P2Coin,
    Action::P2CoinStart,
];

impl Action {
    pub fn from_index(index: usize) -> Option<Self> {
        ACTION_SPACE.get(index).copied()
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let mut normalized = name.trim().to_ascii_lowercase();
        normalized = normalized.replace(['_', '-'], "+");
        if let Some(suffix) = normalized.strip_prefix("2p+") {
            normalized = format!("p2+{suffix}");
        }

        if normalized == "none" || normalized == "no+op" {
            return Some(Action::Noop);
        }
        if normalized == "enter" {
            return Some(Action::CoinStart);
        }
        if matches!(normalized.as_str(), "p2+enter" | "p2+return") {
            return Some(Action::P2CoinStart);
        }
        if matches!(normalized.as_str(), "z" | "space" | "j" | "f") {
            return Some(Action::Punch);
        }
        if matches!(normalized.as_str(), "x" | "k" | "h") {
            return Some(Action::Kick);
        }
        if matches!(normalized.as_str(), "q" | "l" | "b") {
            return Some(Action::Beast);
        }
        if matches!(normalized.as_str(), "e" | "i" | "g") {
            return Some(Action::Guard);
        }
        if matches!(normalized.as_str(), "p2+w") {
            return Some(Action::P2Up);
        }
        if matches!(normalized.as_str(), "p2+s") {
            return Some(Action::P2Down);
        }
        if matches!(normalized.as_str(), "p2+a") {
            return Some(Action::P2Left);
        }
        if matches!(normalized.as_str(), "p2+d") {
            return Some(Action::P2Right);
        }
        if matches!(normalized.as_str(), "p2+j") {
            return Some(Action::P2Punch);
        }
        if matches!(normalized.as_str(), "p2+k") {
            return Some(Action::P2Kick);
        }
        if matches!(normalized.as_str(), "p2+u") {
            return Some(Action::P2Beast);
        }
        if matches!(normalized.as_str(), "p2+i") {
            return Some(Action::P2Guard);
        }
        if matches!(normalized.as_str(), "p2+o") {
            return Some(Action::P2Start);
        }
        if matches!(normalized.as_str(), "p2+n") {
            return Some(Action::P2Coin);
        }

        ACTION_SPACE
            .iter()
            .copied()
            .find(|action| action.name() == normalized)
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
            Action::Start => "start",
            Action::Coin => "coin",
            Action::Service => "service",
            Action::CoinStart => "coin+start",
            Action::P2Up => "p2+up",
            Action::P2Down => "p2+down",
            Action::P2Left => "p2+left",
            Action::P2Right => "p2+right",
            Action::P2Punch => "p2+punch",
            Action::P2Kick => "p2+kick",
            Action::P2Beast => "p2+beast",
            Action::P2Guard => "p2+guard",
            Action::P2UpPunch => "p2+up+punch",
            Action::P2DownKick => "p2+down+kick",
            Action::P2LeftGuard => "p2+left+guard",
            Action::P2RightPunch => "p2+right+punch",
            Action::P2PunchKick => "p2+punch+kick",
            Action::P2BeastPunch => "p2+beast+punch",
            Action::P2BeastKick => "p2+beast+kick",
            Action::P2Start => "p2+start",
            Action::P2Coin => "p2+coin",
            Action::P2CoinStart => "p2+coin+start",
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
            Action::Start => ActionButtons {
                start: true,
                ..ActionButtons::default()
            },
            Action::Coin => ActionButtons {
                coin: true,
                ..ActionButtons::default()
            },
            Action::Service => ActionButtons {
                service: true,
                ..ActionButtons::default()
            },
            Action::CoinStart => ActionButtons {
                coin: true,
                start: true,
                ..ActionButtons::default()
            },
            Action::P2Up => ActionButtons {
                p2_up: true,
                ..ActionButtons::default()
            },
            Action::P2Down => ActionButtons {
                p2_down: true,
                ..ActionButtons::default()
            },
            Action::P2Left => ActionButtons {
                p2_left: true,
                ..ActionButtons::default()
            },
            Action::P2Right => ActionButtons {
                p2_right: true,
                ..ActionButtons::default()
            },
            Action::P2Punch => ActionButtons {
                p2_punch: true,
                ..ActionButtons::default()
            },
            Action::P2Kick => ActionButtons {
                p2_kick: true,
                ..ActionButtons::default()
            },
            Action::P2Beast => ActionButtons {
                p2_beast: true,
                ..ActionButtons::default()
            },
            Action::P2Guard => ActionButtons {
                p2_guard: true,
                ..ActionButtons::default()
            },
            Action::P2UpPunch => ActionButtons {
                p2_up: true,
                p2_punch: true,
                ..ActionButtons::default()
            },
            Action::P2DownKick => ActionButtons {
                p2_down: true,
                p2_kick: true,
                ..ActionButtons::default()
            },
            Action::P2LeftGuard => ActionButtons {
                p2_left: true,
                p2_guard: true,
                ..ActionButtons::default()
            },
            Action::P2RightPunch => ActionButtons {
                p2_right: true,
                p2_punch: true,
                ..ActionButtons::default()
            },
            Action::P2PunchKick => ActionButtons {
                p2_punch: true,
                p2_kick: true,
                ..ActionButtons::default()
            },
            Action::P2BeastPunch => ActionButtons {
                p2_beast: true,
                p2_punch: true,
                ..ActionButtons::default()
            },
            Action::P2BeastKick => ActionButtons {
                p2_beast: true,
                p2_kick: true,
                ..ActionButtons::default()
            },
            Action::P2Start => ActionButtons {
                p2_start: true,
                ..ActionButtons::default()
            },
            Action::P2Coin => ActionButtons {
                p2_coin: true,
                ..ActionButtons::default()
            },
            Action::P2CoinStart => ActionButtons {
                p2_coin: true,
                p2_start: true,
                ..ActionButtons::default()
            },
        }
    }
}

impl ActionButtons {
    pub fn json(self) -> String {
        format!(
            "{{\"start\":{},\"coin\":{},\"service\":{},\"up\":{},\"down\":{},\"left\":{},\"right\":{},\"punch\":{},\"kick\":{},\"beast\":{},\"guard\":{},\"p2_start\":{},\"p2_coin\":{},\"p2_up\":{},\"p2_down\":{},\"p2_left\":{},\"p2_right\":{},\"p2_punch\":{},\"p2_kick\":{},\"p2_beast\":{},\"p2_guard\":{}}}",
            self.start,
            self.coin,
            self.service,
            self.up,
            self.down,
            self.left,
            self.right,
            self.punch,
            self.kick,
            self.beast,
            self.guard,
            self.p2_start,
            self.p2_coin,
            self.p2_up,
            self.p2_down,
            self.p2_left,
            self.p2_right,
            self.p2_punch,
            self.p2_kick,
            self.p2_beast,
            self.p2_guard
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTION_SPACE, Action, ActionButtons};

    #[test]
    fn action_indices_round_trip() {
        for (index, action) in ACTION_SPACE.iter().enumerate() {
            assert_eq!(Action::from_index(index), Some(*action));
            assert_eq!(action.index(), index);
        }
    }

    #[test]
    fn p1_action_indices_are_preserved() {
        let p1_actions = [
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
            Action::Start,
            Action::Coin,
            Action::Service,
            Action::CoinStart,
        ];

        assert_eq!(&ACTION_SPACE[..20], &p1_actions);
    }

    #[test]
    fn p2_actions_are_appended_after_p1_space() {
        let p2_actions = [
            Action::P2Up,
            Action::P2Down,
            Action::P2Left,
            Action::P2Right,
            Action::P2Punch,
            Action::P2Kick,
            Action::P2Beast,
            Action::P2Guard,
            Action::P2UpPunch,
            Action::P2DownKick,
            Action::P2LeftGuard,
            Action::P2RightPunch,
            Action::P2PunchKick,
            Action::P2BeastPunch,
            Action::P2BeastKick,
            Action::P2Start,
            Action::P2Coin,
            Action::P2CoinStart,
        ];

        assert_eq!(&ACTION_SPACE[20..], &p2_actions);
        assert_eq!(Action::P2Up.index(), 20);
        assert_eq!(Action::P2CoinStart.index(), 37);
    }

    #[test]
    fn action_names_accept_cli_friendly_aliases() {
        assert_eq!(Action::from_name("noop"), Some(Action::Noop));
        assert_eq!(Action::from_name("no-op"), Some(Action::Noop));
        assert_eq!(Action::from_name("coin+start"), Some(Action::CoinStart));
        assert_eq!(Action::from_name("coin_start"), Some(Action::CoinStart));
        assert_eq!(Action::from_name("enter"), Some(Action::CoinStart));
        assert_eq!(Action::from_name("space"), Some(Action::Punch));
        assert_eq!(Action::from_name("x"), Some(Action::Kick));
        assert_eq!(Action::from_name("q"), Some(Action::Beast));
        assert_eq!(Action::from_name("e"), Some(Action::Guard));
        assert_eq!(Action::from_name("service"), Some(Action::Service));
        assert_eq!(Action::from_name("BEAST-KICK"), Some(Action::BeastKick));
        assert_eq!(Action::from_name("p2-up"), Some(Action::P2Up));
        assert_eq!(Action::from_name("2p_right"), Some(Action::P2Right));
        assert_eq!(Action::from_name("p2+j"), Some(Action::P2Punch));
        assert_eq!(Action::from_name("p2+n"), Some(Action::P2Coin));
        assert_eq!(Action::from_name("p2+enter"), Some(Action::P2CoinStart));
        assert_eq!(Action::from_name("invalid"), None);
    }

    #[test]
    fn p2_action_buttons_and_json_are_explicit() {
        let buttons = Action::P2LeftGuard.buttons();

        assert_eq!(
            buttons,
            ActionButtons {
                p2_left: true,
                p2_guard: true,
                ..ActionButtons::default()
            }
        );
        let json = buttons.json();
        assert!(json.contains("\"p2_left\":true"));
        assert!(json.contains("\"p2_guard\":true"));
        assert!(json.contains("\"left\":false"));
        assert!(json.contains("\"guard\":false"));
    }
}
