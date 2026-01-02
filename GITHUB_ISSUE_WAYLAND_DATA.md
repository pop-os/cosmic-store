# GitHub Issue: Question - Would Flathub Consider Adding Wayland Compatibility Metadata?

---

## Title
Question: Adding Wayland compatibility metadata to Flathub infrastructure

---

## Issue Description

### Summary
We're working on optimizations for COSMIC Store and would like to explore the possibility of adding Wayland compatibility information to Flathub's metadata infrastructure. We believe this data could be useful not only for COSMIC Store, but potentially for other app stores and desktop environments that want to help users make informed decisions about app compatibility.

**We'd love to hear your thoughts on whether this is something Flathub would be interested in supporting.**

### Background
We're developing a badge system in COSMIC Store to help users identify app compatibility with Wayland:
- ✅ **Green checkmark** - Low risk apps (native Wayland support)
- ⚠️ **Orange warning** - High risk apps (known Wayland issues)
- 🔴 **Red warning** - Critical risk apps (X11-only)

Currently, our implementation uses three detection tiers:
1. **Pre-computed data** (ideal, but currently limited)
2. Runtime metadata parsing (works for installed apps only)
3. Heuristic detection (conservative, limited coverage)

**We're wondering if Flathub would be open to providing pre-computed compatibility data that could benefit multiple app stores and desktop environments.**

### Potential Benefits Beyond COSMIC Store
This data could be useful for:
- **GNOME Software** - Help GNOME users identify compatible apps
- **KDE Discover** - Assist Plasma users on Wayland
- **Other app stores** - Any store wanting to display compatibility info
- **Desktop environments** - Future Wayland-based DEs
- **Web interfaces** - Flathub.org could display compatibility badges
- **CLI tools** - Scripts and tools for system administrators

---

## Proposed Approaches (For Discussion)

We've been exploring different approaches and would love your feedback on what might work best for Flathub's infrastructure.

### Option A: Bitcode in AppStream Metadata (Our Preferred Approach)

**Idea:** Encode Wayland compatibility as a compact bitcode directly in AppStream metadata.

This would involve adding a custom `<wayland_compat>` tag to each app's AppStream XML:

```xml
<component>
  <id>org.gnome.Epiphany</id>
  <name>Web</name>
  <!-- ... other metadata ... -->
  <custom>
    <value key="wayland_compat">0x14</value>
  </custom>
</component>
```

**Bitcode Format (8-bit):**
```
Bits 0-1: Wayland Support
  00 = Unknown
  01 = XWayland only
  10 = Native
  11 = Reserved

Bits 2-5: Framework
  0000 = Unknown
  0001 = GTK3
  0010 = GTK4
  0011 = Qt5
  0100 = Qt6
  0101 = Electron
  0110 = QtWebEngine
  0111 = SDL2
  1000-1111 = Reserved

Bits 6-7: Risk Level
  00 = Low
  01 = Medium
  10 = High
  11 = Critical
```

**Examples:**
- `0x14` = `00010100` = GTK4 + Native + Low risk
- `0x95` = `10010101` = Electron + Native + High risk
- `0x63` = `01100011` = Qt5 + Native + Medium risk

**Why we like this approach:**
- ✅ **Extremely compact** - 1 byte per app (vs. ~100 bytes JSON)
- ✅ **No new files** - Uses existing AppStream infrastructure
- ✅ **Fast parsing** - Simple bitwise operations
- ✅ **Automatic distribution** - Comes with AppStream data
- ✅ **Versioning built-in** - Can extend with more bits if needed
- ✅ **No CDN changes** - Works with existing Flathub infrastructure
- ✅ **Benefits all clients** - Any app store can use this data

**Potential concerns:**
- Requires AppStream metadata updates (though this is standard practice)
- Less human-readable (though tools can decode it)
- Need to ensure it doesn't conflict with AppStream standards

---

### Option B: Extend Existing `flathub-stats.json`

**Idea:** Add a new `wayland_compat` field to each app entry in the existing stats file.

```json
{
  "app_id": "org.gnome.Epiphany",
  "installs_total": 123456,
  "wayland_compat": {
    "support": "Native",
    "framework": "GTK4",
    "risk_level": "Low"
  }
}
```

**Advantages:**
- Single file to maintain
- Human-readable
- Easy to implement

**Trade-offs:**
- Mixes different types of data (stats vs. metadata)
- Much larger file size (~50KB+ extra for all apps)
- Requires separate download and parsing

---

### Option C: Create Dedicated `flathub-wayland-compat.json`

**Idea:** Create a separate file specifically for Wayland compatibility data.

**Advantages:**
- Cleaner separation of concerns
- Can include additional metadata (notes, version info)

**Trade-offs:**
- Requires code changes to load additional file
- Two files to maintain
- Larger bandwidth usage

---

## What We're Asking

We'd appreciate your thoughts on:

1. **Is this something Flathub would be interested in supporting?**
2. **Which approach (A, B, or C) would work best with Flathub's infrastructure?**
3. **Are there any concerns or alternative approaches we should consider?**
4. **Would this be useful for other projects beyond COSMIC Store?**

