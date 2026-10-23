"""Build the DER tables, charts and headline figures from the raw downloads.

    python build.py [--raw raw]

Reads the latest raw/cer/<date>/ and every raw/aemo/ROOFTOP_PV_ACTUAL/*.zip,
and writes:
- data/*.csv: tables by state or NEM region and month;
- charts/*.png: the charts in the README;
- the README's headline block, between its <!-- headline --> markers.
"""

import argparse
import io
import json
import re
import sys
import zipfile
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.dates as mdates  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import pandas as pd  # noqa: E402
from matplotlib.ticker import FuncFormatter  # noqa: E402

from regions import REGION_NAMES, REGIONS, STATE_REGION, STATES, state_of  # noqa: E402

HERE = Path(__file__).resolve().parent

# Chart tokens (light theme). Region hues in fixed order, validated for
# colour-vision deficiency; three sit below 3:1 on the surface, so every
# chart labels bands directly and the data ship as CSV.
COLOURS = dict(zip(REGIONS, ["#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4"]))
SURFACE, INK, INK2, MUTED, GRID, AXIS = "#fcfcfb", "#0b0b0b", "#52514e", "#898781", "#e1e0d9", "#c3c2b7"


# ---------------------------------------------------------------- CER


def num(s: pd.Series) -> pd.Series:
    return pd.to_numeric(s.astype(str).str.replace(",", ""), errors="coerce").fillna(0.0)


def postcode_monthly(path: Path, suffix: str, value: str) -> pd.DataFrame:
    """A CER postcode file to (state, month, value); the pre-2011 historic
    total, where present, becomes the month 2010-12."""
    df = pd.read_csv(path, dtype=str)
    pc = df.columns[0]
    df = df[df[pc].str.fullmatch(r"\d+", na=False)].copy()
    df["state"] = df[pc].astype(int).map(state_of)
    df = df.dropna(subset=["state"])
    monthly = [c for c in df.columns if c.endswith(suffix) and re.match(r"[A-Z][a-z]{2} \d{4}", c)]
    long = df.melt(id_vars="state", value_vars=monthly, var_name="col", value_name=value)
    long["month"] = pd.to_datetime(long.col.str.replace(suffix, "", regex=False).str.strip(), format="%b %Y")
    long[value] = num(long[value])
    out = long.groupby(["state", "month"])[value].sum().reset_index()
    hist = [c for c in df.columns if c.startswith("Historic Total")]
    if hist:
        h = df.assign(v=num(df[hist[0]])).groupby("state").v.sum().reset_index()
        h["month"] = pd.Timestamp("2010-12-01")
        out = pd.concat([h.rename(columns={"v": value}), out])
    return out


def with_region(df: pd.DataFrame) -> pd.DataFrame:
    df = df.copy()
    df["region"] = df.state.map(STATE_REGION)
    return df


def cer_tables(raw: Path) -> dict[str, pd.DataFrame]:
    n = postcode_monthly(raw / "sgu-battery-installations-2011-to-present-and-totals.csv", " - Installation Quantity", "installs")
    k = postcode_monthly(raw / "sgu-battery-capacity-2011-to-present-and-totals.csv", " - Usable capacity in kWh", "usable_kwh")
    batteries = with_region(n.merge(k, on=["state", "month"], how="outer").fillna(0.0))

    sn = postcode_monthly(raw / "sgu-solar-installations-2011-to-present-and-totals.csv", " - Installation Quantity", "installs")
    sk = postcode_monthly(raw / "sgu-solar-capacity-2011-to-present-and-totals.csv", " - Rated Power Output in kW", "rated_kw")
    solar = with_region(sn.merge(sk, on=["state", "month"], how="outer").fillna(0.0))

    v = pd.read_csv(raw / "historical-voluntary-state-data-battery-installation-small-scale-systems.csv", dtype=str)
    v = v.melt(id_vars="Installation year", value_vars=[s for s in STATES if s in v.columns], var_name="state", value_name="installs")
    v["year"] = v["Installation year"].astype(int)
    v["installs"] = num(v.installs)
    voluntary = with_region(v[["state", "year", "installs"]])
    return {"batteries": batteries, "solar": solar, "voluntary": voluntary}


