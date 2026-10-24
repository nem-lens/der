// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//! Downloads from the CER and AEMO, each recorded with its URL, size,
//! SHA-256 and fetch time in a `sources.json` next to the files.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate, Utc};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::cer;

const USER_AGENT: &str = "nem-lens-der/0.1 (+https://github.com/nem-lens/der)";
const CER_BASE: &str = "https://cer.gov.au/document/";
const ARCHIVE: &str = "https://nemweb.com.au/Data_Archive/Wholesale_Electricity/MMSDM";
const TABLE: &str = "ROOFTOP_PV_ACTUAL";

fn client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(300))
        .build()?)
}

fn record(url: &str, body: &[u8]) -> Value {
    json!({
        "url": url,
        "bytes": body.len(),
        "sha256": hex::encode(Sha256::digest(body)),
        "fetched": Utc::now().format("%Y-%m-%dT%H:%M:%S+00:00").to_string(),
    })
}

fn read_sources(path: &Path) -> Result<Map<String, Value>> {
    if !path.exists() {
        return Ok(Map::new());
    }
    serde_json::from_slice(&std::fs::read(path)?)
        .with_context(|| format!("reading {}", path.display()))
}

/// The five CER files into `<out>/<today>/`. CER content is licensed under
/// Creative Commons Attribution 4.0.
pub fn cer(out: &Path) -> Result<()> {
    let dest = out.join(Utc::now().date_naive().to_string());
    std::fs::create_dir_all(&dest)?;
    let http = client()?;
    let mut sources = Map::new();
    for name in [
        cer::BATTERY_INSTALLS,
        cer::BATTERY_CAPACITY,
        cer::SOLAR_INSTALLS,
        cer::SOLAR_CAPACITY,
        cer::VOLUNTARY,
    ] {
        let url = format!("{CER_BASE}{}", name.trim_end_matches(".csv"));
        let body = http.get(&url).send()?.error_for_status()?.bytes()?;
        std::fs::write(dest.join(name), &body)?;
        sources.insert(name.to_string(), record(&url, &body));
        println!("{name}: {} bytes", body.len());
    }
    std::fs::write(
        dest.join("sources.json"),
        serde_json::to_string_pretty(&sources)?,
    )?;
    println!("wrote {}", dest.display());
    Ok(())
}

/// The archive's file names for a month: `PUBLIC_ARCHIVE#...#FILE01#` from
/// August 2024, `PUBLIC_DVD_...` before.
pub fn archive_urls(month: NaiveDate) -> [String; 2] {
    let (y, m) = (month.year(), month.month());
    let stamp = format!("{y}{m:02}010000");
    let base = format!("{ARCHIVE}/{y}/MMSDM_{y}_{m:02}/MMSDM_Historical_Data_SQLLoader/DATA/");
    [
        format!("{base}PUBLIC_ARCHIVE%23{TABLE}%23FILE01%23{stamp}.zip"),
        format!("{base}PUBLIC_DVD_{TABLE}_{stamp}.zip"),
    ]
}

/// AEMO's rooftop PV archive, one zip per month from `from` to `to`, into
/// `<out>/ROOFTOP_PV_ACTUAL/<YYYY_MM>.zip`. Months already there are
/// skipped; months AEMO has not archived yet are reported and skipped.
pub fn aemo(out: &Path, from: NaiveDate, to: NaiveDate) -> Result<()> {
    let dest = out.join(TABLE);
    std::fs::create_dir_all(&dest)?;
    let manifest = dest.join("sources.json");
    let mut sources = read_sources(&manifest)?;
    let http = client()?;
    let mut month = from.with_day(1).context("bad --from")?;
    while month <= to {
        let name = format!("{}_{:02}.zip", month.year(), month.month());
        let path = dest.join(&name);
        if !path.exists() {
            let mut got = None;
            for url in archive_urls(month) {
                let resp = http.get(&url).send()?;
                if resp.status() == StatusCode::NOT_FOUND {
                    continue;
                }
                got = Some((url, resp.error_for_status()?.bytes()?));
                break;
            }
            match got {
                None => println!("{}: not published", month.format("%Y-%m")),
                Some((url, body)) => {
                    std::fs::write(&path, &body)?;
                    sources.insert(name, record(&url, &body));
                    std::fs::write(&manifest, serde_json::to_string_pretty(&sources)?)?;
                    println!("{}: {} bytes", month.format("%Y-%m"), body.len());
                }
            }
        }
        month = month.checked_add_months(chrono::Months::new(1)).unwrap();
    }
    Ok(())
}