We're happy to contribute the implementation if this is something you'd like to support!

---

## Data Schema

### Fields Required

```typescript
interface WaylandCompatibility {
  support: "Native" | "XWayland" | "Unknown";
  framework: "GTK3" | "GTK4" | "Qt5" | "Qt6" | "Electron" | "QtWebEngine" | "SDL2" | "Other";
  risk_level: "Low" | "Medium" | "High" | "Critical";
  notes?: string;  // Optional: Additional context
}
```

### Risk Level Guidelines

- **Low** - Native Wayland, no known issues (GTK3/GTK4, modern Qt6)
- **Medium** - Native Wayland, minor issues possible (Qt5, SDL2)
- **High** - Known issues with Wayland (Electron, QtWebEngine)
- **Critical** - X11-only, no Wayland support

---

## Data Source & Automated Generation

### Extraction from Flatpak Metadata

Parse the `metadata` file for each Flatpak app on Flathub:

```ini
[Application]
runtime=org.gnome.Platform/x86_64/46

[Context]
sockets=wayland;x11;
```

**Detection Logic:**

1. **Wayland Support** (bits 0-1):
   - `sockets=wayland` → Native (10)
   - Only `sockets=x11` → XWayland only (01)
   - Neither → Unknown (00)

2. **Framework** (bits 2-5):
   - `runtime=org.gnome.Platform` + version ≥ 40 → GTK4 (0010)
   - `runtime=org.gnome.Platform` + version < 40 → GTK3 (0001)
   - `runtime=org.kde.Platform` + version ≥ 6 → Qt6 (0100)
   - `runtime=org.kde.Platform` + version < 6 → Qt5 (0011)
   - `base=org.electronjs.Electron2.BaseApp` → Electron (0101)
   - Check desktop file for other frameworks

3. **Risk Level** (bits 6-7):
   - GTK3/GTK4 → Low (00)
   - Qt6 → Medium (01)
   - Qt5 → Medium (01)
   - Electron → High (10)
   - X11-only → Critical (11)

### Bitcode Generation Script

```python
def generate_wayland_bitcode(app_metadata):
    # Parse metadata file
    support = detect_wayland_support(app_metadata)  # 0-3
    framework = detect_framework(app_metadata)       # 0-15
    risk = calculate_risk_level(support, framework)  # 0-3

    # Encode as 8-bit value
    bitcode = (risk << 6) | (framework << 2) | support
    return f"0x{bitcode:02x}"

# Example outputs:
# GTK4 + Native + Low = (00 << 6) | (0010 << 2) | 10 = 0x0A
# Electron + Native + High = (10 << 6) | (0101 << 2) | 10 = 0x96
```

---

## Potential Implementation Plan (If You're Interested)

If Flathub is interested in supporting this, here's how we envision it could work:

### Phase 1: Flathub Side (We Can Help!)
1. **Create extraction script** to scan all Flatpak metadata files
2. **Generate bitcodes** for each app using the 8-bit format
3. **Add to AppStream XML** (or chosen format)
4. **Regenerate AppStream data** and publish to Flathub

### Phase 2: Client Side (COSMIC Store + Others)
1. **Parse bitcode** from AppStream metadata when loading apps
2. **Decode bits** to extract support/framework/risk data
3. **Display badges** based on risk level
4. **Fall back** to runtime parsing for apps without bitcode

**We're happy to contribute the extraction script and help with implementation if this is something you'd like to pursue!**

### Bitcode Decoder (Rust)

```rust
pub fn decode_wayland_bitcode(bitcode: u8) -> WaylandCompatibility {
    let support = match bitcode & 0b00000011 {
        0b00 => WaylandSupport::Unknown,
        0b01 => WaylandSupport::XWayland,
        0b10 => WaylandSupport::Native,
        _ => WaylandSupport::Unknown,
    };

    let framework = match (bitcode >> 2) & 0b00001111 {
        0x01 => AppFramework::GTK3,
        0x02 => AppFramework::GTK4,
        0x03 => AppFramework::Qt5,
        0x04 => AppFramework::Qt6,
        0x05 => AppFramework::Electron,
        0x06 => AppFramework::QtWebEngine,
        0x07 => AppFramework::SDL2,
        _ => AppFramework::Other,
    };

    let risk_level = match (bitcode >> 6) & 0b00000011 {
        0b00 => RiskLevel::Low,
        0b01 => RiskLevel::Medium,
        0b10 => RiskLevel::High,
        0b11 => RiskLevel::Critical,
        _ => RiskLevel::Low,
    };

    WaylandCompatibility { support, framework, risk_level }
}
```

---

## Example Apps with Bitcodes

### High Priority (Popular Apps)

**Low Risk (Green Checkmark):**
- `org.gnome.Epiphany` - GTK4 + Native + Low = `0x0A` (00001010)
- `org.gnome.Nautilus` - GTK4 + Native + Low = `0x0A`
- `org.gnome.TextEditor` - GTK4 + Native + Low = `0x0A`
- `org.kde.kate` - Qt6 + Native + Medium = `0x52` (01010010)
- `org.kde.okular` - Qt6 + Native + Medium = `0x52`

