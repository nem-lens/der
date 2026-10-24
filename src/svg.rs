// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//! Static SVG charts for the README: stacked areas and bars by region, and a
//! line.
//!
//! Region hues are in a fixed order and were checked for colour-vision
//! deficiency; three sit below 3:1 contrast on the surface, so the area
//! charts label bands directly, every chart has a legend, and the data are
//! in `data/` as CSV. Each mark carries a `<title>` for hover where the SVG
//! is opened directly.

use std::fmt::Write as _;

use chrono::{Datelike, NaiveDate};

use crate::regions::{region_name, REGIONS};

const COLOURS: [&str; 5] = ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4"];
const SURFACE: &str = "#fcfcfb";
const INK: &str = "#0b0b0b";
const INK2: &str = "#52514e";
const MUTED: &str = "#898781";
const GRID: &str = "#e1e0d9";
const AXIS: &str = "#c3c2b7";
const FONT: &str = "system-ui,-apple-system,'Segoe UI',Roboto,Helvetica,Arial,sans-serif";

const W: f64 = 900.0;
const H: f64 = 460.0;
const LEFT: f64 = 64.0;
const TOP: f64 = 80.0;
const BOTTOM: f64 = H - 96.0;

/// Titles, units and the source line shared by every chart.
pub struct Labels<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    pub unit: &'a str,
    pub note: &'a str,
    /// Formats axis ticks and labels.
    pub fmt: &'a dyn Fn(f64) -> String,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A round step giving about five intervals up to `max`.
fn nice_step(max: f64) -> f64 {
    let raw = (max / 5.0).max(f64::MIN_POSITIVE);
    let mag = 10f64.powf(raw.log10().floor());
    let f = raw / mag;
    mag * if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 2.5 {
        2.5
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    }
}

struct Frame {
    out: String,
    right: f64,
    ymax: f64,
}

impl Frame {
    fn new(l: &Labels, right_margin: f64, max: f64) -> Frame {
        let step = nice_step(max);
        let ymax = (max / step).ceil().max(1.0) * step;
        let right = W - right_margin;
        let mut out = String::new();
        let _ = write!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" role="img" aria-labelledby="t d" font-family="{FONT}">"#
        );
        let _ = write!(
            out,
            r#"<title id="t">{}</title><desc id="d">{}</desc>"#,
            esc(l.title),
            esc(l.subtitle)
        );
        let _ = write!(out, r#"<rect width="{W}" height="{H}" fill="{SURFACE}"/>"#);
        let _ = write!(
            out,
            r#"<text x="{LEFT}" y="30" font-size="17" font-weight="700" fill="{INK}">{}</text>"#,
            esc(l.title)
        );
        let _ = write!(
            out,
            r#"<text x="{LEFT}" y="53" font-size="13" fill="{INK2}">{}</text>"#,
            esc(l.subtitle)
        );
        let mut v = 0.0;
        while v <= ymax + step * 1e-9 {
            let y = BOTTOM - (BOTTOM - TOP) * v / ymax;
            if v > 0.0 {
                let _ = write!(
                    out,
                    r#"<line x1="{LEFT}" x2="{right}" y1="{y:.1}" y2="{y:.1}" stroke="{GRID}" stroke-width="0.8"/>"#
                );
            }
            let _ = write!(
                out,
                r#"<text x="{}" y="{:.1}" font-size="12" fill="{MUTED}" text-anchor="end">{}</text>"#,
                LEFT - 8.0,
                y + 4.0,
                esc(&(l.fmt)(v))
            );
            v += step;
        }
        let _ = write!(
            out,
            r#"<text transform="translate(16 {:.1}) rotate(-90)" font-size="12" fill="{INK2}" text-anchor="middle">{}</text>"#,
            (TOP + BOTTOM) / 2.0,
            esc(l.unit)
        );
        Frame { out, right, ymax }
    }

    fn y(&self, v: f64) -> f64 {
        BOTTOM - (BOTTOM - TOP) * v / self.ymax
    }

    fn x_label(&mut self, x: f64, text: &str) {
        let _ = write!(
            self.out,
            r#"<text x="{x:.1}" y="{}" font-size="12" fill="{MUTED}" text-anchor="middle">{}</text>"#,
            BOTTOM + 20.0,
            esc(text)
        );
    }

