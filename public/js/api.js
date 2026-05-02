export const api = {
  async get(url) {
    const r = await fetch(url)
    return r.json()
  },
  async post(url, body) {
    const r = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    })
    return r.json()
  },
  async put(url, body) {
    const r = await fetch(url, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    })
    return r.json()
  },

  bilibili: {
    start: (params) => api.post("/api/bilibili/start", params),
    stop: () => api.post("/api/bilibili/stop"),
  },

  coyote: {
    qrcode: () => api.get("/api/coyote/qrcode"),
    strength: (channel, value) => api.post("/api/coyote/strength", { channel, value }),
    emergency: () => api.post("/api/coyote/emergency"),
  },

  config: {
    get: () => api.get("/api/config"),
    set: (data) => api.put("/api/config", data),
    rules: {
      get: () => api.get("/api/config/rules"),
      set: (rules) => api.put("/api/config/rules", rules),
    },
  },

  status: () => api.get("/api/status"),
}
