// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//! The data tables written to `data/`, as CSV.

use std::fmt::Write as _;

use chrono::NaiveDate;

use crate::cer::{Cer, Monthly};
use crate::regions::state_region;
use crate::rooftop::Rooftop;

/// Home batteries are reported by month from July 2025.
pub fn batteries_from() -> NaiveDate {
    NaiveDate::from_ymd_opt(2025, 7, 1).unwrap()
}

/// Batteries from July 2025, by month and state.
pub fn batteries(cer: &Cer) -> Monthly {
    cer.batteries
        .range((batteries_from(), "")..)
        .map(|(k, v)| (*k, *v))
        .collect()
}

fn monthly_csv(t: &Monthly, size: &str) -> String {
    let mut s = format!("state,month,installs,{size},region\n");
    for (&(month, state), f) in t {
        let _ = writeln!(
            s,
            "{state},{month},{:.0},{:.3},{}",
            f.installs,
            f.size,
            state_region(state).unwrap_or("")
        );
    }
    s
}

/// `batteries_monthly_by_state.csv`.
pub fn batteries_csv(cer: &Cer) -> String {
    monthly_csv(&batteries(cer), "usable_kwh")
}

/// `solar_monthly_by_state.csv`. Month 2010-12 holds the total before 2011.
pub fn solar_csv(cer: &Cer) -> String {
    monthly_csv(&cer.solar, "rated_kw")
}

/// `batteries_voluntary_yearly_by_state.csv`.
pub fn voluntary_csv(cer: &Cer) -> String {
    let mut s = String::from("state,year,installs,region\n");
    for (&(year, state), n) in &cer.voluntary {
        let _ = writeln!(
            s,
            "{state},{year},{n:.0},{}",
            state_region(state).unwrap_or("")
        );
    }
    s
}

/// `rooftop_pv_monthly_by_region.csv`.
pub fn rooftop_csv(r: &Rooftop) -> String {
    let mut s = String::from("region,month,mean_mw,peak_mw,energy_gwh,measured_share\n");
    for (&(month, region), v) in &r.by_region {
        let _ = writeln!(
            s,
            "{region},{month},{:.3},{:.3},{:.3},{:.3}",
            v.mean_mw, v.peak_mw, v.energy_gwh, v.measured_share
        );
    }
    s
}

/// `rooftop_pv_nem_monthly_peak.csv`.
pub fn peaks_csv(r: &Rooftop) -> String {
    let mut s = String::from("month,peak_at,nem_peak_mw\n");
    for (month, p) in &r.peaks {
        let _ = writeln!(
            s,
            "{month},{},{:.1}",
            p.at.format("%Y-%m-%d %H:%M:%S"),
            p.mw
        );
    }
    s
}
