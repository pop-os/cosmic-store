use cosmic::iced::keyboard::{Key, Modifiers};
use std::{collections::HashMap, fmt};

use crate::Action;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Modifier {
    Super,
    Ctrl,
    Alt,
    Shift,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KeyBind {
    pub modifiers: Vec<Modifier>,
    pub key: Key,
}

impl KeyBind {
    pub fn matches(&self, modifiers: Modifiers, key: &Key) -> bool {
        key == &self.key
            && modifiers.logo() == self.modifiers.contains(&Modifier::Super)
            && modifiers.control() == self.modifiers.contains(&Modifier::Ctrl)
            && modifiers.alt() == self.modifiers.contains(&Modifier::Alt)
            && modifiers.shift() == self.modifiers.contains(&Modifier::Shift)
    }
}

impl fmt::Display for KeyBind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for modifier in self.modifiers.iter() {
            write!(f, "{:?} + ", modifier)?;
        }
        match &self.key {
            Key::Character(c) => write!(f, "{}", c.to_uppercase()),
            Key::Named(named) => write!(f, "{:?}", named),
            other => write!(f, "{:?}", other),
        }
    }
}

//TODO: load from config
pub fn key_binds() -> HashMap<KeyBind, Action> {
    let mut key_binds = HashMap::new();

    macro_rules! bind {
        ([$($modifier:ident),+ $(,)?], $key:expr, $action:ident) => {{
            key_binds.insert(
                KeyBind {
                    modifiers: vec![$(Modifier::$modifier),+],
                    key: $key,
                },
                Action::$action,
            );
        }};
    }

    bind!([Ctrl], Key::Character("f".into()), SearchActivate);

    key_binds
}
