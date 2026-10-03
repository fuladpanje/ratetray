mod icon;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
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
    /// Key inside the `current` object of tgju's `call*.tgju.org/ajax.json`
    /// response (single request returns ALL prices at once).
    ajax: &'static str,
}

const CURRENCIES: &[Currency] = &[
    Currency {
        id: "geram18",
        name: "طلای ۱۸ عیار (گرم)",
        symbol: "Au",
        group: "metal",
        unit: Unit::Toman,
        ajax: "geram18",
    },
    Currency {
        id: "silver999",
        name: "نقره ۹۹۹ (گرم)",
        symbol: "Ag",
        group: "metal",
        unit: Unit::Toman,
        ajax: "silver_999",
    },
    Currency {
        id: "aluminum",
        name: "آلومینیوم",
        symbol: "Al",
        group: "base",
        unit: Unit::Dollar,
        // NOTE: tgju uses the British spelling in ajax.json
        ajax: "aluminium",
    },
    Currency {
        id: "copper",
        name: "مس",
        symbol: "Cu",
        group: "base",
        unit: Unit::Dollar,
        ajax: "copper",
    },
    Currency {
        id: "lead",
        name: "سرب",
        symbol: "Pb",
        group: "base",
        unit: Unit::Dollar,
        ajax: "lead",
    },
    Currency {
        id: "zinc",
        name: "روی",
        symbol: "Zn",
        group: "base",
        unit: Unit::Dollar,
        ajax: "zinc",
    },
    Currency {
        id: "nickel",
        name: "نیکل",
        symbol: "Ni",
        group: "base",
        unit: Unit::Dollar,
        ajax: "nickel",
    },
    Currency {
        id: "oilbrent",
        name: "نفت برنت",
        symbol: "Br",
        group: "oil",
        unit: Unit::Dollar,
        ajax: "oil_brent",
    },
    Currency {
        id: "btc",
        name: "بیت کوین",
        symbol: "BTC",
        group: "crypto",
        unit: Unit::Dollar,
        ajax: "crypto-bitcoin",
    },
    Currency {
        id: "eth",
        name: "اتریوم",
        symbol: "ETH",
        group: "crypto",
        unit: Unit::Dollar,
        ajax: "crypto-ethereum",
    },
    Currency {
        id: "trx",
        name: "ترون",
        symbol: "TRX",
        group: "crypto",
        unit: Unit::Dollar,
        ajax: "crypto-tron",
    },
    Currency {
        id: "xrp",
        name: "ریپل",
        symbol: "XRP",
        group: "crypto",
        unit: Unit::Dollar,
        ajax: "crypto-ripple",
    },
    Currency {
        id: "ada",
        name: "کاردانو",
        symbol: "ADA",
        group: "crypto",
        unit: Unit::Dollar,
        ajax: "crypto-cardano",
    },
    Currency {
        id: "doge",
        name: "دوج کوین",
        symbol: "DOGE",
        group: "crypto",
        unit: Unit::Dollar,
        ajax: "crypto-dogecoin",
    },
    Currency {
        id: "sol",
        name: "سولانا",
        symbol: "SOL",
        group: "crypto",
        unit: Unit::Dollar,
        ajax: "crypto-solana",
    },
    Currency {
        id: "dollar",
        name: "دلار",
        symbol: "$",
        group: "fx",
        unit: Unit::Toman,
        ajax: "price_dollar_rl",
    },
    Currency {
        id: "eur",
        name: "یورو",
        symbol: "€",
        group: "fx",
        unit: Unit::Toman,
        ajax: "price_eur",
    },
    Currency {
        id: "cad",
        name: "دلار کانادا",
        symbol: "C$",
        group: "fx",
        unit: Unit::Toman,
        ajax: "price_cad",
    },
    Currency {
        id: "try",
        name: "لیر ترکیه",
        symbol: "₺",
        group: "fx",
        unit: Unit::Toman,
        ajax: "price_try",
    },
    Currency {
        id: "gbp",
        name: "پوند انگلیس",
        symbol: "£",
        group: "fx",
        unit: Unit::Toman,
        ajax: "price_gbp",
    },
    Currency {
        id: "aed",
        name: "درهم امارات",
        symbol: "د.إ",
        group: "fx",
        unit: Unit::Toman,
        ajax: "price_aed",
    },
    Currency {
        id: "cny",
        name: "یوان چین",
        symbol: "¥",
        group: "fx",
        unit: Unit::Toman,
        ajax: "price_cny",
    },
    Currency {
        id: "jpy",
        name: "ین ژاپن (۱۰۰)",
        symbol: "¥",
        group: "fx",
        unit: Unit::Toman,
        ajax: "price_jpy",
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
    show_number: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            prices: Mutex::new(HashMap::new()),
            selected: Mutex::new("dollar".to_string()),
            refresh_secs: AtomicU64::new(DEFAULT_REFRESH_SECS),
            show_number: AtomicBool::new(true),
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

/// One shared HTTP client for the whole process: keeps connections alive
/// (connection pooling), so each refresh is 1 small request instead of
/// 24 separate TLS handshakes. This alone removes most of the old errors.
fn http_client() -> reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
                .timeout(Duration::from_secs(15))
                .pool_max_idle_per_host(4)
                .build()
                .expect("http client")
        })
        .clone()
}

