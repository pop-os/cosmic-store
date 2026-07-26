// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

//! Comparison of package versions.
//!
//! Backends normally let the package manager decide what is an update, but a
//! package loaded from a local file (such as a `.deb` or `.rpm` opened from a
//! browser) has to be compared against the installed version by hand.
//!
//! Both formats use an `epoch:version-release` string, but they order the rest
//! differently, so the ordering is picked from the file the package came from:
//!
//! * Debian policy
//!   (<https://www.debian.org/doc/debian-policy/ch-controlfields.html#version>)
//! * `rpmvercmp` from RPM
//!   (<https://github.com/rpm-software-management/rpm/blob/master/rpmio/rpmvercmp.c>)

use std::{cmp::Ordering, path::Path};

/// Version ordering used by a package format
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    /// Debian ordering, used for `.deb` packages
    Deb,
    /// RPM ordering, used for `.rpm` packages
    Rpm,
}

impl Format {
    /// Format of the package file at `path`.
    ///
    /// Anything that is not an RPM is compared with Debian ordering, which is
    /// the only kind of package file the store has opened so far.
    pub fn from_path(path: &str) -> Self {
        match Path::new(path).extension().and_then(|ext| ext.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("rpm") => Self::Rpm,
            _ => Self::Deb,
        }
    }

    /// Compare two package versions
    pub fn compare(self, a: &str, b: &str) -> Ordering {
        let (a_epoch, a_version, a_release) = split(a);
        let (b_epoch, b_version, b_release) = split(b);
        let compare_part = match self {
            Self::Deb => compare_part_deb,
            Self::Rpm => compare_part_rpm,
        };
        a_epoch
            .cmp(&b_epoch)
            .then_with(|| compare_part(a_version, b_version))
            .then_with(|| compare_part(a_release, b_release))
    }
}

/// Split a version into its epoch, version, and release
fn split(version: &str) -> (u64, &str, &str) {
    let version = version.trim();
    let (epoch, rest) = match version.split_once(':') {
        // An unparsable epoch means this version does not have one
        Some((epoch, rest)) => match epoch.parse::<u64>() {
            Ok(epoch) => (epoch, rest),
            Err(_) => (0, version),
        },
        None => (0, version),
    };
    match rest.rsplit_once('-') {
        Some((version, release)) => (epoch, version, release),
        None => (epoch, rest, ""),
    }
}

/// Sort order of a single character of a non-digit part, following dpkg:
/// `~` sorts before the end of a part, which sorts before letters, which sort
/// before all other characters.
fn order(c_opt: Option<u8>) -> i32 {
    match c_opt {
        Some(b'~') => -1,
        None => 0,
        Some(c) if c.is_ascii_digit() => 0,
        Some(c) if c.is_ascii_alphabetic() => i32::from(c),
        Some(c) => i32::from(c) + 256,
    }
}

/// Compare the version or release part of two Debian versions
fn compare_part_deb(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        // Compare non-digit runs one character at a time
        while (i < a.len() && !a[i].is_ascii_digit()) || (j < b.len() && !b[j].is_ascii_digit()) {
            let a_order = order(a.get(i).copied());
            let b_order = order(b.get(j).copied());
            if a_order != b_order {
                return a_order.cmp(&b_order);
            }
            i += 1;
            j += 1;
        }

        // Compare digit runs numerically, which means ignoring leading zeros
        // and treating the longer run as the larger number
        while i < a.len() && a[i] == b'0' {
            i += 1;
        }
        while j < b.len() && b[j] == b'0' {
            j += 1;
        }
        let mut first_diff = Ordering::Equal;
        while i < a.len() && j < b.len() && a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            if first_diff == Ordering::Equal {
                first_diff = a[i].cmp(&b[j]);
            }
            i += 1;
            j += 1;
        }
        if i < a.len() && a[i].is_ascii_digit() {
            return Ordering::Greater;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            return Ordering::Less;
        }
        if first_diff != Ordering::Equal {
            return first_diff;
        }
    }
    Ordering::Equal
}

/// Characters between segments of an RPM version, which are not compared
/// themselves
fn is_separator(c: u8) -> bool {
    !c.is_ascii_alphanumeric() && c != b'~' && c != b'^'
}

/// End of the alphabetic or numeric segment that starts at `i`
fn segment_end(v: &[u8], mut i: usize, numeric: bool) -> usize {
    while i < v.len()
        && if numeric {
            v[i].is_ascii_digit()
        } else {
            v[i].is_ascii_alphabetic()
        }
    {
        i += 1;
    }
    i
}

