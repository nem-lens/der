// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//! The headline figures at the top of the README.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};

use crate::cer::Cer;
use crate::regions::{region_name, state_region};
use crate::rooftop::Rooftop;
use crate::tables::batteries;

/// `x` rounded to `decimals`, with thousands separators: 1234567.8 -> "1,234,568".
pub fn commas(x: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, x.abs());
    let (int, frac) = s
        .split_once('.')
        .map_or((s.as_str(), None), |(i, f)| (i, Some(f)));
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    if x < 0.0 && out.chars().any(|c| c.is_ascii_digit() && c != '0') {
        out.insert(0, '-');
    }
    out
}

fn months_back(m: NaiveDate, n: i32) -> NaiveDate {
    let k = m.year() * 12 + m.month0() as i32 - n;
    NaiveDate::from_ymd_opt(k.div_euclid(12), k.rem_euclid(12) as u32 + 1, 1).unwrap()
}

/// The headline lines, as Markdown bullets.
pub fn lines(cer: &Cer, rooftop: &Rooftop) -> Result<Vec<String>> {
    let b = batteries(cer);
    let mut per_month: BTreeMap<NaiveDate, (f64, f64)> = BTreeMap::new();
    let mut by_region: BTreeMap<&str, f64> = BTreeMap::new();
    let (mut nat_n, mut nat_kwh, mut nem_n, mut nem_kwh) = (0.0, 0.0, 0.0, 0.0);
    for (&(month, state), f) in &b {
        let e = per_month.entry(month).or_default();
        e.0 += f.installs;
        e.1 += f.size;
        nat_n += f.installs;
        nat_kwh += f.size;
        if let Some(r) = state_region(state) {
            nem_n += f.installs;
            nem_kwh += f.size;
            *by_region.entry(r).or_default() += f.size;
        }
    }
    let (&first, _) = per_month.first_key_value().context("no battery months")?;
    let (&latest, _) = per_month.last_key_value().unwrap();
    let (big, &(big_n, big_kwh)) = per_month
        .iter()
        .fold(
            None,
            |acc: Option<(&NaiveDate, &(f64, f64))>, (m, v)| match acc {
                Some((_, a)) if a.0 >= v.0 => acc,
                _ => Some((m, v)),
            },
        )
        .unwrap();
    let size: Vec<(NaiveDate, f64)> = per_month.iter().map(|(m, (n, k))| (*m, k / n)).collect();
    let (size_max_at, size_max) =
        size.iter().fold(
            (first, f64::MIN),
            |acc, &(m, s)| if s > acc.1 { (m, s) } else { acc },
        );
    let mut shares: Vec<(&str, f64)> = by_region
        .into_iter()
        .map(|(r, k)| (r, k / nem_kwh))
        .collect();
    shares.sort_by(|a, b| b.1.total_cmp(&a.1));
    let vol: f64 = cer.voluntary.values().sum();

    let solar_gw = cer.solar.values().map(|f| f.size).sum::<f64>() / 1e6;
    let cutoff = months_back(latest, 12);
    let last12 = cer
        .solar
        .iter()
        .filter(|((m, _), _)| *m > cutoff)
        .map(|(_, f)| f.size)
        .sum::<f64>()
        / 1e6;

    let mut nem_mean: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for (&(m, _), v) in &rooftop.by_region {
        *nem_mean.entry(m).or_default() += v.mean_mw;
    }
    let means: Vec<f64> = nem_mean.values().copied().collect();
    let n = means.len();
    anyhow::ensure!(n >= 12, "fewer than 12 complete rooftop PV months");
    let recent = means[n - 12..].iter().sum::<f64>() / 12.0;
    let earlier = (n >= 72).then(|| means[n - 72..n - 60].iter().sum::<f64>() / 12.0);
    let rec = rooftop
        .peaks
        .values()
        .fold(None, |acc: Option<&crate::rooftop::Peak>, p| match acc {
            Some(a) if a.mw >= p.mw => acc,
            _ => Some(p),
        })
        .context("no rooftop PV peaks")?;

    let span = format!("{} to {}", first.format("%B %Y"), latest.format("%B %Y"));
    let where_ = shares
        .iter()
        .map(|(r, v)| format!("{} {:.0}%", region_name(r), 100.0 * v))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(vec![
        format!(
            "- **Home batteries:** {} installed with {} GWh of usable storage, {span} (Cheaper Home Batteries Program). \
             In the NEM's five regions: {} batteries, {} GWh. The voluntary record for September 2014 to June 2025 counts {}.",
            commas(nat_n, 0),
            commas(nat_kwh / 1e6, 1),
            commas(nem_n, 0),
            commas(nem_kwh / 1e6, 1),
            commas(vol, 0)
        ),
        format!("- **Where:** {where_} of the NEM's new battery storage."),
        format!(
            "- **Busiest month:** {}, {} batteries ({} MWh). Average size {:.1} kWh in {}, {:.1} kWh at its largest ({}), {:.1} kWh in {}.",
            big.format("%B %Y"),
            commas(big_n, 0),
            commas(big_kwh / 1e3, 0),
            size[0].1,
            first.format("%b %Y"),
            size_max,
            size_max_at.format("%b %Y"),
            size[size.len() - 1].1,
            latest.format("%b %Y")
        ),
        format!(
            "- **Small-scale solar:** {} GW registered with the CER nationally, {} GW of it in the last 12 months.",
            commas(solar_gw, 1),
            commas(last12, 1)
        ),
        format!(
            "- **Rooftop PV output (AEMO):** a NEM record of {} GW in the half hour ending {}. Mean output over the last 12 months {} GW{}",
            commas(rec.mw / 1e3, 1),
            rec.at.format("%H:%M on %d %B %Y"),
            commas(recent / 1e3, 2),
            earlier.map_or(".".to_string(), |e| format!(", {:.1}× five years earlier.", recent / e))
        ),
        "- Recent months are incomplete: CER counts a system once its certificates are created, which can be up to a year after installation."
            .to_string(),
    ])
}

/// Replace the block between the README's headline markers.
pub fn into_readme(readme: &str, lines: &[String]) -> Result<String> {
    const START: &str = "<!-- headline:start -->";
    const END: &str = "<!-- headline:end -->";
    let a = readme.find(START).context("no headline:start marker")?;
    let b = readme[a..].find(END).context("no headline:end marker")? + a;
    Ok(format!(
        "{}{START}\n{}\n{}",
        &readme[..a],
        lines.join("\n"),
        &readme[b..]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands() {
        assert_eq!(commas(546789.0, 0), "546,789");
        assert_eq!(commas(15.34, 1), "15.3");
        assert_eq!(commas(999.6, 0), "1,000");
        assert_eq!(commas(3.564, 2), "3.56");
        assert_eq!(commas(-1234.0, 0), "-1,234");
    }

    #[test]
    fn readme_block() {
        let r = "a\n<!-- headline:start -->\nold\n<!-- headline:end -->\nb\n";
        let out = into_readme(r, &["- new".into()]).unwrap();
        assert_eq!(
            out,
            "a\n<!-- headline:start -->\n- new\n<!-- headline:end -->\nb\n"
        );
    }

    #[test]
    fn twelve_months_back() {
        let m = |y, mo| NaiveDate::from_ymd_opt(y, mo, 1).unwrap();
        assert_eq!(months_back(m(2026, 9), 12), m(2025, 9));
        assert_eq!(months_back(m(2026, 1), 1), m(2025, 12));
    }
}
