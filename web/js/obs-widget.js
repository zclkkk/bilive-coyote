import { api } from "./api.js";
import { PanelWS } from "./ws.js";

const CIRCUMFERENCE = 2 * Math.PI * 90;
const DEFAULT_LIMIT = 200;

const $ = (sel) => document.querySelector(sel);

const params = new URLSearchParams(location.search);
const state = {
  a: 0,
  b: 0,
  effectiveLimitA: DEFAULT_LIMIT,
  effectiveLimitB: DEFAULT_LIMIT,
  paired: false,
};

const el = {
  ringA: $("#ring-a"),
  ringB: $("#ring-b"),
  valueA: $("#value-a"),
  valueB: $("#value-b"),
  maxValue: $("#max-value"),
  statusMark: $("#status-mark"),
};

init();

async function init() {
  applyOptions();
  resetMeters();
  await loadStatus();
  setupEvents();
}

function applyOptions() {
  const size = clampNumber(params.get("size"), 180, 720, 360);
  const ratio = size / 360;
  const theme = params.get("theme") === "dark" ? "dark" : "light";
  document.documentElement.style.setProperty("--size", `${size}px`);
  document.documentElement.style.setProperty("--ratio", String(ratio));
  document.documentElement.dataset.theme = theme;
}

function resetMeters() {
  el.ringA.style.strokeDasharray = `0 ${CIRCUMFERENCE}`;
  el.ringB.style.strokeDasharray = `0 ${CIRCUMFERENCE}`;
}

async function loadStatus() {
  try {
    const status = await api.status();
    if (status.strength) applyStrengthStatus(status.strength);
    if (status.coyote) applyCoyoteStatus(status.coyote);
  } catch (e) {
    console.error("[OBS] load status failed:", e);
  }
}

function setupEvents() {
  const ws = new PanelWS();
  ws.on("strength:status", applyStrengthStatus);
  ws.on("strength", applyStrengthEvent);
  ws.on("coyote:status", applyCoyoteStatus);
  ws.on("reconnect", loadStatus);
}

function applyStrengthStatus(data) {
  state.effectiveLimitA = normalizedLimit(data.effectiveLimitA);
  state.effectiveLimitB = normalizedLimit(data.effectiveLimitB);
  state.a = clampStrength(data.a, state.effectiveLimitA);
  state.b = clampStrength(data.b, state.effectiveLimitB);
  render();
}

function applyCoyoteStatus(data) {
  state.paired = Boolean(data.paired);
  state.effectiveLimitA = normalizedLimit(data.effectiveLimitA);
  state.effectiveLimitB = normalizedLimit(data.effectiveLimitB);
  if (data.strengthA !== undefined) {
    state.a = clampStrength(data.strengthA, state.effectiveLimitA);
  }
  if (data.strengthB !== undefined) {
    state.b = clampStrength(data.strengthB, state.effectiveLimitB);
  }
  if (!state.paired) {
    state.a = 0;
    state.b = 0;
  }
  render();
}

function applyStrengthEvent(data) {
  if (data.channel === "A") {
    state.a = clampStrength(data.value, state.effectiveLimitA);
  }
  if (data.channel === "B") {
    state.b = clampStrength(data.value, state.effectiveLimitB);
  }
  render();
}

function render() {
  el.valueA.textContent = state.a;
  el.valueB.textContent = state.b;
  el.maxValue.textContent = maxLabel();
  renderMeter(el.ringA, state.a, state.effectiveLimitA);
  renderMeter(el.ringB, state.b, state.effectiveLimitB);
  el.statusMark.textContent = state.paired ? "II" : "--";
  el.statusMark.classList.toggle("offline", !state.paired);
}

function renderMeter(ring, value, limit) {
  const ratio = limit > 0 ? Math.min(value / limit, 1) : 0;
  const length = CIRCUMFERENCE * ratio;
  ring.style.strokeDasharray = `${length} ${CIRCUMFERENCE}`;
  ring.style.opacity = value > 0 ? "1" : "0";
}

function maxLabel() {
  if (state.effectiveLimitA === state.effectiveLimitB) {
    return state.effectiveLimitA;
  }
  return `${state.effectiveLimitA}/${state.effectiveLimitB}`;
}

function normalizedLimit(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n < 1) return DEFAULT_LIMIT;
  return Math.min(Math.round(n), DEFAULT_LIMIT);
}

function clampStrength(value, limit) {
  const n = Number(value);
  if (!Number.isFinite(n) || n < 0) return 0;
  return Math.min(Math.round(n), limit);
}

function clampNumber(value, min, max, fallback) {
  const n = Number(value);
  if (!Number.isFinite(n)) return fallback;
  return Math.min(Math.max(n, min), max);
}