/// Compare the version or release part of two RPM versions
fn compare_part_rpm(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        // Separators are not compared, only the segments around them
        while i < a.len() && is_separator(a[i]) {
            i += 1;
        }
        while j < b.len() && is_separator(b[j]) {
            j += 1;
        }

        // `~` sorts before everything, including the end of a version
        if a.get(i) == Some(&b'~') || b.get(j) == Some(&b'~') {
            if a.get(i) != Some(&b'~') {
                return Ordering::Greater;
            }
            if b.get(j) != Some(&b'~') {
                return Ordering::Less;
            }
            i += 1;
            j += 1;
            continue;
        }

        // `^` sorts before everything as well, but after the end of a version
        if a.get(i) == Some(&b'^') || b.get(j) == Some(&b'^') {
            if i >= a.len() {
                return Ordering::Less;
            }
            if j >= b.len() {
                return Ordering::Greater;
            }
            if a.get(i) != Some(&b'^') {
                return Ordering::Greater;
            }
            if b.get(j) != Some(&b'^') {
                return Ordering::Less;
            }
            i += 1;
            j += 1;
            continue;
        }

        // One of the versions ran out of segments
        if i >= a.len() || j >= b.len() {
            break;
        }

        // Compare the next segment, which is either all digits or all letters
        let numeric = a[i].is_ascii_digit();
        let a_end = segment_end(a, i, numeric);
        let b_end = segment_end(b, j, numeric);
        if j == b_end {
            // Segments of different kinds, digits are newer than letters
            return if numeric {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        let (mut a_segment, mut b_segment) = (&a[i..a_end], &b[j..b_end]);
        if numeric {
            // Leading zeros are not significant, so the number with more digits
            // left is the larger one
            while let [b'0', rest @ ..] = a_segment {
                a_segment = rest;
            }
            while let [b'0', rest @ ..] = b_segment {
                b_segment = rest;
            }
            match a_segment.len().cmp(&b_segment.len()) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        match a_segment.cmp(b_segment) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        i = a_end;
        j = b_end;
    }

    // Whichever version has segments left over is the newer one
    match (i >= a.len(), j >= b.len()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, _) => Ordering::Greater,
    }
}

#[cfg(test)]
mod tests {
    use super::Format;
    use std::cmp::Ordering;

    #[track_caller]
    fn assert_order(format: Format, a: &str, ordering: Ordering, b: &str) {
        assert_eq!(format.compare(a, b), ordering, "comparing {a:?} with {b:?}");
        assert_eq!(
            format.compare(b, a),
            ordering.reverse(),
            "comparing {b:?} with {a:?}"
        );
    }

    #[track_caller]
    fn assert_deb(a: &str, ordering: Ordering, b: &str) {
        assert_order(Format::Deb, a, ordering, b);
    }

    /// Cases from the RPM test suite, which uses -1, 0, and 1
    #[track_caller]
    fn assert_rpm(a: &str, expected: i32, b: &str) {
        let ordering = match expected {
            -1 => Ordering::Less,
            0 => Ordering::Equal,
            _ => Ordering::Greater,
        };
        assert_order(Format::Rpm, a, ordering, b);
    }

    #[test]
    fn format_from_path() {
        assert_eq!(
            Format::from_path("/tmp/foo-1.2.3-1.x86_64.rpm"),
            Format::Rpm
        );
        assert_eq!(Format::from_path("/tmp/FOO.RPM"), Format::Rpm);
        assert_eq!(Format::from_path("/tmp/foo_1.2.3-1_amd64.deb"), Format::Deb);
        // Anything unknown falls back to Debian ordering
        assert_eq!(Format::from_path("/tmp/foo"), Format::Deb);
    }

    #[test]
    fn deb_equal() {
        assert_deb("1.0", Ordering::Equal, "1.0");
        assert_deb("1.0-1", Ordering::Equal, "1.0-1");
        // Leading zeros are not significant
        assert_deb("1.007", Ordering::Equal, "1.7");
        assert_deb("0:1.0", Ordering::Equal, "1.0");
        assert_deb("", Ordering::Equal, "");
    }

    #[test]
    fn deb_version() {
        assert_deb("2.0", Ordering::Greater, "1.0");
        // Numbers are compared numerically, not as text
        assert_deb("1.10", Ordering::Greater, "1.9");
        assert_deb("1.0.1", Ordering::Greater, "1.0");
        assert_deb("1.0a", Ordering::Greater, "1.0");
        // Versions of the packages from the issue reports
        assert_deb("2026.01.0-392", Ordering::Greater, "2025.09.2-418");
        assert_deb("0.11.0-1", Ordering::Greater, "0.9.0-1");
    }

    #[test]
    fn deb_tilde_sorts_first() {
        assert_deb("1.0", Ordering::Greater, "1.0~rc1");
        assert_deb("1.0~rc2", Ordering::Greater, "1.0~rc1");
        assert_deb("1.0~rc1", Ordering::Greater, "0.9");
    }

    #[test]
    fn deb_epoch() {
        assert_deb("1:1.0", Ordering::Greater, "2.0");
        assert_deb("2:1.0", Ordering::Greater, "1:9.0");
        // A colon that is not part of an epoch is compared as a character
        assert_deb("1.0:a", Ordering::Greater, "1.0");
    }

    #[test]
    fn deb_release() {
        assert_deb("1.0-2", Ordering::Greater, "1.0-1");
        assert_deb("1.0-1", Ordering::Greater, "1.0");
        assert_deb("1.0-1ubuntu2", Ordering::Greater, "1.0-1ubuntu1");
        assert_deb("1.0-1", Ordering::Greater, "1.0-1~bpo1");
        // Only the last hyphen separates the release
        assert_deb("1.0-a-2", Ordering::Greater, "1.0-a-1");
    }

    /// Cases taken from rpm's own tests/rpmvercmp.at
    #[test]
    fn rpm_vercmp() {
        assert_rpm("1.0", 0, "1.0");
        assert_rpm("1.0", -1, "2.0");
        assert_rpm("2.0.1", 0, "2.0.1");
        assert_rpm("2.0", -1, "2.0.1");
        assert_rpm("2.0.1a", 1, "2.0.1");
        assert_rpm("5.5p1", 0, "5.5p1");
        assert_rpm("5.5p1", -1, "5.5p2");
        assert_rpm("5.5p10", 0, "5.5p10");
        assert_rpm("5.5p1", -1, "5.5p10");
        assert_rpm("10xyz", -1, "10.1xyz");
        assert_rpm("xyz10", 0, "xyz10");
        assert_rpm("xyz10", -1, "xyz10.1");
        assert_rpm("xyz.4", 0, "xyz.4");
        assert_rpm("xyz.4", -1, "8");
        assert_rpm("20101121", -1, "20101122");
        assert_rpm("1.0", 1, "1.fc4");
        assert_rpm("3.0.0_fc", 0, "3.0.0.fc");
        // Separators are not significant
        assert_rpm("2_0", 0, "2.0");
        assert_rpm("a+", 0, "a_");
        // Leading zeros are not significant
        assert_rpm("1.005", 0, "1.5");
        assert_rpm("1.0010", 1, "1.9");
    }

    #[test]
    fn rpm_tilde_and_caret() {
        assert_rpm("1.0~rc1", -1, "1.0");
        assert_rpm("1.0~rc1", -1, "1.0~rc2");
        assert_rpm("1.0~rc1~git123", -1, "1.0~rc1");
        assert_rpm("1.0^", 1, "1.0");
        assert_rpm("1.0^", 0, "1.0^");
        assert_rpm("1.0^git1", 1, "1.0");
        assert_rpm("1.0^git1", -1, "1.0^git2");
        assert_rpm("1.0^20160101", -1, "1.0.1");
        assert_rpm("1.0^git1~pre", -1, "1.0^git1");
    }

    #[test]
    fn rpm_epoch_and_release() {
        assert_rpm("1:1.0-1", 1, "2.0-1");
        assert_rpm("1.0-2.fc42", 1, "1.0-1.fc42");
        assert_rpm("1.0-1.fc42", 1, "1.0-1.fc9");
        // Releases in the style used by upstream RPM downloads
        assert_rpm("2026.01.0-392.el9", 1, "2025.09.2-418.el9");
    }

    /// Versions exactly as PackageKit reports them for local `.rpm` files,
    /// where the epoch is part of the version field of the package ID
    #[test]
    fn rpm_package_kit_versions() {
        assert_rpm("1.1-1.fc44", 1, "1.0-1.fc44");
        assert_rpm("1:0.9-1.fc44", 1, "1.0-1.fc44");
        assert_rpm("1:1.2-1", 1, "1.0-1");
    }


    /// Cases where the two formats disagree, which is why the file the package
    /// came from decides how it is compared
    #[test]
    fn formats_disagree() {
        // RPM treats digits as newer than letters, dpkg does not
        assert_rpm("1.0", 1, "1.fc4");
        assert_deb("1.0", Ordering::Less, "1.fc4");
        // RPM ignores separators, dpkg compares them
        assert_rpm("1.0^20160101", -1, "1.0.1");
        assert_deb("1.0^20160101", Ordering::Greater, "1.0.1");
    }
}
