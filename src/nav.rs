// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;

use cosmic::widget;

use crate::app_id::AppId;
use crate::fl;
use crate::icon_cache::icon_cache_icon;

// From https://specifications.freedesktop.org/menu-spec/latest/apa.html
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    AudioVideo,
    Development,
    Education,
    Game,
    Graphics,
    Network,
    Office,
    Science,
    Settings,
    System,
    Utility,
    CosmicApplet,
}

impl Category {
    pub fn id(&self) -> &'static str {
        match self {
            Self::AudioVideo => "AudioVideo",
            Self::Development => "Development",
            Self::Education => "Education",
            Self::Game => "Game",
            Self::Graphics => "Graphics",
            Self::Network => "Network",
            Self::Office => "Office",
            Self::Science => "Science",
            Self::Settings => "Settings",
            Self::System => "System",
            Self::Utility => "Utility",
            Self::CosmicApplet => "CosmicApplet",
        }
    }
}

pub type CategoryIndex = HashMap<String, Vec<AppId>>;

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub enum NavPage {
    #[default]
    Explore,
    Create,
    Work,
    Develop,
    Learn,
    Game,
    Relax,
    Socialize,
    Utilities,
    Applets,
    Installed,
    Updates,
}

impl NavPage {
    pub fn all() -> &'static [Self] {
        &[
            Self::Explore,
            Self::Create,
            Self::Work,
            Self::Develop,
            Self::Learn,
            Self::Game,
            Self::Relax,
            Self::Socialize,
            Self::Utilities,
            Self::Applets,
            Self::Installed,
            Self::Updates,
        ]
    }

    pub fn title(&self) -> String {
        match self {
            Self::Explore => fl!("explore"),
            Self::Create => fl!("create"),
            Self::Work => fl!("work"),
            Self::Develop => fl!("develop"),
            Self::Learn => fl!("learn"),
            Self::Game => fl!("game"),
            Self::Relax => fl!("relax"),
            Self::Socialize => fl!("socialize"),
            Self::Utilities => fl!("utilities"),
            Self::Applets => fl!("applets"),
            Self::Installed => fl!("installed-apps"),
            Self::Updates => fl!("updates"),
        }
    }

    // From https://specifications.freedesktop.org/menu-spec/latest/apa.html
    pub fn categories(&self) -> Option<&'static [Category]> {
        match self {
            Self::Create => Some(&[Category::AudioVideo, Category::Graphics]),
            Self::Work => Some(&[Category::Development, Category::Office, Category::Science]),
            Self::Develop => Some(&[Category::Development]),
            Self::Learn => Some(&[Category::Education]),
            Self::Game => Some(&[Category::Game]),
            Self::Relax => Some(&[Category::AudioVideo]),
            Self::Socialize => Some(&[Category::Network]),
            Self::Utilities => Some(&[Category::Settings, Category::System, Category::Utility]),
            Self::Applets => Some(&[Category::CosmicApplet]),
            _ => None,
        }
    }

    pub fn icon(&self) -> widget::icon::Icon {
        match self {
            Self::Explore => icon_cache_icon("store-home-symbolic", 16),
            Self::Create => icon_cache_icon("store-create-symbolic", 16),
            Self::Work => icon_cache_icon("store-work-symbolic", 16),
            Self::Develop => icon_cache_icon("store-develop-symbolic", 16),
            Self::Learn => icon_cache_icon("store-learn-symbolic", 16),
            Self::Game => icon_cache_icon("store-game-symbolic", 16),
            Self::Relax => icon_cache_icon("store-relax-symbolic", 16),
            Self::Socialize => icon_cache_icon("store-socialize-symbolic", 16),
            Self::Utilities => icon_cache_icon("store-utilities-symbolic", 16),
            Self::Applets => icon_cache_icon("store-applets-symbolic", 16),
            Self::Installed => icon_cache_icon("store-installed-symbolic", 16),
            Self::Updates => icon_cache_icon("store-updates-symbolic", 16),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ScrollContext {
    NavPage,
    ExplorePage,
    SearchResults,
    Selected,
}

impl ScrollContext {
    pub fn unused_contexts(&self) -> &'static [ScrollContext] {
        // Contexts that can be safely removed when another is active
        match self {
            Self::NavPage => &[Self::Selected, Self::SearchResults, Self::ExplorePage],
            Self::ExplorePage => &[Self::Selected, Self::SearchResults],
            Self::SearchResults => &[Self::Selected],
            Self::Selected => &[],
        }
    }
}