/// tgju's live-price mirrors. `call` is primary; `call1..call5` are the
/// same data used by tgju.org itself for its live tables.
const AJAX_MIRRORS: &[&str] = &[
    "https://call.tgju.org/ajax.json",
    "https://call1.tgju.org/ajax.json",
    "https://call2.tgju.org/ajax.json",
    "https://call3.tgju.org/ajax.json",
    "https://call4.tgju.org/ajax.json",
    "https://call5.tgju.org/ajax.json",
];

/// A single entry of ajax.json's `current` object, e.g.
/// `"price_dollar_rl": {"p": "2,679,000", "dp": 0, ...}`.
/// `p`/`dp` can be numbers or comma-formatted strings, hence `Value`.
#[derive(Debug, Clone, serde::Deserialize, Default)]
struct AjaxEntry {
    #[serde(default)]
    p: Option<serde_json::Value>,
    #[serde(default)]
    dp: Option<serde_json::Value>,
}

fn json_num(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.replace(',', "").trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// Converts one ajax.json entry to (value, change_pct).
/// Toman items are quoted in rial by tgju → divide by 10.
fn ajax_value(cur: &Currency, e: &AjaxEntry) -> Result<(f64, Option<f64>), String> {
    let raw = e
        .p
        .as_ref()
        .and_then(json_num)
        .ok_or_else(|| "اطلاعات قیمت در پاسخ سایت پیدا نشد".to_string())?;
    let value = match cur.unit {
        Unit::Toman => raw / 10.0,
        Unit::Dollar => raw,
    };
    let change = e.dp.as_ref().and_then(json_num).and_then(|dp| {
        if dp.abs() < 0.0005 {
            None
        } else {
            Some(dp)
        }
    });
    Ok((value, change))
}

/// Fetches the whole market in ONE request (~170KB JSON) instead of
/// 24 separate HTML page scrapes. Tries mirrors in order.
async fn fetch_ajax_map() -> Result<HashMap<String, AjaxEntry>, String> {
    let client = http_client();
    let mut last_err = "مشکل در برقراری ارتباط".to_string();
    for url in AJAX_MIRRORS {
        match client.get(*url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(v) => {
                        if let Some(cur) = v.get("current").and_then(|c| c.as_object()) {
                            let map: HashMap<String, AjaxEntry> = cur
                                .iter()
                                .map(|(k, val)| {
                                    (
                                        k.clone(),
                                        serde_json::from_value::<AjaxEntry>(val.clone())
                                            .unwrap_or_default(),
                                    )
                                })
                                .collect();
                            if !map.is_empty() {
                                return Ok(map);
                            }
                            last_err = "اطلاعات قیمت در پاسخ سایت پیدا نشد".to_string();
                        } else {
                            last_err = "پاسخ سایت قابل پردازش نبود".to_string();
                        }
                    }
                    Err(_) => last_err = "پاسخ سایت قابل پردازش نبود".to_string(),
                },
                Err(_) => last_err = "مشکل در برقراری ارتباط".to_string(),
            },
            Err(e) => {
                last_err = friendly(&e.to_string());
            }
        }
    }
    Err(last_err)
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