# ---------------------------------------------------------------- AEMO


def rooftop_month(path: Path) -> pd.DataFrame:
    with zipfile.ZipFile(path) as z:
        with z.open(z.namelist()[0]) as h:
            text = io.TextIOWrapper(h, "utf-8")
            text.readline()
            header = text.readline().rstrip("\n").split(",")
            df = pd.read_csv(text, header=None, names=header, low_memory=False)
    df = df[df.iloc[:, 0] == "D"]
    df = df[df.REGIONID.isin(REGIONS)]
    df["t"] = pd.to_datetime(df.INTERVAL_DATETIME, format="%Y/%m/%d %H:%M:%S")
    df["POWER"] = pd.to_numeric(df.POWER, errors="coerce")

    df["rank"] = (df.TYPE != "MEASUREMENT").astype(int)
    return df[["t", "REGIONID", "POWER", "TYPE", "rank", "LASTCHANGED"]]


def one_estimate(df: pd.DataFrame) -> pd.DataFrame:
    """One value per half hour and region: MEASUREMENT where AEMO has it,
    otherwise SATELLITE, each at its latest revision. Monthly archive
    files overlap at their edges, so this runs on all months together."""
    df = df.sort_values(["t", "REGIONID", "rank", "LASTCHANGED"])
    df = df.drop_duplicates(["t", "REGIONID", "rank"], keep="last")
    return df.drop_duplicates(["t", "REGIONID"], keep="first")


def rooftop_tables(folder: Path) -> tuple[pd.DataFrame, pd.DataFrame]:
    half = one_estimate(pd.concat(rooftop_month(p) for p in sorted(folder.glob("*.zip"))))
    # A half hour belongs to the month it starts in; keep only months with
    # at least 90% of their half hours in every region.
    half["month"] = (half.t - pd.Timedelta(minutes=30)).dt.to_period("M").dt.to_timestamp()
    counts = half.groupby(["month", "REGIONID"]).size().unstack()
    expected = pd.Series(counts.index.days_in_month * 48, index=counts.index)
    complete = counts.index[(counts.min(axis=1) >= 0.9 * expected) & (counts.shape[1] == len(REGIONS))]
    half = half[half.month.isin(complete)]
    by_region = (
        half.groupby(["REGIONID", "month"])
        .agg(mean_mw=("POWER", "mean"), peak_mw=("POWER", "max"), energy_gwh=("POWER", lambda p: p.sum() * 0.5 / 1000), measured_share=("TYPE", lambda s: (s == "MEASUREMENT").mean()))
        .reset_index()
        .rename(columns={"REGIONID": "region"})
    )
    nem = half.groupby(["t", "month"]).POWER.sum().rename("nem_mw").reset_index()
    idx = nem.groupby("month").nem_mw.idxmax()
    peaks = nem.loc[idx, ["month", "t", "nem_mw"]].rename(columns={"t": "peak_at", "nem_mw": "nem_peak_mw"})
    return by_region, peaks.reset_index(drop=True)


# ---------------------------------------------------------------- charts


def style(ax, title: str, subtitle: str, ylabel: str) -> None:
    ax.set_facecolor(SURFACE)
    for side in ["top", "right", "left"]:
        ax.spines[side].set_visible(False)
    ax.spines["bottom"].set_color(AXIS)
    ax.tick_params(colors=MUTED, labelsize=9, length=0)
    ax.yaxis.grid(True, color=GRID, linewidth=0.6)
    ax.set_axisbelow(True)
    ax.set_ylabel(ylabel, color=INK2, fontsize=9)
    ax.set_title(title, loc="left", color=INK, fontsize=12, fontweight="bold", pad=26)
    ax.text(0, 1.035, subtitle, transform=ax.transAxes, color=INK2, fontsize=9, va="bottom")


def legend(ax) -> None:
    handles = [plt.Rectangle((0, 0), 1, 1, color=COLOURS[r]) for r in REGIONS]
    ax.legend(handles, [REGION_NAMES[r] for r in REGIONS], ncol=5, loc="upper left", bbox_to_anchor=(0, -0.08), frameon=False, fontsize=9, labelcolor=INK2, handlelength=1, handleheight=1)


