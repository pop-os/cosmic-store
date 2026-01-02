# Next Steps: Bitcode Implementation for Wayland Compatibility

**Status:** Ready for implementation with Claude Code  
**Branch:** `feature/wayland-compatibility-detection`  
**Date:** 2026-01-02

---

## 🎯 Goal

Prepare COSMIC Store code to work with Wayland compatibility data encoded as bitcodes in AppStream metadata, so we can create a PR and then post the Flathub issue.

---

## ✅ Completed So Far

1. ✅ Implemented basic Wayland compatibility badge system
2. ✅ Created three-tier detection (pre-computed, metadata parsing, heuristics)
3. ✅ Verified badges working (green for GNOME Notes, orange for Chrome/Brave)
4. ✅ Created comprehensive GitHub issue template for Flathub
5. ✅ Designed 8-bit bitcode format for encoding compatibility data

---

## 🚀 TODO: Implementation Tasks

### Task 1: Create Mock AppStream Data with Bitcodes

**File:** `res/mock-appstream-wayland.xml` (or similar)

Create sample AppStream XML with bitcode data for testing:

```xml
<components>
  <component>
    <id>org.gnome.Epiphany</id>
    <name>Web</name>
    <custom>
      <value key="wayland_compat">0x0A</value>
    </custom>
  </component>
  <component>
    <id>com.brave.Browser</id>
    <name>Brave</name>
    <custom>
      <value key="wayland_compat">0x96</value>
    </custom>
  </component>
  <!-- Add more examples -->
</components>
```

**Include these example apps:**
- `org.gnome.Epiphany` - `0x0A` (GTK4 + Native + Low)
- `org.gnome.Nautilus` - `0x0A` (GTK4 + Native + Low)
- `org.gnome.TextEditor` - `0x0A` (GTK4 + Native + Low)
- `org.kde.kate` - `0x52` (Qt6 + Native + Medium)
- `org.kde.okular` - `0x52` (Qt6 + Native + Medium)
- `com.brave.Browser` - `0x96` (Electron + Native + High)
- `com.visualstudio.code` - `0x96` (Electron + Native + High)
- `com.discordapp.Discord` - `0x96` (Electron + Native + High)

---

### Task 2: Implement Bitcode Decoder

**File:** `src/app_info.rs` (or new `src/wayland_bitcode.rs`)

Add bitcode decoding function:

```rust
/// Decode 8-bit Wayland compatibility bitcode
/// 
/// Format:
/// - Bits 0-1: Wayland Support (00=Unknown, 01=XWayland, 10=Native)
/// - Bits 2-5: Framework (0001=GTK3, 0010=GTK4, 0011=Qt5, 0100=Qt6, 0101=Electron, etc.)
/// - Bits 6-7: Risk Level (00=Low, 01=Medium, 10=High, 11=Critical)
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_decode_gtk4_native_low() {
        let compat = decode_wayland_bitcode(0x0A);
        assert_eq!(compat.framework, AppFramework::GTK4);
        assert_eq!(compat.support, WaylandSupport::Native);
        assert_eq!(compat.risk_level, RiskLevel::Low);
    }
    
    #[test]
    fn test_decode_electron_native_high() {
        let compat = decode_wayland_bitcode(0x96);
        assert_eq!(compat.framework, AppFramework::Electron);
        assert_eq!(compat.support, WaylandSupport::Native);
        assert_eq!(compat.risk_level, RiskLevel::High);
    }
}
```

---

### Task 3: Parse Bitcode from AppStream Metadata

**File:** `src/appstream_cache.rs` or wherever AppStream parsing happens

Add parsing logic to extract `wayland_compat` from AppStream custom fields:

```rust
// When parsing AppStream XML, look for:
// <custom>
//   <value key="wayland_compat">0xXX</value>
// </custom>

// Parse hex string to u8
if let Some(bitcode_str) = custom_value.strip_prefix("0x") {
    if let Ok(bitcode) = u8::from_str_radix(bitcode_str, 16) {
        app_info.wayland_compat = Some(decode_wayland_bitcode(bitcode));
    }
}
```

