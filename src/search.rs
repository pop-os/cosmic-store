// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use cosmic::{
    Element, cosmic_theme,
    iced::{Alignment, Length},
    theme, widget,
};

use crate::app_id::AppId;
use crate::app_info::AppInfo;
use crate::backend::{BackendName, Backends};
use crate::explore::ExplorePage;
use crate::{ICON_SIZE_SEARCH, MAX_RESULTS, Message};

pub struct GridMetrics {
    pub cols: usize,
    pub item_width: usize,
    pub column_spacing: u16,
}

impl GridMetrics {
    pub fn new(width: usize, min_width: usize, column_spacing: u16) -> Self {
        let width_m1 = width.saturating_sub(min_width);
        let cols_m1 = width_m1 / (min_width + column_spacing as usize);
        let cols = cols_m1 + 1;
        let item_width = width
            .saturating_sub(cols_m1 * column_spacing as usize)
            .checked_div(cols)
            .unwrap_or(0);
        Self {
            cols,
            item_width,
            column_spacing,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub backend_name: BackendName,
    pub id: AppId,
    pub icon_opt: Option<widget::icon::Handle>,
    // Info from selected source
    pub info: Arc<AppInfo>,
    pub weight: i64,
}

/// Cached version of SearchResult for disk storage (with resolved icon paths)
#[derive(Clone, Debug, bitcode::Decode, bitcode::Encode)]
struct CachedSearchResult {
    backend_name: String,
    id: AppId,
    info: AppInfo,
    icon_path: Option<String>,
    weight: i64,
}

/// Cached explore page results
#[derive(Clone, Debug, bitcode::Decode, bitcode::Encode)]
pub struct CachedExploreResults {
    results: Vec<(ExplorePage, Vec<CachedSearchResult>)>,
}

impl CachedExploreResults {
    fn cache_path() -> Option<std::path::PathBuf> {
        dirs::cache_dir().map(|p| p.join("cosmic-store").join("explore_cache.bin.zst"))
    }

    pub fn load() -> Option<Self> {
        let total_start = Instant::now();
        let path = Self::cache_path()?;

        let disk_start = Instant::now();
        let compressed = std::fs::read(&path).ok()?;
        let disk_time = disk_start.elapsed();

        let decompress_start = Instant::now();
        let data = zstd::decode_all(compressed.as_slice()).ok()?;
        let decompress_time = decompress_start.elapsed();

        let decode_start = Instant::now();
        let result: Self = bitcode::decode(&data).ok()?;
        let decode_time = decode_start.elapsed();

        log::info!(
            "cache load: {} KB compressed -> {} KB uncompressed, disk={:?}, decompress={:?}, decode={:?}, total={:?}",
            compressed.len() / 1024,
            data.len() / 1024,
            disk_time,
            decompress_time,
            decode_time,
            total_start.elapsed()
        );
        Some(result)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let total_start = Instant::now();
        let path = Self::cache_path().ok_or("no cache dir")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let encode_start = Instant::now();
        let data = bitcode::encode(self);
        let encode_time = encode_start.elapsed();

        let compress_start = Instant::now();
        let compressed = zstd::encode_all(data.as_slice(), 3)?;
        let compress_time = compress_start.elapsed();

        let disk_start = Instant::now();
        std::fs::write(&path, &compressed)?;
        let disk_time = disk_start.elapsed();

        log::info!(
            "cache save: {} KB uncompressed -> {} KB compressed, encode={:?}, compress={:?}, disk={:?}, total={:?}",
            data.len() / 1024,
            compressed.len() / 1024,
            encode_time,
            compress_time,
            disk_time,
            total_start.elapsed()
        );
        Ok(())
    }

    pub fn from_results(
        results: &HashMap<ExplorePage, Vec<SearchResult>>,
        backends: &Backends,
    ) -> Self {
        let cached: Vec<_> = results
            .iter()
            .map(|(page, search_results)| {
                let cached_results: Vec<_> = search_results
                    .iter()
                    .take(MAX_RESULTS)
                    .map(|r| {
                        // Resolve icon path using backend
                        let icon_path = backends
                            .get(&r.backend_name)
                            .and_then(|backend| {
                                backend
                                    .info_caches()
                                    .iter()
                                    .find(|c| c.source_id == r.info.source_id)
                            })
                            .and_then(|cache| cache.icon_path_for_info(&r.info))
                            .map(|p| p.to_string_lossy().into_owned());

                        CachedSearchResult {
                            backend_name: r.backend_name.to_string(),
                            id: r.id.clone(),
                            info: (*r.info).clone(),
                            icon_path,
                            weight: r.weight,
                        }
                    })
                    .collect();
                (*page, cached_results)
            })
            .collect();
        Self { results: cached }
    }

    pub fn to_results(&self) -> HashMap<ExplorePage, Vec<SearchResult>> {
        self.results
            .iter()
            .map(|(page, cached_results)| {
                let results: Vec<_> = cached_results
                    .iter()
                    .filter_map(|c| {
                        // Parse backend name, skip if unknown
                        let backend_name: BackendName = c.backend_name.parse().ok()?;
                        // Create icon from cached path, or use default icon
                        let icon_opt = Some(match &c.icon_path {
                            Some(path) => widget::icon::from_path(std::path::PathBuf::from(path)),
                            None => widget::icon::from_name("package-x-generic")
                                .size(128)
                                .handle(),
                        });
                        Some(SearchResult {
                            backend_name,
                            id: c.id.clone(),
                            icon_opt,
                            info: Arc::new(c.info.clone()),
                            weight: c.weight,
                        })
                    })
                    .collect();
                (*page, results)
            })
            .collect()
    }
}

/// Preserve icons from old results when new results arrive (avoids flicker)
pub fn preserve_icons_from(old_results: &[SearchResult], new_results: &mut [SearchResult]) {
    let old_icons: HashMap<&AppId, &widget::icon::Handle> = old_results
        .iter()
        .filter_map(|r| r.icon_opt.as_ref().map(|icon| (&r.id, icon)))
        .collect();
    for result in new_results {
        if result.icon_opt.is_none() {
            if let Some(icon) = old_icons.get(&result.id) {
                result.icon_opt = Some((*icon).clone());
            }
        }
    }
}

/// Apply loaded icons to search results
pub fn apply_icons_to_results(
    results: &mut [SearchResult],
    icons: Vec<(usize, widget::icon::Handle)>,
) {
    for (i, icon) in icons {
        if let Some(result) = results.get_mut(i) {
            result.icon_opt = Some(icon);
        }
    }
}

impl SearchResult {
    pub fn grid_metrics(spacing: &cosmic_theme::Spacing, width: usize) -> GridMetrics {
        GridMetrics::new(width, 240 + 2 * spacing.space_s as usize, spacing.space_xxs)
    }

    pub fn grid_view<'a, F: Fn(usize) -> Message + 'a>(
        results: &'a [Self],
        spacing: cosmic_theme::Spacing,
        width: usize,
        callback: F,
    ) -> Element<'a, Message> {
        let GridMetrics {
            cols,
            item_width,
            column_spacing,
        } = Self::grid_metrics(&spacing, width);

        let mut grid = widget::grid();
        let mut col = 0;
        for (result_i, result) in results.iter().enumerate() {
            if col >= cols {
                grid = grid.insert_row();
                col = 0;
            }
            grid = grid.push(
                widget::mouse_area(result.card_view(&spacing, item_width))
                    .on_press(callback(result_i)),
            );
            col += 1;
        }
        grid.column_spacing(column_spacing)
            .row_spacing(column_spacing)
            .into()
    }

    pub fn card_view<'a>(
        &'a self,
        spacing: &cosmic_theme::Spacing,
        width: usize,
    ) -> Element<'a, Message> {
        widget::container(
            widget::row::with_children(vec![
                match &self.icon_opt {
                    Some(icon) => widget::icon::icon(icon.clone())
                        .size(ICON_SIZE_SEARCH)
                        .into(),
                    None => {
                        widget::Space::with_width(Length::Fixed(ICON_SIZE_SEARCH as f32)).into()
                    }
                },
                widget::column::with_children(vec![
                    widget::text::body(&self.info.name)
                        .height(Length::Fixed(20.0))
                        .into(),
                    widget::text::caption(&self.info.summary)
                        .height(Length::Fixed(28.0))
                        .into(),
                ])
                .into(),
            ])
            .align_y(Alignment::Center)
            .spacing(spacing.space_s),
        )
        .align_y(Alignment::Center)
        .width(Length::Fixed(width as f32))
        .height(Length::Fixed(48.0 + (spacing.space_xxs as f32) * 2.0))
        .padding([spacing.space_xxs, spacing.space_s])
        .class(theme::Container::Card)
        .into()
    }
}
