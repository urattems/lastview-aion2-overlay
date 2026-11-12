//! La bulle au-dessus du jeu (section 5.3 et 5.4), le point critique :
//!
//! - **au repos**, elle laisse passer tous les clics au jeu (fenêtre
//!   traversante) et reste à 30 % d'opacité ;
//! - **survol volontaire** : curseur système visible (Alt tenu) **et**
//!   au-dessus de la bulle → 100 % et cliquable. Détection par sondage à
//!   ~30 Hz (`GetCursorInfo`), puisqu'une fenêtre traversante ne reçoit aucun
//!   événement de survol ;
//! - curseur qui sort ou disparaît : retour au repos après 300 ms ;
//! - **jamais de focus** : style `WS_EX_NOACTIVATE`, affichage sans
//!   activation. Seul le champ de recherche prend le clavier, et le rend ;
//! - **cachée** quand le jeu n'est pas au premier plan, sauf si c'est une
//!   fenêtre de LastView qui a le focus (fiche, paramètres).
//!
//! Tout est en lecture seule vis-à-vis du jeu : on lit l'état du curseur et
//! la fenêtre au premier plan, rien d'autre.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lastview_core::capture::system;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

const POLL: Duration = Duration::from_millis(33);
const LEAVE_DELAY: Duration = Duration::from_millis(300);
/// Le curseur doit s'arrêter sur la bulle avant qu'elle devienne cliquable :
/// un simple passage (curseur visible qui traverse la bulle en jouant) ne
/// doit jamais lui donner un clic destiné au jeu. Ajouté le 12/11/2026 après
/// six passages de 0,06 à 0,19 s vus en 30 min de jeu.
const ENTER_DWELL: Duration = Duration::from_millis(250);
const GAME_PIDS_EVERY: Duration = Duration::from_secs(2);

#[link(name = "user32")]
extern "system" {
    fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: *mut c_void, index: i32, value: isize) -> isize;
    fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
    fn SetWindowPos(hwnd: *mut c_void, after: *mut c_void, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    fn GetForegroundWindow() -> *mut c_void;
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    fn IsWindowVisible(hwnd: *mut c_void) -> i32;
}

const GWL_EXSTYLE: i32 = -20;
const WS_EX_NOACTIVATE: isize = 0x0800_0000;
const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
const SW_HIDE: i32 = 0;
const SW_SHOWNOACTIVATE: i32 = 4;
const HWND_TOPMOST: isize = -1;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOACTIVATE: u32 = 0x0010;

fn hwnd(w: &WebviewWindow) -> Option<*mut c_void> {
    w.hwnd().ok().map(|h| h.0 as *mut c_void)
}

/// État partagé avec les commandes.
pub struct Overlay {
    /// Le champ de recherche a le clavier : pas de retour au repos ni de masquage.
    pub searching: AtomicBool,
    /// Masquée par le raccourci « masquer / afficher la bulle ».
    pub user_hidden: AtomicBool,
    /// Fenêtre au premier plan avant que la recherche prenne le clavier.
    previous_foreground: Mutex<usize>,
    /// Opacités (paramètres), en %.
    pub opacity: Mutex<(u8, u8)>,
}

impl Overlay {
    pub fn new(rest: u8, hover: u8) -> Arc<Self> {
        Arc::new(Self { searching: AtomicBool::new(false), user_hidden: AtomicBool::new(false), previous_foreground: Mutex::new(0), opacity: Mutex::new((rest, hover)) })
    }
}

#[derive(Clone, Serialize)]
struct Survol {
    actif: bool,
    opacite: u8,
}

/// Prépare la fenêtre de la bulle : jamais activée par un clic, hors Alt+Tab.
pub fn prepare(w: &WebviewWindow) {
    if let Some(h) = hwnd(w) {
        unsafe {
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW);
        }
    }
    let _ = w.set_ignore_cursor_events(true);
}

