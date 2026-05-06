import { api } from "./api.js";
import { escapeHtml } from "./utils.js";
import { PanelWS } from "./ws.js";

const ws = new PanelWS();

const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

let currentConfig = {};
let currentRules = [];
let waveforms = [];
let waveformById = new Map();
const state = {
  strengthA: 0,
  strengthB: 0,
  appLimitA: 200,
  appLimitB: 200,
  effectiveLimitA: 200,
  effectiveLimitB: 200,
};
const waveformState = {
  selectedA: "breath",
  selectedB: "breath",
};
const feedbackLabels = ["○", "△", "□", "☆", "⬡"];
let qrLoaded = false;

async function init() {
  await loadWaveforms();
  await loadConfig();
  await loadStatus();
  setupEventListeners();
  setupWSEvents();
}

async function loadConfig() {
  try {
    currentConfig = await api.config.get();
    currentRules = await api.config.rules.get();
    renderConfig();
    renderRules();
  } catch (e) {
    console.error("Load config failed:", e);
  }
}

async function loadWaveforms() {
  try {
    const data = await api.coyote.waveforms();
    waveforms = data.items || [];
    waveformById = new Map(waveforms.map((item) => [item.id, item]));
    updateWaveformStatus({
      waveformA: data.selectedA || "breath",
      waveformB: data.selectedB || "breath",
    });
    renderWaveformOptions();
    renderRules();
  } catch (e) {
    console.error("Load waveforms failed:", e);
  }
}

async function loadStatus() {
  try {
    const status = await api.status();
    updateBilibiliStatus(status.bilibili);
    updateCoyoteStatus(status.coyote);
    if (status.strength) {
      const s = status.strength;
      applyLimits(s.appLimitA, s.appLimitB, s.effectiveLimitA, s.effectiveLimitB);
      state.strengthA = Math.min(s.a, state.effectiveLimitA);
      state.strengthB = Math.min(s.b, state.effectiveLimitB);
      renderStrength();
    }
  } catch (e) {
    console.error("Load status failed:", e);
  }
}

function renderConfig() {
  const b = currentConfig.bilibili || {};
  const op = b.openPlatform || {};
  const bc = b.broadcast || {};

  $("#source-type").value = b.source || "open-platform";
  toggleSourceFields(b.source || "open-platform");

  $("#appKey").value = op.appKey || "";
  $("#appSecret").value = op.appSecret || "";
  $("#code").value = op.code || "";
  $("#appId").value = op.appId || "";
  $("#roomId").value = bc.roomId || "";

  const s = currentConfig.safety || {};
  $("#limitA").value = s.limitA ?? 80;
  $("#limitB").value = s.limitB ?? 80;
  $("#decayEnabled").checked = s.decayEnabled !== false;
  $("#decayRate").value = s.decayRate ?? 2;
  renderLimitHints();
}

function toggleSourceFields(source) {
  const isOP = source === "open-platform";
  $("#op-fields").style.display = isOP ? "" : "none";
  $("#bc-fields").style.display = isOP ? "none" : "";
}

function applyLimits(appA, appB, effA, effB) {
  if (appA !== undefined) {
    state.appLimitA = appA;
    state.appLimitB = appB;
    const elA = $("#app-limit-a");
    const elB = $("#app-limit-b");
    if (elA) elA.textContent = appA;
    if (elB) elB.textContent = appB;
  }
  if (effA !== undefined) {
    state.effectiveLimitA = effA;
    state.effectiveLimitB = effB;
  }
  renderLimitHints();
}

function renderLimitHints() {
  const elA = $("#hint-limitA");
  const elB = $("#hint-limitB");
  if (elA) elA.textContent = `(APP上限: ${state.appLimitA})`;
  if (elB) elB.textContent = `(APP上限: ${state.appLimitB})`;
}

function renderWaveformOptions() {
  const options = waveforms
    .map((item) => `<option value="${escapeHtml(item.id)}">${escapeHtml(item.name)}</option>`)
    .join("");

  const selectA = $("#waveform-a");
  const selectB = $("#waveform-b");
  const ruleSelect = $("#rule-waveform");
  if (selectA) selectA.innerHTML = options;
  if (selectB) selectB.innerHTML = options;
  if (ruleSelect) {
    ruleSelect.innerHTML = `
      <option value="">不切换</option>
      <option value="next">下一个</option>
      ${options}
    `;
  }
  renderWaveformControls();
}

function updateWaveformStatus(data) {
  if (data.waveformA) waveformState.selectedA = data.waveformA;
  if (data.waveformB) waveformState.selectedB = data.waveformB;
  renderWaveformControls();
}

function renderWaveformControls() {
  const selectA = $("#waveform-a");
  const selectB = $("#waveform-b");
  if (selectA) selectA.value = waveformState.selectedA;
  if (selectB) selectB.value = waveformState.selectedB;
}

