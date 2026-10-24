// Copyright (c) 2026 Archie C. Chapman and contributors
// SPDX-License-Identifier: BSD-3-Clause
//
// DER in the NEM: interactive charts from the CSVs in data/, which `der build`
// writes. No libraries. Charts are inline SVG coloured by region tokens in
// der.css; every value a tooltip shows is also in the chart's table.
"use strict";

const REGIONS = ["NSW1", "QLD1", "VIC1", "SA1", "TAS1"];
const NAMES = { NSW1: "NSW + ACT", QLD1: "QLD", VIC1: "VIC", SA1: "SA", TAS1: "TAS" };
const SVGNS = "http://www.w3.org/2000/svg";
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const CER_SRC = "Based on Clean Energy Regulator material, CC BY 4.0. Postcodes aggregated to NEM regions; ACT in NSW.";
const AEMO_SRC = "Derived from AEMO ROOFTOP_PV_ACTUAL estimates, used under AEMO's copyright permissions.";

const view = { range: "all", on: new Set(REGIONS) };

// ------------------------------------------------------------------ data

async function csv(name) {
  const res = await fetch(`data/${name}`);
  if (!res.ok) throw new Error(`${name}: HTTP ${res.status}`);
  const lines = (await res.text()).trim().split(/\r?\n/);
  const head = lines[0].split(",");
  return lines.slice(1).map((l) => {
    const cells = l.split(",");
    return Object.fromEntries(head.map((h, i) => [h, cells[i] ?? ""]));
  });
}

/** Rows keyed by `key`, each an array of five region values (summed). */
function byRegion(rows, key, value) {
  const out = new Map();
  for (const r of rows) {
    const i = REGIONS.indexOf(r.region);
    const k = r[key];
    if (!out.has(k)) out.set(k, [0, 0, 0, 0, 0]);
    if (i >= 0) out.get(k)[i] += Number(r[value]);
  }
  return new Map([...out.entries()].sort((a, b) => (a[0] < b[0] ? -1 : 1)));
}

function cumulative(m) {
  const acc = [0, 0, 0, 0, 0];
  return new Map([...m].map(([k, v]) => [k, v.map((x, i) => (acc[i] += x))]));
}

const monthLabel = (k, long) => {
  const [y, m] = k.split("-");
  return long ? `${MONTHS[m - 1]} ${y}` : `${MONTHS[m - 1]} ${y.slice(2)}`;
};

function monthsBefore(k, n) {
  const [y, m] = k.split("-").map(Number);
  const t = y * 12 + (m - 1) - n;
  return `${Math.floor(t / 12)}-${String((t % 12) + 1).padStart(2, "0")}-01`;
}

const fmt = (d) => (v) => v.toLocaleString("en-AU", { minimumFractionDigits: d, maximumFractionDigits: d });

// ------------------------------------------------------------------ chart specs

