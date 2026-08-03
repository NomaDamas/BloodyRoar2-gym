use crate::action::ActionButtons;

pub const BLOODY_ROAR_2_ROSTER: [&str; 11] = [
    "yugo", "stun", "marvel", "bakuryu", "long", "shenlong", "alice", "uriko", "busuzima", "jenny",
    "gado",
];
pub const MAX_ACTION_SEQUENCE_SEGMENTS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Player {
    One,
    Two,
}

impl Player {
    pub fn from_number(value: u64) -> Option<Self> {
        match value {
            1 => Some(Self::One),
            2 => Some(Self::Two),
            _ => None,
        }
    }

    pub fn number(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }

    pub fn default_facing(self) -> Facing {
        match self {
            Self::One => Facing::Right,
            Self::Two => Facing::Left,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Facing {
    Left,
    Right,
}

impl Facing {
    pub fn from_name(value: &str) -> Option<Self> {
        match normalized_name(value).as_str() {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionSegment {
    pub buttons: ActionButtons,
    pub frames: u32,
}

impl ActionSegment {
    pub fn new(buttons: ActionButtons, frames: u32) -> Self {
        Self { buttons, frames }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamedActionSpec {
    pub name: &'static str,
    pub description: &'static str,
}

pub const NAMED_ACTIONS: [NamedActionSpec; 18] = [
    NamedActionSpec {
        name: "idle",
        description: "Release every control for a short settling interval.",
    },
    NamedActionSpec {
        name: "move_forward",
        description: "Walk toward the opponent using the supplied facing direction.",
    },
    NamedActionSpec {
        name: "move_backward",
        description: "Walk away from the opponent using the supplied facing direction.",
    },
    NamedActionSpec {
        name: "crouch",
        description: "Hold down, then release.",
    },
    NamedActionSpec {
        name: "jump",
        description: "Tap up, then release.",
    },
    NamedActionSpec {
        name: "dash_forward",
        description: "Double-tap toward the opponent.",
    },
    NamedActionSpec {
        name: "dash_backward",
        description: "Double-tap away from the opponent.",
    },
    NamedActionSpec {
        name: "punch",
        description: "Tap Punch, then release.",
    },
    NamedActionSpec {
        name: "kick",
        description: "Tap Kick, then release.",
    },
    NamedActionSpec {
        name: "guard",
        description: "Hold Guard briefly, then release.",
    },
    NamedActionSpec {
        name: "transform",
        description: "Tap Beast and include a release/settle phase.",
    },
    NamedActionSpec {
        name: "jump_punch",
        description: "Press Up and Punch together, then release.",
    },
    NamedActionSpec {
        name: "crouch_kick",
        description: "Press Down and Kick together, then release.",
    },
    NamedActionSpec {
        name: "forward_punch",
        description: "Press toward the opponent and Punch together, then release.",
    },
    NamedActionSpec {
        name: "back_guard",
        description: "Press away from the opponent and Guard together, then release.",
    },
    NamedActionSpec {
        name: "punch_kick",
        description: "Press Punch and Kick together, then release.",
    },
    NamedActionSpec {
        name: "beast_punch",
        description: "Press Beast and Punch together, then release.",
    },
    NamedActionSpec {
        name: "beast_kick",
        description: "Press Beast and Kick together, then release.",
    },
];

#[derive(Clone, Copy, Debug, Default)]
struct LocalButtons {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    punch: bool,
    kick: bool,
    beast: bool,
    guard: bool,
}

impl LocalButtons {
    fn for_player(self, player: Player) -> ActionButtons {
        match player {
            Player::One => ActionButtons {
                up: self.up,
                down: self.down,
                left: self.left,
                right: self.right,
                punch: self.punch,
                kick: self.kick,
                beast: self.beast,
                guard: self.guard,
                ..ActionButtons::default()
            },
            Player::Two => ActionButtons {
                p2_up: self.up,
                p2_down: self.down,
                p2_left: self.left,
                p2_right: self.right,
                p2_punch: self.punch,
                p2_kick: self.kick,
                p2_beast: self.beast,
                p2_guard: self.guard,
                ..ActionButtons::default()
            },
        }
    }
}

pub fn canonical_character_name(value: &str) -> Option<&'static str> {
    let normalized = normalized_name(value);
    if let Ok(slot) = normalized.parse::<usize>() {
        return BLOODY_ROAR_2_ROSTER.get(slot).copied();
    }
    let compact = normalized.replace('_', "");
    BLOODY_ROAR_2_ROSTER
        .iter()
        .copied()
        .find(|candidate| *candidate == compact)
}

pub fn canonical_named_action(value: &str) -> Option<&'static str> {
    let normalized = normalized_name(value);
    let alias = match normalized.as_str() {
        "noop" | "none" => "idle",
        "forward" | "toward" => "move_forward",
        "back" | "backward" | "away" => "move_backward",
        "beast" | "beast_transform" => "transform",
        "up_punch" => "jump_punch",
        "down_kick" => "crouch_kick",
        "left_guard" => "back_guard",
        "right_punch" => "forward_punch",
        other => other,
    };
    NAMED_ACTIONS
        .iter()
        .find(|spec| spec.name == alias)
        .map(|spec| spec.name)
}

pub fn named_action_sequence(
    character: &str,
    player: Player,
    action: &str,
    facing: Facing,
) -> Result<Vec<ActionSegment>, String> {
    let character = canonical_character_name(character)
        .ok_or_else(|| format!("unknown Bloody Roar 2 character: {character}"))?;
    let action = canonical_named_action(action)
        .ok_or_else(|| format!("unknown named character action: {action}"))?;
    let _ = character;

    let forward = match facing {
        Facing::Left => LocalButtons {
            left: true,
            ..LocalButtons::default()
        },
        Facing::Right => LocalButtons {
            right: true,
            ..LocalButtons::default()
        },
    };
    let backward = match facing {
        Facing::Left => LocalButtons {
            right: true,
            ..LocalButtons::default()
        },
        Facing::Right => LocalButtons {
            left: true,
            ..LocalButtons::default()
        },
    };
    let release = ActionSegment::new(ActionButtons::default(), 3);
    let tap =
        |buttons: LocalButtons| vec![ActionSegment::new(buttons.for_player(player), 4), release];

    Ok(match action {
        "idle" => vec![ActionSegment::new(ActionButtons::default(), 6)],
        "move_forward" => tap(forward),
        "move_backward" => tap(backward),
        "crouch" => tap(LocalButtons {
            down: true,
            ..LocalButtons::default()
        }),
        "jump" => tap(LocalButtons {
            up: true,
            ..LocalButtons::default()
        }),
        "dash_forward" => double_tap(forward, player),
        "dash_backward" => double_tap(backward, player),
        "punch" => tap(LocalButtons {
            punch: true,
            ..LocalButtons::default()
        }),
        "kick" => tap(LocalButtons {
            kick: true,
            ..LocalButtons::default()
        }),
        "guard" => vec![
            ActionSegment::new(
                LocalButtons {
                    guard: true,
                    ..LocalButtons::default()
                }
                .for_player(player),
                8,
            ),
            release,
        ],
        "transform" => vec![
            ActionSegment::new(
                LocalButtons {
                    beast: true,
                    ..LocalButtons::default()
                }
                .for_player(player),
                6,
            ),
            ActionSegment::new(ActionButtons::default(), 18),
        ],
        "jump_punch" => tap(LocalButtons {
            up: true,
            punch: true,
            ..LocalButtons::default()
        }),
        "crouch_kick" => tap(LocalButtons {
            down: true,
            kick: true,
            ..LocalButtons::default()
        }),
        "forward_punch" => tap(LocalButtons {
            punch: true,
            ..forward
        }),
        "back_guard" => tap(LocalButtons {
            guard: true,
            ..backward
        }),
        "punch_kick" => tap(LocalButtons {
            punch: true,
            kick: true,
            ..LocalButtons::default()
        }),
        "beast_punch" => tap(LocalButtons {
            beast: true,
            punch: true,
            ..LocalButtons::default()
        }),
        "beast_kick" => tap(LocalButtons {
            beast: true,
            kick: true,
            ..LocalButtons::default()
        }),
        _ => unreachable!("canonical_named_action returned an unsupported action"),
    })
}

fn double_tap(buttons: LocalButtons, player: Player) -> Vec<ActionSegment> {
    vec![
        ActionSegment::new(buttons.for_player(player), 2),
        ActionSegment::new(ActionButtons::default(), 2),
        ActionSegment::new(buttons.for_player(player), 5),
        ActionSegment::new(ActionButtons::default(), 3),
    ]
}

fn normalized_name(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-', '+'], "_")
}

#[cfg(test)]
mod tests {
    use super::{
        BLOODY_ROAR_2_ROSTER, Facing, Player, canonical_character_name, canonical_named_action,
        named_action_sequence,
    };

    #[test]
    fn all_roster_characters_have_named_actions() {
        for character in BLOODY_ROAR_2_ROSTER {
            let sequence =
                named_action_sequence(character, Player::One, "transform", Facing::Right)
                    .expect("character action");
            assert!(sequence[0].buttons.beast);
            assert_eq!(sequence.last().unwrap().buttons, Default::default());
        }
        assert_eq!(canonical_character_name("4"), Some("long"));
        assert_eq!(canonical_character_name("Shen-Long"), Some("shenlong"));
    }

    #[test]
    fn named_actions_map_facing_and_player_without_sticky_buttons() {
        let p1 = named_action_sequence("long", Player::One, "forward-punch", Facing::Left)
            .expect("P1 action");
        assert!(p1[0].buttons.left);
        assert!(p1[0].buttons.punch);
        assert_eq!(p1.last().unwrap().buttons, Default::default());

        let p2 = named_action_sequence("jenny", Player::Two, "dash_forward", Facing::Left)
            .expect("P2 action");
        assert!(p2[0].buttons.p2_left);
        assert!(p2[2].buttons.p2_left);
        assert_eq!(p2.last().unwrap().buttons, Default::default());
        assert_eq!(canonical_named_action("beast"), Some("transform"));
    }
}