def figure():
    fig, ax = plt.subplots(figsize=(9, 4.6), dpi=150)
    fig.patch.set_facecolor(SURFACE)
    return fig, ax


def save(fig, path: Path, note: str) -> None:
    fig.text(0.01, 0.01, note, color=MUTED, fontsize=7.5, ha="left", va="bottom")
    fig.tight_layout(rect=(0, 0.04, 1, 1))
    fig.savefig(path, facecolor=SURFACE)
    plt.close(fig)


def stacked_area(wide: pd.DataFrame, path: Path, title: str, subtitle: str, ylabel: str, fmt, note: str) -> None:
    fig, ax = figure()
    x = wide.index
    ys = [wide[r].to_numpy() for r in REGIONS]
    ax.stackplot(x, ys, colors=[COLOURS[r] for r in REGIONS], edgecolor=SURFACE, linewidth=1.0)
    total = wide[REGIONS].sum(axis=1)
    # Direct labels at the right end, for bands thick enough to hold one.
    base = 0.0
    last = wide[REGIONS].iloc[-1]
    for r in REGIONS:
        v = last[r]
        if v >= 0.06 * total.iloc[-1]:
            ax.annotate(f"{REGION_NAMES[r]}  {fmt(v)}", (x[-1], base + v / 2), xytext=(6, 0), textcoords="offset points", va="center", fontsize=8.5, color=INK2)
        base += v
    ax.annotate(f"Total {fmt(total.iloc[-1])}", (x[-1], total.iloc[-1]), xytext=(6, 4), textcoords="offset points", va="bottom", fontsize=9, color=INK, fontweight="bold")
    ax.set_xlim(x[0], x[-1])
    ax.margins(x=0)
    ax.yaxis.set_major_formatter(FuncFormatter(lambda v, _: fmt(v)))
    ax.xaxis.set_major_formatter(mdates.DateFormatter("%b %y" if (x[-1] - x[0]).days < 3 * 365 else "%Y"))
    style(ax, title, subtitle, ylabel)
    legend(ax)
    fig.subplots_adjust(right=0.82)
    save(fig, path, note)


