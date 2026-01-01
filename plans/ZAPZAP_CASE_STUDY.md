# ZapZap Case Study: Why Socket Permissions Aren't Enough

## The Problem

**User Report**: "ZapZap is not working well on Pop!_OS/COSMIC"

**Initial Assumption**: Must be an X11-only app, right?

**Reality**: Much more complex! 🤯

## Investigation Results

### ZapZap Metadata Analysis

```ini
[Application]
name=com.rtosta.zapzap
runtime=org.kde.Platform/x86_64/6.10
sdk=org.kde.Sdk/x86_64/6.10
base=app/com.riverbankcomputing.PyQt.BaseApp/x86_64/6.10

[Context]
sockets=x11;wayland;pulseaudio;fallback-x11;  ← ✅ HAS WAYLAND!
devices=all;

[Environment]
QTWEBENGINEPROCESS_PATH=/app/bin/QtWebEngineProcess  ← 🔴 RED FLAG!
QTWEBENGINE_DICTIONARIES_PATH=/app/qtwebengine_dictionaries
```

### What We Expected

Based on `sockets=x11;wayland;fallback-x11`:
- ✅ App supports Wayland
- ✅ Should work on COSMIC
- ✅ No warning needed

### What Actually Happens

**On Wayland (COSMIC, GNOME, KDE)**:
```
The Wayland connection experienced a fatal error: Protocol error
[CRASH]
```

**On X11**:
- ✅ Works perfectly

**Workaround**:
```bash
# Force X11 mode
flatpak run --env=QT_QPA_PLATFORM="xcb" com.rtosta.zapzap
```

## Root Cause

### Qt WebEngine + Wayland = 💥

**What is Qt WebEngine?**
- Chromium-based web rendering engine for Qt apps
- Used for displaying web content (WhatsApp Web in ZapZap's case)
- Essentially embedding a browser in the app

**Why does it crash?**
1. **Incomplete Wayland support** in Qt WebEngine
2. **NVIDIA driver issues** (drivers 555+, 560+ have problems)
3. **Qt version issues** (Qt 6.10 specifically has bugs)
4. **Protocol errors** between Qt WebEngine and Wayland compositor

**Affected Apps**:
- ZapZap (WhatsApp client)
- Any Qt app using QtWebEngine
- Calibre (ebook manager)
- Many other Qt-based web apps

## Implications for Detection

### ❌ Simple Detection (What We Planned)

```rust
if metadata.contains("wayland") {
    return WaylandSupport::Native;  // ✅ Works!
}
```

**Result**: FALSE POSITIVE
- We say "✅ Works on Wayland"
- User installs
- App crashes
- User frustrated 😡

### ✅ Enhanced Detection (What We Need)

```rust
let has_wayland = metadata.contains("wayland");
let has_qtwebengine = metadata.contains("QTWEBENGINEPROCESS_PATH");
let runtime = parse_runtime(metadata);

if !has_wayland {
    return RiskLevel::Critical;  // Won't work at all
} else if has_qtwebengine {
    return RiskLevel::High;  // Claims Wayland but likely to crash
} else if runtime.contains("Qt/6.10") {
    return RiskLevel::Medium;  // Qt 6.10 has issues
} else {
    return RiskLevel::Low;  // Should work
}
```

## Detection Strategy

### Layer 1: Socket Permissions (Basic)
- ❌ X11-only → Critical risk
- ⚠️ Fallback → Medium risk
- ✅ Wayland → **Need more checks!**

### Layer 2: Framework Detection (Advanced)
- 🔴 Qt WebEngine → **High risk** (even with Wayland socket!)
- 🔴 Electron → **High risk** (Chromium-based)
- ⚠️ Qt 6.10 → **Medium risk** (known bugs)
- ⚠️ Qt 5.x → **Medium risk** (varies)
- ✅ GTK4 → **Low risk**
- ✅ GTK3 → **Low risk**

### Layer 3: Runtime Version
- Newer runtimes (24.08+) → Better support
- Older runtimes (20.08, 21.08) → More issues

### Layer 4: Known Issues Database
- Community-reported problems
- GitHub issue tracking
- User feedback

## UI Recommendations

### For ZapZap Specifically

**Search Results**:
```
┌─────────────────────────────────────┐
│ [ICON]  ZapZap          🔴 Issues  │
│          WhatsApp...     45K        │
└─────────────────────────────────────┘
```

**Details Page**:
```
🔴 Compatibility Warning: Known Issues on Wayland

This app uses Qt WebEngine which has known compatibility issues
on Wayland-based desktops like COSMIC.

Known problems:
• App may crash on startup
• Window management issues
• Specific issues with NVIDIA drivers 555+

Workarounds:
• Use X11 session instead of Wayland
• Or force X11 mode: flatpak run --env=QT_QPA_PLATFORM="xcb" com.rtosta.zapzap

Status: Developers are aware (GitHub issue #215)
```

### General Framework Warnings

**Qt WebEngine Apps**:
```
⚠️ This app uses Qt WebEngine which may have issues on Wayland.
Consider checking the app's issue tracker before installing.
```

**Electron Apps**:
```
⚠️ This Electron app may have compatibility issues on Wayland.
Most Electron apps work better on X11.
```

## Lessons Learned

1. **Socket permissions are necessary but NOT sufficient**
2. **Framework matters more than socket permissions**
3. **Qt WebEngine is a major red flag for Wayland**
4. **Need multi-layered detection approach**
5. **Community feedback is essential**

## Next Steps

1. ✅ Update detection algorithm to check for Qt WebEngine
2. ✅ Add framework detection (Qt, GTK, Electron)
3. ✅ Create risk levels (Low, Medium, High, Critical)
4. ⏭️ Build known issues database
5. ⏭️ Add user feedback mechanism
6. ⏭️ Test with other problematic apps

---

**Key Takeaway**: A simple "has Wayland socket" check would have given us a **false positive** for ZapZap. We need sophisticated framework detection to avoid misleading users.

