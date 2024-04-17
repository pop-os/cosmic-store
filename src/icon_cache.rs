// SPDX-License-Identifier: GPL-3.0-only

use cosmic::widget::icon;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IconCacheKey {
    name: &'static str,
    size: u16,
}

pub struct IconCache {
    cache: HashMap<IconCacheKey, icon::Handle>,
}

impl IconCache {
    pub fn new() -> Self {
        let mut cache = HashMap::new();

        macro_rules! bundle {
            ($name:expr, $size:expr) => {
                let data: &'static [u8] = include_bytes!(concat!("../res/icons/", $name, ".svg"));
                cache.insert(
                    IconCacheKey {
                        name: $name,
                        size: $size,
                    },
                    icon::from_svg_bytes(data).symbolic(true),
                );
            };
        }

        bundle!("store-create-symbolic", 16);
        bundle!("store-develop-symbolic", 16);
        bundle!("store-game-symbolic", 16);
        bundle!("store-home-symbolic", 16);
        bundle!("store-installed-symbolic", 16);
        bundle!("store-learn-symbolic", 16);
        bundle!("store-relax-symbolic", 16);
        bundle!("store-socialize-symbolic", 16);
        bundle!("store-updates-symbolic", 16);
        bundle!("store-utilities-symbolic", 16);
        bundle!("store-work-symbolic", 16);

        Self { cache }
    }

    pub fn get(&mut self, name: &'static str, size: u16) -> icon::Handle {
        self.cache
            .entry(IconCacheKey { name, size })
            .or_insert_with(|| icon::from_name(name).size(size).handle())
            .clone()
    }
}

static ICON_CACHE: OnceLock<Mutex<IconCache>> = OnceLock::new();

pub fn icon_cache_handle(name: &'static str, size: u16) -> icon::Handle {
    let mut icon_cache = ICON_CACHE
        .get_or_init(|| Mutex::new(IconCache::new()))
        .lock()
        .unwrap();
    icon_cache.get(name, size)
}

pub fn icon_cache_icon(name: &'static str, size: u16) -> icon::Icon {
    icon::icon(icon_cache_handle(name, size)).size(size)
}
