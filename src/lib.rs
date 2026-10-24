// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//! Household batteries and rooftop solar in Australia's National Electricity
//! Market, by region and month.
//!
//! - [`cer`]: the Clean Energy Regulator's small-scale installation data
//!   (batteries and solar by postcode and month) summed to states.
//! - [`rooftop`]: AEMO's rooftop PV estimates, read with the shared
//!   `nem-lens-aemo` parsers, by region and month.
//! - [`tables`], [`headline`] and [`svg`]: the CSVs, README figures and
//!   charts that `der build` writes.
//! - [`fetch`]: the downloads behind them, with checksums.
//!
//! CER material is licensed under Creative Commons Attribution 4.0. AEMO's
//! data is © AEMO and used under AEMO's copyright permissions.

pub mod build;
pub mod cer;
pub mod fetch;
pub mod headline;
pub mod regions;
pub mod rooftop;
pub mod svg;
pub mod tables;
