// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//! Australian postcodes to states, and states to NEM regions.

/// NEM regions in a fixed order (largest first), used for every chart.
pub const REGIONS: [&str; 5] = ["NSW1", "QLD1", "VIC1", "SA1", "TAS1"];

/// States and territories, as CER reports them.
pub const STATES: [&str; 8] = ["NSW", "ACT", "QLD", "VIC", "SA", "TAS", "WA", "NT"];

/// A region's name on charts.
pub fn region_name(region: &str) -> &'static str {
    match region {
        "NSW1" => "NSW + ACT",
        "QLD1" => "QLD",
        "VIC1" => "VIC",
        "SA1" => "SA",
        "TAS1" => "TAS",
        _ => "",
    }
}

/// The NEM region a state is in. The ACT is part of NSW1; WA and NT are
/// outside the NEM.
pub fn state_region(state: &str) -> Option<&'static str> {
    match state {
        "NSW" | "ACT" => Some("NSW1"),
        "QLD" => Some("QLD1"),
        "VIC" => Some("VIC1"),
        "SA" => Some("SA1"),
        "TAS" => Some("TAS1"),
        _ => None,
    }
}

/// A postcode's state, by Australia Post's ranges (delivery and PO box).
pub fn state_of(p: u32) -> Option<&'static str> {
    Some(match p {
        200..=299 | 2600..=2618 | 2900..=2920 => "ACT",
        1000..=2999 => "NSW",
        3000..=3999 | 8000..=8999 => "VIC",
        4000..=4999 | 9000..=9999 => "QLD",
        5000..=5999 => "SA",
        6000..=6999 => "WA",
        7000..=7999 => "TAS",
        800..=999 => "NT",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postcodes() {
        assert_eq!(state_of(2600), Some("ACT"));
        assert_eq!(state_of(2619), Some("NSW"));
        assert_eq!(state_of(2911), Some("ACT"));
        assert_eq!(state_of(800), Some("NT"));
        assert_eq!(state_of(8576), Some("VIC"));
        assert_eq!(state_of(9729), Some("QLD"));
        assert_eq!(state_of(0), None);
        assert_eq!(state_region("ACT"), Some("NSW1"));
        assert_eq!(state_region("WA"), None);
    }
}
