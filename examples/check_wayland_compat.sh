#!/bin/bash
# Quick script to check Wayland compatibility of installed Flatpak apps
# This demonstrates the detection logic we'll implement in Rust

echo "=== Flatpak Wayland Compatibility Checker ==="
echo ""

# Colors
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# Temporary files for counting
tmpdir=$(mktemp -d)
trap "rm -rf $tmpdir" EXIT

# Get all installed apps
flatpak list --app --columns=application 2>/dev/null | while read app; do
    if [ -z "$app" ]; then
        continue
    fi
    
    # Get metadata
    metadata=$(flatpak info --show-metadata "$app" 2>/dev/null)
    
    if [ -z "$metadata" ]; then
        echo -e "${YELLOW}⚠️  $app${NC} - No metadata"
        echo "1" >> "$tmpdir/unknown"
        continue
    fi
    
    # Extract sockets line
    sockets=$(echo "$metadata" | grep "^sockets=" | cut -d= -f2)
    
    if [ -z "$sockets" ]; then
        echo -e "${YELLOW}ℹ️  $app${NC} - No socket info"
        echo "1" >> "$tmpdir/unknown"
        continue
    fi
    
    # Check compatibility
    has_wayland=$(echo "$sockets" | grep -o "wayland")
    has_x11=$(echo "$sockets" | grep -o "x11" | grep -v "fallback-x11")
    has_fallback=$(echo "$sockets" | grep -o "fallback-x11")
    
    if [ -n "$has_wayland" ]; then
        echo -e "${GREEN}✅ $app${NC} - Wayland native ($sockets)"
        echo "1" >> "$tmpdir/wayland"
    elif [ -n "$has_fallback" ]; then
        echo -e "${YELLOW}⚠️  $app${NC} - Fallback to X11 ($sockets)"
        echo "1" >> "$tmpdir/fallback"
    elif [ -n "$has_x11" ]; then
        echo -e "${RED}❌ $app${NC} - X11 ONLY ($sockets)"
        echo "1" >> "$tmpdir/x11only"
    else
        echo -e "${YELLOW}❓ $app${NC} - Unknown ($sockets)"
        echo "1" >> "$tmpdir/unknown"
    fi
done

# Count results
wayland_native=$(wc -l < "$tmpdir/wayland" 2>/dev/null || echo 0)
fallback=$(wc -l < "$tmpdir/fallback" 2>/dev/null || echo 0)
x11_only=$(wc -l < "$tmpdir/x11only" 2>/dev/null || echo 0)
unknown=$(wc -l < "$tmpdir/unknown" 2>/dev/null || echo 0)

echo ""
echo "=== Summary ==="
echo "✅ Wayland Native: $wayland_native"
echo "⚠️  Fallback X11: $fallback"
echo "❌ X11 Only: $x11_only"
echo "❓ Unknown: $unknown"
echo ""

if [ $x11_only -gt 0 ]; then
    echo -e "${RED}Warning: $x11_only apps may not work properly on COSMIC (Wayland)${NC}"
fi