**High Risk (Orange Warning):**
- `com.brave.Browser` - Electron + Native + High = `0x96` (10010110)
- `com.visualstudio.code` - Electron + Native + High = `0x96`
- `com.discordapp.Discord` - Electron + Native + High = `0x96`
- `com.slack.Slack` - Electron + Native + High = `0x96`

**Critical Risk (Red Warning):**
- Any app with only `sockets=x11` = X11-only + Critical = `0xC1` (11000001)

### Bitcode Quick Reference

```
0x0A = GTK4 + Native + Low (most GNOME apps)
0x06 = GTK3 + Native + Low (older GNOME apps)
0x52 = Qt6 + Native + Medium (modern KDE apps)
0x4E = Qt5 + Native + Medium (older KDE apps)
0x96 = Electron + Native + High (Electron apps)
0xC1 = X11-only + Critical (legacy apps)
```

---

## Specific Questions for Flathub Team

We'd love your input on these questions:

1. **Interest level:** Is this something Flathub would consider supporting?
2. **Preferred approach:** Would Option A (bitcode in AppStream) work with your infrastructure, or would Options B/C be better?
3. **AppStream compatibility:** If using Option A, is adding `<custom><value key="wayland_compat">` acceptable within AppStream standards?
4. **Update frequency:** How often would this data be regenerated? (Per-build? Daily? Weekly?)
5. **Automation:** Could this be automated in Flathub's CI/CD pipeline?
6. **Broader adoption:** Do you think other app stores (GNOME Software, KDE Discover) would benefit from this?
7. **Alternative ideas:** Are there other approaches we haven't considered that might work better?

We're flexible and happy to adapt to whatever works best for Flathub's infrastructure!

---

## Potential Benefits (If Implemented)

### For End Users
- ✅ Clear visibility into app compatibility before installation
- ✅ Reduced risk of installing apps with display issues
- ✅ Better experience on Wayland-native desktops (COSMIC, GNOME, Plasma, etc.)

### For App Developers
- ✅ Incentive to add proper Wayland support
- ✅ Visibility into which apps need Wayland improvements
- ✅ Community-driven data quality improvements

### For App Stores & Desktop Environments
- ✅ Accurate compatibility info for all apps (not just installed ones)
- ✅ **Minimal bandwidth** - 1 byte per app vs. ~100 bytes JSON (if using bitcode)
- ✅ **Fast parsing** - Simple bitwise operations
- ✅ **Automatic updates** - Comes with AppStream data
- ✅ Consistent user experience across different stores

### For Flathub
- ✅ **No new infrastructure** - Could use existing AppStream system
- ✅ **Automatic generation** - Can be fully automated in CI
- ✅ **Minimal storage** - ~2KB for 2000 apps vs. ~200KB JSON (if using bitcode)
- ✅ **Value-add** - Helps users make better decisions
- ✅ **Ecosystem benefit** - Useful for multiple projects

---

## Related Work

- **COSMIC Store PR:** [Link to wayland-compat-badges branch]
- **Flathub Stats:** https://flathub.org/stats.json
- **Flatpak Metadata Docs:** https://docs.flatpak.org/en/latest/flatpak-metadata.html

---

## Next Steps

1. **Flathub team:** Review and approve approach (Option A or B)
2. **Create extraction script:** Parse metadata from all Flathub apps
3. **Generate initial dataset:** Create first version of compatibility data
4. **Set up hosting:** Make file available via CDN
5. **COSMIC Store integration:** Update code to consume the data

---

## Our Commitment

If Flathub is interested in this:
- ✅ We can develop the extraction script
- ✅ We can help with testing and validation
- ✅ We can document the format for other app stores
- ✅ We can maintain the COSMIC Store integration

We see this as a potential benefit to the broader Linux desktop ecosystem, not just COSMIC Store.

---

## Potential Timeline (If Approved)

- **Week 1:** Discussion and approach finalization
- **Week 2-3:** Script development and data extraction
- **Week 3-4:** Data validation and integration testing
- **Week 4+:** Rollout and documentation

We're flexible on timeline and happy to work at whatever pace works for Flathub!

---

## Related Work

- **COSMIC Store PR:** https://github.com/pop-os/cosmic-store (wayland-compat-badges branch)
- **Flathub Stats:** https://flathub.org/stats.json
- **Flatpak Metadata Docs:** https://docs.flatpak.org/en/latest/flatpak-metadata.html
- **AppStream Specification:** https://www.freedesktop.org/software/appstream/docs/

---

## Final Note

We understand this is a significant request and may not align with Flathub's current priorities. We're genuinely interested in your feedback and open to alternative approaches or being told this isn't something Flathub wants to support at this time.

Thank you for considering this proposal! 🙏

---

**Suggested Labels:** `question`, `enhancement`, `discussion`, `metadata`

