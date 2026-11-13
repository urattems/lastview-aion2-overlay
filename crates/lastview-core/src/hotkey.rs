//! Raccourcis globaux par `RegisterHotKey` (sections 5 et 8). Le jeu tourne
//! en administrateur et `GetAsyncKeyState` ne voit rien en jeu, mais un
//! raccourci enregistré auprès de Windows est bien reçu : vérifié en jeu le
//! 12/11/2026 (12 appuis sur 12, avec `lastview-dump raccourci`).
//! Les combinaisons s'écrivent en texte : « Ctrl+Maj+F9 ».

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

const MOD_WIN: u32 = 0x8;

/// Une combinaison lue depuis son texte (« Ctrl+Maj+F9 ») : modificateurs et touche virtuelle.
pub fn parse(texte: &str) -> Option<(u32, u32)> {
    let mut mods = 0;
    let mut vk = None;
    for part in texte.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" => mods |= MOD_CONTROL,
            "alt" => mods |= MOD_ALT,
            "maj" | "shift" => mods |= MOD_SHIFT,
            "win" => mods |= MOD_WIN,
            k => {
                if vk.is_some() {
                    return None;
                }
                vk = Some(touche(k)?);
            }
        }
    }
    let vk = vk?;
    // Une touche seule n'est permise que pour F1 à F24 (sinon elle volerait une touche du jeu).
    if mods == 0 && !(0x70..=0x87).contains(&vk) {
        return None;
    }
    Some((mods, vk))
}

fn touche(k: &str) -> Option<u32> {
    let b = k.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_alphabetic() || b[0].is_ascii_digit()) {
        return Some(u32::from(b[0].to_ascii_uppercase()));
    }
    if let Some(n) = k.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
        return (1..=24).contains(&n).then(|| 0x6F + n);
    }
    if let Some(n) = k.strip_prefix("pavnum").and_then(|n| n.parse::<u32>().ok()) {
        return (n <= 9).then(|| 0x60 + n);
    }
    Some(match k {
        "inser" => 0x2D,
        "suppr" => 0x2E,
        "début" => 0x24,
        "fin" => 0x23,
        "pgprec" => 0x21,
        "pgsuiv" => 0x22,
        "pause" => 0x13,
        "arrêtdéfil" => 0x91,
        _ => return None,
    })
}

/// Résultat de l'enregistrement d'un raccourci.
pub type Enregistrement = (i32, Result<(), String>);

/// Enregistre plusieurs raccourcis `(id, modificateurs, touche)` sur un fil
/// dédié, envoie le résultat de chacun sur `pret` avec l'id du fil (pour
/// [`stop`]), puis appelle `on_press(id)` à chaque appui, jusqu'à [`stop`].
pub fn listen_many(touches: Vec<(i32, u32, u32)>, pret: std::sync::mpsc::Sender<(u32, Vec<Enregistrement>)>, mut on_press: impl FnMut(i32)) {
    unsafe {
        let mut resultats = Vec::new();
        let mut pris = Vec::new();
        for (id, mods, vk) in touches {
            if RegisterHotKey(std::ptr::null_mut(), id, mods | MOD_NOREPEAT, vk) == 0 {
                resultats.push((id, Err("déjà pris par une autre application".to_string())));
            } else {
                pris.push(id);
                resultats.push((id, Ok(())));
            }
        }
        let _ = pret.send((GetCurrentThreadId(), resultats));
        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                on_press(msg.wparam as i32);
            }
        }
        for id in pris {
            UnregisterHotKey(std::ptr::null_mut(), id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combinaisons() {
        assert_eq!(parse("Ctrl+Maj+F9"), Some((MOD_CONTROL | MOD_SHIFT, 0x78)));
        assert_eq!(parse("Alt+L"), Some((MOD_ALT, 0x4C)));
        assert_eq!(parse("Ctrl+PavNum5"), Some((MOD_CONTROL, 0x65)));
        assert_eq!(parse("F10"), Some((0, 0x79)));
        // Une lettre seule volerait une touche du jeu.
        assert_eq!(parse("L"), None);
        assert_eq!(parse("Ctrl+Maj"), None);
        assert_eq!(parse("Ctrl+A+B"), None);
        assert_eq!(parse(""), None);
    }

    /// Vrai appel à Windows, sur une combinaison que personne n'utilise :
    /// enregistrée une fois, refusée la seconde (conflit), puis libérée.
    #[test]
    fn enregistrement_et_conflit() {
        let (mods, vk) = parse("Ctrl+Alt+Maj+F23").unwrap();
        let (tx1, rx1) = std::sync::mpsc::channel();
        let t1 = std::thread::spawn(move || listen_many(vec![(7, mods, vk)], tx1, |_| {}));
        let (fil1, r1) = rx1.recv().unwrap();
        assert_eq!(r1, vec![(7, Ok(()))]);
        let (tx2, rx2) = std::sync::mpsc::channel();
        let t2 = std::thread::spawn(move || listen_many(vec![(8, mods, vk)], tx2, |_| {}));
        let (fil2, r2) = rx2.recv().unwrap();
        assert!(r2[0].1.is_err(), "le second enregistrement doit être refusé");
        stop(fil1);
        stop(fil2);
        t1.join().unwrap();
        t2.join().unwrap();
    }
}
