// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//! AEMO's rooftop PV estimates (ROOFTOP_PV_ACTUAL), by region and month.
//!
//! AEMO publishes a half-hourly estimate of rooftop PV output per region,
//! some intervals both as a MEASUREMENT estimate and a SATELLITE one, and
//! revises them. The monthly archive files overlap at their edges, so all
//! months are read before choosing one value per half hour and region.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};
use nem_lens_aemo::files::{visit_csvs, MAX_MEMBER_BYTES};
use nem_lens_aemo::report::visit_report;

use crate::regions::REGIONS;

/// One estimate as published.
#[derive(Debug, Clone, PartialEq)]
pub struct Estimate {
    /// End of the half hour.
    pub t: NaiveDateTime,
    /// Index into [`REGIONS`].
    pub region: usize,
    /// MW; NaN if AEMO left it blank.
    pub power: f64,
    pub measured: bool,
    pub last_changed: String,
}

/// Every estimate for the NEM's regions in one archive zip.
pub fn read_zip(bytes: &[u8]) -> Result<Vec<Estimate>> {
    let mut out = Vec::new();
    visit_csvs(bytes, MAX_MEMBER_BYTES, |_, r| {
        visit_report(r, |rec| {
            let Some(region) = rec
                .text("REGIONID")
                .and_then(|r| REGIONS.iter().position(|x| *x == r))
            else {
                return Ok(());
            };
            let t = rec
                .text("INTERVAL_DATETIME")
                .context("no INTERVAL_DATETIME")?;
            out.push(Estimate {
                t: NaiveDateTime::parse_from_str(t, "%Y/%m/%d %H:%M:%S")
                    .with_context(|| format!("time {t:?}"))?,
                region,
                power: rec.num("POWER").unwrap_or(f64::NAN),
                measured: rec.text("TYPE") == Some("MEASUREMENT"),
                last_changed: rec.text("LASTCHANGED").unwrap_or_default().to_string(),
            });
            Ok(())
        })
    })?;
    Ok(out)
}

/// One value per half hour and region: the MEASUREMENT estimate where AEMO
/// has one, otherwise another (SATELLITE), each at its latest revision.
/// Among equal revisions the one read last wins, so pass estimates in file
/// order, oldest file first.
pub fn one_estimate(all: impl IntoIterator<Item = Estimate>) -> Vec<Estimate> {
    let mut best: HashMap<(NaiveDateTime, usize), [Option<Estimate>; 2]> = HashMap::new();
    for e in all {
        let slot = &mut best.entry((e.t, e.region)).or_default()[usize::from(!e.measured)];
        if slot
            .as_ref()
            .is_none_or(|old| e.last_changed >= old.last_changed)
        {
            *slot = Some(e);
        }
    }
    let mut out: Vec<Estimate> = best.into_values().filter_map(|[m, s]| m.or(s)).collect();
    out.sort_by_key(|e| (e.t, e.region));
    out
}

/// The month a half hour belongs to: the one it starts in.
pub fn month_of(t: NaiveDateTime) -> NaiveDate {
    (t - Duration::minutes(30)).date().with_day(1).unwrap()
}

fn days_in_month(m: NaiveDate) -> i64 {
    let next = if m.month() == 12 {
        NaiveDate::from_ymd_opt(m.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(m.year(), m.month() + 1, 1)
    };
    (next.unwrap() - m).num_days()
}

/// One region's month.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionMonth {
    pub mean_mw: f64,
    pub peak_mw: f64,
    pub energy_gwh: f64,
    /// Share of half hours with a MEASUREMENT estimate.
    pub measured_share: f64,
}

/// The NEM's highest half hour in a month (sum of the five regions).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peak {
    pub at: NaiveDateTime,
    pub mw: f64,
}

