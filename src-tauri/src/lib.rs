mod icon;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};

const DEFAULT_REFRESH_SECS: u64 = 60;

#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Toman,
    Dollar,
}

impl Unit {
    fn label(self) -> &'static str {
        match self {
            Unit::Toman => "تومان",
            Unit::Dollar => "دلار",
        }
    }
}

struct Currency {
    id: &'static str,
    name: &'static str,
    symbol: &'static str,
    /// metal | base | oil | crypto | fx
    group: &'static str,
    unit: Unit,
    url: &'static str,
    /// info-bar element id; `None` = use the main price span of the profile page.
    elem: Option<&'static str>,
}

const CURRENCIES: &[Currency] = &[
    Currency {
        id: "geram18",
        name: "طلای ۱۸ عیار (گرم)",
        symbol: "Au",
        group: "metal",
        unit: Unit::Toman,
        url: "https://tgju.org/profile/geram18",
        elem: None,
    },
    Currency {
        id: "silver999",
        name: "نقره ۹۹۹ (گرم)",
        symbol: "Ag",
        group: "metal",
        unit: Unit::Toman,
        url: "https://tgju.org/profile/silver_999",
        elem: None,
    },
    Currency {
        id: "aluminum",
        name: "آلومینیوم",
        symbol: "Al",
        group: "base",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/aluminum",
        elem: None,
    },
    Currency {
        id: "copper",
        name: "مس",
        symbol: "Cu",
        group: "base",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/copper",
        elem: None,
    },
    Currency {
        id: "lead",
        name: "سرب",
        symbol: "Pb",
        group: "base",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/lead",
        elem: None,
    },
    Currency {
        id: "zinc",
        name: "روی",
        symbol: "Zn",
        group: "base",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/zinc",
        elem: None,
    },
    Currency {
        id: "nickel",
        name: "نیکل",
        symbol: "Ni",
        group: "base",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/nickel",
        elem: None,
    },
    Currency {
        id: "oilbrent",
        name: "نفت برنت",
        symbol: "Br",
        group: "oil",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/oil_brent",
        elem: None,
    },
    Currency {
        id: "btc",
        name: "بیت کوین",
        symbol: "BTC",
        group: "crypto",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/crypto-bitcoin",
        elem: None,
    },
    Currency {
        id: "eth",
        name: "اتریوم",
        symbol: "ETH",
        group: "crypto",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/crypto-ethereum",
        elem: None,
    },
    Currency {
        id: "usdt",
        name: "تتر",
        symbol: "USDT",
        group: "crypto",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/crypto-tether",
        elem: None,
    },
    Currency {
        id: "trx",
        name: "ترون",
        symbol: "TRX",
        group: "crypto",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/crypto-tron",
        elem: None,
    },
    Currency {
        id: "xrp",
        name: "ریپل",
        symbol: "XRP",
        group: "crypto",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/crypto-ripple",
        elem: None,
    },
    Currency {
        id: "ada",
        name: "کاردانو",
        symbol: "ADA",
        group: "crypto",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/crypto-cardano",
        elem: None,
    },
    Currency {
        id: "doge",
        name: "دوج کوین",
        symbol: "DOGE",
        group: "crypto",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/crypto-dogecoin",
        elem: None,
    },
    Currency {
        id: "sol",
        name: "سولانا",
        symbol: "SOL",
        group: "crypto",
        unit: Unit::Dollar,
        url: "https://tgju.org/profile/crypto-solana",
        elem: None,
    },
    Currency {
        id: "dollar",
        name: "دلار",
        symbol: "$",
        group: "fx",
        unit: Unit::Toman,
        url: "https://tgju.org/profile/price_dollar_rtl",
        elem: Some("l-price_dollar_rl"),
    },
    Currency {
        id: "eur",
        name: "یورو",
        symbol: "€",
        group: "fx",
        unit: Unit::Toman,
        url: "https://tgju.org/profile/price_eur",
        elem: None,
    },
    Currency {
        id: "cad",
        name: "دلار کانادا",
        symbol: "C$",
        group: "fx",
        unit: Unit::Toman,
        url: "https://tgju.org/profile/price_cad",
        elem: None,
    },
    Currency {
        id: "try",
        name: "لیر ترکیه",
        symbol: "₺",
        group: "fx",
        unit: Unit::Toman,
        url: "https://tgju.org/profile/price_try",
        elem: None,
    },
    Currency {
        id: "gbp",
        name: "پوند انگلیس",
        symbol: "£",
        group: "fx",
        unit: Unit::Toman,
        url: "https://tgju.org/profile/price_gbp",
        elem: None,
    },
    Currency {
        id: "aed",
        name: "درهم امارات",
        symbol: "د.إ",
        group: "fx",
        unit: Unit::Toman,
        url: "https://tgju.org/profile/price_aed",
        elem: None,
    },
    Currency {
        id: "cny",
        name: "یوان چین",
        symbol: "¥",
        group: "fx",
        unit: Unit::Toman,
        url: "https://tgju.org/profile/price_cny",
        elem: None,
    },
    Currency {
        id: "jpy",
        name: "ین ژاپن (۱۰۰)",
        symbol: "¥",
        group: "fx",
        unit: Unit::Toman,
        url: "https://tgju.org/profile/price_jpy",
        elem: None,
    },
];

