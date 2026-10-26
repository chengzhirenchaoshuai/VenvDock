// 检查更新：查询 GitHub 上的最新发布版本（GitHub API 允许跨域，不需要后端参与）
import { getVersion } from "@tauri-apps/api/app";

const REPO = "chengzhirenchaoshuai/VenvDock";
/** 自动检查的最短间隔 */
const AUTO_INTERVAL = 24 * 3600 * 1000;
const LAST_CHECK_KEY = "venvdock.lastUpdateCheck";

/** 发布中的一个文件，与 Rust 端 updater::ReleaseAsset 对应 */
export interface ReleaseAsset {
  name: string;
  url: string;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  /** 发布页地址 */
  url: string;
  hasUpdate: boolean;
  assets: ReleaseAsset[];
}

/** 比较 x.y.z 形式的版本号，a 比 b 新时返回 true */
function newer(a: string, b: string): boolean {
  const pa = a.split(".").map((n) => parseInt(n, 10) || 0);
  const pb = b.split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d) return d > 0;
  }
  return false;
}

export async function checkUpdate(): Promise<UpdateInfo> {
  const [current, res] = await Promise.all([
    getVersion(),
    fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
    }),
  ]);
  if (!res.ok) throw new Error(`查询最新版本失败（HTTP ${res.status}）`);
  const release = (await res.json()) as {
    tag_name: string;
    html_url: string;
    assets: { name: string; browser_download_url: string }[];
  };
  const latest = release.tag_name.replace(/^v/i, "");
  const assets = release.assets.map((a) => ({ name: a.name, url: a.browser_download_url }));
  return { current, latest, url: release.html_url, hasUpdate: newer(latest, current), assets };
}

/** 距上次自动检查超过一天才检查；失败时静默。浏览器存储不可用时每次都检查 */
export async function autoCheckUpdate(): Promise<UpdateInfo | null> {
  try {
    const last = Number(localStorage.getItem(LAST_CHECK_KEY) ?? 0);
    if (Date.now() - last < AUTO_INTERVAL) return null;
    localStorage.setItem(LAST_CHECK_KEY, String(Date.now()));
  } catch {
    // 忽略存储错误
  }
  try {
    const info = await checkUpdate();
    return info.hasUpdate ? info : null;
  } catch {
    return null;
  }
}