/// Monthly tables, complete months only.
#[derive(Debug, Clone, Default)]
pub struct Rooftop {
    /// (month, region) to figures.
    pub by_region: BTreeMap<(NaiveDate, &'static str), RegionMonth>,
    pub peaks: BTreeMap<NaiveDate, Peak>,
}

/// Monthly figures from chosen estimates. A month is kept only if every
/// region has at least 90% of its half hours.
pub fn monthly(half: &[Estimate]) -> Rooftop {
    let mut groups: BTreeMap<(NaiveDate, usize), Vec<&Estimate>> = BTreeMap::new();
    for e in half {
        groups.entry((month_of(e.t), e.region)).or_default().push(e);
    }
    let complete = |m: NaiveDate| {
        (0..REGIONS.len()).all(|r| {
            groups.get(&(m, r)).map_or(0, Vec::len) as f64 >= 0.9 * (days_in_month(m) * 48) as f64
        })
    };
    let mut out = Rooftop::default();
    let mut nem: BTreeMap<NaiveDateTime, f64> = BTreeMap::new();
    for ((m, r), es) in &groups {
        if !complete(*m) {
            continue;
        }
        let powers: Vec<f64> = es.iter().map(|e| e.power).filter(|p| !p.is_nan()).collect();
        let sum: f64 = powers.iter().sum();
        out.by_region.insert(
            (*m, REGIONS[*r]),
            RegionMonth {
                mean_mw: if powers.is_empty() {
                    f64::NAN
                } else {
                    sum / powers.len() as f64
                },
                peak_mw: powers.iter().copied().fold(f64::NAN, f64::max),
                energy_gwh: sum * 0.5 / 1000.0,
                measured_share: es.iter().filter(|e| e.measured).count() as f64 / es.len() as f64,
            },
        );
        for e in es {
            *nem.entry(e.t).or_insert(0.0) += if e.power.is_nan() { 0.0 } else { e.power };
        }
    }
    for (&t, &mw) in &nem {
        let m = month_of(t);
        // Ties keep the earliest half hour.
        if out.peaks.get(&m).is_none_or(|p| mw > p.mw) {
            out.peaks.insert(m, Peak { at: t, mw });
        }
    }
    out
}

/// Every `*.zip` in a folder written by `der fetch-aemo`, in name order.
pub fn read(dir: &Path) -> Result<Rooftop> {
    let mut zips: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "zip"))
        .collect();
    zips.sort();
    let mut all = Vec::new();
    for p in &zips {
        all.extend(
            read_zip(&std::fs::read(p)?).with_context(|| format!("reading {}", p.display()))?,
        );
    }
    Ok(monthly(&one_estimate(all)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    fn est(t: &str, region: usize, power: f64, measured: bool, changed: &str) -> Estimate {
        Estimate {
            t: at(t),
            region,
            power,
            measured,
            last_changed: changed.into(),
        }
    }

    #[test]
    fn measurement_wins_then_latest_revision() {
        let chosen = one_estimate([
            est("2026-01-01 12:00", 0, 100.0, false, "2026/01/02"),
            est("2026-01-01 12:00", 0, 90.0, true, "2026/01/01"),
            est("2026-01-01 12:00", 0, 95.0, true, "2026/01/03"),
            est("2026-01-01 12:30", 0, 50.0, false, "2026/01/01"),
            est("2026-01-01 12:30", 0, 55.0, false, "2026/01/01"),
        ]);
        assert_eq!(chosen.len(), 2);
        assert_eq!(chosen[0].power, 95.0);
        // Equal revisions: the one read last.
        assert_eq!(chosen[1].power, 55.0);
    }

    #[test]
    fn midnight_belongs_to_the_month_it_starts_in() {
        assert_eq!(
            month_of(at("2026-10-01 00:00")),
            NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()
        );
        assert_eq!(
            month_of(at("2026-10-01 00:30")),
            NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()
        );
    }

    #[test]
    fn incomplete_months_dropped() {
        // February 2026 complete in every region, March with one half hour.
        let mut half = Vec::new();
        let mut t = at("2026-02-01 00:30");
        while t <= at("2026-03-01 00:00") {
            for r in 0..REGIONS.len() {
                half.push(Estimate {
                    t,
                    region: r,
                    power: (r + 1) as f64,
                    measured: r == 0,
                    last_changed: String::new(),
                });
            }
            t += Duration::minutes(30);
        }
        for r in 0..REGIONS.len() {
            half.push(Estimate {
                t: at("2026-03-01 00:30"),
                region: r,
                power: 1.0,
                measured: true,
                last_changed: String::new(),
            });
        }
        let out = monthly(&half);
        let feb = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        assert_eq!(out.by_region.len(), 5);
        assert_eq!(out.by_region[&(feb, "NSW1")].measured_share, 1.0);
        assert_eq!(
            out.by_region[&(feb, "QLD1")].energy_gwh,
            2.0 * 28.0 * 48.0 * 0.5 / 1000.0
        );
        assert_eq!(
            out.peaks[&feb],
            Peak {
                at: at("2026-02-01 00:30"),
                mw: 15.0
            }
        );
        assert_eq!(out.peaks.len(), 1);
    }
}
