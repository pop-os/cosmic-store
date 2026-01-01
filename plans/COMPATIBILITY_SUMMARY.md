# App Compatibility Detection - Executive Summary

## 🎯 The Problem

**Users on Pop!_OS/COSMIC are installing apps that don't work properly.**

COSMIC is a Wayland-native desktop environment with **no X11 support**. Many Flatpak apps still require X11, leading to:
- Apps that won't start
- Broken window management (minimize/restore issues)
- Non-functional file pickers
- Screen sharing failures
- Random crashes

**User Experience**: Install popular app → It doesn't work → Frustration → Loss of trust in the store

## ✅ The Solution

**We CAN detect incompatible apps before installation!**

### How It Works

Every Flatpak app has a metadata file that declares its requirements:

```ini
[Context]
sockets=x11;wayland;pulseaudio;
```

**Detection Logic**:
- ✅ **Wayland Native**: Has `wayland` socket → Works perfectly
- ⚠️ **May Have Issues**: Has `fallback-x11` → Mostly works, minor issues
- ❌ **X11 Only**: Has `x11` but NOT `wayland` → **Will NOT work on COSMIC**

### Proof of Concept

I created a test script that scans your installed apps:

**Results from your system**:
- ✅ **7 apps** are Wayland-native (Chrome, Brave, Notes, etc.)
- ❌ **1 app** is X11-only: **RustDesk** ← This would have issues on COSMIC!

## 📋 Implementation Plan

### Phase 1: Data Collection (1-2 days)
1. Extend `flathub-stats` utility to fetch app manifests from Flathub
2. Parse `finish-args` section to detect socket permissions
3. Store compatibility data in bitcode alongside download stats

### Phase 2: Data Model (1 day)
```rust
pub struct AppInfo {
    // ... existing fields ...
    pub wayland_support: WaylandSupport,
}

pub enum WaylandSupport {
    Native,      // ✅ Works on Wayland
    Fallback,    // ⚠️ May have issues
    X11Only,     // ❌ Won't work on COSMIC
    Unknown,     // No data available
}
```

### Phase 3: UI Warnings (2-3 days)

**Search Results**:
```
┌─────────────────────────────────────┐
│ [ICON]  RustDesk        ⚠️ X11     │
│          Remote desktop  45K        │
└─────────────────────────────────────┘
```

**Details Page**:
```
⚠️ Compatibility Warning
This app requires X11 and will NOT work on COSMIC desktop (Wayland).

Known issues:
• App may not start
• Window management problems
• File pickers won't work

Consider using a Wayland-compatible alternative.
```

**Badges**:
- ❌ Red warning badge for X11-only apps
- ⚠️ Yellow info badge for fallback apps
- ✅ Optional green badge for Wayland-native (don't clutter)

### Phase 4: Smart Filtering (1 day)
- Add "Show only Wayland-compatible apps" filter
- Sort by compatibility score
- Optionally hide X11-only apps by default on COSMIC

## 🔍 Technical Details

### Where to Get the Data

**Option 1: Flathub GitHub** (Recommended)
```
https://raw.githubusercontent.com/flathub/{APP_ID}/master/{APP_ID}.json
```
- Contains full manifest with `finish-args`
- Can be fetched during stats update
- Cached locally

**Option 2: Local Metadata** (For installed apps only)
```
~/.local/share/flatpak/app/{APP_ID}/current/active/metadata
```
- Only available after installation
- Not useful for pre-install warnings

### Parsing Example

```rust
fn parse_wayland_support(manifest: &str) -> WaylandSupport {
    let sockets = extract_sockets(manifest);
    
    match (sockets.contains("wayland"), sockets.contains("x11")) {
        (true, _) => WaylandSupport::Native,
        (false, true) if sockets.contains("fallback-x11") => WaylandSupport::Fallback,
        (false, true) => WaylandSupport::X11Only,
        _ => WaylandSupport::Unknown,
    }
}
```

## 📊 Expected Impact

### User Benefits
- **Avoid frustration**: Know before installing if app will work
- **Save time**: Don't waste time troubleshooting broken apps
- **Better choices**: Discover Wayland-compatible alternatives
- **Trust**: Store shows it cares about compatibility

### Metrics to Track
- % of apps with compatibility data
- % of X11-only apps in catalog
- User installation patterns (do warnings reduce X11-only installs?)
- User feedback on accuracy

## 🚀 Quick Wins

### Immediate (No Code Changes)
1. ✅ Run `examples/check_wayland_compat.sh` to see your app compatibility
2. Document known problematic apps

### Short Term (1 week)
1. Add `WaylandSupport` field to `AppInfo`
2. Update `flathub-stats` to fetch manifests
3. Display warning badges in search results

### Medium Term (2-3 weeks)
1. Add detailed compatibility warnings in app details
2. Add filtering/sorting by compatibility
3. Localize all warning messages

## ❓ Open Questions

1. **Should we block installation?** Or just warn and let users decide?
   - **Recommendation**: Warn but allow installation (user choice)

2. **How prominent should warnings be?**
   - **Recommendation**: Clear but not scary - use info/warning colors

3. **Should we auto-hide X11-only apps on COSMIC?**
   - **Recommendation**: Add as optional filter, not default

4. **What about false positives?**
   - **Recommendation**: Allow user feedback to report working apps

5. **Desktop environment detection?**
   - **Recommendation**: Check `$XDG_SESSION_TYPE` and `$XDG_CURRENT_DESKTOP`

## 📝 Next Steps

1. Review this plan with team
2. Decide on warning UX (severity, placement, wording)
3. Start with Phase 1: Update flathub-stats utility
4. Create UI mockups for warning badges
5. Implement and test with known X11-only apps

---

**See Also**:
- [Full Technical Analysis](./app-compatibility-detection.md)
- [Test Script](../examples/check_wayland_compat.sh)
- [Rating System Analysis](./rating-system-analysis.md)

**Status**: ✅ Ready for Implementation  
**Estimated Effort**: 1-2 weeks  
**Priority**: High (affects user trust and satisfaction)