fn find_currency(id: &str) -> Option<&'static Currency> {
    CURRENCIES.iter().find(|c| c.id == id)
}

/// Display order of the sections in the window.
const GROUP_ORDER: &[&str] = &["metal", "fx", "crypto", "oil", "base"];

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
struct CurrencyPrice {
    /// Price in native unit: toman for Toman items, dollars for Dollar items.
    value: Option<f64>,
    change_pct: Option<f64>,
    updated_at: Option<String>,
    error: Option<String>,
}

struct AppState {
    prices: Mutex<HashMap<String, CurrencyPrice>>,
    selected: Mutex<String>,
    refresh_secs: AtomicU64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            prices: Mutex::new(HashMap::new()),
            selected: Mutex::new("dollar".to_string()),
            refresh_secs: AtomicU64::new(DEFAULT_REFRESH_SECS),
        }
    }
}

#[derive(serde::Serialize, Clone)]
struct CurrencyRow {
    id: String,
    name: String,
    symbol: String,
    group: String,
    unit: String,
    value: Option<f64>,
    change_pct: Option<f64>,
    updated_at: Option<String>,
    error: Option<String>,
}

#[derive(serde::Serialize, Clone)]
struct AllPrices {
    selected: String,
    items: Vec<CurrencyRow>,
}

fn snapshot(state: &AppState) -> AllPrices {
    let selected = state.selected.lock().unwrap().clone();
    let map = state.prices.lock().unwrap();
    let items = GROUP_ORDER
        .iter()
        .flat_map(|g| CURRENCIES.iter().filter(move |c| c.group == *g))
        .map(|c| {
            let p = map.get(c.id).cloned().unwrap_or_default();
            CurrencyRow {
                id: c.id.to_string(),
                name: c.name.to_string(),
                symbol: c.symbol.to_string(),
                group: c.group.to_string(),
                unit: c.unit.label().to_string(),
                value: p.value,
                change_pct: p.change_pct,
                updated_at: p.updated_at,
                error: p.error,
            }
        })
        .collect();
    AllPrices { selected, items }
}

