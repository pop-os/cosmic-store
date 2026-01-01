# Phase 1 Implementation Plan: Visualize Existing Data

## Overview

This plan implements quick wins to improve user experience by visualizing existing data (monthly downloads, verified status, editor's choice) in Cosmic Store search results.

## Goals

1. Display download counts in search results
2. Add sorting options (Relevance, Most Popular, Recently Updated)
3. Highlight "Editor's Choice" apps with gold star badge
4. Display "Verified" badge for apps officially maintained by developers

## Implementation Tasks

### Task 1: Add Download Count Display

**Files to Modify**:
- [`src/main.rs`](../src/main.rs) - Update `SearchResult::card_view()`
- [`i18n/en/cosmic_store.ftl`](../i18n/en/cosmic_store.ftl) - Add localization strings

**Changes Required**:

1. **Add download count formatting function**:
   ```rust
   fn format_download_count(count: u64) -> String {
       if count >= 1_000_000 {
           format!("{:.1}M", count as f64 / 1_000_000.0)
       } else if count >= 1_000 {
           format!("{:.1}K", count as f64 / 1_000.0)
       } else {
           count.to_string()
       }
   }
   ```

2. **Update SearchResult::card_view()** to show download count:
   ```rust
   // Add download count row below summary
   widget::row::with_children(vec![
       widget::text::caption(&self.info.summary).into(),
       widget::horizontal_space().width(Length::Fill).into(),
       widget::text::caption(format_download_count(self.info.monthly_downloads)).into(),
   ])
   ```

3. **Add localization string**:
   ```ftl
   monthly-downloads = Flathub monthly downloads
   ```

### Task 2: Add Sorting Options

**Files to Modify**:
- [`src/main.rs`](../src/main.rs) - Add sort mode enum and update search logic
- [`i18n/en/cosmic_store.ftl`](../i18n/en/cosmic_store.ftl) - Add localization strings

**Changes Required**:

1. **Add SearchSortMode enum**:
   ```rust
   pub enum SearchSortMode {
       Relevance,      // Current behavior (default)
       MostDownloads,   // By monthly_downloads descending
       RecentlyUpdated, // By release timestamp descending
   }
   ```

2. **Update App struct** to include current sort mode:
   ```rust
   pub struct App {
       // ... existing fields ...
       search_sort_mode: SearchSortMode,
   }
   ```

3. **Update search() function** to use sort mode:
   ```rust
   fn search(&self, sort_mode: SearchSortMode) -> Task<Message> {
       // ... existing search logic ...
       // Use sort_mode to determine ordering
       let results = match sort_mode {
           SearchSortMode::Relevance => {
               // Current weight-based sorting
           }
           SearchSortMode::MostDownloads => {
               // Sort by monthly_downloads descending
           }
           SearchSortMode::RecentlyUpdated => {
               // Sort by latest release timestamp
           }
       };
   }
   ```

4. **Add sort dropdown to search UI**:
   ```rust
   // In header_start() or search UI
   widget::dropdown(
       &vec![fl!("sort-relevance"), fl!("sort-popular"), fl!("sort-recent")],
       Some(sort_mode_index),
       Message::SearchSortMode(sort_mode),
   )
   ```

5. **Add localization strings**:
   ```ftl
   sort-relevance = Relevance
   sort-popular = Most Popular
   sort-recent = Recently Updated
   ```

### Task 3: Highlight Editor's Choice

**Files to Modify**:
- [`src/main.rs`](../src/main.rs) - Update `SearchResult::card_view()`
- [`i18n/en/cosmic_store.ftl`](../i18n/en/cosmic_store.ftl) - Add localization strings

**Changes Required**:

1. **Add EDITORS_CHOICE constant**:
   ```rust
   // Already exists in src/editors_choice.rs
   // We need to check if app is in this list
   ```

2. **Update SearchResult::card_view()** to add gold star:
   ```rust
   // Check if app is in editors_choice
   let is_editors_choice = EDITORS_CHOICE
       .iter()
       .any(|choice_id| choice_id == &self.info.id.normalized());

   // Add gold star badge
   let mut card_children = vec![
       widget::text::body(&self.info.name).into(),
       widget::text::caption(&self.info.summary).into(),
       widget::row::with_children(vec![
           widget::text::caption(format_download_count(self.info.monthly_downloads)).into(),
           // Gold star for Editor's Choice
           is_editors_choice.then(|| {
               widget::icon::from_name("starred-symbolic")
                   .size(16)
                   .into()
           }),
       ]).into(),
   ];
   ```

3. **Add localization string**:
   ```ftl
   editors-choice = Editor's Choice
   ```

### Task 4: Add Verified Badge

**Files to Modify**:
- [`src/main.rs`](../src/main.rs) - Update `SearchResult::card_view()`
- [`src/app_info.rs`](../src/app_info.rs) - Add `verified` field to `AppInfo`
- [`src/appstream_cache.rs`](../src/appstream_cache.rs) - Pass verified status when creating `AppInfo`

**Changes Required**:

1. **Add verified field to AppInfo**:
   ```rust
   pub struct AppInfo {
       // ... existing fields ...
       pub verified: bool,  // From Flathub
   }
   ```

2. **Update AppInfo::new()** to accept verified parameter:
   ```rust
   pub fn new(
       // ... existing parameters ...
       verified: bool,  // New parameter
   ) -> Self {
       // ... existing code ...
       Self {
           // ... existing fields ...
           verified,
       }
   }
   ```

3. **Update appstream_cache.rs** to pass verified status:
   ```rust
   // When creating AppInfo from Component
   Arc::new(AppInfo::new(
       &self.source_id,
       &self.source_name,
       origin_opt.as_deref(),
       component,
       &self.locale,
       monthly_downloads,
       verified,  // Add this parameter
   ))
   ```

4. **Update SearchResult::card_view()** to show verified badge:
   ```rust
   // Add verified badge
   let mut card_children = vec![
       widget::text::body(&self.info.name).into(),
       widget::text::caption(&self.info.summary).into(),
       widget::row::with_children(vec![
           widget::text::caption(format_download_count(self.info.monthly_downloads)).into(),
           // Verified badge
           self.info.verified.then(|| {
               widget::icon::from_name("emblem-ok-symbolic")
                   .size(16)
                   .into()
           }),
       ]).into(),
   ];
   ```

5. **Add localization string**:
   ```ftl
   verified = Verified
   ```

## Implementation Order

1. **Task 1**: Add download count formatting and display
2. **Task 2**: Add sorting options (enum, logic, UI)
3. **Task 3**: Highlight Editor's Choice apps
4. **Task 4**: Add verified badge display

## Testing Checklist

- [ ] Download counts display correctly (1.2M, 45K, etc.)
- [ ] Sorting dropdown appears and works correctly
- [ ] "Most Popular" sort orders by monthly downloads descending
- [ ] "Recently Updated" sort orders by release timestamp
- [ ] Editor's Choice apps show gold star badge
- [ ] Verified apps show checkmark badge
- [ ] Badges don't clutter the UI
- [ ] Search results remain responsive with new elements
- [ ] Localization strings work for all supported languages

## Files Modified

- [`src/main.rs`](../src/main.rs)
  - `format_download_count()` function
  - `SearchSortMode` enum
  - `App` struct (add search_sort_mode field)
  - `search()` function (add sort_mode parameter)
  - `SearchResult::card_view()` (add download count, verified badge, editors choice badge)
  - `header_start()` or search UI (add sort dropdown)

- [`src/app_info.rs`](../src/app_info.rs)
  - `AppInfo` struct (add `verified` field)
  - `AppInfo::new()` (add `verified` parameter)

- [`src/appstream_cache.rs`](../src/appstream_cache.rs)
  - Pass `verified` parameter when creating `AppInfo`

- [`i18n/en/cosmic_store.ftl`](../i18n/en/cosmic_store.ftl)
  - Add localization strings

## Notes

- Verified status data source needs to be determined (Flathub API or manual mapping)
- Editor's Choice list is in [`src/editors_choice.rs`](../src/editors_choice.rs)
- The cosmic-widget library provides icon widgets needed
- Download count formatting should handle edge cases (0, very large numbers)