    fn finish(mut self, l: &Labels, legend: bool) -> String {
        let _ = write!(
            self.out,
            r#"<line x1="{LEFT}" x2="{}" y1="{BOTTOM}" y2="{BOTTOM}" stroke="{AXIS}" stroke-width="1"/>"#,
            self.right
        );
        if legend {
            let mut x = LEFT;
            for (i, r) in REGIONS.iter().enumerate() {
                let name = region_name(r);
                let _ = write!(
                    self.out,
                    r#"<rect x="{x}" y="{}" width="12" height="12" rx="2" fill="{}"/><text x="{}" y="{}" font-size="13" fill="{INK2}">{}</text>"#,
                    BOTTOM + 40.0,
                    COLOURS[i],
                    x + 18.0,
                    BOTTOM + 51.0,
                    esc(name)
                );
                x += 30.0 + 8.0 * name.len() as f64;
            }
        }
        let _ = write!(
            self.out,
            r#"<text x="8" y="{}" font-size="11" fill="{MUTED}">{}</text>"#,
            H - 10.0,
            esc(l.note)
        );
        self.out.push_str("</svg>\n");
        self.out
    }
}

/// Tick positions for a monthly series: every k months as "Jul 25" when it
/// spans under three years, otherwise Januaries as years.
fn month_ticks(x: &[NaiveDate]) -> Vec<(usize, String)> {
    let span_years = (x[x.len() - 1] - x[0]).num_days() as f64 / 365.25;
    if span_years < 3.0 {
        let k = x.len().div_ceil(12).max(1);
        (0..x.len())
            .step_by(k)
            .map(|i| (i, x[i].format("%b %y").to_string()))
            .collect()
    } else {
        let every = (span_years / 10.0).ceil().max(1.0) as i32;
        x.iter()
            .enumerate()
            .filter(|(_, d)| d.month() == 1 && d.year() % every == 0)
            .map(|(i, d)| (i, d.year().to_string()))
            .collect()
    }
}

/// Values stacked over regions, one area per region in [`REGIONS`] order,
/// with the latest value of each wide band labelled at the right.
pub fn stacked_area(x: &[NaiveDate], ys: &[[f64; 5]], l: &Labels) -> String {
    let totals: Vec<f64> = ys.iter().map(|v| v.iter().sum()).collect();
    let mut f = Frame::new(l, 160.0, totals.iter().copied().fold(0.0, f64::max));
    let n = x.len();
    let right = f.right;
    let px = move |i: usize| LEFT + (right - LEFT) * i as f64 / (n - 1).max(1) as f64;
    let mut lower = vec![0.0; n];
    for (r, colour) in COLOURS.iter().enumerate() {
        let upper: Vec<f64> = (0..n).map(|i| lower[i] + ys[i][r]).collect();
        let mut pts = String::new();
        for (i, u) in upper.iter().enumerate() {
            let _ = write!(pts, "{:.1},{:.1} ", px(i), f.y(*u));
        }
        for i in (0..n).rev() {
            let _ = write!(pts, "{:.1},{:.1} ", px(i), f.y(lower[i]));
        }
        let _ = write!(
            f.out,
            r#"<polygon points="{}" fill="{colour}" stroke="{SURFACE}" stroke-width="1" stroke-linejoin="round"><title>{}: {} at {}</title></polygon>"#,
            pts.trim_end(),
            region_name(REGIONS[r]),
            esc(&(l.fmt)(ys[n - 1][r])),
            x[n - 1].format("%b %Y")
        );
        lower = upper;
    }
    let total = totals[n - 1];
    let mut base = 0.0;
    for (r, region) in REGIONS.iter().enumerate() {
        let v = ys[n - 1][r];
        if v >= 0.06 * total {
            let _ = write!(
                f.out,
                r#"<text x="{:.1}" y="{:.1}" font-size="12" fill="{INK2}">{}  {}</text>"#,
                f.right + 8.0,
                f.y(base + v / 2.0) + 4.0,
                region_name(region),
                esc(&(l.fmt)(v))
            );
        }
        base += v;
    }
    let _ = write!(
        f.out,
        r#"<text x="{:.1}" y="{:.1}" font-size="13" font-weight="700" fill="{INK}">Total {}</text>"#,
        f.right + 8.0,
        f.y(total) - 6.0,
        esc(&(l.fmt)(total))
    );
    for (i, label) in month_ticks(x) {
        f.x_label(px(i), &label);
    }
    f.finish(l, true)
}

