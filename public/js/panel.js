import { api } from "./api.js"
import { PanelWS } from "./ws.js"
import { escapeHtml } from "./utils.js"

const ws = new PanelWS()

const $ = (sel) => document.querySelector(sel)
const $$ = (sel) => document.querySelectorAll(sel)

let currentConfig = {}
let currentRules = []
let appLimits = { a: 200, b: 200 }
let lastStrengthA = 0, lastStrengthB = 0
let qrLoaded = false

async function init() {
  await loadConfig()
  await loadStatus()
  setupEventListeners()
  setupWSEvents()
}

async function loadConfig() {
  try {
    currentConfig = await api.config.get()
    currentRules = await api.config.rules.get()
    renderConfig()
    renderRules()
  } catch (e) {
    console.error("Load config failed:", e)
  }
}

async function loadStatus() {
  try {
    const status = await api.status()
    updateBilibiliStatus(status.bilibili)
    updateCoyoteStatus(status.coyote)
    if (status.strength) {
      if (status.strength.appLimitA !== undefined) {
        appLimits.a = status.strength.appLimitA
        appLimits.b = status.strength.appLimitB
        updateLimitHints()
      }
      updateStrengthBars(status.strength.a, status.strength.b)
    }
  } catch {}
}

function renderConfig() {
  const b = currentConfig.bilibili || {}
  $("#appKey").value = b.appKey || ""
  $("#appSecret").value = b.appSecret || ""
  $("#code").value = b.code || ""
  $("#appId").value = b.appId || ""

  const s = currentConfig.safety || {}
  $("#limitA").value = s.limitA ?? 80
  $("#limitB").value = s.limitB ?? 80
  $("#decayEnabled").checked = s.decayEnabled !== false
  $("#decayRate").value = s.decayRate ?? 2
  updateLimitHints()
}

function updateLimitHints() {
  const setHint = (id, appLimit) => {
    const el = $(id)
    if (el) el.textContent = `(APP上限: ${appLimit})`
  }
  setHint("#hint-limitA", appLimits.a)
  setHint("#hint-limitB", appLimits.b)
}

function safetyLimit(channel) {
  const safety = currentConfig.safety || {}
  return channel === "A" ? (safety.limitA ?? 80) : (safety.limitB ?? 80)
}

function effectiveLimit(channel) {
  return Math.min(safetyLimit(channel), appLimits[channel.toLowerCase()])
}

function renderRules() {
  const container = $("#rules-list")
  container.innerHTML = ""
  for (let i = 0; i < currentRules.length; i++) {
    const r = currentRules[i]
    const div = document.createElement("div")
    div.className = "rule-item"
    div.innerHTML = `
      <span class="rule-name">${escapeHtml(r.giftName)}</span>
      <span class="rule-effect">${r.channel === "both" ? "双通道" : r.channel + "通道"} +${r.strengthAdd} 持续${r.duration}s</span>
      <button class="btn btn-danger btn-small" data-idx="${i}">删除</button>
    `
    container.appendChild(div)
  }

  container.querySelectorAll("[data-idx]").forEach(btn => {
    btn.onclick = async () => {
      const idx = parseInt(btn.dataset.idx)
      currentRules = currentRules.filter((_, i) => i !== idx)
      await api.config.rules.set(currentRules)
      renderRules()
    }
  })
}

function setupEventListeners() {
  $("#emergency-btn").onclick = async () => {
    await api.coyote.emergency()
  }

  $("#zero-btn").onclick = async () => {
    await api.coyote.strength("A", 0)
    await api.coyote.strength("B", 0)
  }

  $$("[data-channel][data-action]").forEach(btn => {
    btn.onclick = () => {
      const ch = btn.dataset.channel
      const action = btn.dataset.action
      let delta = 0
      switch (action) {
        case "inc": delta = 1; break
        case "inc5": delta = 5; break
        case "dec": delta = -1; break
        case "dec5": delta = -5; break
      }
      const current = ch === "A" ? lastStrengthA : lastStrengthB
      const newVal = Math.max(0, Math.min(current + delta, effectiveLimit(ch)))
      api.coyote.strength(ch, newVal)
    }
  })

  $("#bilibili-start").onclick = async () => {
    const appKey = $("#appKey").value.trim()
    const appSecret = $("#appSecret").value.trim()
    const code = $("#code").value.trim()
    const appId = parseInt($("#appId").value)

    if (!appKey || !appSecret) {
      alert("请填写 AppKey 和 AppSecret")
      return
    }
    if (!code || !appId) {
      alert("请填写主播身份码和 App ID")
      return
    }

    const startBtn = $("#bilibili-start")
    startBtn.textContent = "连接中..."
    startBtn.disabled = true

    try {
      const startResult = await api.bilibili.start(code, appId, appKey, appSecret)
      if (startResult.error) {
        alert("连接失败: " + startResult.error)
        return
      }
    } catch (e) {
      alert("启动失败: " + (e.message || "未知错误"))
    } finally {
      startBtn.textContent = "开始监听"
      startBtn.disabled = false
    }
  }

  $("#bilibili-stop").onclick = async () => {
    await api.bilibili.stop()
  }

  $("#save-safety").onclick = async () => {
    const safety = {
      limitA: parseInt($("#limitA").value),
      limitB: parseInt($("#limitB").value),
      decayEnabled: $("#decayEnabled").checked,
      decayRate: parseInt($("#decayRate").value),
    }

    await api.config.set({ safety })

    currentConfig.safety = safety
    await loadStatus()
  }

  $("#add-rule-btn").onclick = async () => {
    const name = $("#rule-name").value.trim()
    const channel = $("#rule-channel").value
    const strength = parseInt($("#rule-strength").value)
    const duration = parseInt($("#rule-duration").value)

    if (!name || !strength || !duration) return

    currentRules.push({
      giftName: name,
      coinType: "all",
      channel,
      strengthAdd: strength,
      duration,
    })

    await api.config.rules.set(currentRules)
    renderRules()

    $("#rule-name").value = ""
    $("#rule-strength").value = ""
    $("#rule-duration").value = ""
  }
}