function specs(d) {
  const window = (m) => {
    const keys = [...m.keys()];
    if (view.range === "all" || !keys.length) return m;
    const from = monthsBefore(keys[keys.length - 1], Number(view.range) - 1);
    return new Map([...m].filter(([k]) => k >= from));
  };
  const scale = (m, s) => new Map([...m].map(([k, v]) => [k, v.map((x) => x / s)]));
  const since = (m, from) => new Map([...m].filter(([k]) => k >= from));
  const lastYears = (m) => {
    if (view.range === "all") return m;
    const keys = [...m.keys()].map(Number);
    const from = keys[keys.length - 1] - Number(view.range) / 12 + 1;
    return new Map([...m].filter(([k]) => Number(k) >= from));
  };
  return [
    {
      id: "batteries-cumulative", section: "batteries", kind: "area", wide: true,
      title: "Home battery storage installed since July 2025",
      sub: "Cumulative usable capacity, GWh, by NEM region",
      unit: "GWh", fmt: fmt(1), src: CER_SRC, csv: "batteries_monthly_by_state.csv",
      series: () => window(scale(cumulative(d.battKwh), 1e6)),
    },
    {
      id: "batteries-monthly", section: "batteries", kind: "bars", faded: 2,
      title: "Home batteries installed each month",
      sub: "Thousands of systems, by NEM region. Faded: the latest two months, still being registered.",
      unit: "thousand", fmt: fmt(1), src: CER_SRC, csv: "batteries_monthly_by_state.csv",
      series: () => window(scale(d.battN, 1e3)),
    },
    {
      id: "batteries-before", section: "batteries", kind: "bars", years: true,
      title: "Home batteries before the subsidy",
      sub: "Thousands of systems a year, voluntary reports to the CER (an under-count). 2025 is January to June.",
      unit: "thousand", fmt: fmt(1), src: CER_SRC, csv: "batteries_voluntary_yearly_by_state.csv",
      series: () => lastYears(scale(d.vol, 1e3)),
    },
    {
      id: "solar-cumulative", section: "solar", kind: "area",
      title: "Small-scale solar installed",
      sub: "Cumulative rated output, GW, by NEM region (systems up to 100 kW)",
      unit: "GW", fmt: fmt(1), src: CER_SRC, csv: "solar_monthly_by_state.csv",
      series: () => window(since(scale(cumulative(d.solarKw), 1e6), "2011-01-01")),
    },
    {
      id: "solar-monthly", section: "solar", kind: "bars", faded: 2,
      title: "Small-scale solar added each month",
      sub: "Rated output, MW, by NEM region. Faded: the latest two months, still being registered.",
      unit: "MW", fmt: fmt(0), src: CER_SRC, csv: "solar_monthly_by_state.csv",
      series: () => window(since(scale(d.solarKw, 1e3), "2018-01-01")),
    },
    {
      id: "rooftop-monthly", section: "solar", kind: "area",
      title: "Rooftop PV output",
      sub: "Mean output over each month, GW, by NEM region (AEMO estimates)",
      unit: "GW", fmt: fmt(2), src: AEMO_SRC, csv: "rooftop_pv_monthly_by_region.csv",
      series: () => window(scale(d.roof, 1e3)),
    },
    {
      id: "rooftop-peak", section: "solar", kind: "line",
      title: "Rooftop PV at its peak",
      sub: "Highest half-hour output across the NEM each month, GW (AEMO estimates; all five regions, whatever the region filter)",
      unit: "GW", fmt: fmt(1), src: AEMO_SRC, csv: "rooftop_pv_nem_monthly_peak.csv",
      series: () => window(d.peaks),
    },
  ];
}

// ------------------------------------------------------------------ svg helpers

function el(name, attrs = {}, parent) {
  const e = document.createElementNS(SVGNS, name);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  if (parent) parent.appendChild(e);
  return e;
}

function text(parent, x, y, s, cls, anchor = "start") {
  const t = el("text", { x, y, class: cls, "text-anchor": anchor }, parent);
  t.textContent = s;
  return t;
}

function niceStep(max) {
  const raw = Math.max(max / 5, 1e-9);
  const mag = 10 ** Math.floor(Math.log10(raw));
  const f = raw / mag;
  return mag * (f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10);
}

/** x tick indices and labels, at most about one per 80px. */
function xTicks(keys, years, width) {
  const n = keys.length;
  const maxTicks = Math.max(2, Math.floor(width / 80));
  if (years) {
    const step = Math.ceil(n / maxTicks);
    return keys.map((k, i) => [i, k === "2025" ? "2025 H1" : k]).filter((_, i) => i % step === 0);
  }
  const spanYears = n / 12;
  if (spanYears < 3) {
    const step = Math.max(1, Math.ceil(n / maxTicks));
    return keys.map((k, i) => [i, monthLabel(k)]).filter((_, i) => i % step === 0);
  }
  const every = Math.max(1, Math.ceil(spanYears / maxTicks));
  return keys
    .map((k, i) => [i, k])
    .filter(([, k]) => k.slice(5, 7) === "01" && Number(k.slice(0, 4)) % every === 0)
    .map(([i, k]) => [i, k.slice(0, 4)]);
}

// ------------------------------------------------------------------ tooltip

const tip = document.getElementById("tip");

function showTip(when, rows, total, unit, at) {
  tip.replaceChildren();
  const w = document.createElement("div");
  w.className = "when";
  w.textContent = when;
  tip.appendChild(w);
  const row = (value, label, colour, cls) => {
    const r = document.createElement("div");
    r.className = "row" + (cls ? ` ${cls}` : "");
    const b = document.createElement("b");
    b.textContent = value;
    r.appendChild(b);
    if (colour) {
      const s = document.createElement("s");
      s.style.borderColor = colour;
      r.appendChild(s);
    }
    const sp = document.createElement("span");
    sp.textContent = label;
    r.appendChild(sp);
    tip.appendChild(r);
  };
  for (const [value, label, colour] of rows) row(value, label, colour);
  if (total) row(total, `Total, ${unit}`, null, "total");
  tip.hidden = false;
  const pad = 14;
  const { width, height } = tip.getBoundingClientRect();
  let x = at.x + pad;
  let y = at.y + pad;
  if (x + width > window.innerWidth - 8) x = at.x - width - pad;
  if (y + height > window.innerHeight - 8) y = at.y - height - pad;
  tip.style.left = `${Math.max(8, x)}px`;
  tip.style.top = `${Math.max(8, y)}px`;
}

