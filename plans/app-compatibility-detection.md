# App Compatibility Detection for Pop!_OS/COSMIC

## Problem Statement

Users on Pop!_OS with COSMIC desktop (Wayland-based) are installing apps from the store that don't work properly:
- Apps that require X11 may have GUI issues
- Apps may not minimize/restore correctly
- File picker dialogs may not work properly
- Some apps crash or freeze on Wayland

**User Impact**: Users install popular apps only to find they don't work, leading to frustration and loss of trust in the app store.

## Root Causes

### 1. **Wayland vs X11 Compatibility** ⚠️ (Simple to detect)
- COSMIC is Wayland-native (no X11 support)
- Many apps still require X11 or work better with X11
- Apps using `--socket=x11` without `--socket=wayland` will fail
- **Detection**: Check `sockets=` in Flatpak metadata

### 2. **Qt WebEngine + Wayland Issues** 🔴 (HARD to detect)
**Real-world example: ZapZap**
- Has `sockets=x11;wayland;fallback-x11` ✅ (should work!)
- Uses Qt WebEngine (Chromium-based)
- **Crashes on Wayland** with certain NVIDIA drivers
- Works fine on X11 or with older drivers
- **Issue**: Qt WebEngine Wayland support is buggy/incomplete

**Other affected apps**:
- Any Qt app using QtWebEngine
- Electron apps (Chromium-based)
- Apps using specific Qt versions (6.10 in ZapZap's case)

**Workaround**: Force X11 mode via `QT_QPA_PLATFORM=xcb`

### 3. **NVIDIA Driver Compatibility** 🔴 (VERY HARD to detect)
- Newer NVIDIA drivers (555+, 560+) have Wayland issues
- Older drivers (535, 550) work better
- Affects Qt WebEngine, some Vulkan apps
- **Cannot detect from metadata alone**

### 4. **Portal Implementation Differences** ⚠️ (Medium difficulty)
- File pickers, screen sharing, notifications use XDG Desktop Portals
- Some apps don't use portals correctly
- COSMIC's portal implementation may differ from GNOME/KDE
- **Detection**: Check for portal usage in manifest

### 5. **GTK/Qt Version Mismatches** ⚠️ (Medium difficulty)
- Older GTK3 apps may have issues
- Qt apps may need specific Wayland platform plugins
- Some apps hardcode X11-specific features
- **Detection**: Check runtime version

### 6. **Missing Runtime Features** ⚠️ (Medium difficulty)
- Apps may depend on specific D-Bus services
- Some apps need specific environment variables
- Compositor-specific features (e.g., GNOME Shell extensions)
- **Detection**: Check D-Bus policy in metadata

## 🚨 CRITICAL DISCOVERY: Socket Permissions Are Not Enough!

### Real-World Case Study: ZapZap

**App**: ZapZap (WhatsApp client)
**Flatpak ID**: `com.rtosta.zapzap`
**Metadata**: `sockets=x11;wayland;pulseaudio;fallback-x11;` ✅
**Expected**: Should work on Wayland
**Reality**: **CRASHES on Wayland** 💥

**Why?**
- Uses **Qt WebEngine** (Chromium-based rendering)
- Qt WebEngine has incomplete/buggy Wayland support
- Specific issue with NVIDIA drivers 555+, 560+
- Works fine with older NVIDIA drivers (535, 550)
- Works fine when forced to X11 mode

**Workaround**:
```bash
flatpak run --env=QT_QPA_PLATFORM="xcb" com.rtosta.zapzap
```

### Implications for Detection

**Simple socket-based detection will give FALSE POSITIVES**:
- App claims Wayland support → We say "✅ Works"
- User installs → App crashes → User frustrated

**We need multi-layered detection**:
1. ✅ **Socket permissions** (basic check)
2. ⚠️ **Framework detection** (Qt WebEngine, Electron, etc.)
3. ⚠️ **Runtime version** (older runtimes = more issues)
4. ⚠️ **Known problematic apps** (community-reported issues)

## Detection Strategy

### Available Data Sources

#### 1. **Flatpak Metadata** (✅ Best Option)
Location: `~/.local/share/flatpak/app/{APP_ID}/current/active/metadata`

**Key Fields**:
```ini
[Context]
sockets=x11;wayland;pulseaudio;  # What display servers the app supports
shared=network;ipc;              # Shared resources
devices=all;                     # Hardware access
features=bluetooth;devel;        # Special features
```

**Detection Rules**:
- ❌ **X11-only apps**: `sockets=x11` WITHOUT `wayland` → **Will NOT work**
- ⚠️ **Fallback apps**: `sockets=fallback-x11;wayland` → **May have issues**
- ⚠️ **Qt WebEngine apps**: Uses Qt + WebEngine → **May crash on Wayland**
- ⚠️ **Electron apps**: Chromium-based → **May have issues**
- ✅ **Wayland-native**: `sockets=wayland` + GTK4/native toolkit → **Should work**

**IMPORTANT DISCOVERY**: Socket permissions alone are NOT enough!
- ZapZap has `wayland` socket but still crashes
- Need to detect Qt WebEngine, Electron, and other problematic frameworks

#### 2. **AppStream Metadata** (Limited)
- Does NOT contain socket/permission information
- Only has categories, description, screenshots
- Cannot detect Wayland compatibility from AppStream alone

#### 3. **Runtime Requirements** (Useful!)
```ini
runtime=org.freedesktop.Platform/x86_64/25.08
base=app/org.chromium.Chromium.BaseApp/x86_64/25.08
```
- Newer runtimes (24.08+) have better Wayland support
- Older runtimes (20.08, 21.08) more likely to have issues
- **Base app reveals framework**: Chromium.BaseApp = Electron/WebEngine

#### 4. **Environment Variables** (Framework detection!)
```ini
[Environment]
QTWEBENGINEPROCESS_PATH=/app/bin/QtWebEngineProcess
```
- Presence of `QTWEBENGINEPROCESS_PATH` = Qt WebEngine app
- Presence of `ELECTRON_*` variables = Electron app
- These apps are **high risk** for Wayland issues

#### 5. **Command and SDK** (Additional clues)
```ini
runtime=org.kde.Platform/x86_64/6.10
sdk=org.kde.Sdk/x86_64/6.10
base=app/com.riverbankcomputing.PyQt.BaseApp/x86_64/6.10
```
- KDE Platform = Qt-based app
- PyQt.BaseApp = Python + Qt
- Qt 6.10 specifically has known Wayland issues

### Implementation Approach

#### Phase 1: Read Flatpak Metadata (Quick Win)

**Add to AppInfo struct**:
```rust
pub struct AppInfo {
    // ... existing fields ...
    pub wayland_compatibility: WaylandCompatibility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub struct WaylandCompatibility {
    pub socket_support: SocketSupport,
    pub risk_level: RiskLevel,
    pub framework: AppFramework,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum SocketSupport {
    WaylandNative,  // Has wayland socket
    Fallback,       // Has fallback-x11
    X11Only,        // Only has x11 socket
    Unknown,        // No metadata available
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum RiskLevel {
    Low,      // ✅ Should work fine
    Medium,   // ⚠️ May have minor issues
    High,     // 🔴 Likely to have problems
    Critical, // ❌ Will NOT work
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum AppFramework {
    Native,        // GTK4, native Wayland toolkit
    GTK3,          // GTK3 (mostly works)
    Qt5,           // Qt5 (varies)
    Qt6,           // Qt6 (varies)
    QtWebEngine,   // 🔴 Qt + WebEngine (HIGH RISK)
    Electron,      // 🔴 Electron (HIGH RISK)
    Unknown,
}
```

**Parse metadata file** (Enhanced with framework detection):
```rust
fn parse_flatpak_metadata(app_id: &str) -> Result<WaylandCompatibility, Error> {
    let metadata_path = format!(
        "{}/.local/share/flatpak/app/{}/current/active/metadata",
        env::var("HOME")?,
        app_id
    );

    let content = fs::read_to_string(metadata_path)?;

    // Parse socket support
    let mut wayland = false;
    let mut x11 = false;
    let mut fallback_x11 = false;

    // Detect framework
    let mut has_qtwebengine = false;
    let mut has_electron = false;
    let mut runtime = String::new();
    let mut base = String::new();

    for line in content.lines() {
        if line.starts_with("sockets=") {
            let sockets = line.strip_prefix("sockets=").unwrap();
            wayland = sockets.contains("wayland");
            x11 = sockets.contains("x11") && !sockets.contains("fallback-x11");
            fallback_x11 = sockets.contains("fallback-x11");
        }

        // Detect Qt WebEngine
        if line.contains("QTWEBENGINEPROCESS_PATH") {
            has_qtwebengine = true;
        }

        // Detect Electron
        if line.contains("ELECTRON_") || line.contains("Chromium.BaseApp") {
            has_electron = true;
        }

        // Get runtime info
        if line.starts_with("runtime=") {
            runtime = line.strip_prefix("runtime=").unwrap().to_string();
        }

        if line.starts_with("base=") {
            base = line.strip_prefix("base=").unwrap().to_string();
        }
    }

    // Determine socket support
    let socket_support = match (wayland, x11, fallback_x11) {
        (true, _, _) => SocketSupport::WaylandNative,
        (false, _, true) => SocketSupport::Fallback,
        (false, true, false) => SocketSupport::X11Only,
        _ => SocketSupport::Unknown,
    };

    // Determine framework
    let framework = if has_qtwebengine {
        AppFramework::QtWebEngine
    } else if has_electron {
        AppFramework::Electron
    } else if runtime.contains("org.kde.Platform") {
        if runtime.contains("/6.") {
            AppFramework::Qt6
        } else {
            AppFramework::Qt5
        }
    } else if runtime.contains("org.gnome.Platform") {
        AppFramework::GTK3  // Could be GTK4, need more detection
    } else {
        AppFramework::Unknown
    };

    // Calculate risk level
    let risk_level = match (&socket_support, &framework) {
        // X11-only = critical
        (SocketSupport::X11Only, _) => RiskLevel::Critical,

        // Qt WebEngine or Electron with Wayland = HIGH RISK
        (SocketSupport::WaylandNative, AppFramework::QtWebEngine) => RiskLevel::High,
        (SocketSupport::WaylandNative, AppFramework::Electron) => RiskLevel::High,
        (SocketSupport::Fallback, AppFramework::QtWebEngine) => RiskLevel::High,
        (SocketSupport::Fallback, AppFramework::Electron) => RiskLevel::High,

        // Qt6 with Wayland = medium risk (version-dependent)
        (SocketSupport::WaylandNative, AppFramework::Qt6) => RiskLevel::Medium,

        // Fallback = medium risk
        (SocketSupport::Fallback, _) => RiskLevel::Medium,

        // Wayland native with safe framework = low risk
        (SocketSupport::WaylandNative, AppFramework::Native) => RiskLevel::Low,
        (SocketSupport::WaylandNative, AppFramework::GTK3) => RiskLevel::Low,

        // Unknown
        _ => RiskLevel::Medium,
    };

    Ok(WaylandCompatibility {
        socket_support,
        risk_level,
        framework,
    })
}
```

#### Phase 2: Display Warnings in UI

**Search Results**:
```
┌─────────────────────────────────────┐
│ [ICON]  App Name        ⚠️ X11    │
│          App summary...  1.2M      │
└─────────────────────────────────────┘
```

**Details Page**:
```
⚠️ Compatibility Warning
This app requires X11 and may not work properly on COSMIC (Wayland).
Known issues: Window management, file pickers, screen sharing
```

**Badge System**:
- ⚠️ **X11 Only** - Red/orange warning badge
- ℹ️ **May have issues** - Yellow info badge (for fallback-x11)
- ✅ **Wayland Native** - Green checkmark (optional, don't clutter)

#### Phase 3: Smart Filtering

**Add filter options**:
- "Show only Wayland-compatible apps"
- "Hide X11-only apps"
- Sort by compatibility score

**Compatibility Score**:
```rust
fn compatibility_score(info: &AppInfo) -> i32 {
    match info.wayland_support {
        WaylandSupport::Native => 100,
        WaylandSupport::Fallback => 50,
        WaylandSupport::X11Only => 0,
        WaylandSupport::Unknown => 25,
    }
}
```

## Technical Challenges

### 1. **Metadata Not Available Before Install**
- Flatpak metadata only exists for installed apps
- Need to fetch from Flathub API or parse manifest

**Solution**: Cache metadata from Flathub's manifest repository
```
https://github.com/flathub/{APP_ID}/blob/master/{APP_ID}.json
```

### 2. **System vs User Installations**
- Metadata in different locations
- Need to check both `/var/lib/flatpak` and `~/.local/share/flatpak`

### 3. **Performance**
- Reading metadata files for every app is slow
- Need to cache parsed metadata

**Solution**: Parse during AppStream cache build, store in bitcode

## Recommended Implementation Plan

### Step 1: Fetch Metadata from Flathub API ✅ (Easiest)

Flathub provides app manifests via their API:
```
https://flathub.org/api/v2/appstream/{APP_ID}
```

This returns full AppStream data including runtime info. However, it doesn't include finish-args.

**Better approach**: Parse from Flathub's GitHub repo during stats update:
```bash
# In flathub-stats utility
curl https://raw.githubusercontent.com/flathub/{APP_ID}/master/{APP_ID}.json
```

### Step 2: Add Fields to AppInfo

```rust
// In src/app_info.rs
pub struct AppInfo {
    // ... existing fields ...
    pub wayland_support: WaylandSupport,
    pub runtime_version: Option<String>,  // e.g., "25.08"
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum WaylandSupport {
    Native,      // Supports Wayland
    Fallback,    // Has fallback-x11
    X11Only,     // Only X11
    Unknown,     // No data
}
```

### Step 3: Update Flathub Stats Utility

Modify `flathub-stats/src/main.rs` to:
1. Fetch app manifests from Flathub GitHub
2. Parse `finish-args` section
3. Detect socket permissions
4. Store in bitcode alongside download stats

### Step 4: Display Warnings in UI

**In search results** (`src/main.rs`):
```rust
// Add warning badge for X11-only apps
if self.info.wayland_support == WaylandSupport::X11Only {
    widget::tooltip(
        widget::icon::icon(icon_cache_handle("dialog-warning-symbolic", 16))
            .size(16),
        widget::text::caption(&fl!("x11-only-warning"))
    ).into()
}
```

**In details page**:
```rust
// Add compatibility section
if info.wayland_support == WaylandSupport::X11Only {
    widget::container(
        widget::column::with_children(vec![
            widget::icon::icon(icon_cache_handle("dialog-warning", 48))
                .size(48)
                .into(),
            widget::text::heading(&fl!("compatibility-warning")).into(),
            widget::text::body(&fl!("x11-only-description")).into(),
        ])
    )
    .class(theme::Container::Warning)
    .into()
}
```

### Step 5: Add Localization Strings

```fluent
# i18n/en/cosmic_store.ftl
x11-only-warning = X11 Only
x11-only-tooltip = This app requires X11 and may not work on Wayland
compatibility-warning = Compatibility Warning
x11-only-description = This app requires X11 display server and may not work properly on COSMIC desktop (Wayland). You may experience issues with window management, file pickers, or the app may not start at all.
wayland-native = Wayland Native
wayland-native-tooltip = This app is optimized for Wayland
fallback-x11-warning = May have issues
fallback-x11-tooltip = This app prefers Wayland but can fall back to X11
```

## Testing Strategy

### Test Cases

1. **X11-only app** (e.g., older Electron apps)
   - Should show warning badge
   - Should display compatibility warning in details
   - Should still allow installation (user choice)

2. **Wayland-native app** (e.g., GNOME apps, modern GTK4 apps)
   - Should show no warning
   - Optional: Show "Wayland Native" badge

3. **Fallback app** (e.g., Chrome, Firefox)
   - Should show info badge
   - Should mention "may have minor issues"

### Manual Testing

```bash
# Check metadata for installed apps
flatpak list --app --columns=application | while read app; do
    echo "=== $app ==="
    flatpak info --show-metadata "$app" | grep "sockets="
done
```

## Known X11-Only Apps (Examples)

Based on research, these apps are known to have issues on Wayland:
- Older Electron apps (pre-v28)
- Some Java/Swing applications
- Legacy Qt4 applications
- Apps using X11-specific features (e.g., global hotkeys)

## Future Enhancements

### Phase 4: Community Feedback

Allow users to report compatibility issues:
```rust
pub struct CompatibilityReport {
    pub app_id: AppId,
    pub works_on_wayland: bool,
    pub issues: Vec<String>,
    pub desktop_environment: String,
}
```

### Phase 5: Automatic Detection

For installed apps, detect actual runtime behavior:
- Check if app uses Wayland or XWayland
- Monitor crash reports
- Track user uninstalls after short usage

### Phase 6: Alternative Suggestions

When showing X11-only app, suggest Wayland alternatives:
```
⚠️ This app requires X11. Consider these Wayland-native alternatives:
  • Alternative App 1
  • Alternative App 2
```

## Questions for Discussion

1. **Warning Severity**: Should we block installation of X11-only apps or just warn?
2. **Badge Visibility**: Show warning badge in search results or only in details?
3. **Default Filtering**: Should "Hide X11-only apps" be default for COSMIC users?
4. **Desktop Detection**: How to detect if user is on COSMIC vs GNOME vs KDE?
5. **False Positives**: How to handle apps that claim X11 but work fine on Wayland?

## Next Steps

1. ✅ Research completed - understand the problem
2. ⏭️ Prototype metadata parsing from Flathub
3. ⏭️ Add WaylandSupport enum to AppInfo
4. ⏭️ Update flathub-stats utility
5. ⏭️ Implement UI warnings
6. ⏭️ Test with known problematic apps
7. ⏭️ Gather user feedback

---

**Document Version**: 1.0
**Last Updated**: 2026-01-01
**Status**: Analysis Complete - Ready for Implementation

