import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { openUrl } from "@tauri-apps/plugin-opener";

interface CurrencyRow {
  id: string;
  name: string;
  symbol: string;
  group: string;
  unit: string;
  value: number | null;
  change_pct: number | null;
  updated_at: string | null;
  error: string | null;
}

interface AllPrices {
  selected: string;
  items: CurrencyRow[];
}

const errorEl = document.getElementById("error")!;
const copyBtn = document.getElementById("copy") as HTMLButtonElement;
const intervalSel = document.getElementById("interval") as HTMLSelectElement;
const autostartChk = document.getElementById("autostart") as HTMLInputElement;
const shownumberChk = document.getElementById("shownumber") as HTMLInputElement;
const listEl = document.getElementById("cur-list")!;

let lastValue: number | null = null;
let lastName = "دلار";
let lastUnit = "تومان";

document.addEventListener("contextmenu", (e) => e.preventDefault());

function fmtValue(v: number | null, unit: string): string {
  if (v === null) return "—";
  if (unit === "دلار") {
    return v >= 100 ? Math.round(v).toLocaleString("en-US") : v.toFixed(2);
  }
  return Math.floor(v).toLocaleString("en-US");
}

function fmtPrice(r: CurrencyRow): string {
  return fmtValue(r.value, r.unit);
}

const GROUP_META: Record<string, { title: string; unit: string }> = {
  metal: { title: "طلا و نقره", unit: "تومان" },
  base: { title: "فلزات پایه", unit: "دلار" },
  oil: { title: "نفت", unit: "دلار" },
  crypto: { title: "ارز دیجیتال", unit: "دلار" },
  fx: { title: "ارزها", unit: "تومان" },
};

function makeSection(group: string): HTMLDivElement {
  const d = document.createElement("div");
  d.className = "cur-sec";
  const meta = GROUP_META[group] || { title: group, unit: "" };
  const title = document.createElement("span");
  title.textContent = meta.title;
  const unit = document.createElement("span");
  unit.className = "sec-unit";
  unit.textContent = meta.unit;
  d.append(title, unit);
  return d;
}

function makeRow(it: CurrencyRow, selected: boolean): HTMLButtonElement {
  const b = document.createElement("button");
  b.className = "cur-row" + (selected ? " selected" : "");
  b.type = "button";

  const sym = document.createElement("span");
  sym.className = "cur-sym" + (it.symbol.length > 3 ? " long" : "");
  sym.textContent = it.symbol;

  const name = document.createElement("span");
  name.className = "cur-name";
  name.textContent = it.name;

  const price = document.createElement("span");
  price.className = "cur-price";
  price.textContent = fmtPrice(it);

  const change = document.createElement("span");
  if (it.change_pct !== null) {
    const up = it.change_pct >= 0;
    change.className = `cur-change ${up ? "up" : "down"}`;
    change.textContent = `${up ? "▲" : "▼"} ${Math.abs(it.change_pct).toFixed(2)}%`;
  } else {
    change.className = "cur-change";
  }

  const time = document.createElement("span");
  time.className = "cur-time";
  time.textContent = it.updated_at || "";

  b.append(sym, name, price, change, time);
  b.addEventListener("click", async () => {
    const p = await invoke<AllPrices>("set_currency", { id: it.id });
    render(p);
  });
  return b;
}

function render(p: AllPrices) {
  const sel = p.items.find((i) => i.id === p.selected);
  if (sel) {
    lastName = sel.name;
    lastValue = sel.value;
    lastUnit = sel.unit;
  }

  listEl.replaceChildren();
  let lastGroup = "";
  for (const it of p.items) {
    if (it.group !== lastGroup) {
      listEl.appendChild(makeSection(it.group));
      lastGroup = it.group;
    }
    listEl.appendChild(makeRow(it, it.id === p.selected));
  }

  const firstErr = p.items.map((i) => i.error).find((e) => e);
  errorEl.textContent = firstErr ? `⚠ ${firstErr}` : "";
}

const copyOrig = copyBtn.innerHTML;

async function copyPrice() {
  if (lastValue === null) return;
  const text = `${lastName}: ${fmtValue(lastValue, lastUnit)} ${lastUnit}`;
  try {
    await writeText(text);
    copyBtn.innerHTML = "کپی شد ✓";
    setTimeout(() => {
      copyBtn.innerHTML = copyOrig;
    }, 1500);
  } catch (e) {
    errorEl.textContent = `⚠ کپی ناموفق: ${e}`;
  }
}

async function initInterval() {
  const saved = localStorage.getItem("refresh_secs");
  if (saved) {
    const secs = await invoke<number>("set_refresh_interval", {
      secs: Number(saved),
    });
    intervalSel.value = String(secs);
  } else {
    intervalSel.value = String(await invoke<number>("get_refresh_interval"));
  }

  intervalSel.addEventListener("change", async () => {
    const secs = await invoke<number>("set_refresh_interval", {
      secs: Number(intervalSel.value),
    });
    intervalSel.value = String(secs);
    localStorage.setItem("refresh_secs", String(secs));
  });
}

async function initAutostart() {
  try {
    autostartChk.checked = await isEnabled();
  } catch {
    autostartChk.disabled = true;
    errorEl.textContent = "⚠ دسترسی اجرای خودکار در دسترس نیست";
  }

  autostartChk.addEventListener("change", async () => {
    try {
      if (autostartChk.checked) {
        await enable();
      } else {
        await disable();
      }
      autostartChk.checked = await isEnabled();
    } catch (e) {
      autostartChk.checked = await isEnabled().catch(() => false);
      errorEl.textContent = `⚠ خطا در تنظیم اجرا خودکار: ${e}`;
    }
  });
}

async function initShowNumber() {
  try {
    shownumberChk.checked = await invoke<boolean>("get_show_number");
  } catch {
    shownumberChk.checked = true;
  }

  shownumberChk.addEventListener("change", async () => {
    try {
      shownumberChk.checked = await invoke<boolean>("set_show_number", {
        enabled: shownumberChk.checked,
      });
    } catch (e) {
      errorEl.textContent = `⚠ خطا: ${e}`;
    }
  });
}

window.addEventListener("DOMContentLoaded", () => {
  invoke<AllPrices>("get_all_prices").then(render);
  listen<AllPrices>("prices-updated", (e) => render(e.payload));

  document.getElementById("refresh")!.addEventListener("click", () => {
    invoke("refresh_now");
  });
  copyBtn.addEventListener("click", copyPrice);

  document.getElementById("gh-link")!.addEventListener("click", (e) => {
    e.preventDefault();
    openUrl("https://github.com/fuladpanje").catch((err) => {
      errorEl.textContent = `⚠ باز نشدن لینک: ${err}`;
    });
  });

  initInterval();
  initAutostart();
  initShowNumber();
});
