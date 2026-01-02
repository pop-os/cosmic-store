# Testing Wayland Compatibility Detection

This document explains how to test the Wayland compatibility detection, filtering, and sorting features.

## Current State

The feature is implemented but requires generating compatibility data to see the full functionality.

### What Works Now (with v0-6 stats only):
- ✅ Download numbers display correctly
- ✅ Filter dropdown with 6 options (All, Excellent, Good, Caution, Limited, Unknown)
- ✅ Sort by "Best Wayland Support" option
- ⚠️ All apps show grey "?" icon (Unknown compatibility) - this is expected without v0-7 data

### What You'll See With v0-7 Stats:
- 🟢 Green checkmark - Excellent Wayland support (GTK4, native Wayland apps)
- 🔵 Blue info icon - Good Wayland support (Qt5/Qt6 with native Wayland)
- 🟠 Orange warning - Caution, may have issues (Electron, older frameworks)
- 🔴 Red warning - Limited support (X11-only apps)
- ⚪ Grey question mark - Unknown compatibility (no data available)

## How to Generate v0-7 Stats with Compatibility Data

### Prerequisites
- GitHub API access (no token needed for public repos, but rate limits apply)
- Internet connection to fetch Flathub stats and app manifests

### Steps

1. **Navigate to the stats generator:**
   ```bash
   cd flathub-stats
   ```

2. **Run the generator:**
   ```bash
   cargo run --release
   ```
   
   This will:
   - Download Flathub download statistics for September 2025
   - Fetch app manifests from GitHub to analyze Wayland compatibility
   - Detect frameworks (GTK3, GTK4, Qt5, Qt6, Electron, etc.)
   - Analyze Wayland support based on manifest data
   - Generate `res/flathub-stats.bitcode-v0-7` with both downloads and compatibility data

   **Note:** This process takes several minutes as it fetches manifests for thousands of apps.
   Rate limiting: The tool waits 100ms between GitHub requests to avoid rate limits.

3. **Clear the app cache:**
   ```bash
   rm -rf ~/.cache/cosmic-store/*
   ```

4. **Rebuild and run:**
   ```bash
   cd ..
   cargo build --release
   ./target/release/cosmic-store
   ```

5. **Test the features:**
   - Search for apps (e.g., "firefox", "gimp", "blender")
   - Look for colored compatibility badges next to app names
   - Use the filter dropdown to filter by compatibility level
   - Use the sort dropdown to sort by "Best Wayland Support"

## Testing Without Generating v0-7

If you don't want to generate the full stats file, you can still test the UI:

1. **Test filtering:**
   - Select "Unknown Compatibility" filter - should show all apps
   - Select other filters - should show no results (since all apps are "Unknown")

2. **Test sorting:**
   - Select "Best Wayland Support" - apps will be sorted, but all have same priority (Unknown)

3. **Verify download numbers:**
   - Download counts should appear below app summaries in search results
   - Hover over numbers to see "Monthly downloads" tooltip

## Compatibility Detection Logic

The tool analyzes Flatpak manifests to determine:

### Framework Detection:
- **GTK4** - Uses `org.gnome.Sdk` runtime + GTK4 modules
- **GTK3** - Uses `org.gnome.Sdk` runtime + GTK3 modules  
- **Qt6** - Uses Qt6 modules or extensions
- **Qt5** - Uses Qt5 modules or extensions
- **Electron** - Uses Electron base app
- **QtWebEngine** - Uses QtWebEngine modules

### Risk Level Assignment:
- **Low (Excellent)** - GTK4 native, modern Wayland-first apps
- **Medium (Good)** - Qt5/Qt6 with good Wayland support
- **High (Caution)** - Electron, older frameworks, may have issues
- **Critical (Limited)** - X11-only apps, no Wayland support

## Troubleshooting

### No download numbers showing:
```bash
# Clear cache and restart
rm -rf ~/.cache/cosmic-store/*
./target/release/cosmic-store
```

### Stats file not loading:
Check the logs:
```bash
RUST_LOG=cosmic_store=info ./target/release/cosmic-store 2>&1 | grep stats
```

You should see either:
- `loaded flathub statistics v0-7 in ...` (if v0-7 exists)
- `loaded flathub statistics v0-6 in ...` (fallback to downloads only)

### All apps show "Unknown":
This is expected if you haven't generated the v0-7 file yet. The v0-6 file only contains download statistics.

## File Formats

- **v0-6**: `HashMap<AppId, u64>` - Downloads only
- **v0-7**: `{ downloads: HashMap<AppId, u64>, compatibility: HashMap<AppId, WaylandCompatibility> }` - Downloads + compatibility data

