// ANSI 控制符解析：把一行日志拆成带颜色样式的片段（只处理 SGR 颜色和字形，其余控制符直接去掉）

export interface Span {
  text: string;
  /** 内联样式，空字符串表示默认样式 */
  style: string;
}

/** 标准 8 色与高亮 8 色，按深色日志底调过亮度 */
const BASE = ["#52525b", "#f87171", "#4ade80", "#facc15", "#60a5fa", "#c084fc", "#22d3ee", "#d4d4d8"];
const BRIGHT = ["#a1a1aa", "#fca5a5", "#86efac", "#fde047", "#93c5fd", "#d8b4fe", "#67e8f9", "#fafafa"];
const CUBE = [0, 95, 135, 175, 215, 255];

/** 256 色表：0-15 为基本色，16-231 为 6×6×6 色立方，232-255 为灰阶 */
function color256(n: number): string | undefined {
  if (n < 0 || n > 255 || !Number.isInteger(n)) return undefined;
  if (n < 8) return BASE[n];
  if (n < 16) return BRIGHT[n - 8];
  if (n < 232) {
    const c = n - 16;
    return `rgb(${CUBE[Math.floor(c / 36)]} ${CUBE[Math.floor(c / 6) % 6]} ${CUBE[c % 6]})`;
  }
  const g = 8 + 10 * (n - 232);
  return `rgb(${g} ${g} ${g})`;
}

interface State {
  fg?: string;
  bg?: string;
  bold: boolean;
  dim: boolean;
  italic: boolean;
  underline: boolean;
}

const plain = (): State => ({ bold: false, dim: false, italic: false, underline: false });

function toStyle(s: State): string {
  let css = "";
  if (s.fg) css += `color:${s.fg};`;
  if (s.bg) css += `background-color:${s.bg};`;
  if (s.bold) css += "font-weight:600;";
  if (s.dim) css += "opacity:0.7;";
  if (s.italic) css += "font-style:italic;";
  if (s.underline) css += "text-decoration:underline;";
  return css;
}

/** 解析 38/48 后面的扩展颜色参数，返回颜色和消耗的参数个数 */
function extended(codes: number[], i: number): [string | undefined, number] {
  if (codes[i + 1] === 5) return [color256(codes[i + 2]), 2];
  if (codes[i + 1] === 2) {
    const [r, g, b] = codes.slice(i + 2, i + 5);
    return [[r, g, b].every((v) => v >= 0 && v <= 255) ? `rgb(${r} ${g} ${b})` : undefined, 4];
  }
  return [undefined, 0];
}

function apply(s: State, params: string): State {
  const codes = params === "" ? [0] : params.split(";").map((p) => (p === "" ? 0 : Number(p)));
  for (let i = 0; i < codes.length; i++) {
    const c = codes[i];
    if (c === 0) s = plain();
    else if (c === 1) s.bold = true;
    else if (c === 2) s.dim = true;
    else if (c === 3) s.italic = true;
    else if (c === 4) s.underline = true;
    else if (c === 22) s.bold = s.dim = false;
    else if (c === 23) s.italic = false;
    else if (c === 24) s.underline = false;
    else if (c >= 30 && c <= 37) s.fg = BASE[c - 30];
    else if (c >= 90 && c <= 97) s.fg = BRIGHT[c - 90];
    else if (c === 39) s.fg = undefined;
    else if (c >= 40 && c <= 47) s.bg = BASE[c - 40];
    else if (c >= 100 && c <= 107) s.bg = BRIGHT[c - 100];
    else if (c === 49) s.bg = undefined;
    else if (c === 38 || c === 48) {
      const [color, used] = extended(codes, i);
      if (c === 38) s.fg = color;
      else s.bg = color;
      i += used;
    }
  }
  return s;
}

const CSI = /\x1b\[([0-9;?]*)([A-Za-z])/g;

/**
 * 解析一行：返回去掉控制符的纯文本，以及带样式的片段。
 * 整行没有任何颜色样式时 spans 为 null，调用方可按内容自行着色
 */
export function parseAnsi(line: string): { text: string; spans: Span[] | null } {
  if (!line.includes("\x1b")) return { text: line, spans: null };
  const spans: Span[] = [];
  let state = plain();
  let style = "";
  let last = 0;
  for (const m of line.matchAll(CSI)) {
    if (m.index > last) spans.push({ text: line.slice(last, m.index), style });
    last = m.index + m[0].length;
    if (m[2] === "m" && !m[1].includes("?")) {
      state = apply({ ...state }, m[1]);
      style = toStyle(state);
    }
  }
  if (last < line.length) spans.push({ text: line.slice(last), style });
  // 去掉孤立的 ESC 等其余控制字符
  for (const s of spans) s.text = s.text.replace(/\x1b/g, "");
  const styled = spans.some((s) => s.style && s.text);
  const text = spans.map((s) => s.text).join("");
  return { text, spans: styled ? spans.filter((s) => s.text) : null };
}