/// The static app icon (used when the number display is turned off).
fn app_icon_image() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/32x32.png"))
        .unwrap_or_else(|_| Image::new_owned(vec![0u8; (W * H * 4) as usize], W, H))
}

/// Paints the tray icon: the live number, or the static app icon.
fn update_tray_icon(app: &AppHandle) {
    let state = app.state::<AppState>();
    let img = if state.show_number.load(Ordering::Relaxed) {
        let sel = state.selected.lock().unwrap().clone();
        let entry = state
            .prices
            .lock()
            .unwrap()
            .get(&sel)
            .cloned()
            .unwrap_or_default();
        let usd = matches!(
            find_currency(&sel).map(|c| c.unit),
            Some(Unit::Dollar)
        );
        Image::new_owned(icon::render(&icon_text(entry.value, usd)), W, H)
    } else {
        app_icon_image()
    };
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_icon(Some(img));
    }
}

fn apply_selected(app: &AppHandle) {
    update_tray_icon(app);
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

/// Refreshes ALL currencies with a single ajax.json request.
/// One request instead of 24 page scrapes: much faster and almost
/// never rate-limited. Values are preserved on failure so the UI can
/// keep showing the last good price with a red timestamp.
async fn do_fetch_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    // One retry for transient blips (mirror rotation inside makes it cheap).
    let mut result = fetch_ajax_map().await;
    if result.is_err() {
        result = fetch_ajax_map().await;
    }
    match result {
        Ok(map) => {
            let now = chrono::Local::now().format("%H:%M").to_string();
            let mut prices = state.prices.lock().unwrap();
            for c in CURRENCIES {
                match map.get(c.ajax) {
                    Some(entry) => match ajax_value(c, entry) {
                        Ok((value, change)) => {
                            let e = prices.entry(c.id.to_string()).or_default();
                            e.value = Some(value);
                            e.change_pct = change;
                            e.updated_at = Some(now.clone());
                            e.error = None;
                        }
                        Err(msg) => {
                            prices.entry(c.id.to_string()).or_default().error = Some(msg);
                        }
                    },
                    None => {
                        prices.entry(c.id.to_string()).or_default().error =
                            Some("اطلاعات قیمت در پاسخ سایت پیدا نشد".to_string());
                    }
                }
            }
        }
        Err(e) => {
            // Total failure: keep old values, flag every row as stale.
            // (Frontend paints their timestamps red.)
            let mut prices = state.prices.lock().unwrap();
            for c in CURRENCIES {
                prices.entry(c.id.to_string()).or_default().error = Some(e.clone());
            }
        }
    }
    save_prices(app);
    apply_selected(app);
    let _ = app.emit("prices-updated", snapshot(&state));
}

/// Blocking wrapper for the periodic timer thread (single request,
/// no thread fan-out needed anymore).
fn fetch_all_blocking(app: &AppHandle) {
    tauri::async_runtime::block_on(do_fetch_all(app));
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
    // A full refresh is just 1 small JSON request now, so reuse it:
    // selecting a currency refreshes everything, not just one row.
    spawn_all(app);
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

fn config_file(app: &AppHandle, name: &str) -> Option<std::path::PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join(name))
}

fn currency_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    config_file(app, "currency")
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

fn load_show_number(app: &AppHandle) -> bool {
    config_file(app, "show_number")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim() != "0")
        .unwrap_or(true)
}