fn parse_price(html: &str, cur: &Currency) -> Result<(f64, Option<f64>), String> {
    if let Some(elem) = cur.elem {
        let re = regex::Regex::new(&format!(
            r#"(?s)<li id="{elem}".*?info-price">([\d,]+)</span>.*?info-change">\((-?[\d.]+)%\)"#
        ))
        .map_err(|_| "خطای داخلی برنامه".to_string())?;
        let caps = re
            .captures(html)
            .ok_or_else(|| "اطلاعات قیمت در پاسخ سایت پیدا نشد".to_string())?;
        let rial: f64 = caps[1]
            .replace(',', "")
            .parse::<f64>()
            .map_err(|_| "پاسخ سایت قابل پردازش نبود".to_string())?;
        let change: f64 = caps[2]
            .parse()
            .map_err(|_| "پاسخ سایت قابل پردازش نبود".to_string())?;
        Ok((rial / 10.0, Some(change)))
    } else {
        let re_price = regex::Regex::new(
            r#"<span class="price" data-col="info\.last_trade\.PDrCotVal">([\d,.]+)</span>"#,
        )
        .map_err(|_| "خطای داخلی برنامه".to_string())?;
        let re_change = regex::Regex::new(
            r#"<span class="change change-(up|down|no) change-percentage">([^<]*)<"#,
        )
        .map_err(|_| "خطای داخلی برنامه".to_string())?;

        let price_cap = re_price
            .captures(html)
            .ok_or_else(|| "اطلاعات قیمت در پاسخ سایت پیدا نشد".to_string())?;
        let raw: f64 = price_cap[1]
            .replace(',', "")
            .parse::<f64>()
            .map_err(|_| "پاسخ سایت قابل پردازش نبود".to_string())?;
        let value = match cur.unit {
            Unit::Toman => raw / 10.0,
            Unit::Dollar => raw,
        };

        let change_cap = re_change
            .captures(html)
            .ok_or_else(|| "اطلاعات قیمت در پاسخ سایت پیدا نشد".to_string())?;
        let change = if &change_cap[1] == "no" {
            None
        } else {
            let magnitude: f64 = change_cap[2]
                .trim()
                .parse()
                .map_err(|_| "پاسخ سایت قابل پردازش نبود".to_string())?;
            Some(if &change_cap[1] == "down" {
                -magnitude.abs()
            } else {
                magnitude.abs()
            })
        };
        Ok((value, change))
    }
}

/// Maps technical request errors to a user-friendly Persian message.
fn friendly(err: &str) -> String {
    if err.contains("error sending request")
        || err.contains("connection")
        || err.contains("timed out")
        || err.contains("dns")
        || err.contains("certificate")
        || err.contains("timeout")
    {
        "مشکل در برقراری ارتباط".to_string()
    } else {
        "مشکل در دریافت اطلاعات از سایت".to_string()
    }
}

async fn fetch_price(cur: &Currency) -> Result<(f64, Option<f64>), String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| "خطای داخلی برنامه".to_string())?;

    let html = client
        .get(cur.url)
        .send()
        .await
        .map_err(|e| friendly(&e.to_string()))?
        .text()
        .await
        .map_err(|_| "مشکل در برقراری ارتباط".to_string())?;

    parse_price(&html, cur)
}

