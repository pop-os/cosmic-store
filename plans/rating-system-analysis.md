# Cosmic Store Rating System Analysis

## Executive Summary

This document analyzes current search result ordering and display mechanisms in the Pop OS Cosmic Store, and explores options for implementing a rating/recommendation system to help users identify quality applications.

## Flathub Research Findings

**Key Discovery**: Flathub.org has implemented a **new quality evaluation system** that influences how apps are presented, though it does not provide traditional star ratings.

### What Flathub Shows:
1. **Verified Badge**: Apps maintained directly by official developers (1,000+ apps as of April 2024)
   - Indicates the Flatpak package is officially maintained by the source
   - Does NOT guarantee safety - trust remains the user's responsibility
   - Can be displayed in Cosmic Store

2. **Monthly Downloads**: Download statistics are available via stats API
   - Used as a popularity metric
   - Already integrated into Cosmic Store

3. **Quality Evaluation System**:
   - Flathub editors rate apps based on metadata quality criteria
   - Developers can view these ratings on their app's page to identify improvement areas
   - Criteria include:
     * Avoiding icons that fill entire canvas
     * Ensuring summaries don't repeat the app name
     * Including window shadows in screenshots
   - High-quality metadata apps may be featured more prominently (banners, curated lists, recommendations)

4. **NO Star Ratings**: Traditional user star rating system is NOT available
   - No public API for fetching average ratings
   - No user review count available

### API Endpoints Available:
- `/stats/{year}/{month}/{day}.json` - Download statistics (already used by Cosmic Store)
- Appstream data - XML/YAML metadata files
- Search API - Basic app search
- **Quality ratings**: Internal system for evaluating metadata quality (not publicly accessible)

### What This Means:
- **No External Rating Source**: Flathub doesn't expose star ratings through public API
- **Verified Status Available**: Can be fetched and displayed to indicate official maintenance
- **Custom Rating System Required**: Any star rating system would need to be built from scratch
- **Download-Based Popularity**: Monthly downloads remain the only quantitative popularity metric
- **Quality Indicators**: Flathub's internal quality ratings could potentially be leveraged for curation

## Current State Analysis

### Search Result Ordering Mechanism

**Location**: [`src/main.rs:909-975`](../src/main.rs:909) - `generic_search()` function

The search uses a **weight-based ranking system** where lower weight = higher priority:

```rust
// Weight calculation formula (lines 1196-1199)
let stats_weight = |weight: i64| -> i64 {
    (weight << 56) - (info.monthly_downloads as i64)
};
```

**Match Priority Weights**:
| Field | Match Type | Weight |
|-------|------------|--------|
| Name | Exact match | 0 |
| Name | Starts with | 1 |
| Name | Contains | 2 |
| Summary | Exact match | 3 |
| Summary | Starts with | 4 |
| Summary | Contains | 5 |
| Description | Exact match | 6 |
| Description | Starts with | 7 |
| Description | Contains | 8 |

**Final Sort Order** (lines 952-958):
1. Primary: Weight (lower = higher priority)
2. Secondary: App name (alphabetical)
3. Tertiary: Backend name

### Available Data Sources

**From AppStream/AppInfo** ([`src/app_info.rs:148-169`](../src/app_info.rs:148)):
- `monthly_downloads: u64` - Popularity metric from Flathub
- `name`, `summary`, `description` - Text fields for search
- `developer_name`, `categories`, `releases` - Metadata
- `source_id`, `source_name` - Repository information
- `license_opt` - License information
- `screenshots`, `icons` - Visual assets
- `urls` - Links to homepage, bug tracker, etc.

**Flathub Statistics** ([`src/stats.rs`](../src/stats.rs)):
- Monthly downloads data is fetched from Flathub's public API
- Compiled into bitcode file: [`res/flathub-stats.bitcode-v0-6`](../res/flathub-stats.bitcode-v0-6)
- Updated via [`flathub-stats`](../flathub-stats/) utility

**Explore Pages** ([`src/main.rs:1021-1111`](../src/main.rs:1021)):
- "Popular apps" - Sorted by monthly downloads (descending)
- "Editor's choice" - Hardcoded list in [`src/editors_choice.rs`](../src/editors_choice.rs)
- "Recently updated" - Sorted by release timestamp
- Category pages - Sorted by monthly downloads within category

### Current UI Display