fn save_show_number(app: &AppHandle, enabled: bool) {
    if let Some(p) = config_file(app, "show_number") {
        let _ = std::fs::write(p, if enabled { "1" } else { "0" });
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

#[tauri::command]
fn get_show_number(state: State<AppState>) -> bool {
    state.show_number.load(Ordering::Relaxed)
}

#[tauri::command]
fn set_show_number(app: AppHandle, enabled: bool) -> bool {
    save_show_number(&app, enabled);
    app.state::<AppState>()
        .show_number
        .store(enabled, Ordering::Relaxed);
    update_tray_icon(&app);
    enabled
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
            set_currency,
            get_show_number,
            set_show_number
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
            let show = load_show_number(app.handle());
            app.state::<AppState>()
                .show_number
                .store(show, Ordering::Relaxed);
            load_prices(app.handle());
            // Paint the tray instantly from cache; fresh prices arrive in seconds.
            apply_selected(app.handle());

            if let Some(w) = app.get_webview_window("main") {
                if let Ok(icon) = Image::from_bytes(include_bytes!("../icons/128x128.png")) {
                    let _ = w.set_icon(icon);
                }
            }

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
        assert_eq!(icon_text(Some(1.0), true), "1.0"); // one dollar
        assert_eq!(icon_text(Some(0.335), true), "0.3"); // tron
        assert_eq!(icon_text(Some(0.0937), true), "0.0"); // dogecoin
        assert_eq!(icon_text(None, false), "--");
        assert_eq!(icon_text(None, true), "--");
    }

    #[test]
    fn friendly_maps_connection_errors() {
        let raw = "error sending request for url (https://call.tgju.org/ajax.json)";
        assert_eq!(friendly(raw), "مشکل در برقراری ارتباط");
        assert_eq!(friendly("dns error"), "مشکل در برقراری ارتباط");
        assert_eq!(friendly("operation timed out"), "مشکل در برقراری ارتباط");
    }

    fn toman_cur() -> Currency {
        Currency {
            id: "t",
            name: "تست",
            symbol: "$",
            group: "fx",
            unit: Unit::Toman,
            ajax: "price_dollar_rl",
        }
    }

    fn dollar_cur() -> Currency {
        Currency {
            id: "t",
            name: "تست",
            symbol: "Br",
            group: "oil",
            unit: Unit::Dollar,
            ajax: "oil_brent",
        }
    }

    fn entry(p: &str, dp: f64) -> AjaxEntry {
        AjaxEntry {
            p: Some(serde_json::Value::String(p.to_string())),
            dp: Some(serde_json::Value::from(dp)),
        }
    }

    #[test]
    fn ajax_toman_divides_rial_by_ten() {
        // ajax.json quotes toman items in rial ("2,679,000" → 267,900 toman)
        assert_eq!(
            ajax_value(&toman_cur(), &entry("2,679,000", 0.0)),
            Ok((267_900.0, None))
        );
        assert_eq!(
            ajax_value(&toman_cur(), &entry("262,253,000", 2.07)),
            Ok((26_225_300.0, Some(2.07)))
        );
    }

    #[test]
    fn ajax_dollar_keeps_value() {
        assert_eq!(
            ajax_value(&dollar_cur(), &entry("102.563", 0.13)),
            Ok((102.563, Some(0.13)))
        );
        assert_eq!(
            ajax_value(&dollar_cur(), &entry("3,216.5", 0.0)),
            Ok((3_216.5, None))
        );
    }

    #[test]
    fn ajax_numeric_price_forms() {
        // `p`/`dp` may arrive as JSON numbers instead of strings.
        let e = AjaxEntry {
            p: Some(serde_json::Value::from(84866.26)),
            dp: Some(serde_json::Value::from(0.03)),
        };
        assert_eq!(ajax_value(&dollar_cur(), &e), Ok((84866.26, Some(0.03))));
    }

    #[test]
    fn ajax_missing_price_is_friendly() {
        let err = ajax_value(&toman_cur(), &AjaxEntry::default()).unwrap_err();
        assert_eq!(err, "اطلاعات قیمت در پاسخ سایت پیدا نشد");
    }

    #[test]
    fn ajax_keys_cover_all_currencies() {
        // Every currency must map to a key we know exists in ajax.json.
        // (Full live-key check happens at runtime; here we assert the
        // mapping table itself is complete and non-empty.)
        assert_eq!(CURRENCIES.len(), 23);
        for c in CURRENCIES {
            assert!(!c.ajax.is_empty(), "missing ajax key for {}", c.id);
        }
        let dollar = CURRENCIES.iter().find(|c| c.id == "dollar").unwrap();
        assert_eq!(dollar.ajax, "price_dollar_rl");
        let alu = CURRENCIES.iter().find(|c| c.id == "aluminum").unwrap();
        assert_eq!(alu.ajax, "aluminium"); // British spelling on tgju side
    }
}