function renderRules() {
  const container = $("#rules-list");
  container.innerHTML = "";
  for (let i = 0; i < currentRules.length; i++) {
    const r = currentRules[i];
    const div = document.createElement("div");
    div.className = "rule-item";
    div.innerHTML = `
      <span class="rule-name">${escapeHtml(r.giftName)}</span>
      <span class="rule-effect">${escapeHtml(formatRuleEffect(r))}</span>
      <button class="btn btn-danger btn-small" data-idx="${i}">删除</button>
    `;
    container.appendChild(div);
  }

  container.querySelectorAll("[data-idx]").forEach((btn) => {
    btn.onclick = async () => {
      const idx = parseInt(btn.dataset.idx, 10);
      currentRules = currentRules.filter((_, i) => i !== idx);
      await api.config.rules.set(currentRules);
      renderRules();
    };
  });
}

function formatRuleEffect(rule) {
  const channel = rule.channel === "both" ? "双通道" : `${rule.channel}通道`;
  const parts = [];
  if ((rule.strengthAdd || 0) > 0) {
    parts.push(`${channel} +${rule.strengthAdd} 持续${rule.duration}s`);
  }
  if (rule.waveform) {
    const waveformName = rule.waveform === "next" ? "下一个波形" : waveformLabel(rule.waveform);
    parts.push(`${channel} ${waveformName}`);
  }
  return parts.join(" / ");
}

function waveformLabel(id) {
  const waveform = waveformById.get(id);
  return waveform ? waveform.name : id;
}

function setupEventListeners() {
  $("#emergency-btn").onclick = async () => {
    await api.coyote.emergency();
  };

  $("#source-type").onchange = () => {
    toggleSourceFields($("#source-type").value);
  };

  $("#zero-btn").onclick = async () => {
    await api.coyote.strength("A", 0);
    await api.coyote.strength("B", 0);
  };

  const deltas = { inc: 1, inc5: 5, dec: -1, dec5: -5 };
  $$("[data-channel][data-action]").forEach((btn) => {
    btn.onclick = () => {
      const ch = btn.dataset.channel;
      const delta = deltas[btn.dataset.action] ?? 0;
      const current = ch === "A" ? state.strengthA : state.strengthB;
      const limit = ch === "A" ? state.effectiveLimitA : state.effectiveLimitB;
      const newVal = Math.max(0, Math.min(current + delta, limit));
      api.coyote.strength(ch, newVal);
    };
  });

  $("#waveform-a").onchange = async () => {
    await api.coyote.waveform({
      action: "select",
      channel: "A",
      waveformId: $("#waveform-a").value,
    });
    await loadWaveforms();
  };

  $("#waveform-b").onchange = async () => {
    await api.coyote.waveform({
      action: "select",
      channel: "B",
      waveformId: $("#waveform-b").value,
    });
    await loadWaveforms();
  };

  $$("[data-wave-channel][data-wave-action]").forEach((btn) => {
    btn.onclick = async () => {
      await api.coyote.waveform({
        action: btn.dataset.waveAction,
        channel: btn.dataset.waveChannel,
      });
      await loadWaveforms();
    };
  });

  $("#bilibili-start").onclick = async () => {
    const source = $("#source-type").value;
    const params = buildStartParams(source);
    if (!params) return;

    const startBtn = $("#bilibili-start");
    startBtn.textContent = "连接中...";
    startBtn.disabled = true;
    try {
      await api.bilibili.start(params);
    } catch (e) {
      alert(`启动失败: ${e.message}`);
    } finally {
      startBtn.textContent = "开始监听";
      startBtn.disabled = false;
    }
  };

  $("#bilibili-stop").onclick = async () => {
    await api.bilibili.stop();
  };

  $("#save-safety").onclick = async () => {
    const safety = {
      limitA: parseInt($("#limitA").value, 10),
      limitB: parseInt($("#limitB").value, 10),
      decayEnabled: $("#decayEnabled").checked,
      decayRate: parseInt($("#decayRate").value, 10),
    };

    await api.config.set({ safety });

    currentConfig.safety = safety;
    await loadStatus();
  };

  $("#add-rule-btn").onclick = async () => {
    const name = $("#rule-name").value.trim();
    const channel = $("#rule-channel").value;
    const strength = parseInt($("#rule-strength").value, 10);
    const duration = parseInt($("#rule-duration").value, 10);
    const waveform = $("#rule-waveform").value;

    if (!name) return;
    if (Number.isNaN(strength) || Number.isNaN(duration)) return;
    if (strength === 0 && !waveform) return;
    if (strength > 0 && duration < 1) return;
    if (strength === 0 && duration !== 0) return;

    const rule = {
      giftName: name,
      coinType: "all",
      channel,
      strengthAdd: strength,
      duration,
    };
    if (waveform) rule.waveform = waveform;
    currentRules.push(rule);

    await api.config.rules.set(currentRules);
    renderRules();

    $("#rule-name").value = "";
    $("#rule-strength").value = "";
    $("#rule-duration").value = "";
    $("#rule-waveform").value = "";
  };
}

