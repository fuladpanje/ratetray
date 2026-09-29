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
  rial: number | null;
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
const listEl = document.getElementById("cur-list")!;

let lastToman: number | null = null;
let lastName = "دلار";

document.addEventListener("contextmenu", (e) => e.preventDefault());

function tomanOf(r: CurrencyRow): number | null {
  return r.rial === null ? null : Math.floor(r.rial / 10);
}

function fmtPrice(t: number | null): string {
  return t === null ? "—" : t.toLocaleString("en-US");
}

const GROUP_TITLES: Record<string, string> = {
  metal: "طلا و نقره",
  fx: "ارزها",
};

function makeSection(group: string): HTMLDivElement {
  const d = document.createElement("div");
  d.className = "cur-sec";
  d.textContent = GROUP_TITLES[group] || group;
  return d;
}

function makeRow(it: CurrencyRow, selected: boolean): HTMLButtonElement {
  const b = document.createElement("button");
  b.className = "cur-row" + (selected ? " selected" : "");
  b.type = "button";

  const sym = document.createElement("span");
  sym.className = "cur-sym";
  sym.textContent = it.symbol;

  const name = document.createElement("span");
  name.className = "cur-name";
  name.textContent = it.name;

  const t = tomanOf(it);
  const price = document.createElement("span");
  price.className = "cur-price";
  price.textContent = fmtPrice(t);

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
    lastToman = tomanOf(sel);
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
  if (lastToman === null) return;
  const text = `${lastName}: ${lastToman.toLocaleString("en-US")} تومان`;
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
});
