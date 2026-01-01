# Wayland Compatibility Detection - Implementation Analysis

## Codebase Structure Overview

### Key Files

1. **`src/app_info.rs`** - Core data structures
   - `AppInfo` struct (lines 147-170) - Main app metadata
   - Uses `bitcode::Decode` and `bitcode::Encode` for serialization
   - Currently has: name, summary, categories, downloads, verified flag, etc.
   - **Need to add**: Wayland compatibility fields

2. **`src/backend/flatpak.rs`** - Flatpak backend
   - Handles Flatpak installation and metadata
   - Uses `libflatpak` crate for Flatpak API
   - Parses AppStream XML from remotes
   - **Need to add**: Flatpak metadata file parsing

3. **`src/appstream_cache.rs`** - AppStream cache management
   - Caches app metadata from AppStream XML
   - Creates `AppInfo` objects from AppStream components
   - **Need to add**: Wayland compatibility enrichment

4. **`src/main.rs`** - UI and application logic
   - Search results display (lines 706-773)
   - Detail page display (lines 2342-2646)
   - Already has badges: "Editor's Choice" ⭐, "Verified" ✓
   - **Need to add**: Wayland compatibility warnings

### Current Data Flow

```
Flatpak Remote
    ↓
AppStream XML (appstream.xml.gz)
    ↓
AppstreamCache::reload() → parses XML
    ↓
AppInfo::new() → creates AppInfo from Component
    ↓
Stored in AppstreamCache.infos HashMap
    ↓
Used in UI (search results, details page)
```

## Implementation Strategy

### Phase 1: Add Data Structures

**File**: `src/app_info.rs`

Add new enums and struct to `AppInfo`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum WaylandSupport {
    Native,      // Has wayland socket
    Fallback,    // Has fallback-x11
    X11Only,     // Only has x11 socket
    Unknown,     // No metadata available
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum AppFramework {
    Native,        // GTK4, native Wayland
    GTK3,          // GTK3
    Qt5,           // Qt5
    Qt6,           // Qt6
    QtWebEngine,   // Qt + WebEngine (HIGH RISK)
    Electron,      // Electron (HIGH RISK)
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub struct WaylandCompatibility {
    pub support: WaylandSupport,
    pub framework: AppFramework,
}

// Add to AppInfo struct:
pub struct AppInfo {
    // ... existing fields ...
    pub wayland_compat: Option<WaylandCompatibility>,
}
```

### Phase 2: Parse Flatpak Metadata

**File**: `src/backend/flatpak.rs`

Add function to parse Flatpak metadata files:

```rust
fn parse_flatpak_metadata(app_id: &str) -> Option<WaylandCompatibility> {
    // Path: ~/.local/share/flatpak/app/{app_id}/current/active/metadata
    // OR: /var/lib/flatpak/app/{app_id}/current/active/metadata
    
    // Parse [Context] section for sockets
    // Parse [Environment] section for QTWEBENGINEPROCESS_PATH, ELECTRON_*
    // Parse [Application] section for runtime, base
    
    // Return WaylandCompatibility
}
```

### Phase 3: Enrich AppInfo

**File**: `src/appstream_cache.rs`

In `AppInfo::new()` or after creation, call `parse_flatpak_metadata()`:

```rust
let wayland_compat = if !info.flatpak_refs.is_empty() {
    parse_flatpak_metadata(&id)
} else {
    None
};
```

### Phase 4: Display in UI

**File**: `src/main.rs`

#### Search Results (lines 706-773)

Add warning badge next to "Editor's Choice" and "Verified":

```rust
// After line 763
if let Some(compat) = &self.info.wayland_compat {
    if matches!(compat.support, WaylandSupport::X11Only) {
        widget::tooltip(
            widget::icon::icon(icon_cache_handle("dialog-warning-symbolic", 16))
                .size(16),
            widget::text(fl!("x11-only-warning")),
            widget::tooltip::Position::Bottom,
        )
        .into()
    }
}
```

#### Details Page (lines 2342-2646)

Add warning banner at top of details:

```rust
if let Some(compat) = &selected.info.wayland_compat {
    match compat.support {
        WaylandSupport::X11Only => {
            column = column.push(
                widget::container(
                    widget::text(fl!("x11-only-details"))
                )
                .class(theme::Container::Warning)
            );
        }
        WaylandSupport::Fallback if matches!(compat.framework, AppFramework::QtWebEngine | AppFramework::Electron) => {
            column = column.push(
                widget::container(
                    widget::text(fl!("wayland-issues-warning"))
                )
                .class(theme::Container::Warning)
            );
        }
        _ => {}
    }
}
```

## Challenges & Considerations

### 1. **Flatpak Metadata Location**
- User install: `~/.local/share/flatpak/app/{app_id}/current/active/metadata`
- System install: `/var/lib/flatpak/app/{app_id}/current/active/metadata`
- Need to check both locations
- Metadata only exists for **installed** apps!

### 2. **Non-Installed Apps**
- AppStream XML doesn't contain socket permissions
- Can't detect Wayland compatibility for non-installed apps
- **Solution**: Only show warnings for installed apps, or use heuristics

### 3. **Cache Invalidation**
- `AppstreamCache` is serialized with `bitcode`
- Adding new fields requires cache version bump
- See `cache_filename()` in `appstream_cache.rs` (line 198)
- Current version: `appstream_cache-v2.bitcode-v0-6`
- **Need**: `appstream_cache-v3.bitcode-v0-6`

### 4. **Performance**
- Parsing metadata files for every app could be slow
- **Solution**: Parse lazily or cache results

### 5. **Localization**
- Need to add strings to `i18n/en/cosmic_store.ftl`
- Examples:
  - `x11-only-warning = X11 only`
  - `x11-only-details = This app only works on X11...`
  - `wayland-issues-warning = May have issues on Wayland`

## Next Steps

1. ✅ Understand codebase structure
2. ⏭️ Add data structures to `app_info.rs`
3. ⏭️ Implement metadata parsing in `backend/flatpak.rs`
4. ⏭️ Integrate parsing into `appstream_cache.rs`
5. ⏭️ Add UI warnings in `main.rs`
6. ⏭️ Add localization strings
7. ⏭️ Test with real apps (ZapZap, etc.)
8. ⏭️ Handle cache versioning

