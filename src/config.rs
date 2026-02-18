// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
    theme,
};
use serde::{Deserialize, Serialize};

pub const CONFIG_VERSION: u64 = 2;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AppTheme {
    Dark,
    Light,
    System,
}

impl AppTheme {
    pub fn theme(&self) -> theme::Theme {
        match self {
            Self::Dark => theme::Theme::dark(),
            Self::Light => theme::Theme::light(),
            Self::System => theme::system_preference(),
        }
    }
}

#[derive(Clone, CosmicConfigEntry, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Config {
    pub app_theme: AppTheme,
    pub update_check_interval_minutes: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_theme: AppTheme::System,
            update_check_interval_minutes: 60,
        }
    }
}