/// One stacked bar per label; the last `faded` bars are drawn lighter.
pub fn stacked_bars(labels: &[String], ys: &[[f64; 5]], faded: usize, l: &Labels) -> String {
    let totals: Vec<f64> = ys.iter().map(|v| v.iter().sum()).collect();
    let mut f = Frame::new(l, 24.0, totals.iter().copied().fold(0.0, f64::max));
    let n = labels.len();
    let band = (f.right - LEFT) / n as f64;
    let w = band * 0.82;
    for (i, v) in ys.iter().enumerate() {
        let x = LEFT + band * i as f64 + (band - w) / 2.0;
        let opacity = if i + faded >= n {
            r#" fill-opacity="0.45""#
        } else {
            ""
        };
        let mut base = 0.0;
        let _ = write!(
            f.out,
            "<g><title>{}: total {}",
            esc(&labels[i]),
            esc(&(l.fmt)(totals[i]))
        );
        for (r, region) in REGIONS.iter().enumerate() {
            let _ = write!(f.out, ", {} {}", region_name(region), esc(&(l.fmt)(v[r])));
        }
        f.out.push_str("</title>");
        for (r, colour) in COLOURS.iter().enumerate() {
            let (y0, y1) = (f.y(base), f.y(base + v[r]));
            if v[r] > 0.0 {
                let _ = write!(
                    f.out,
                    r#"<rect x="{x:.1}" y="{y1:.1}" width="{w:.1}" height="{:.1}" fill="{colour}"{opacity} stroke="{SURFACE}" stroke-width="0.8"/>"#,
                    y0 - y1
                );
            }
            base += v[r];
        }
        f.out.push_str("</g>");
    }
    let step = n.div_ceil(12).max(1);
    for i in (0..n).step_by(step) {
        f.x_label(LEFT + band * (i as f64 + 0.5), &labels[i]);
    }
    f.finish(l, true)
}

/// One line over months, with its highest point marked.
pub fn line(x: &[NaiveDate], y: &[f64], l: &Labels) -> String {
    let max = y.iter().copied().fold(f64::MIN, f64::max);
    let mut f = Frame::new(l, 24.0, max);
    let n = x.len();
    let right = f.right;
    let px = move |i: usize| LEFT + (right - LEFT) * i as f64 / (n - 1).max(1) as f64;
    let mut d = String::new();
    for (i, v) in y.iter().enumerate() {
        let _ = write!(
            d,
            "{}{:.1},{:.1}",
            if i == 0 { "M" } else { " L" },
            px(i),
            f.y(*v)
        );
    }
    let _ = write!(
        f.out,
        r#"<path d="{d}" fill="none" stroke="{}" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>"#,
        COLOURS[0]
    );
    for i in 0..n {
        let _ = write!(
            f.out,
            r#"<circle cx="{:.1}" cy="{:.1}" r="6" fill="transparent"><title>{}: {}</title></circle>"#,
            px(i),
            f.y(y[i]),
            x[i].format("%b %Y"),
            esc(&(l.fmt)(y[i]))
        );
    }
    let top = y.iter().position(|v| *v == max).unwrap_or(0);
    let (cx, cy) = (px(top), f.y(max));
    let _ = write!(
        f.out,
        r#"<circle cx="{cx:.1}" cy="{cy:.1}" r="4.5" fill="{}" stroke="{SURFACE}" stroke-width="2"/><text x="{:.1}" y="{:.1}" font-size="13" fill="{INK}" text-anchor="end">Record {}</text>"#,
        COLOURS[0],
        cx - 8.0,
        cy - 8.0,
        esc(&(l.fmt)(max))
    );
    for (i, label) in month_ticks(x) {
        f.x_label(px(i), &label);
    }
    f.finish(l, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_round() {
        assert_eq!(nice_step(16.0), 5.0);
        assert_eq!(nice_step(80.0), 20.0);
        assert_eq!(nice_step(11.0), 2.5);
        assert_eq!(nice_step(0.9), 0.2);
    }

    #[test]
    fn charts_are_well_formed() {
        let fmt = |v: f64| format!("{v:.1}");
        let l = Labels {
            title: "T & co",
            subtitle: "S",
            unit: "GW",
            note: "N",
            fmt: &fmt,
        };
        let x: Vec<NaiveDate> = (1..=12)
            .map(|m| NaiveDate::from_ymd_opt(2026, m, 1).unwrap())
            .collect();
        let ys: Vec<[f64; 5]> = (0..12).map(|i| [i as f64, 1.0, 2.0, 0.5, 0.1]).collect();
        for svg in [
            stacked_area(&x, &ys, &l),
            stacked_bars(
                &x.iter()
                    .map(|d| d.format("%b").to_string())
                    .collect::<Vec<_>>(),
                &ys,
                2,
                &l,
            ),
            line(&x, &ys.iter().map(|v| v[0]).collect::<Vec<_>>(), &l),
        ] {
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>\n"));
            assert!(svg.contains("T &amp; co"));
            assert!(!svg.contains("NaN"));
        }
    }
}