function buildStartParams(source) {
  if (source === "open-platform") {
    const appKey = $("#appKey").value.trim();
    const appSecret = $("#appSecret").value.trim();
    const code = $("#code").value.trim();
    const appId = parseInt($("#appId").value, 10);
    if (!appKey || !appSecret) {
      alert("请填写 AppKey 和 AppSecret");
      return null;
    }
    if (!code || !appId) {
      alert("请填写主播身份码和 App ID");
      return null;
    }
    return { source, code, appId, appKey, appSecret };
  }
  const roomId = parseInt($("#roomId").value, 10);
  if (!roomId) {
    alert("请填写房间号");
    return null;
  }
  const loginJson = $("#broadcastLoginJson").value.trim();
  return loginJson ? { source, roomId, loginJson } : { source, roomId };
}

function setupWSEvents() {
  ws.on("bilibili:status", updateBilibiliStatus);
  ws.on("coyote:status", updateCoyoteStatus);
  ws.on("strength", (data) => {
    if (data.channel === "A") state.strengthA = Math.min(data.value, state.effectiveLimitA);
    if (data.channel === "B") state.strengthB = Math.min(data.value, state.effectiveLimitB);
    renderStrength();
  });
  ws.on("gift", addGiftLog);
  ws.on("waveform:status", updateWaveformStatus);
  ws.on("coyote:feedback", updateCoyoteFeedback);
  // 断线重连后重新拉取状态，避免错过断开期间的状态变化
  ws.on("reconnect", async () => {
    console.log("[Panel] Reconnected, refreshing state");
    qrLoaded = false;
    await loadWaveforms();
    await loadConfig();
    await loadStatus();
  });
}

function updateBilibiliStatus(data) {
  const dot = $("#bili-dot");
  const text = $("#bili-text");
  if (data.connected) {
    dot.className = "status-dot online";
    text.textContent = data.roomId ? `已连接 房间 ${data.roomId}` : "已连接";
  } else {
    dot.className = "status-dot offline";
    text.textContent = data.error || "未连接";
  }
}

function updateCoyoteStatus(data) {
  const dot = $("#coyote-dot");
  const text = $("#coyote-text");
  const qrView = $("#qr-view");
  const pairDetail = $("#pair-detail");
  applyLimits(data.limitA, data.limitB, data.effectiveLimitA, data.effectiveLimitB);
  if (data.paired) {
    dot.className = "status-dot online";
    text.textContent = "已配对";
    qrView.style.display = "none";
    pairDetail.style.display = "block";
    if (data.strengthA !== undefined) {
      state.strengthA = Math.min(data.strengthA, state.effectiveLimitA);
      state.strengthB = Math.min(data.strengthB, state.effectiveLimitB);
    }
    renderStrength();
    renderPairDetail();
  } else {
    dot.className = "status-dot offline";
    text.textContent = "等待配对";
    qrView.style.display = "block";
    pairDetail.style.display = "none";
    state.strengthA = 0;
    state.strengthB = 0;
    const elA = $("#app-limit-a");
    if (elA) elA.textContent = "--";
    const elB = $("#app-limit-b");
    if (elB) elB.textContent = "--";
    $("#coyote-feedback").textContent = "--";
    renderStrength();
    if (!qrLoaded) loadQRCode();
  }
}

function renderPairDetail() {
  const elPort = $("#pair-ws-port");
  if (elPort) elPort.textContent = currentConfig.coyote?.wsPort ?? 9999;
}

function updateCoyoteFeedback(data) {
  $("#coyote-feedback").textContent = `${data.channel} ${feedbackLabels[data.button]}`;
}

function renderStrength() {
  renderBar("A", state.strengthA, state.effectiveLimitA);
  renderBar("B", state.strengthB, state.effectiveLimitB);
}

function renderBar(channel, value, limit) {
  const suffix = channel.toLowerCase();
  const fill = $(`#bar-${suffix}-fill`);
  const label = $(`#bar-${suffix}-val`);
  const ctrl = $(`#ctrl-${suffix}-val`);
  if (fill) fill.style.width = limit > 0 ? `${(value / limit) * 100}%` : "0%";
  if (label) label.textContent = `${value}/${limit}`;
  if (ctrl) ctrl.textContent = value;
}

function addGiftLog(data) {
  const log = $("#gift-log");
  const item = document.createElement("div");
  item.className = "gift-log-item";
  const time = new Date(data.timestamp * 1000).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  item.innerHTML = `
    <span class="time">${time}</span>
    <span class="user">${escapeHtml(data.uname)}</span>
    <span class="gift">${escapeHtml(data.giftName)} x${data.num}</span>
    <span class="effect">${escapeHtml(data.strengthDelta)}</span>
  `;
  log.prepend(item);
  while (log.children.length > 50) log.removeChild(log.lastChild);
}

async function loadQRCode() {
  try {
    const { qrcode } = await api.coyote.qrcode();
    const img = $("#qr-img");
    img.src = qrcode;
    img.style.display = "block";
    $("#qr-status").textContent = "用 DG-LAB APP 扫描二维码配对";
    qrLoaded = true;
  } catch (e) {
    $("#qr-status").textContent = e.message || "二维码生成失败";
  }
}

init();