fn group(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Short text for the tray icon: plain thousands (e.g. "254"),
/// one decimal below 10 thousand (e.g. "5.2"),
/// or millions with an M suffix once it no longer fits (e.g. "25M").
///
/// `usd` switches the same tiers to thousands of dollars.
fn icon_text(value: Option<f64>, usd: bool) -> String {
    match value {
        Some(v) => {
            if usd {
                // Dollars are already human-scale: show them directly.
                if v >= 10_000.0 {
                    let k = ((v + 500.0) / 1000.0).floor() as u64;
                    if k >= 1000 {
                        format!("{}M", ((v + 500_000.0) / 1_000_000.0).floor() as u64)
                    } else {
                        format!("{k}K")
                    }
                } else if v >= 10.0 {
                    format!("{}", (v + 0.5).floor() as u64)
                } else {
                    format!("{}.{}", v.floor() as u64, (v.fract() * 10.0).floor() as u64)
                }
            } else {
                let thousands = ((v + 500.0) / 1000.0).floor() as u64;
                if thousands >= 1000 {
                    format!("{}M", ((v + 500_000.0) / 1_000_000.0).floor() as u64)
                } else if thousands >= 10 {
                    format!("{thousands}")
                } else {
                    let t = v.floor() as u64;
                    format!("{}.{}", t / 1000, (t % 1000) / 100)
                }
            }
        }
        None => "--".to_string(),
    }
}

/// Row/tooltip price for dollar-denominated items.
fn fmt_usd(v: f64) -> String {
    if v >= 100.0 {
        group((v + 0.5).floor() as u64)
    } else {
        format!("{:.2}", (v * 100.0).round() / 100.0)
    }
}

fn apply_selected(app: &AppHandle) {
    let state = app.state::<AppState>();
    let sel = state.selected.lock().unwrap().clone();
    let entry = state
        .prices
        .lock()
        .unwrap()
        .get(&sel)
        .cloned()
        .unwrap_or_default();
    let cur = find_currency(&sel);
    let usd = matches!(cur.map(|c| c.unit), Some(Unit::Dollar));
    let name = cur.map(|c| c.name).unwrap_or("قیمت");
    let unit_label = cur.map(|c| c.unit.label()).unwrap_or("تومان");
    if let Some(tray) = app.tray_by_id("main") {
        let text = icon_text(entry.value, usd);
        let img = Image::new_owned(icon::render(&text), W, H);
        let _ = tray.set_icon(Some(img));
        let tooltip = match entry.value {
            Some(v) => {
                let price = if usd {
                    fmt_usd(v)
                } else {
                    group(v.floor() as u64)
                };
                let change = entry
                    .change_pct
                    .map(|c| format!(" | {c:+.2}%"))
                    .unwrap_or_default();
                let time = entry
                    .updated_at
                    .as_deref()
                    .map(|t| format!(" | {t}"))
                    .unwrap_or_default();
                let err = if entry.error.is_some() { " | خطا" } else { "" };
                format!("{name}: {price} {unit_label}{change}{time}{err}")
            }
            None => format!("در حال دریافت قیمت {name}..."),
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

const W: u32 = 32;
const H: u32 = 32;

async fn do_fetch(app: &AppHandle, id: &str) {
    let cur = match find_currency(id) {
        Some(c) => c,
        None => return,
    };
    let state = app.state::<AppState>();
    match fetch_price(cur).await {
        Ok((value, change)) => {
            let mut map = state.prices.lock().unwrap();
            let entry = map.entry(id.to_string()).or_default();
            entry.value = Some(value);
            entry.change_pct = change;
            entry.updated_at = Some(chrono::Local::now().format("%H:%M").to_string());
            entry.error = None;
        }
        Err(e) => {
            let mut map = state.prices.lock().unwrap();
            map.entry(id.to_string()).or_default().error = Some(e);
        }
    }
    save_prices(app);
    let selected = state.selected.lock().unwrap().clone();
    if selected == id {
        apply_selected(app);
    }
    let _ = app.emit("prices-updated", snapshot(&state));
}

/// Fetches all currencies with limited concurrency (batches of 6):
/// fast, but gentle enough not to get throttled by the source site.
/// Failed items get one retry pass in smaller batches.
fn fetch_all_blocking(app: &AppHandle) {
    for chunk in CURRENCIES.chunks(6) {
        let handles: Vec<std::thread::JoinHandle<()>> = chunk
            .iter()
            .map(|c| {
                let app = app.clone();
                let id = c.id;
                std::thread::spawn(move || {
                    tauri::async_runtime::block_on(do_fetch(&app, id));
                })
            })
            .collect();
        for h in handles {
            let _ = h.join();
        }
    }
    let retry: Vec<&'static str> = {
        let state = app.state::<AppState>();
        let map = state.prices.lock().unwrap();
        CURRENCIES
            .iter()
            .filter(|c| map.get(c.id).is_some_and(|e| e.error.is_some()))
            .map(|c| c.id)
            .collect()
    };
    for chunk in retry.chunks(3) {
        let handles: Vec<std::thread::JoinHandle<()>> = chunk
            .iter()
            .map(|id| {
                let app = app.clone();
                let id = *id;
                std::thread::spawn(move || {
                    tauri::async_runtime::block_on(do_fetch(&app, id));
                })
            })
            .collect();
        for h in handles {
            let _ = h.join();
        }
    }
}

fn spawn_all(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        fetch_all_blocking(&app);
    });
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn spawn_selected(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let id = app.state::<AppState>().selected.lock().unwrap().clone();
        tauri::async_runtime::block_on(do_fetch(&app, &id));
    });
}

fn prices_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("prices.json"))
}

fn save_prices(app: &AppHandle) {
    if let Some(p) = prices_file(app) {
        let state = app.state::<AppState>();
        let map = state.prices.lock().unwrap();
        if let Ok(json) = serde_json::to_string(&*map) {
            let _ = std::fs::write(p, json);
        }
    }
}