**Search Results Card** ([`src/main.rs:687-721`](../src/main.rs:687)):
```
┌─────────────────────────────────────┐
│ [ICON]  App Name                 │
│          App summary text...       │
└─────────────────────────────────────┘
```
- Shows: Icon, name, summary
- **NO**: Rating, download count, or quality indicator

**Details Page** ([`src/main.rs:2299-2307`](../src/main.rs:2299)):
- Shows monthly downloads count (but only in details view)
- Shows developer name, source, license, screenshots, etc.
- **NO**: Star rating or user reviews

## Identified Issues

### User Experience Problems

1. **No Visual Quality Indicators**: Users cannot quickly identify which apps are highly rated or popular
2. **Hidden Ordering Logic**: Users don't understand why results appear in a certain order
3. **No "Recommended" Visuals**: Popular apps look identical to obscure ones in search results
4. **Limited Sorting Options**: Users cannot sort by popularity, rating, or recency in search

### Technical Limitations

1. **No Rating Data**: AppStream format doesn't include star ratings
2. **Flathub Limitations**: Flathub API only provides download statistics, no ratings
3. **Single Metric**: Monthly downloads is the only popularity metric available

## Potential Solutions

### Option 0: Leverage Flathub Quality Indicators (New Discovery)

**Approach**: Utilize Flathub's existing quality evaluation and verified status

**What's Available**:
1. **Verified Status**: Apps maintained directly by official developers (1,000+ apps)
   - Can be displayed as a checkmark badge
   - Indicates official maintenance, not safety
2. **Quality Ratings**: Internal system for metadata quality
   - May influence curation and featured apps
   - Not publicly accessible via API

**Pros**:
- Uses existing Flathub data
- "Verified" badge provides trust signal
- Quality ratings could inform curation decisions

**Cons**:
- Quality ratings not publicly accessible
- Verified ≠ safe (user must still trust developer)
- No star ratings for user feedback

**Implementation**:
```rust
// Add to AppInfo struct
pub struct AppInfo {
    // ... existing fields ...
    pub verified: bool,              // From Flathub
    pub quality_score: Option<f32>,   // If available from Flathub
}
```

### Option 1: Visualize Existing Data (Quick Win)

**Approach**: Display monthly downloads as a quality indicator

**Pros**:
- Uses existing data, no API changes needed
- Simple to implement
- Clear metric (more downloads = more popular)

**Cons**:
- Downloads ≠ quality (popular app could be poorly maintained)
- New apps have disadvantage

**UI Ideas**:
```
┌─────────────────────────────────────┐
│ [ICON]  App Name        ★★★★★ │
│          App summary...  1.2M   │
└─────────────────────────────────────┘
```

### Option 2: Build Custom Rating System

**Approach**: Create a local rating/review system

**Features**:
- Allow users to rate apps (1-5 stars)
- Store ratings locally or sync to a cloud service
- Show average ratings in search results
- Display user reviews in details page

**Pros**:
- Provides actual user feedback
- More meaningful than download counts
- Can show recent reviews
- Community-driven quality assessment

**Cons**:
- Requires backend infrastructure for storing/syncing
- Privacy considerations for user data
- Cold start problem (no ratings initially)
- Moderation needed for spam/abuse

### Option 3: Hybrid Quality Score

**Approach**: Combine multiple metrics into a quality score

**Factors to Consider**:
1. Monthly downloads (popularity)
2. Recent updates (maintenance activity)
3. License type (open source preference)
4. Age of app (stability indicator)
5. Editor's choice badge

**Formula Example**:
```
Quality Score = (downloads_weight * 0.4) +
               (recency_weight * 0.3) +
               (update_frequency * 0.2) +
               (open_source_bonus * 0.1)
```

**Pros**:
- More nuanced than single metric
- Can be tuned/adjusted
- Encourages best practices

**Cons**:
- Complex to explain to users
- Weights are subjective
- May be controversial

### Option 4: Badge System

**Approach**: Use visual badges to highlight quality indicators

**Badge Types**:
- 🏆 Editor's Choice (already implemented)
- 🔥 Popular (high downloads)
- 🆕 Recently Updated
- 📦 Open Source
- ✅ Verified

**Pros**:
- Visual and intuitive
- Can show multiple attributes
- Easy to understand

**Cons**:
- Badge fatigue if overused
- Still needs underlying data

## Recommended Approach

### Phase 1: Leverage Flathub Quality Indicators (Quick Win)

**Changes Needed**:

1. **Add "Verified" badge**
   - Display checkmark for verified apps (from Flathub)
   - Show in both search results and details page
   - Modify [`SearchResult::card_view()`](../src/main.rs:687)
   - Use widget::icon::from_name("emblem-ok-symbolic")

2. **Add download count to search results**
   - Modify [`SearchResult::card_view()`](../src/main.rs:687)
   - Add formatted download count display
   - Use icons/abbreviations (e.g., "1.2M", "45K")

3. **Add sorting options**
   - Add dropdown for sort order: "Relevance", "Most Popular", "Recently Updated"
   - Modify [`generic_search()`](../src/main.rs:909) to accept sort parameter
   - Update search message handling

4. **Highlight "Editor's Choice"**
   - Add badge to search results for apps in editors_choice list
   - Make visually distinct (e.g., gold star icon)

### Phase 2: Build Custom Rating System (Medium Term)

### Phase 2: Custom Rating System (Medium Term)

**Changes Needed**:

1. **Add rating data model**
   - Create new struct for user ratings
   - Store locally in `~/.config/cosmic-store/ratings.json`
   - Consider optional cloud sync

2. **Add rating UI to details page**
   - Star rating widget (1-5 stars)
   - Text review input
   - Submit button

3. **Display ratings in search results**
   - Show average star rating
   - Show rating count (e.g., "4.5 ★ (23 reviews)")
   - Add to [`SearchResult::card_view()`](../src/main.rs:687)

4. **Add "Top Rated" explore page**
   - Sort by average rating
   - Add to navigation

### Phase 3: Advanced Features (Long Term)

1. **User review system**
   - Display user reviews in details page
   - Allow filtering by rating/date
   - Report inappropriate reviews

2. **Personalized recommendations**
   - Track user's installed apps
   - Recommend similar apps
   - "Users who installed X also installed Y"

3. **Quality badges**
   - "Verified" badge for well-maintained apps
   - "Safe" badge for sandboxed apps
   - "Open Source" badge

## Implementation Considerations

### Data Model Changes

```rust
// Add to AppInfo struct
pub struct AppInfo {
    // ... existing fields ...
    pub verified: bool,              // Verified by Flathub
}

// New struct for ratings
pub struct AppRating {
    pub app_id: AppId,
    pub user_rating: Option<u8>,     // 1-5 stars
    pub user_review: Option<String>,   // User's review text
}

// Store ratings locally
pub struct RatingsStore {
    pub app_ratings: HashMap<AppId, Vec<AppRating>>,
}
```

### UI Component Changes

**Search Result Card Enhancement**:
```rust
// In SearchResult::card_view()
widget::column::with_children(vec![
    widget::text::body(&self.info.name).into(),
    widget::text::caption(&self.info.summary).into(),
    // Add quality indicators row
    widget::row::with_children(vec![
        // Verified badge
        self.info.verified.then(|| widget::icon::from_name("emblem-ok-symbolic")),
        // Download count
        widget::text::caption(format_download_count(self.info.monthly_downloads)),
    ]).into(),
])
```

### Sorting Logic Changes

```rust
// Add sort mode to search
pub enum SearchSortMode {
    Relevance,      // Current behavior
    MostDownloads,   // By monthly_downloads
    RecentlyUpdated, // By release timestamp
}
```

## Questions for Further Discussion

1. **Verified Badge Display**: Should the "Verified" badge be shown prominently or subtly?
2. **Download Count Format**: How should download counts be formatted (e.g., "1.2M" vs "1,234,567")?
3. **Quality Score Integration**: If Flathub provides quality scores, how should they influence search ranking?
4. **User Rating System**: If implementing custom ratings, should they be stored locally or synced to a cloud service?
5. **Privacy**: Should user ratings be anonymous or tied to system user?
6. **Offline Support**: Should ratings work offline (cached)?
7. **Minimum Threshold**: Should apps with few ratings be treated differently?
8. **Display Density**: How to show ratings without cluttering the UI?
9. **Moderation**: How to handle spam/abuse in reviews?

## Next Steps

1. Prototype UI mockups for rating display
2. Implement Phase 1 (visualize existing data)
3. Gather user feedback on Phase 1
4. Design custom rating system architecture
5. Plan Phase 2 based on feedback

---

**Document Version**: 3.0
**Last Updated**: 2026-01-01
**Status**: Analysis Complete - Awaiting Implementation Decision