def stacked_bars(wide: pd.DataFrame, path: Path, title: str, subtitle: str, ylabel: str, fmt, note: str, labels=None, incomplete: int = 0) -> None:
    fig, ax = figure()
    x = np.arange(len(wide))
    bottom = np.zeros(len(wide))
    for r in REGIONS:
        v = wide[r].to_numpy()
        bars = ax.bar(x, v, bottom=bottom, color=COLOURS[r], edgecolor=SURFACE, linewidth=0.8, width=0.82)
        if incomplete:
            for b in bars[-incomplete:]:
                b.set_alpha(0.45)
        bottom += v

    labels = labels if labels is not None else [d.strftime("%b %y") for d in wide.index]
    step = max(1, len(x) // 12)
    ax.set_xticks(x[::step])
    ax.set_xticklabels(labels[::step])
    ax.yaxis.set_major_formatter(FuncFormatter(lambda v, _: fmt(v)))
    style(ax, title, subtitle, ylabel)
    legend(ax)
    save(fig, path, note)


def line(series: pd.Series, path: Path, title: str, subtitle: str, ylabel: str, fmt, note: str) -> None:
    fig, ax = figure()
    ax.plot(series.index, series.to_numpy(), color=COLOURS["NSW1"], linewidth=2)
    i = series.idxmax()
    ax.plot([i], [series[i]], "o", color=COLOURS["NSW1"], markersize=6, markeredgecolor=SURFACE, markeredgewidth=1.5)
    ax.annotate(f"Record {fmt(series[i])}", (i, series[i]), xytext=(-8, 6), textcoords="offset points", ha="right", fontsize=9, color=INK)
    ax.yaxis.set_major_formatter(FuncFormatter(lambda v, _: fmt(v)))
    ax.set_xlim(series.index[0], series.index[-1])
    ax.xaxis.set_major_formatter(mdates.DateFormatter("%Y"))
    style(ax, title, subtitle, ylabel)
    save(fig, path, note)


# ---------------------------------------------------------------- headline


def headline(cer: dict[str, pd.DataFrame], rooftop: pd.DataFrame, peaks: pd.DataFrame) -> list[str]:
    b = cer["batteries"]
    b = b[b.month >= "2025-07-01"]
    months = sorted(b.month.unique())
    first, latest = pd.Timestamp(months[0]), pd.Timestamp(months[-1])
    nat_n, nat_kwh = b.installs.sum(), b.usable_kwh.sum()
    nem = b.dropna(subset=["region"])
    nem_n, nem_kwh = nem.installs.sum(), nem.usable_kwh.sum()
    vol = cer["voluntary"].installs.sum()
    per_month = b.groupby("month")[["installs", "usable_kwh"]].sum()
    big = per_month.installs.idxmax()
    size = per_month.usable_kwh / per_month.installs
    by_region = nem.groupby("region").usable_kwh.sum() / nem_kwh
    top = by_region.sort_values(ascending=False)

    s = cer["solar"]
    solar_gw = s.rated_kw.sum() / 1e6
    last12 = s[s.month > latest - pd.DateOffset(months=12)].rated_kw.sum() / 1e6

    nem_mean = rooftop.groupby("month").mean_mw.sum()
    recent = nem_mean.iloc[-12:].mean()
    earlier = nem_mean.iloc[-72:-60].mean() if len(nem_mean) >= 72 else np.nan
    rec = peaks.loc[peaks.nem_peak_mw.idxmax()]

    span = f"{first:%B %Y} to {latest:%B %Y}"
    lines = [
        f"- **Home batteries:** {nat_n:,.0f} installed with {nat_kwh / 1e6:,.1f} GWh of usable storage, {span} (Cheaper Home Batteries Program). "
        f"In the NEM's five regions: {nem_n:,.0f} batteries, {nem_kwh / 1e6:,.1f} GWh. The voluntary record for September 2014 to June 2025 counts {vol:,.0f}.",
        f"- **Where:** {', '.join(f'{REGION_NAMES[r]} {100 * v:.0f}%' for r, v in top.items())} of the NEM's new battery storage.",
        f"- **Busiest month:** {big:%B %Y}, {per_month.installs[big]:,.0f} batteries ({per_month.usable_kwh[big] / 1e3:,.0f} MWh). "
        f"Average size {size.iloc[0]:.1f} kWh in {first:%b %Y}, {size.max():.1f} kWh at its largest ({size.idxmax():%b %Y}), {size.iloc[-1]:.1f} kWh in {latest:%b %Y}.",
        f"- **Small-scale solar:** {solar_gw:,.1f} GW registered with the CER nationally, {last12:,.1f} GW of it in the last 12 months.",
        f"- **Rooftop PV output (AEMO):** a NEM record of {rec.nem_peak_mw / 1e3:,.1f} GW in the half hour ending {pd.Timestamp(rec.peak_at):%H:%M on %d %B %Y}. "
        f"Mean output over the last 12 months {recent / 1e3:,.2f} GW"
        + (f", {recent / earlier:.1f}× five years earlier." if np.isfinite(earlier) else "."),
        f"- Recent months are incomplete: CER counts a system once its certificates are created, which can be up to a year after installation.",
    ]
    return lines


def write_readme(lines: list[str]) -> None:
    path = HERE / "README.md"
    text = path.read_text(encoding="utf-8")
    block = "<!-- headline:start -->\n" + "\n".join(lines) + "\n<!-- headline:end -->"
    text = re.sub(r"<!-- headline:start -->.*?<!-- headline:end -->", lambda _: block, text, flags=re.S)
    path.write_text(text, encoding="utf-8")


# ---------------------------------------------------------------- main


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--raw", type=Path, default=HERE / "raw")
    args = ap.parse_args()
    cer_raw = max(p for p in (args.raw / "cer").iterdir() if p.is_dir())
    data, charts = HERE / "data", HERE / "charts"
    data.mkdir(exist_ok=True)
    charts.mkdir(exist_ok=True)

    cer = cer_tables(cer_raw)
    rooftop, peaks = rooftop_tables(args.raw / "aemo" / "ROOFTOP_PV_ACTUAL")
    b = cer["batteries"][cer["batteries"].month >= "2025-07-01"]
    b.sort_values(["month", "state"]).to_csv(data / "batteries_monthly_by_state.csv", index=False, float_format="%.3f")
    cer["voluntary"].sort_values(["year", "state"]).to_csv(data / "batteries_voluntary_yearly_by_state.csv", index=False)
    cer["solar"].sort_values(["month", "state"]).to_csv(data / "solar_monthly_by_state.csv", index=False, float_format="%.3f")
    rooftop.sort_values(["month", "region"]).to_csv(data / "rooftop_pv_monthly_by_region.csv", index=False, float_format="%.3f")
    peaks.to_csv(data / "rooftop_pv_nem_monthly_peak.csv", index=False, float_format="%.1f")
    json.dump(json.loads((cer_raw / "sources.json").read_text()), open(data / "cer_sources.json", "w"), indent=2)

    cer_note = "Data: Clean Energy Regulator, small-scale installation postcode data (CC BY 4.0). NEM regions; the ACT counts with NSW. nem-lens/der"
    aemo_note = "Data: AEMO, ROOFTOP_PV_ACTUAL estimates (MMSDM archive). nem-lens/der"

    nb = b.dropna(subset=["region"])
    w = nb.pivot_table(index="month", columns="region", values="usable_kwh", aggfunc="sum").reindex(columns=REGIONS).fillna(0)
    stacked_area(w.cumsum() / 1e6, charts / "batteries_cumulative.png", "Home battery storage installed since July 2025", "Cumulative usable capacity, GWh, by NEM region", "GWh", lambda v: f"{v:,.1f}", cer_note)
    wn = nb.pivot_table(index="month", columns="region", values="installs", aggfunc="sum").reindex(columns=REGIONS).fillna(0)
    stacked_bars(wn / 1e3, charts / "batteries_monthly.png", "Home batteries installed each month", "Thousands of systems, by NEM region. Faded: the latest two months, still being registered.", "thousand", lambda v: f"{v:,.0f}", cer_note, incomplete=2)
    v = cer["voluntary"].dropna(subset=["region"]).pivot_table(index="year", columns="region", values="installs", aggfunc="sum").reindex(columns=REGIONS).fillna(0)
    labels = [f"{y}" if y < 2025 else "2025 H1" for y in v.index]
    stacked_bars(v / 1e3, charts / "batteries_before.png", "Home batteries before the subsidy", "Thousands of systems a year, voluntary reports to the CER (an under-count)", "thousand", lambda v: f"{v:,.0f}", cer_note, labels=labels)

    s = cer["solar"].dropna(subset=["region"])
    ws = s.pivot_table(index="month", columns="region", values="rated_kw", aggfunc="sum").reindex(columns=REGIONS).fillna(0)
    stacked_area(ws.cumsum().loc["2011-01-01":] / 1e6, charts / "solar_cumulative.png", "Small-scale solar installed", "Cumulative rated output, GW, by NEM region (systems up to 100 kW)", "GW", lambda v: f"{v:,.1f}", cer_note)
    stacked_bars(ws.loc["2018-01-01":] / 1e3, charts / "solar_monthly.png", "Small-scale solar added each month", "Rated output, MW, by NEM region. Faded: the latest two months, still being registered.", "MW", lambda v: f"{v:,.0f}", cer_note, incomplete=2)

    wr = rooftop.pivot_table(index="month", columns="region", values="mean_mw", aggfunc="sum").reindex(columns=REGIONS).fillna(0)
    stacked_area(wr / 1e3, charts / "rooftop_pv_monthly.png", "Rooftop PV output", "Mean output over each month, GW, by NEM region (AEMO estimates)", "GW", lambda v: f"{v:,.1f}", aemo_note)
    pk = peaks.set_index("month").nem_peak_mw / 1e3
    line(pk, charts / "rooftop_pv_peak.png", "Rooftop PV at its peak", "Highest half-hour output across the NEM each month, GW (AEMO estimates)", "GW", lambda v: f"{v:,.1f}", aemo_note)

    lines = headline(cer, rooftop, peaks)
    write_readme(lines)
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
