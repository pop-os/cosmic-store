# Wayland Compatibility Detection, Filtering, and Sorting

This PR adds comprehensive Wayland compatibility detection, filtering, and sorting features to COSMIC Store, helping users identify apps that work well on Wayland.

## Features

### 1. Wayland Compatibility Badges
- **Always-visible badges** on all app cards showing Wayland compatibility status
- **Color-coded indicators:**
  - 🟢 Green checkmark - Excellent Wayland support (GTK4, native Wayland)
  - 🔵 Blue info icon - Good Wayland support (Qt5/Qt6)
  - 🟠 Orange warning - Caution, may have issues (Electron, older frameworks)
  - 🔴 Red warning - Limited support (X11-only apps)
  - ⚪ Grey question mark - Unknown compatibility (no data available)
- Tooltips provide detailed information about framework and compatibility

### 2. Wayland Compatibility Filtering
- **Filter dropdown** in search header with 6 options:
  - All Apps (no filtering)
  - Excellent Wayland Support
  - Good Wayland Support
  - Caution - May Have Issues
  - Limited Wayland Support
  - Unknown Compatibility
- Filters apply to search results in real-time

### 3. Wayland Compatibility Sorting
- **"Best Wayland Support" sort mode** added to sort dropdown
- Sorts apps by compatibility: Excellent → Good → Caution → Limited → Unknown
- Helps users discover the best Wayland-compatible apps first

### 4. Bitcode-based Compatibility Data
- Compatibility data stored in `res/flathub-stats.bitcode-v0-7`
- Efficient binary format for fast loading
- Backward compatible with v0-6 (downloads only)
- Automatic fallback if v0-7 not available

### 5. Framework Detection
The `flathub-stats` tool analyzes Flatpak manifests to detect:
- GTK3, GTK4
- Qt5, Qt6
- Electron
- QtWebEngine
- Native Wayland support vs X11 fallback

## Implementation Details

### Architecture
- **Lazy loading**: Compatibility data loaded on-demand via `wayland_compat_lazy()`
- **Dual format support**: v0-7 (downloads + compatibility) with v0-6 fallback (downloads only)
- **Efficient caching**: Data cached in AppInfo, no repeated lookups
- **Non-breaking**: Works with existing v0-6 stats file, enhanced with v0-7

### Risk Level Logic
- **Low**: GTK4 native, modern Wayland-first apps
- **Medium**: Qt5/Qt6 with good Wayland support
- **High**: Electron, older frameworks, potential issues
- **Critical**: X11-only apps, no Wayland support

### Files Changed
- `src/main.rs` - UI components, filtering, sorting
- `src/app_info.rs` - Compatibility data structures, lazy loading
- `src/stats.rs` - Dual format loading (v0-6/v0-7)
- `flathub-stats/src/main.rs` - Manifest analysis, compatibility detection
- `i18n/en/cosmic_store.ftl` - Localization strings

## Current State & Deployment Strategy

### ⚠️ Important: Grey Icons Until v0-7 Data Available

**This PR currently shows grey "?" icons for all apps** because the v0-7 bitcode file with compatibility data is not yet included. This is intentional and safe:

- ✅ All functionality works (filtering, sorting, download numbers)
- ⚪ All apps show grey "?" (Unknown compatibility) until v0-7 data is generated
- ✅ Non-breaking: Falls back gracefully to v0-6 (downloads only)
- ✅ Ready for v0-7 data when available

### Deployment Options

**Option 1: Merge as-is (Recommended for initial release)**
- Merge this PR now with grey icons
- Wait for Flathub to provide v0-7 compatibility data
- Add v0-7 file in a follow-up PR when Flathub makes it available
- Users see grey icons initially, then colored badges after v0-7 is added

**Option 2: Wait for Flathub v0-7 data**
- Hold merge until Flathub provides compatibility data
- Include v0-7 file in this PR
- Users see colored badges immediately upon release

**Option 3: Revert to heuristic approach**
- If the Flathub dependency is problematic, we can revert to the earlier heuristic-based implementation
- That version analyzed app metadata in real-time (no external data file needed)
- Available in git history if needed

**Note:** The `flathub-stats` tool can generate v0-7 data by fetching manifests from GitHub, but ideally this should be provided by Flathub as an official data source to ensure consistency and reduce maintenance burden.

### Why Bitcode Approach?

The bitcode approach was chosen over heuristics because:
- More accurate (analyzes actual Flatpak manifests)
- Faster (pre-computed, no runtime analysis)
- Maintainable (centralized data source)
- Extensible (can add more metadata in future)

## Testing

See [WAYLAND_COMPAT_TESTING.md](WAYLAND_COMPAT_TESTING.md) for detailed testing instructions.

### Quick Test (current state - v0-6 only):
```bash
cargo build --release
rm -rf ~/.cache/cosmic-store/*
./target/release/cosmic-store
```

**Expected behavior:**
- ✅ Download numbers visible
- ✅ Filter and sort dropdowns functional
- ⚪ All apps show grey "?" (Unknown) - expected without v0-7 data

### Full Test (with v0-7 - when available from Flathub):
```bash
cd flathub-stats
cargo run --release  # Generates v0-7 by fetching manifests from GitHub
cd ..
cargo build --release
rm -rf ~/.cache/cosmic-store/*
./target/release/cosmic-store
```

**Expected behavior:**
- ✅ Download numbers visible
- ✅ Colored compatibility badges
- ✅ Filtering by compatibility level works
- ✅ Sorting by Wayland support works

**Note:** The `flathub-stats` tool can generate this data independently, but ideally Flathub would provide it as an official data source.

## Screenshots

### Before
- No compatibility information
- Users had to research app Wayland support manually

### After
- Clear visual indicators on every app
- Easy filtering and sorting by compatibility
- Informed decision-making for Wayland users

## Benefits

1. **User Experience**: Users can quickly identify Wayland-compatible apps
2. **Transparency**: Clear indication of app compatibility status
3. **Discovery**: Sorting helps users find the best Wayland apps
4. **Informed Choices**: Users know what to expect before installing
5. **Ecosystem Health**: Encourages developers to improve Wayland support

## Future Enhancements

- Automated v0-7 generation in CI/CD
- Community contributions to compatibility data
- More granular compatibility information (features, known issues)
- Integration with app ratings/reviews

## Related Issues

- Addresses user requests for Wayland compatibility information
- Improves app discovery for Wayland users
- Helps identify apps needing Wayland support improvements

