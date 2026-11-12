//! Test de `RegisterHotKey` (section 5) : le jeu tourne en administrateur,
//! `GetAsyncKeyState` ne voit rien en jeu. Un raccourci global enregistré
//! auprès de Windows est-il reçu quand le jeu a le focus ? Ce module sert à
//! le vérifier avec `lastview-dump raccourci`. L'app utilisera le plugin
//! Tauri `global-shortcut`, qui repose sur le même appel.

use std::ffi::c_void;

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Msg {
    hwnd: *mut c_void,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: Point,
    private: u32,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterHotKey(hwnd: *mut c_void, id: i32, modifiers: u32, vk: u32) -> i32;
    fn UnregisterHotKey(hwnd: *mut c_void, id: i32) -> i32;
    fn GetMessageW(msg: *mut Msg, hwnd: *mut c_void, min: u32, max: u32) -> i32;
    fn PostThreadMessageW(thread: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentThreadId() -> u32;
}

pub const MOD_ALT: u32 = 0x1;
pub const MOD_CONTROL: u32 = 0x2;
pub const MOD_SHIFT: u32 = 0x4;
const MOD_NOREPEAT: u32 = 0x4000;
pub const VK_F9: u32 = 0x78;
const WM_HOTKEY: u32 = 0x0312;
const WM_QUIT: u32 = 0x0012;

/// Id du fil courant, pour arrêter l'écoute depuis un autre fil.
pub fn current_thread() -> u32 {
    unsafe { GetCurrentThreadId() }
}

/// Arrête l'écoute lancée sur le fil `thread`.
pub fn stop(thread: u32) {
    unsafe {
        PostThreadMessageW(thread, WM_QUIT, 0, 0);
    }
}

/// Enregistre le raccourci et appelle `on_press` à chaque appui, jusqu'à
/// [`stop`]. Bloque le fil appelant (boucle de messages Windows).
pub fn listen(modifiers: u32, vk: u32, mut on_press: impl FnMut()) -> Result<(), String> {
    unsafe {
        if RegisterHotKey(std::ptr::null_mut(), 1, modifiers | MOD_NOREPEAT, vk) == 0 {
            return Err("raccourci déjà pris par une autre application".into());
        }
        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                on_press();
            }
        }
        UnregisterHotKey(std::ptr::null_mut(), 1);
    }
    Ok(())
}