---

### Task 4: Update wayland_compat_lazy() to Use Bitcode

**File:** `src/app_info.rs`

Update the tier 1 detection to use bitcode from AppStream:

```rust
pub fn wayland_compat_lazy(&self) -> Option<WaylandCompatibility> {
    // Tier 1: Use pre-computed bitcode from AppStream (if available)
    if let Some(compat) = &self.wayland_compat {
        log::debug!("Using pre-computed Wayland compat for {}: {:?}", self.name, compat);
        return Some(compat.clone());
    }
    
    // Tier 2: Parse metadata from disk for installed apps
    // ... (existing code)
    
    // Tier 3: Heuristics
    // ... (existing code)
}
```

---

### Task 5: Add Unit Tests

**File:** `src/app_info.rs` or `tests/wayland_bitcode.rs`

Test all bitcode combinations:

```rust
#[test]
fn test_all_risk_levels() {
    assert_eq!(decode_wayland_bitcode(0x0A).risk_level, RiskLevel::Low);
    assert_eq!(decode_wayland_bitcode(0x52).risk_level, RiskLevel::Medium);
    assert_eq!(decode_wayland_bitcode(0x96).risk_level, RiskLevel::High);
    assert_eq!(decode_wayland_bitcode(0xC1).risk_level, RiskLevel::Critical);
}

#[test]
fn test_all_frameworks() {
    assert_eq!(decode_wayland_bitcode(0x06).framework, AppFramework::GTK3);
    assert_eq!(decode_wayland_bitcode(0x0A).framework, AppFramework::GTK4);
    assert_eq!(decode_wayland_bitcode(0x0E).framework, AppFramework::Qt5);
    assert_eq!(decode_wayland_bitcode(0x12).framework, AppFramework::Qt6);
    assert_eq!(decode_wayland_bitcode(0x16).framework, AppFramework::Electron);
}
```

---

### Task 6: Test with Mock Data

1. Load mock AppStream data with bitcodes
2. Verify badges display correctly
3. Check that bitcode takes precedence over heuristics
4. Ensure fallback still works for apps without bitcodes

---

## 📝 Bitcode Quick Reference

```
0x0A = 00001010 = GTK4 + Native + Low (GNOME apps)
0x06 = 00000110 = GTK3 + Native + Low (older GNOME apps)
0x52 = 01010010 = Qt6 + Native + Medium (modern KDE apps)
0x4E = 01001110 = Qt5 + Native + Medium (older KDE apps)
0x96 = 10010110 = Electron + Native + High (Electron apps)
0xC1 = 11000001 = X11-only + Critical (legacy apps)
```

---

## 🎯 Success Criteria

- ✅ Bitcode decoder implemented and tested
- ✅ AppStream parser extracts bitcodes
- ✅ Badges display correctly based on bitcode data
- ✅ Unit tests pass for all bitcode combinations
- ✅ Mock data works in running application
- ✅ Fallback to tier 2/3 still works

---

## 📋 After Implementation

1. **Create PR** for COSMIC Store with bitcode support
2. **Post GitHub issue** to Flathub (use `GITHUB_ISSUE_WAYLAND_DATA.md`)
3. **Link PR in the issue** to show we're ready to consume the data

---

## 🔗 Reference Files

- `GITHUB_ISSUE_WAYLAND_DATA.md` - Issue template for Flathub
- `WAYLAND_COMPAT_STATUS.md` - Current implementation status
- `src/app_info.rs` - Main compatibility detection logic
- `src/backend/flatpak.rs` - Metadata parsing (tier 2)

---

## 💡 Notes for Claude Code

- Focus on implementing the bitcode decoder first (Task 2)
- Then create mock data (Task 1)
- Then integrate parsing (Task 3)
- Test thoroughly with unit tests (Task 5)
- The existing badge rendering code should work as-is

**Current working directory:** `/home/martin/Ontwikkel/cosmic-store`  
**Branch:** `feature/wayland-compatibility-detection`

