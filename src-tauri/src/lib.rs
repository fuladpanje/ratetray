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

struct Currency {
    id: &'static str,
    name: &'static str,
    symbol: &'static str,
    /// "metal" for gold/silver, "fx" for currencies.
    group: &'static str,
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
        url: "https://tgju.org/profile/geram18",
        elem: None,
    },
    Currency {
        id: "silver999",
        name: "نقره ۹۹۹ (گرم)",
        symbol: "Ag",
        group: "metal",
        url: "https://tgju.org/profile/silver_999",
        elem: None,
    },
    Currency {
        id: "dollar",
        name: "دلار",
        symbol: "$",
        group: "fx",
        url: "https://tgju.org/profile/price_dollar_rtl",
        elem: Some("l-price_dollar_rl"),
    },
    Currency {
        id: "eur",
        name: "یورو",
        symbol: "€",
        group: "fx",
        url: "https://tgju.org/profile/price_eur",
        elem: None,
    },
    Currency {
        id: "cad",
        name: "دلار کانادا",
        symbol: "C$",
        group: "fx",
        url: "https://tgju.org/profile/price_cad",
        elem: None,
    },
    Currency {
        id: "try",
        name: "لیر ترکیه",
        symbol: "₺",
        group: "fx",
        url: "https://tgju.org/profile/price_try",
        elem: None,
    },
    Currency {
        id: "gbp",
        name: "پوند انگلیس",
        symbol: "£",
        group: "fx",
        url: "https://tgju.org/profile/price_gbp",
        elem: None,
    },
    Currency {
        id: "aed",
        name: "درهم امارات",
        symbol: "د.إ",
        group: "fx",
        url: "https://tgju.org/profile/price_aed",
        elem: None,
    },
    Currency {
        id: "cny",
        name: "یوان چین",
        symbol: "¥",
        group: "fx",
        url: "https://tgju.org/profile/price_cny",
        elem: None,
    },
    Currency {
        id: "jpy",
        name: "ین ژاپن (۱۰۰)",
        symbol: "¥",
        group: "fx",
        url: "https://tgju.org/profile/price_jpy",
        elem: None,
    },
];

fn find_currency(id: &str) -> Option<&'static Currency> {
    CURRENCIES.iter().find(|c| c.id == id)
}

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
struct CurrencyPrice {
    rial: Option<u64>,
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
    rial: Option<u64>,
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
    let items = CURRENCIES
        .iter()
        .map(|c| {
            let p = map.get(c.id).cloned().unwrap_or_default();
            CurrencyRow {
                id: c.id.to_string(),
                name: c.name.to_string(),
                symbol: c.symbol.to_string(),
                group: c.group.to_string(),
                rial: p.rial,
                change_pct: p.change_pct,
                updated_at: p.updated_at,
                error: p.error,
            }
        })
        .collect();
    AllPrices { selected, items }
}