const hideTip = () => { tip.hidden = true; };

// ------------------------------------------------------------------ rendering

function render(card, spec) {
  const box = card.querySelector(".chart");
  const width = Math.max(300, box.clientWidth);
  const narrow = width < 560;
  const height = Math.round(Math.min(380, Math.max(240, width * 0.48)));
  const series = spec.series();
  const keys = [...series.keys()];
  const on = REGIONS.map((_, i) => i).filter((i) => view.on.has(REGIONS[i]));
  const isLine = spec.kind === "line";
  const values = isLine ? keys.map((k) => series.get(k).mw / 1e3) : keys.map((k) => on.reduce((s, i) => s + series.get(k)[i], 0));
  const right = spec.kind === "area" && !narrow ? 120 : 12;
  const m = { l: 44, r: right, t: 14, b: 28 };
  const pw = width - m.l - m.r;
  const ph = height - m.t - m.b;

  const svg = el("svg", { viewBox: `0 0 ${width} ${height}`, width, height, role: "img", tabindex: 0 });
  svg.setAttribute("aria-label", `${spec.title}. ${spec.sub}. Use the left and right arrow keys to read values; the table below lists them all.`);
  if (!keys.length) {
    text(svg, m.l, m.t + 20, "No data in this period.", "tick");
    box.replaceChildren(svg);
    return;
  }
  const max = Math.max(...values, 0);
  const step = niceStep(max);
  const ymax = Math.max(step, Math.ceil(max / step) * step);
  const y = (v) => m.t + ph - (ph * v) / ymax;
  for (let v = 0; v <= ymax + step * 1e-9; v += step) {
    if (v > 0) el("line", { x1: m.l, x2: m.l + pw, y1: y(v), y2: y(v), class: "gridline" }, svg);
    text(svg, m.l - 6, y(v) + 4, spec.fmt(v).replace(/\.0+$/, "").replace(/(\.\d*?)0+$/, "$1"), "tick", "end");
  }

  const n = keys.length;
  const bars = spec.kind === "bars";
  const band = pw / n;
  const x = bars ? (i) => m.l + band * (i + 0.5) : (i) => m.l + (n === 1 ? pw / 2 : (pw * i) / (n - 1));
  const marks = el("g", {}, svg);

  if (spec.kind === "area") {
    const lower = new Array(n).fill(0);
    for (const r of on) {
      const upper = keys.map((k, i) => lower[i] + series.get(k)[r]);
      const pts = upper.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`)
        .concat(lower.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).reverse());
      el("polygon", { points: pts.join(" "), class: "band", style: `fill:var(--r${r})` }, marks);
      upper.forEach((v, i) => { lower[i] = v; });
    }
    if (!narrow) {
      const last = series.get(keys[n - 1]);
      const total = values[n - 1];
      let base = 0;
      for (const r of on) {
        const v = last[r];
        if (v >= 0.06 * total) text(svg, m.l + pw + 8, y(base + v / 2) + 4, `${NAMES[REGIONS[r]]}  ${spec.fmt(v)}`, "dlabel");
        base += v;
      }
      text(svg, m.l + pw + 8, y(total) - 6, `Total ${spec.fmt(total)}`, "dtotal");
    }
  } else if (bars) {
    const w = Math.max(1, band * 0.82);
    keys.forEach((k, i) => {
      const g = el("g", { class: "col" }, marks);
      const faded = spec.faded && i >= n - spec.faded ? " faded" : "";
      let base = 0;
      for (const r of on) {
        const v = series.get(k)[r];
        if (v > 0) el("rect", { x: x(i) - w / 2, y: y(base + v), width: w, height: y(base) - y(base + v), class: `bar${faded}`, style: `fill:var(--r${r})` }, g);
        base += v;
      }
    });
  } else {
    const d = values.map((v, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
    el("path", { d, fill: "none", style: "stroke:var(--r0)", "stroke-width": 2, "stroke-linejoin": "round" }, marks);
    const top = values.indexOf(max);
    el("circle", { cx: x(top), cy: y(max), r: 4.5, class: "dot", style: "fill:var(--r0)" }, marks);
    text(svg, x(top) - 8, y(max) - 8, `Record ${spec.fmt(max)}`, "dlabel", "end");
  }
  el("line", { x1: m.l, x2: m.l + pw, y1: m.t + ph, y2: m.t + ph, class: "baseline" }, svg);
  for (const [i, label] of xTicks(keys, spec.years, pw)) text(svg, x(i), m.t + ph + 18, label, "tick", "middle");

  // Hover and keyboard: one readout for every region at the nearest x.
  const cross = el("line", { y1: m.t, y2: m.t + ph, class: "cross", visibility: "hidden" }, svg);
  const hit = el("rect", { x: m.l, y: m.t, width: pw, height: ph, class: "hit" }, svg);
  let cursor = -1;
  const show = (i, at) => {
    cursor = i;
    const k = keys[i];
    const when = spec.years ? (k === "2025" ? "2025, January to June" : k) : monthLabel(k, true);
    if (bars) {
      marks.querySelectorAll(".col").forEach((g, j) => g.classList.toggle("on", j === i));
      cross.setAttribute("visibility", "hidden");
    } else {
      cross.setAttribute("x1", x(i));
      cross.setAttribute("x2", x(i));
      cross.setAttribute("visibility", "visible");
    }
    if (isLine) {
      const p = series.get(k);
      showTip(when, [[spec.fmt(p.mw / 1e3), `GW, half hour ending ${p.at.slice(11, 16)} on ${Number(p.at.slice(8, 10))} ${MONTHS[p.at.slice(5, 7) - 1]}`, "var(--r0)"]], null, spec.unit, at);
    } else {
      const v = series.get(k);
      const rows = on.slice().reverse().map((r) => [spec.fmt(v[r]), NAMES[REGIONS[r]], `var(--r${r})`]);
      showTip(when, rows, spec.fmt(values[i]), spec.unit, at);
    }
  };
  const nearest = (clientX) => {
    const b = svg.getBoundingClientRect();
    const sx = ((clientX - b.left) / b.width) * width;
    const i = bars ? Math.floor((sx - m.l) / band) : Math.round(((sx - m.l) / pw) * (n - 1));
    return Math.min(n - 1, Math.max(0, i));
  };
  const clear = () => {
    cursor = -1;
    cross.setAttribute("visibility", "hidden");
    marks.querySelectorAll(".col.on").forEach((g) => g.classList.remove("on"));
    hideTip();
  };
  hit.addEventListener("pointermove", (e) => show(nearest(e.clientX), { x: e.clientX, y: e.clientY }));
  hit.addEventListener("pointerleave", clear);
  svg.addEventListener("blur", clear);
  svg.addEventListener("keydown", (e) => {
    if (e.key === "Escape") return clear();
    const delta = { ArrowLeft: -1, ArrowRight: 1, Home: -n, End: n }[e.key];
    if (delta === undefined) return;
    e.preventDefault();
    const i = Math.min(n - 1, Math.max(0, (cursor < 0 ? n - 1 : cursor) + (cursor < 0 ? 0 : delta)));
    const b = svg.getBoundingClientRect();
    show(i, { x: b.left + (x(i) / width) * b.width, y: b.top + b.height * 0.3 });
  });
  box.replaceChildren(svg);

  // Legend and table.
  const legend = card.querySelector(".legend");
  legend.replaceChildren();
  if (!isLine) {
    for (const r of on) {
      const s = document.createElement("span");
      const i = document.createElement("i");
      i.style.background = `var(--r${r})`;
      s.append(i, NAMES[REGIONS[r]]);
      legend.appendChild(s);
    }
  }
  const details = card.querySelector("details");
  const fill = () => table(details.querySelector(".tablewrap"), spec, keys, series, on, values);
  details.ontoggle = () => { if (details.open) fill(); };
  if (details.open) fill();
}

function table(wrap, spec, keys, series, on, values) {
  const t = document.createElement("table");
  const head = t.createTHead().insertRow();
  const cols = spec.kind === "line"
    ? ["Month", `NEM peak, ${spec.unit}`, "Half hour ending"]
    : [spec.years ? "Year" : "Month", ...on.map((r) => NAMES[REGIONS[r]]), `Total, ${spec.unit}`];
  for (const c of cols) {
    const th = document.createElement("th");
    th.textContent = c;
    head.appendChild(th);
  }
  const body = t.createTBody();
  for (let i = keys.length - 1; i >= 0; i--) {
    const k = keys[i];
    const row = body.insertRow();
    const cells = spec.kind === "line"
      ? [monthLabel(k, true), spec.fmt(values[i]), series.get(k).at.slice(0, 16)]
      : [spec.years ? k : monthLabel(k, true), ...on.map((r) => spec.fmt(series.get(k)[r])), spec.fmt(values[i])];
    for (const c of cells) row.insertCell().textContent = c;
  }
  wrap.replaceChildren(t);
}

function card(spec) {
  const c = document.createElement("section");
  c.className = "card" + (spec.wide ? " wide" : "");
  c.id = spec.id;
  const h = document.createElement("h3");
  h.textContent = spec.title;
  const sub = document.createElement("p");
  sub.className = "sub";
  sub.textContent = spec.sub;
  const chart = document.createElement("div");
  chart.className = "chart";
  const legend = document.createElement("div");
  legend.className = "legend";
  const details = document.createElement("details");
  const summary = document.createElement("summary");
  summary.textContent = "Table";
  const wrap = document.createElement("div");
  wrap.className = "tablewrap";
  details.append(summary, wrap);
  const src = document.createElement("p");
  src.className = "src";
  const a = document.createElement("a");
  a.href = `data/${spec.csv}`;
  a.textContent = spec.csv;
  src.append(`${spec.src} CSV: `, a);
  c.append(h, sub, chart, legend, details, src);
  return c;
}

// ------------------------------------------------------------------ page

function headline(lines) {
  const ul = document.getElementById("headline");
  ul.replaceChildren();
  for (const line of lines) {
    const li = document.createElement("li");
    // Lines are Markdown bullets with **bold** labels; render that much only.
    line.replace(/^- /, "").split(/(\*\*[^*]+\*\*)/).forEach((part) => {
      if (part.startsWith("**")) {
        const b = document.createElement("strong");
        b.textContent = part.slice(2, -2);
        li.appendChild(b);
      } else if (part) {
        li.appendChild(document.createTextNode(part));
      }
    });
    ul.appendChild(li);
  }
}

function controls(redraw) {
  const chips = document.getElementById("regions");
  REGIONS.forEach((r, i) => {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "chip";
    b.setAttribute("aria-pressed", "true");
    const sw = document.createElement("i");
    sw.style.background = `var(--r${i})`;
    b.append(sw, NAMES[r]);
    b.addEventListener("click", () => {
      if (view.on.has(r) && view.on.size === 1) return; // keep at least one region
      view.on.has(r) ? view.on.delete(r) : view.on.add(r);
      b.setAttribute("aria-pressed", String(view.on.has(r)));
      redraw();
    });
    chips.appendChild(b);
  });
  document.getElementById("filters").addEventListener("change", (e) => {
    if (e.target.name === "range") {
      view.range = e.target.value;
      redraw();
    }
  });
  const root = document.documentElement;
  try {
    const saved = localStorage.getItem("der-theme");
    if (saved === "light" || saved === "dark") root.dataset.theme = saved;
  } catch (_) { /* storage unavailable: follow the system setting */ }
  document.getElementById("theme").addEventListener("click", () => {
    const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
    root.dataset.theme = dark ? "light" : "dark";
    try { localStorage.setItem("der-theme", root.dataset.theme); } catch (_) { /* not saved */ }
  });
}

async function main() {
  const [batt, vol, solar, roof, peaks, head] = await Promise.all([
    csv("batteries_monthly_by_state.csv"),
    csv("batteries_voluntary_yearly_by_state.csv"),
    csv("solar_monthly_by_state.csv"),
    csv("rooftop_pv_monthly_by_region.csv"),
    csv("rooftop_pv_nem_monthly_peak.csv"),
    fetch("data/headline.json").then((r) => (r.ok ? r.json() : { lines: [] })),
  ]);
  const d = {
    battKwh: byRegion(batt, "month", "usable_kwh"),
    battN: byRegion(batt, "month", "installs"),
    vol: byRegion(vol, "year", "installs"),
    solarKw: byRegion(solar, "month", "rated_kw"),
    roof: byRegion(roof, "month", "mean_mw"),
    peaks: new Map(peaks.map((p) => [p.month, { mw: Number(p.nem_peak_mw), at: p.peak_at }])),
  };
  headline(head.lines);
  const all = specs(d).map((s) => ({ s, c: card(s) }));
  for (const { s, c } of all) document.getElementById(s.section).appendChild(c);
  const redraw = () => all.forEach(({ s, c }) => render(c, s));
  controls(redraw);
  redraw();
  let pending = 0;
  new ResizeObserver(() => {
    cancelAnimationFrame(pending);
    pending = requestAnimationFrame(redraw);
  }).observe(document.querySelector("main"));
}

main().catch((e) => {
  const p = document.createElement("p");
  p.textContent = `Could not load the data: ${e.message}`;
  document.querySelector("main").prepend(p);
});
