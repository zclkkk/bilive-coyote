async function request(url, init) {
  const r = await fetch(url, init);
  const data = r.status === 204 ? null : await r.json().catch(() => null);
  if (!r.ok) throw new Error(data?.error || `HTTP ${r.status}`);
  return data;
}

const json = (method) => (url, body) =>
  request(url, {
    method,
    headers: body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });

const get = (url) => request(url);
const post = json("POST");
const put = json("PUT");

export const api = {
  status: () => get("/api/status"),
  bilibili: {
    start: (params) => post("/api/bilibili/start", params),
    stop: () => post("/api/bilibili/stop"),
  },
  coyote: {
    qrcode: () => get("/api/coyote/qrcode"),
    strength: (channel, value) => post("/api/coyote/strength", { channel, value }),
    emergency: () => post("/api/coyote/emergency"),
  },
  config: {
    get: () => get("/api/config"),
    set: (data) => put("/api/config", data),
    rules: {
      get: () => get("/api/config/rules"),
      set: (rules) => put("/api/config/rules", rules),
    },
  },
};