fn parse_price(html: &str, cur: &Currency) -> Result<(u64, f64), String> {
    if let Some(elem) = cur.elem {
        let re = regex::Regex::new(&format!(
            r#"(?s)<li id="{elem}".*?info-price">([\d,]+)</span>.*?info-change">\((-?[\d.]+)%\)"#
        ))
        .map_err(|_| "خطای داخلی برنامه".to_string())?;
        let caps = re
            .captures(html)
            .ok_or_else(|| "اطلاعات قیمت در پاسخ سایت پیدا نشد".to_string())?;
        let rial: u64 = caps[1]
            .replace(',', "")
            .parse::<f64>()
            .map_err(|_| "پاسخ سایت قابل پردازش نبود".to_string())? as u64;
        let change: f64 = caps[2]
            .parse()
            .map_err(|_| "پاسخ سایت قابل پردازش نبود".to_string())?;
        Ok((rial, change))
    } else {
        let re_price = regex::Regex::new(
            r#"<span class="price" data-col="info\.last_trade\.PDrCotVal">([\d,.]+)</span>"#,
        )
        .map_err(|_| "خطای داخلی برنامه".to_string())?;
        let re_change = regex::Regex::new(
            r#"<span class="change change-(up|down) change-percentage">(-?[\d.]+)<"#,
        )
        .map_err(|_| "خطای داخلی برنامه".to_string())?;

        let price_cap = re_price
            .captures(html)
            .ok_or_else(|| "اطلاعات قیمت در پاسخ سایت پیدا نشد".to_string())?;
        let rial: u64 = price_cap[1]
            .replace(',', "")
            .parse::<f64>()
            .map_err(|_| "پاسخ سایت قابل پردازش نبود".to_string())? as u64;

        let change_cap = re_change
            .captures(html)
            .ok_or_else(|| "اطلاعات قیمت در پاسخ سایت پیدا نشد".to_string())?;
        let magnitude: f64 = change_cap[2]
            .parse()
            .map_err(|_| "پاسخ سایت قابل پردازش نبود".to_string())?;
        let change = if &change_cap[1] == "down" {
            -magnitude.abs()
        } else {
            magnitude.abs()
        };
        Ok((rial, change))
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

async fn fetch_price(cur: &Currency) -> Result<(u64, f64), String> {
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
fn icon_text(rial: Option<u64>) -> String {
    match rial {
        Some(r) => {
            let toman = r / 10;
            let thousands = (toman + 500) / 1000;
            if thousands >= 1000 {
                format!("{}M", (toman + 500_000) / 1_000_000)
            } else if thousands >= 10 {
                format!("{thousands}")
            } else {
                format!("{}.{}", toman / 1000, (toman % 1000) / 100)
            }
        }
        None => "--".to_string(),
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
    let name = find_currency(&sel).map(|c| c.name).unwrap_or("قیمت");
    if let Some(tray) = app.tray_by_id("main") {
        let text = icon_text(entry.rial);
        let img = Image::new_owned(icon::render(&text), W, H);
        let _ = tray.set_icon(Some(img));
        let tooltip = match entry.rial {
            Some(r) => {
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
                format!("{name}: {} تومان{change}{time}{err}", group(r / 10))
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
        Ok((rial, change)) => {
            let mut map = state.prices.lock().unwrap();
            let entry = map.entry(id.to_string()).or_default();
            entry.rial = Some(rial);
            entry.change_pct = Some(change);
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

/// Fetches all currencies concurrently (each on its own thread).
fn spawn_all(app: &AppHandle) {
    for c in CURRENCIES {
        let app = app.clone();
        let id = c.id;
        std::thread::spawn(move || {
            tauri::async_runtime::block_on(do_fetch(&app, id));
        });
    }
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
                        let handles: Vec<std::thread::JoinHandle<()>> = CURRENCIES
                            .iter()
                            .map(|c| {
                                let app = handle.clone();
                                let id = c.id;
                                std::thread::spawn(move || {
                                    tauri::async_runtime::block_on(do_fetch(&app, id));
                                })
                            })
                            .collect();
                        for h in handles {
                            let _ = h.join();
                        }
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
        assert_eq!(icon_text(Some(2_537_000)), "254"); // dollar
        assert_eq!(icon_text(Some(252_655_000)), "25M"); // gold gram
        assert_eq!(icon_text(Some(10_200_000)), "1M"); // 1.02M toman
        assert_eq!(icon_text(Some(9_995_000)), "1M"); // boundary
        assert_eq!(icon_text(Some(9_994_999)), "999"); // just below
        assert_eq!(icon_text(Some(691_090)), "69"); // two digits
        assert_eq!(icon_text(Some(52_660)), "5.2"); // lira with decimal
        assert_eq!(icon_text(Some(17_000)), "1.7");
        assert_eq!(icon_text(Some(5_000)), "0.5");
        assert_eq!(icon_text(None), "--");
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
            url: "",
            elem: Some("l-test"),
        };
        let html = r#"<li id="l-test" class=" high"><span class="info-value"><span class="info-price">2,537,000</span> <span class="info-change">(3.64%)</span></span></li>"#;
        assert_eq!(parse_price(html, &cur), Ok((2_537_000, 3.64)));
    }

    #[test]
    fn parse_main_branch_up_down() {
        let cur = Currency {
            id: "t",
            name: "تست",
            symbol: "₺",
            group: "fx",
            url: "",
            elem: None,
        };
        let up = r#"<span class="price" data-col="info.last_trade.PDrCotVal">52,665</span><span class="change-tag" data-col="info.last_trade.last_change_percentage"><span class="change change-up change-percentage">3.74</span></span>"#;
        assert_eq!(parse_price(up, &cur), Ok((52_665, 3.74)));
        let down = up.replace("change-up", "change-down");
        assert_eq!(parse_price(&down, &cur), Ok((52_665, -3.74)));
    }

    #[test]
    fn parse_failure_is_friendly() {
        let cur = Currency {
            id: "t",
            name: "تست",
            symbol: "$",
            group: "fx",
            url: "",
            elem: Some("l-missing"),
        };
        let err = parse_price("<html></html>", &cur).unwrap_err();
        assert!(!err.contains("error sending request"));
        assert_eq!(err, "اطلاعات قیمت در پاسخ سایت پیدا نشد");
    }
}