fn load_prices(app: &AppHandle) {
    let map: Option<HashMap<String, CurrencyPrice>> = prices_file(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok());
    if let Some(map) = map {
        *app.state::<AppState>().prices.lock().unwrap() = map;
    }
}

fn currency_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("currency"))
}

fn load_currency(app: &AppHandle) -> String {
    currency_file(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| find_currency(s.trim()).map(|c| c.id.to_string()))
        .unwrap_or_else(|| "dollar".to_string())
}

fn save_currency(app: &AppHandle, id: &str) {
    if let Some(p) = currency_file(app) {
        let _ = std::fs::write(p, id);
    }
}

fn select_currency(app: &AppHandle, id: &str) -> AllPrices {
    if find_currency(id).is_some() {
        save_currency(app, id);
        *app.state::<AppState>().selected.lock().unwrap() = id.to_string();
    }
    let state = app.state::<AppState>();
    apply_selected(app);
    let snap = snapshot(&state);
    drop(state);
    let _ = app.emit("prices-updated", snap.clone());
    spawn_selected(app);
    snap
}

#[tauri::command]
fn get_all_prices(state: State<AppState>) -> AllPrices {
    snapshot(&state)
}

#[tauri::command]
fn get_refresh_interval(state: State<AppState>) -> u64 {
    state.refresh_secs.load(Ordering::Relaxed)
}

#[tauri::command]
fn set_refresh_interval(state: State<AppState>, secs: u64) -> u64 {
    let secs = secs.clamp(60, 3600);
    state.refresh_secs.store(secs, Ordering::Relaxed);
    secs
}

#[tauri::command]
fn refresh_now(app: AppHandle) {
    spawn_all(&app);
}

