import { createHash } from "node:crypto";

const MIXIN_KEY_ENC_TAB = [
  46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29, 28, 14, 39, 12, 38, 41,
  13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34,
  44, 52,
];

function getMixinKey(raw: string): string {
  return MIXIN_KEY_ENC_TAB.map((i) => raw[i])
    .join("")
    .substring(0, 32);
}

function signWbi(params: Record<string, unknown>, mixinKey: string): string {
  const wts = Math.floor(Date.now() / 1000);
  const all: Record<string, unknown> = { ...params, wts };
  const query = Object.keys(all)
    .sort()
    .map((k) => `${encodeURIComponent(k)}=${encodeURIComponent(String(all[k]).replace(/[!'()*]/g, ""))}`)
    .join("&");
  const wRid = createHash("md5")
    .update(query + mixinKey)
    .digest("hex");
  return `${query}&w_rid=${wRid}`;
}

const UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

interface BiliResponse<T> {
  code: number;
  data: T;
}
interface SpiData {
  b_3: string;
}
interface RoomInitData {
  room_id: number;
}
interface NavData {
  wbi_img: { img_url: string; sub_url: string };
}
interface DanmuInfoData {
  token: string;
  host_list: { host: string }[];
}

async function apiGet<T>(url: string, headers: Record<string, string> = {}): Promise<T> {
  const res = await fetch(url, { headers: { "User-Agent": UA, ...headers } });
  return res.json() as Promise<T>;
}

async function getBuvid3(): Promise<string> {
  const spi = await apiGet<BiliResponse<SpiData>>("https://api.bilibili.com/x/frontend/finger/spi");
  return spi.data.b_3;
}

async function resolveRoomId(roomId: number): Promise<number> {
  const res = await apiGet<BiliResponse<RoomInitData>>(
    `https://api.live.bilibili.com/room/v1/Room/mobileRoomInit?id=${roomId}`,
  );
  return res.data.room_id;
}

export async function fetchDanmuInfo(roomId: number): Promise<{ key: string; urls: string[]; roomId: number }> {
  const buvid3 = await getBuvid3();
  const longRoomId = await resolveRoomId(roomId);

  const nav = await apiGet<BiliResponse<NavData>>("https://api.bilibili.com/x/web-interface/nav", {
    Cookie: `buvid3=${buvid3}`,
  });
  const { img_url, sub_url } = nav.data.wbi_img;
  const imgKey = img_url.split("/").pop()?.split(".")[0] ?? "";
  const subKey = sub_url.split("/").pop()?.split(".")[0] ?? "";
  const mixinKey = getMixinKey(imgKey + subKey);

  const signed = signWbi({ id: longRoomId }, mixinKey);
  const danmu = await apiGet<BiliResponse<DanmuInfoData>>(
    `https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo?${signed}`,
    { Referer: "https://live.bilibili.com/", Cookie: `buvid3=${buvid3}` },
  );

  if (danmu.code !== 0) throw new Error(`getDanmuInfo failed: code=${danmu.code}`);

  return {
    key: danmu.data.token,
    urls: danmu.data.host_list.map((host) => `wss://${host.host}/sub`),
    roomId: longRoomId,
  };
}
