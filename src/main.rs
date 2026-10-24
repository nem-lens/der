// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//! `der`: fetch CER and AEMO data and build the tables, charts and headline
//! figures.

use std::path::PathBuf;

use anyhow::Result;
use chrono::{Datelike, NaiveDate, Utc};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Household batteries and rooftop solar in the NEM")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Download the CER's small-scale installation files into OUT/DATE/.
    FetchCer {
        #[arg(long, default_value = "raw/cer")]
        out: PathBuf,
    },
    /// Download AEMO's ROOFTOP_PV_ACTUAL archive by month, skipping months
    /// already downloaded and months not yet published.
    FetchAemo {
        /// First month, YYYY-MM.
        #[arg(long, default_value = "2018-01", value_parser = month)]
        from: NaiveDate,
        /// Last month, YYYY-MM (default: this month).
        #[arg(long, value_parser = month)]
        to: Option<NaiveDate>,
        #[arg(long, default_value = "raw/aemo")]
        out: PathBuf,
    },
    /// Write data/, charts/ and the README's headline figures from raw/.
    Build {
        #[arg(long, default_value = "raw")]
        raw: PathBuf,
        /// The repository root holding README.md.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
}

fn month(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d")
        .map_err(|_| format!("expected YYYY-MM, got {s:?}"))
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::FetchCer { out } => nem_lens_der::fetch::cer(&out),
        Cmd::FetchAemo { from, to, out } => {
            let today = Utc::now().date_naive();
            let to = to.unwrap_or_else(|| {
                NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap()
            });
            nem_lens_der::fetch::aemo(&out, from, to)
        }
        Cmd::Build { raw, root } => {
            for line in nem_lens_der::build::run(&raw, &root)? {
                println!("{line}");
            }
            Ok(())
        }
    }
}