#[tauri::command]
fn set_currency(app: AppHandle, id: String) -> AllPrices {
    select_currency(&app, &id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_all_prices,
            refresh_now,
            get_refresh_interval,
            set_refresh_interval,
            set_currency
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            let current = load_currency(app.handle());
            *app.state::<AppState>().selected.lock().unwrap() = current;
            load_prices(app.handle());
            // Paint the tray instantly from cache; fresh prices arrive in seconds.
            apply_selected(app.handle());

            let refresh = MenuItem::with_id(app, "refresh", "تازه‌سازی قیمت", true, None::<&str>)?;
            let details = MenuItem::with_id(app, "details", "نمایش جزئیات", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "خروج", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&refresh, &details, &sep, &quit])?;

            let placeholder = Image::new_owned(icon::render("--"), W, H);

            TrayIconBuilder::with_id("main")
                .icon(placeholder)
                .tooltip("RateTray — در حال دریافت قیمت‌ها...")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "refresh" => spawn_all(app),
                    "details" => show_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_window(tray.app_handle());
                    }
                })
                .build(app)?;

            spawn_all(app.handle());

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut last = Instant::now();
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let interval = handle
                        .state::<AppState>()
                        .refresh_secs
                        .load(Ordering::Relaxed);
                    if last.elapsed().as_secs() >= interval {
                        fetch_all_blocking(&handle);
                        last = Instant::now();
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_text_formats() {
        assert_eq!(icon_text(Some(253_700.0), false), "254"); // dollar
        assert_eq!(icon_text(Some(25_265_500.0), false), "25M"); // gold gram
        assert_eq!(icon_text(Some(1_020_000.0), false), "1M"); // 1.02M toman
        assert_eq!(icon_text(Some(999_500.0), false), "1M"); // boundary
        assert_eq!(icon_text(Some(999_499.9), false), "999"); // just below
        assert_eq!(icon_text(Some(69_109.0), false), "69"); // two digits
        assert_eq!(icon_text(Some(5_266.0), false), "5.2"); // lira with decimal
        assert_eq!(icon_text(Some(1_700.0), false), "1.7");
        assert_eq!(icon_text(Some(500.0), false), "0.5");
        assert_eq!(icon_text(Some(83_650.17), true), "84K"); // bitcoin
        assert_eq!(icon_text(Some(14_404.4), true), "14K"); // copper
        assert_eq!(icon_text(Some(15_985.0), true), "16K"); // nickel
        assert_eq!(icon_text(Some(3_216.5), true), "3217"); // aluminum
        assert_eq!(icon_text(Some(2_692.52), true), "2693"); // ethereum
        assert_eq!(icon_text(Some(102.563), true), "103"); // oil
        assert_eq!(icon_text(Some(118.84), true), "119"); // solana
        assert_eq!(icon_text(Some(1.5), true), "1.5"); // ripple
        assert_eq!(icon_text(Some(1.0), true), "1.0"); // tether
        assert_eq!(icon_text(Some(0.335), true), "0.3"); // tron
        assert_eq!(icon_text(Some(0.0937), true), "0.0"); // dogecoin
        assert_eq!(icon_text(None, false), "--");
        assert_eq!(icon_text(None, true), "--");
    }

    #[test]
    fn friendly_maps_connection_errors() {
        let raw = "error sending request for url (https://tgju.org/profile/price_dollar_rtl)";
        assert_eq!(friendly(raw), "مشکل در برقراری ارتباط");
        assert_eq!(friendly("dns error"), "مشکل در برقراری ارتباط");
        assert_eq!(friendly("operation timed out"), "مشکل در برقراری ارتباط");
    }

    #[test]
    fn parse_infobar_branch() {
        let cur = Currency {
            id: "t",
            name: "تست",
            symbol: "$",
            group: "fx",
            unit: Unit::Toman,
            url: "",
            elem: Some("l-test"),
        };
        let html = r#"<li id="l-test" class=" high"><span class="info-value"><span class="info-price">2,537,000</span> <span class="info-change">(3.64%)</span></span></li>"#;
        assert_eq!(parse_price(html, &cur), Ok((253_700.0, Some(3.64))));
    }

    #[test]
    fn parse_main_branch_up_down() {
        let cur = Currency {
            id: "t",
            name: "تست",
            symbol: "₺",
            group: "fx",
            unit: Unit::Toman,
            url: "",
            elem: None,
        };
        let up = r#"<span class="price" data-col="info.last_trade.PDrCotVal">52,665</span><span class="change-tag" data-col="info.last_trade.last_change_percentage"><span class="change change-up change-percentage">3.74</span></span>"#;
        assert_eq!(parse_price(up, &cur), Ok((5_266.5, Some(3.74))));
        let down = up.replace("change-up", "change-down");
        assert_eq!(parse_price(&down, &cur), Ok((5_266.5, Some(-3.74))));
    }

    #[test]
    fn parse_dollar_branch() {
        let cur = Currency {
            id: "t",
            name: "تست",
            symbol: "Br",
            group: "oil",
            unit: Unit::Dollar,
            url: "",
            elem: None,
        };
        let html = r#"<span class="price" data-col="info.last_trade.PDrCotVal">102.563</span><span class="change-tag" data-col="info.last_trade.last_change_percentage"><span class="change change-up change-percentage">0.13</span></span>"#;
        assert_eq!(parse_price(html, &cur), Ok((102.563, Some(0.13))));
    }

    #[test]
    fn parse_no_change_branch() {
        let cur = Currency {
            id: "t",
            name: "تست",
            symbol: "Al",
            group: "base",
            unit: Unit::Dollar,
            url: "",
            elem: None,
        };
        let html = r#"<span class="price" data-col="info.last_trade.PDrCotVal">3,216.5</span> <span class="change-tag" data-col="info.last_trade.last_change_percentage"> <span class="change change-no change-percentage">-</span> </span>"#;
        assert_eq!(parse_price(html, &cur), Ok((3_216.5, None)));
    }

    #[test]
    fn parse_failure_is_friendly() {
        let cur = Currency {
            id: "t",
            name: "تست",
            symbol: "$",
            group: "fx",
            unit: Unit::Toman,
            url: "",
            elem: Some("l-missing"),
        };
        let err = parse_price("<html></html>", &cur).unwrap_err();
        assert!(!err.contains("error sending request"));
        assert_eq!(err, "اطلاعات قیمت در پاسخ سایت پیدا نشد");
    }
}