fn show_no_activate(w: &WebviewWindow) {
    if let Some(h) = hwnd(w) {
        unsafe {
            ShowWindow(h, SW_SHOWNOACTIVATE);
            // Reste au-dessus du jeu (plein écran fenêtré) sans prendre le focus.
            SetWindowPos(h, HWND_TOPMOST as *mut c_void, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}

/// Affiche une fenêtre sans lui donner le focus (mode test `--fiche`, pour ne
/// jamais prendre la main au jeu).
pub fn afficher_sans_activer(w: &WebviewWindow) {
    if let Some(h) = hwnd(w) {
        unsafe {
            ShowWindow(h, SW_SHOWNOACTIVATE);
        }
    }
}

fn hide(w: &WebviewWindow) {
    if let Some(h) = hwnd(w) {
        unsafe {
            ShowWindow(h, SW_HIDE);
        }
    }
}

fn is_visible(w: &WebviewWindow) -> bool {
    hwnd(w).map(|h| unsafe { IsWindowVisible(h) != 0 }).unwrap_or(false)
}

/// Le champ de recherche prend le clavier (`true`) ou le rend au jeu (`false`).
pub fn search_focus(app: &AppHandle, ov: &Overlay, on: bool) {
    let Some(w) = app.get_webview_window("bulle") else { return };
    let Some(h) = hwnd(&w) else { return };
    unsafe {
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        if on {
            *ov.previous_foreground.lock().unwrap() = GetForegroundWindow() as usize;
            ov.searching.store(true, Ordering::Relaxed);
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex & !WS_EX_NOACTIVATE);
            SetForegroundWindow(h);
            let _ = w.set_focus();
        } else {
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE);
            ov.searching.store(false, Ordering::Relaxed);
            let prev = std::mem::replace(&mut *ov.previous_foreground.lock().unwrap(), 0);
            if prev != 0 {
                SetForegroundWindow(prev as *mut c_void);
            }
        }
    }
    log::info!("bulle : recherche {}", if on { "prend le clavier" } else { "rend le clavier" });
}

/// Le fil de la bulle : visibilité selon le premier plan, survol au curseur.
pub fn start(app: AppHandle, ov: Arc<Overlay>) {
    thread::Builder::new()
        .name("lv-bulle".into())
        .spawn(move || {
            let me = std::process::id();
            let mut game_pids: Vec<u32> = Vec::new();
            let mut last_pids = Instant::now() - GAME_PIDS_EVERY;
            let mut hovering = false;
            let mut outside_since: Option<Instant> = None;
            let mut inside_since: Option<Instant> = None;
            let mut shown = false;
            loop {
                thread::sleep(POLL);
                let Some(w) = app.get_webview_window("bulle") else { continue };
                if last_pids.elapsed() >= GAME_PIDS_EVERY {
                    game_pids = system::find_game_pids();
                    last_pids = Instant::now();
                }
                // 5.4 : visible si le jeu ou une fenêtre de LastView est au premier plan.
                let fg = system::foreground_pid();
                let searching = ov.searching.load(Ordering::Relaxed);
                let want = !ov.user_hidden.load(Ordering::Relaxed) && (searching || (fg != 0 && (game_pids.contains(&fg) || fg == me)));
                if want != shown || (want && !is_visible(&w)) {
                    if want {
                        show_no_activate(&w);
                    } else {
                        hide(&w);
                    }
                    shown = want;
                }
                if !shown {
                    if hovering {
                        hovering = false;
                        let _ = w.set_ignore_cursor_events(true);
                    }
                    continue;
                }
                // 5.3 : survol volontaire = curseur visible ET au-dessus de la bulle.
                let (visible, x, y) = system::cursor();
                let inside = visible && rect_contains(&w, x, y);
                let now = Instant::now();
                if inside {
                    outside_since = None;
                    let since = *inside_since.get_or_insert(now);
                    if !hovering && now.duration_since(since) >= ENTER_DWELL {
                        hovering = true;
                        let _ = w.set_ignore_cursor_events(false);
                        log::debug!("bulle : survol (curseur visible au-dessus)");
                        emit(&app, &ov, true);
                    }
                } else if !hovering {
                    inside_since = None;
                } else if !searching {
                    let since = *outside_since.get_or_insert(now);
                    if now.duration_since(since) >= LEAVE_DELAY {
                        hovering = false;
                        outside_since = None;
                        inside_since = None;
                        let _ = w.set_ignore_cursor_events(true);
                        log::debug!("bulle : repos (traversante)");
                        emit(&app, &ov, false);
                    }
                }
            }
        })
        .expect("fil de la bulle");
}

fn emit(app: &AppHandle, ov: &Overlay, actif: bool) {
    let (rest, hover) = *ov.opacity.lock().unwrap();
    let _ = app.emit_to("bulle", "survol", Survol { actif, opacite: if actif { hover } else { rest } });
}

fn rect_contains(w: &WebviewWindow, x: i32, y: i32) -> bool {
    let (Ok(pos), Ok(size)) = (w.outer_position(), w.outer_size()) else { return false };
    x >= pos.x && y >= pos.y && x < pos.x + size.width as i32 && y < pos.y + size.height as i32
}

/// Position de départ : la position mémorisée si son écran existe encore,
/// sinon à droite de l'écran principal, au tiers de la hauteur : sous la
/// mini-carte du jeu, qui occupe le coin en haut à droite.
pub fn initial_position(w: &WebviewWindow, saved: Option<(i32, i32)>) -> PhysicalPosition<i32> {
    if let Some((x, y)) = saved {
        if let Ok(monitors) = w.available_monitors() {
            let on_screen = monitors.iter().any(|m| {
                let (p, s) = (m.position(), m.size());
                x >= p.x && y >= p.y && x < p.x + s.width as i32 - 40 && y < p.y + s.height as i32 - 20
            });
            if on_screen {
                return PhysicalPosition::new(x, y);
            }
        }
    }
    match w.primary_monitor() {
        Ok(Some(m)) => {
            let scale = m.scale_factor();
            let (p, s) = (m.position(), m.size());
            PhysicalPosition::new(p.x + s.width as i32 - (380.0 * scale) as i32, p.y + s.height as i32 / 3)
        }
        _ => PhysicalPosition::new(100, 100),
    }
}
