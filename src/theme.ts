import { getCurrentWindow } from "@tauri-apps/api/window";

// 主题切换：在 <html> 上设置 data-skin（主题）和 data-mode（浅色/深色），样式由 style.css 中的变量决定
export type Skin = "default" | "mist";
export type Appearance = "system" | "light" | "dark";

const CACHE_KEY = "vd-theme";
const media = window.matchMedia("(prefers-color-scheme: dark)");
let current: { skin: Skin; appearance: Appearance } = { skin: "default", appearance: "system" };

/** 应用主题；外观为“跟随系统”时按系统深浅色决定 */
export function applyTheme(skin: Skin, appearance: Appearance) {
  current = { skin, appearance };
  const mode = appearance === "system" ? (media.matches ? "dark" : "light") : appearance;
  document.documentElement.dataset.skin = skin;
  document.documentElement.dataset.mode = mode;
  // 原生标题栏同步深浅色；null 表示跟随系统
  getCurrentWindow()
    .setTheme(appearance === "system" ? null : appearance)
    .catch(() => {
      /* 非 Tauri 环境（如 mock 预览）下忽略 */
    });
  // 缓存到本地，下次打开窗口时在界面挂载前就套用，避免先闪一下默认主题
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(current));
  } catch {
    /* 存储不可用时忽略，只影响首帧 */
  }
}

/** 启动时套用上次缓存的主题；配置加载后会再按设置应用一次 */
export function restoreCachedTheme() {
  let cached = current;
  try {
    cached = { ...current, ...JSON.parse(localStorage.getItem(CACHE_KEY) ?? "{}") };
  } catch {
    /* 缓存损坏时用默认值 */
  }
  applyTheme(cached.skin, cached.appearance);
}

// 系统深浅色变化时，“跟随系统”立即生效
media.addEventListener("change", () => applyTheme(current.skin, current.appearance));
