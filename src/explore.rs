// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use crate::fl;
use crate::nav::Category;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum ExplorePage {
    EditorsChoice,
    PopularApps,
    MadeForCosmic,
    NewApps,
    RecentlyUpdated,
    DevelopmentTools,
    ScientificTools,
    ProductivityApps,
    GraphicsAndPhotographyTools,
    SocialNetworkingApps,
    Games,
    MusicAndVideoApps,
    AppsForLearning,
    Utilities,
}

impl ExplorePage {
    pub fn all() -> &'static [Self] {
        &[
            Self::EditorsChoice,
            Self::PopularApps,
            Self::MadeForCosmic,
            //TODO: Self::NewApps,
            Self::RecentlyUpdated,
            Self::DevelopmentTools,
            Self::ScientificTools,
            Self::ProductivityApps,
            Self::GraphicsAndPhotographyTools,
            Self::SocialNetworkingApps,
            Self::Games,
            Self::MusicAndVideoApps,
            Self::AppsForLearning,
            Self::Utilities,
        ]
    }

    pub fn title(&self) -> String {
        match self {
            Self::EditorsChoice => fl!("editors-choice"),
            Self::PopularApps => fl!("popular-apps"),
            Self::MadeForCosmic => fl!("made-for-cosmic"),
            Self::NewApps => fl!("new-apps"),
            Self::RecentlyUpdated => fl!("recently-updated"),
            Self::DevelopmentTools => fl!("development-tools"),
            Self::ScientificTools => fl!("scientific-tools"),
            Self::ProductivityApps => fl!("productivity-apps"),
            Self::GraphicsAndPhotographyTools => fl!("graphics-and-photography-tools"),
            Self::SocialNetworkingApps => fl!("social-networking-apps"),
            Self::Games => fl!("games"),
            Self::MusicAndVideoApps => fl!("music-and-video-apps"),
            Self::AppsForLearning => fl!("apps-for-learning"),
            Self::Utilities => fl!("utilities"),
        }
    }

    pub fn categories(&self) -> &'static [Category] {
        match self {
            Self::DevelopmentTools => &[Category::Development],
            Self::ScientificTools => &[Category::Science],
            Self::ProductivityApps => &[Category::Office],
            Self::GraphicsAndPhotographyTools => &[Category::Graphics],
            Self::SocialNetworkingApps => &[Category::Network],
            Self::Games => &[Category::Game],
            Self::MusicAndVideoApps => &[Category::AudioVideo],
            Self::AppsForLearning => &[Category::Education],
            Self::Utilities => &[Category::Settings, Category::System, Category::Utility],
            _ => &[],
        }
    }
}
