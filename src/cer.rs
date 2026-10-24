// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//! The Clean Energy Regulator's small-scale installation data, by state and
//! month.
//!
//! CER publishes one CSV per measure with a row per postcode and a column per
//! month ("Jul 2025 - Installation Quantity"), plus, for solar, one column
//! with the total before 2011. Rows are summed to states here.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use regex::Regex;

use crate::regions::{state_of, STATES};

/// The CER files, by the name they are saved under.
pub const BATTERY_INSTALLS: &str = "sgu-battery-installations-2011-to-present-and-totals.csv";
pub const BATTERY_CAPACITY: &str = "sgu-battery-capacity-2011-to-present-and-totals.csv";
pub const SOLAR_INSTALLS: &str = "sgu-solar-installations-2011-to-present-and-totals.csv";
pub const SOLAR_CAPACITY: &str = "sgu-solar-capacity-2011-to-present-and-totals.csv";
pub const VOLUNTARY: &str =
    "historical-voluntary-state-data-battery-installation-small-scale-systems.csv";

/// The month that holds solar's total before 2011.
pub fn before_2011() -> NaiveDate {
    NaiveDate::from_ymd_opt(2010, 12, 1).unwrap()
}

/// One state's figures for one month: installations and their size (usable
/// kWh for batteries, rated kW for solar).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Figures {
    pub installs: f64,
    pub size: f64,
}

/// (month, state) to figures, sorted by month then state.
pub type Monthly = BTreeMap<(NaiveDate, &'static str), Figures>;

/// Battery and solar tables, and the voluntary battery counts by year.
#[derive(Debug, Clone, Default)]
pub struct Cer {
    pub batteries: Monthly,
    pub solar: Monthly,
    /// (year, state) to batteries installed.
    pub voluntary: BTreeMap<(i32, &'static str), f64>,
}

/// A CER number: thousands separators removed; blank or unparsable is 0.
fn num(s: &str) -> f64 {
    s.trim().replace(',', "").parse().unwrap_or(0.0)
}

/// One postcode file summed to (month, state). A "Historic Total" column,
/// where present, becomes the month [`before_2011`]. Every state in the file
/// gets every month, zeros included.
pub fn postcode_monthly(
    bytes: &[u8],
    suffix: &str,
) -> Result<BTreeMap<(NaiveDate, &'static str), f64>> {
    let month_col = Regex::new(r"^[A-Z][a-z]{2} \d{4}").unwrap();
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(bytes);
    let headers = rdr.headers()?.clone();
    let mut cols: Vec<(usize, NaiveDate)> = Vec::new();
    for (i, h) in headers.iter().enumerate().skip(1) {
        let h = h.trim();
        if h.starts_with("Historic Total") {
            cols.push((i, before_2011()));
        } else if h.ends_with(suffix) && month_col.is_match(h) {
            let label = h[..h.len() - suffix.len()].trim();
            let month = NaiveDate::parse_from_str(&format!("1 {label}"), "%d %b %Y")
                .with_context(|| format!("month column {h:?}"))?;
            cols.push((i, month));
        }
    }
    anyhow::ensure!(!cols.is_empty(), "no monthly columns ending {suffix:?}");
    let mut out = BTreeMap::new();
    for row in rdr.records() {
        let row = row?;
        let pc = row.get(0).unwrap_or("").trim();
        if pc.is_empty() || !pc.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Some(state) = pc.parse().ok().and_then(state_of) else {
            continue;
        };
        for &(i, month) in &cols {
            *out.entry((month, state)).or_insert(0.0) += num(row.get(i).unwrap_or(""));
        }
    }
    Ok(out)
}

/// Installations and sizes joined on (month, state); a side missing a key
/// counts as 0.
fn join(
    installs: BTreeMap<(NaiveDate, &'static str), f64>,
    size: BTreeMap<(NaiveDate, &'static str), f64>,
) -> Monthly {
    let mut out = Monthly::new();
    for (k, v) in installs {
        out.entry(k).or_default().installs = v;
    }
    for (k, v) in size {
        out.entry(k).or_default().size = v;
    }
    out
}

/// The voluntary battery counts: a row per year, a column per state.
pub fn voluntary(bytes: &[u8]) -> Result<BTreeMap<(i32, &'static str), f64>> {
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(bytes);
    let headers = rdr.headers()?.clone();
    let year = headers
        .iter()
        .position(|h| h.trim() == "Installation year")
        .context("no 'Installation year' column")?;
    let cols: Vec<(usize, &'static str)> = STATES
        .iter()
        .filter_map(|s| headers.iter().position(|h| h.trim() == *s).map(|i| (i, *s)))
        .collect();
    let mut out = BTreeMap::new();
    for row in rdr.records() {
        let row = row?;
        let y: i32 = row
            .get(year)
            .unwrap_or("")
            .trim()
            .parse()
            .with_context(|| format!("year in {row:?}"))?;
        for &(i, state) in &cols {
            out.insert((y, state), num(row.get(i).unwrap_or("")));
        }
    }
    Ok(out)
}

/// All three tables from a folder written by `der fetch-cer`.
pub fn read(dir: &Path) -> Result<Cer> {
    let file = |name: &str| {
        std::fs::read(dir.join(name))
            .with_context(|| format!("reading {name} in {}", dir.display()))
    };
    Ok(Cer {
        batteries: join(
            postcode_monthly(&file(BATTERY_INSTALLS)?, " - Installation Quantity")?,
            postcode_monthly(&file(BATTERY_CAPACITY)?, " - Usable capacity in kWh")?,
        ),
        solar: join(
            postcode_monthly(&file(SOLAR_INSTALLS)?, " - Installation Quantity")?,
            postcode_monthly(&file(SOLAR_CAPACITY)?, " - Rated Power Output in kW")?,
        ),
        voluntary: voluntary(&file(VOLUNTARY)?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ym(y: i32, m: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, 1).unwrap()
    }

    #[test]
    fn postcodes_sum_to_states() {
        let csv = "Small Unit Installation Postcode,Historic Total Rated Power Output In kW (2001 - 2010),Jan 2011 - Rated Power Output in kW,Feb 2011 - Rated Power Output in kW,Total Rated Power Output in kW\n\
                   0000,4.46,1,1,6.46\n\
                   2600,1.5,\"1,000.25\",0,1001.75\n\
                   2000,2,3,,5\n\
                   0800,1,0,2,3\n\
                   Total,9,9,9,9\n";
        let t = postcode_monthly(csv.as_bytes(), " - Rated Power Output in kW").unwrap();
        assert_eq!(t[&(before_2011(), "ACT")], 1.5);
        assert_eq!(t[&(ym(2011, 1), "ACT")], 1000.25);
        assert_eq!(t[&(ym(2011, 2), "NSW")], 0.0);
        assert_eq!(t[&(ym(2011, 2), "NT")], 2.0);
        // Postcode 0000 has no state; "Total" is not a postcode; the total column is not a month.
        assert_eq!(t.len(), 9);
    }

    #[test]
    fn voluntary_counts() {
        let csv = "Installation year,ACT,NSW,NT,QLD,SA,TAS,VIC,WA,Total\n2024,679,\"7,957\",485,1,2,3,4,5,9999\n";
        let v = voluntary(csv.as_bytes()).unwrap();
        assert_eq!(v[&(2024, "NSW")], 7957.0);
        assert_eq!(v.len(), 8);
    }
}
