// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//! `der build`: the tables, charts and README figures from the downloads.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use chrono::NaiveDate;

use crate::cer::{self, Cer, Monthly};
use crate::headline::{self, commas};
use crate::regions::{state_region, REGIONS};
use crate::rooftop::{self, Rooftop};
use crate::svg::{self, Labels};
use crate::tables;

const CER_NOTE: &str = "Based on Clean Energy Regulator material, CC BY 4.0. Postcodes aggregated to NEM regions; ACT in NSW. nem-lens/der";
const AEMO_NOTE: &str = "Derived from AEMO ROOFTOP_PV_ACTUAL estimates, used under AEMO's copyright permissions. nem-lens/der";

/// The newest `raw/cer/<date>/` folder.
fn latest_cer(raw: &Path) -> Result<std::path::PathBuf> {
    let dir = raw.join("cer");
    std::fs::read_dir(&dir)
        .with_context(|| format!("reading {}; run `der fetch-cer` first", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .max()
        .with_context(|| format!("no downloads in {}", dir.display()))
}

/// Rows of (key, region, value) summed to one row per key, with a column
/// per region in [`REGIONS`] order. Rows outside the NEM (region `None`)
/// add nothing but still give their key a row.
fn by_region<K: Ord + Copy>(
    rows: impl Iterator<Item = (K, Option<&'static str>, f64)>,
) -> BTreeMap<K, [f64; 5]> {
    let mut out: BTreeMap<K, [f64; 5]> = BTreeMap::new();
    for (k, region, v) in rows {
        let row = out.entry(k).or_default();
        if let Some(r) = region.and_then(|r| REGIONS.iter().position(|x| *x == r)) {
            row[r] += v;
        }
    }
    out
}

fn monthly_by_region(
    t: &Monthly,
    value: impl Fn(&cer::Figures) -> f64,
) -> BTreeMap<NaiveDate, [f64; 5]> {
    by_region(t.iter().map(|(&(m, s), f)| (m, state_region(s), value(f))))
}

fn cumulative(t: &BTreeMap<NaiveDate, [f64; 5]>, scale: f64) -> BTreeMap<NaiveDate, [f64; 5]> {
    let mut acc = [0.0; 5];
    t.iter()
        .map(|(m, v)| {
            for r in 0..5 {
                acc[r] += v[r];
            }
            (*m, acc.map(|a| a / scale))
        })
        .collect()
}

fn split(t: &BTreeMap<NaiveDate, [f64; 5]>, scale: f64) -> (Vec<NaiveDate>, Vec<[f64; 5]>) {
    t.iter().map(|(m, v)| (*m, v.map(|a| a / scale))).unzip()
}

fn ym(y: i32, m: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, 1).unwrap()
}

fn charts(cer: &Cer, roof: &Rooftop, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let write = |name: &str, svg: String| std::fs::write(dir.join(name), svg);
    let one = |v: f64| commas(v, 1);
    let zero = |v: f64| commas(v, 0);
    let label = |m: &NaiveDate| m.format("%b %y").to_string();

    let b = tables::batteries(cer);
    let (x, ys) = split(&cumulative(&monthly_by_region(&b, |f| f.size), 1e6), 1.0);
    write(
        "batteries_cumulative.svg",
        svg::stacked_area(
            &x,
            &ys,
            &Labels {
                title: "Home battery storage installed since July 2025",
                subtitle: "Cumulative usable capacity, GWh, by NEM region",
                unit: "GWh",
                note: CER_NOTE,
                fmt: &one,
            },
        ),
    )?;
    let (x, ys) = split(&monthly_by_region(&b, |f| f.installs), 1e3);
    write(
        "batteries_monthly.svg",
        svg::stacked_bars(&x.iter().map(label).collect::<Vec<_>>(), &ys, 2, &Labels {
            title: "Home batteries installed each month",
            subtitle: "Thousands of systems, by NEM region. Faded: the latest two months, still being registered.",
            unit: "thousand",
            note: CER_NOTE,
            fmt: &zero,
        }),
    )?;
    let v = by_region(
        cer.voluntary
            .iter()
            .map(|(&(y, s), &n)| (y, state_region(s), n)),
    );
    let years: Vec<String> = v
        .keys()
        .map(|y| {
            if *y < 2025 {
                y.to_string()
            } else {
                "2025 H1".into()
            }
        })
        .collect();
    write(
        "batteries_before.svg",
        svg::stacked_bars(
            &years,
            &v.values().map(|r| r.map(|a| a / 1e3)).collect::<Vec<_>>(),
            0,
            &Labels {
                title: "Home batteries before the subsidy",
                subtitle:
                    "Thousands of systems a year, voluntary reports to the CER (an under-count)",
                unit: "thousand",
                note: CER_NOTE,
                fmt: &zero,
            },
        ),
    )?;

    let solar = monthly_by_region(&cer.solar, |f| f.size);
    let cum = cumulative(&solar, 1e6);
    let (x, ys) = split(
        &cum.range(ym(2011, 1)..).map(|(k, v)| (*k, *v)).collect(),
        1.0,
    );
    write(
        "solar_cumulative.svg",
        svg::stacked_area(
            &x,
            &ys,
            &Labels {
                title: "Small-scale solar installed",
                subtitle: "Cumulative rated output, GW, by NEM region (systems up to 100 kW)",
                unit: "GW",
                note: CER_NOTE,
                fmt: &one,
            },
        ),
    )?;
    let (x, ys) = split(
        &solar.range(ym(2018, 1)..).map(|(k, v)| (*k, *v)).collect(),
        1e3,
    );
    write(
        "solar_monthly.svg",
        svg::stacked_bars(&x.iter().map(label).collect::<Vec<_>>(), &ys, 2, &Labels {
            title: "Small-scale solar added each month",
            subtitle: "Rated output, MW, by NEM region. Faded: the latest two months, still being registered.",
            unit: "MW",
            note: CER_NOTE,
            fmt: &zero,
        }),
    )?;

    let mean = by_region(
        roof.by_region
            .iter()
            .map(|(&(m, r), v)| (m, Some(r), v.mean_mw)),
    );
    let (x, ys) = split(&mean, 1e3);
    write(
        "rooftop_pv_monthly.svg",
        svg::stacked_area(
            &x,
            &ys,
            &Labels {
                title: "Rooftop PV output",
                subtitle: "Mean output over each month, GW, by NEM region (AEMO estimates)",
                unit: "GW",
                note: AEMO_NOTE,
                fmt: &one,
            },
        ),
    )?;
    let (x, y): (Vec<NaiveDate>, Vec<f64>) =
        roof.peaks.iter().map(|(m, p)| (*m, p.mw / 1e3)).unzip();
    write(
        "rooftop_pv_peak.svg",
        svg::line(
            &x,
            &y,
            &Labels {
                title: "Rooftop PV at its peak",
                subtitle: "Highest half-hour output across the NEM each month, GW (AEMO estimates)",
                unit: "GW",
                note: AEMO_NOTE,
                fmt: &one,
            },
        ),
    )?;
    Ok(())
}

/// Read `raw/`, write `data/`, `charts/` and the README's headline block
/// under `root`. Returns the headline lines.
pub fn run(raw: &Path, root: &Path) -> Result<Vec<String>> {
    let cer_dir = latest_cer(raw)?;
    let cer = cer::read(&cer_dir)?;
    let roof = rooftop::read(&raw.join("aemo").join("ROOFTOP_PV_ACTUAL"))?;

    let data = root.join("data");
    std::fs::create_dir_all(&data)?;
    std::fs::write(
        data.join("batteries_monthly_by_state.csv"),
        tables::batteries_csv(&cer),
    )?;
    std::fs::write(
        data.join("batteries_voluntary_yearly_by_state.csv"),
        tables::voluntary_csv(&cer),
    )?;
    std::fs::write(
        data.join("solar_monthly_by_state.csv"),
        tables::solar_csv(&cer),
    )?;
    std::fs::write(
        data.join("rooftop_pv_monthly_by_region.csv"),
        tables::rooftop_csv(&roof),
    )?;
    std::fs::write(
        data.join("rooftop_pv_nem_monthly_peak.csv"),
        tables::peaks_csv(&roof),
    )?;
    let sources: serde_json::Value =
        serde_json::from_slice(&std::fs::read(cer_dir.join("sources.json"))?)?;
    std::fs::write(
        data.join("cer_sources.json"),
        serde_json::to_string_pretty(&sources)?,
    )?;

    charts(&cer, &roof, &root.join("charts"))?;

    let lines = headline::lines(&cer, &roof)?;
    let readme = root.join("README.md");
    let text = std::fs::read_to_string(&readme)?;
    std::fs::write(&readme, headline::into_readme(&text, &lines)?)?;
    Ok(lines)
}