function setupWSEvents() {
  ws.on("bilibili:status", updateBilibiliStatus)
  ws.on("coyote:status", updateCoyoteStatus)
  ws.on("strength", (data) => {
    updateStrengthBars(
      data.channel === "A" ? data.value : undefined,
      data.channel === "B" ? data.value : undefined,
    )
  })
  ws.on("gift", addGiftLog)
  // 断线重连后重新同步状态，避免错过断开期间的状态变化
  ws.on("reconnect", async () => {
    console.log("[Panel] Reconnected, refreshing state")
    qrLoaded = false
    await loadConfig()
    await loadStatus()
  })
}

function updateBilibiliStatus(data) {
  const dot = $("#bili-dot")
  const text = $("#bili-text")
  if (data.connected) {
    dot.className = "status-dot online"
    text.textContent = data.roomId ? `已连接 房间 ${data.roomId}` : "已连接"
  } else {
    dot.className = "status-dot offline"
    text.textContent = data.error || "未连接"
  }
}

function updateCoyoteStatus(data) {
  const dot = $("#coyote-dot")
  const text = $("#coyote-text")
  const qrView = $("#qr-view")
  const pairDetail = $("#pair-detail")
  if (data.paired) {
    dot.className = "status-dot online"
    text.textContent = "已配对"
    qrView.style.display = "none"
    pairDetail.style.display = "block"
  } else {
    dot.className = "status-dot offline"
    text.textContent = "等待配对"
    qrView.style.display = "block"
    pairDetail.style.display = "none"
    appLimits = { a: 200, b: 200 }
    const elA = $("#app-limit-a")
    const elB = $("#app-limit-b")
    if (elA) elA.textContent = "--"
    if (elB) elB.textContent = "--"
    updateLimitHints()
    updateStrengthBars(0, 0)
    if (!qrLoaded) loadQRCode()
    return
  }
  if (data.limitA !== undefined) {
    appLimits.a = data.limitA
    appLimits.b = data.limitB
    const elA = $("#app-limit-a")
    const elB = $("#app-limit-b")
    if (elA) elA.textContent = data.limitA
    if (elB) elB.textContent = data.limitB
    updateLimitHints()
  }
  if (data.strengthA !== undefined) {
    updateStrengthBars(data.strengthA, data.strengthB)
  }
  updatePairDetail()
}

function updatePairDetail() {
  const elPort = $("#pair-ws-port")
  if (elPort) elPort.textContent = currentConfig.coyote?.wsPort ?? 9999
}

function updateStrengthBars(a, b) {
  const limitA = effectiveLimit("A")
  const limitB = effectiveLimit("B")
  if (a !== undefined) lastStrengthA = Math.min(a, limitA)
  if (b !== undefined) lastStrengthB = Math.min(b, limitB)

  const fillA = $("#bar-a-fill")
  const fillB = $("#bar-b-fill")
  const valA = $("#bar-a-val")
  const valB = $("#bar-b-val")

  if (fillA) {
    fillA.style.width = limitA > 0 ? `${(lastStrengthA / limitA) * 100}%` : "0%"
    valA.textContent = `${lastStrengthA}/${limitA}`
  }
  if (fillB) {
    fillB.style.width = limitB > 0 ? `${(lastStrengthB / limitB) * 100}%` : "0%"
    valB.textContent = `${lastStrengthB}/${limitB}`
  }

  const ctrlA = $("#ctrl-a-val")
  const ctrlB = $("#ctrl-b-val")
  if (ctrlA) ctrlA.textContent = lastStrengthA
  if (ctrlB) ctrlB.textContent = lastStrengthB
}

function addGiftLog(data) {
  const log = $("#gift-log")
  const item = document.createElement("div")
  item.className = "gift-log-item"
  const time = new Date(data.timestamp * 1000).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" })
  item.innerHTML = `
    <span class="time">${time}</span>
    <span class="user">${escapeHtml(data.uname)}</span>
    <span class="gift">${escapeHtml(data.giftName)} x${data.num}</span>
    <span class="effect">${escapeHtml(data.strengthDelta)}</span>
  `
  log.prepend(item)
  while (log.children.length > 50) log.removeChild(log.lastChild)
}

async function loadQRCode() {
  try {
    const data = await api.coyote.qrcode()
    if (data.qrcode) {
      const img = $("#qr-img")
      img.src = data.qrcode
      img.style.display = "block"
      $("#qr-status").textContent = "用 DG-LAB APP 扫描二维码配对"
      qrLoaded = true
    } else {
      $("#qr-status").textContent = data.error || "二维码生成失败"
    }
  } catch {
    $("#qr-status").textContent = "二维码生成失败"
  }
}

init()
