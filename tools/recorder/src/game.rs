//! Le jeu vu depuis le système : son processus, ses connexions TCP (table
//! TCP du système, pas le processus lui-même), sa fenêtre au premier plan,
//! et l'état de la souris. Tout en lecture seule.

use std::collections::HashSet;
use std::ffi::c_void;

#[repr(C)]
struct ProcessEntry32W {
    dw_size: u32,
    cnt_usage: u32,
    th32_process_id: u32,
    th32_default_heap_id: usize,
    th32_module_id: u32,
    cnt_threads: u32,
    th32_parent_process_id: u32,
    pc_pri_class_base: i32,
    dw_flags: u32,
    sz_exe_file: [u16; 260],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MibTcpRowOwnerPid {
    dw_state: u32,
    dw_local_addr: u32,
    dw_local_port: u32,
    dw_remote_addr: u32,
    dw_remote_port: u32,
    dw_owning_pid: u32,
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct CursorInfo {
    cb_size: u32,
    flags: u32,
    h_cursor: *mut c_void,
    pt_screen_pos: Point,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
    fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vkey: i32) -> i16;
    fn GetCursorInfo(info: *mut CursorInfo) -> i32;
    fn GetForegroundWindow() -> *mut c_void;
    fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn Beep(freq: u32, duration_ms: u32) -> i32;
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: *mut c_void, access: u32, token: *mut *mut c_void) -> i32;
    fn GetTokenInformation(token: *mut c_void, class: i32, info: *mut c_void, len: u32, ret: *mut u32) -> i32;
    fn GetSidSubAuthorityCount(sid: *mut c_void) -> *mut u8;
    fn GetSidSubAuthority(sid: *mut c_void, index: u32) -> *mut u32;
}

#[link(name = "iphlpapi")]
extern "system" {
    fn GetExtendedTcpTable(table: *mut c_void, size: *mut u32, order: i32, af: u32, class: u32, reserved: u32) -> u32;
}

const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const INVALID_HANDLE: isize = -1;
const AF_INET: u32 = 2;
const TCP_TABLE_OWNER_PID_ALL: u32 = 5;
const MIB_TCP_STATE_ESTAB: u32 = 5;
const CURSOR_SHOWING: u32 = 0x0000_0001;
pub const VK_LBUTTON: i32 = 0x01;
pub const VK_MENU: i32 = 0x12;
pub const VK_F8: i32 = 0x77;

/// Les PID des processus `AION2.exe` (il y en a souvent deux : un parent sans
/// fenêtre et le client lui-même), triés.
pub fn find_game_pids() -> Vec<u32> {
    let mut found = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap as isize == INVALID_HANDLE || snap.is_null() {
            return found;
        }
        let mut entry: ProcessEntry32W = std::mem::zeroed();
        entry.dw_size = std::mem::size_of::<ProcessEntry32W>() as u32;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                let len = entry.sz_exe_file.iter().position(|&c| c == 0).unwrap_or(entry.sz_exe_file.len());
                let name = String::from_utf16_lossy(&entry.sz_exe_file[..len]);
                if name.eq_ignore_ascii_case("AION2.exe") {
                    found.push(entry.th32_process_id);
                }
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    found.sort_unstable();
    found
}

/// Une connexion TCP IPv4 établie du jeu : (ip distante, port distant, port local).
pub type Conn = (u32, u16, u16);

/// Les connexions établies de ces processus, depuis la table TCP du système.
pub fn game_connections(pids: &[u32]) -> HashSet<Conn> {
    let mut out = HashSet::new();
    unsafe {
        let mut size: u32 = 0;
        GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_ALL, 0);
        if size == 0 {
            return out;
        }
        let mut buf = vec![0u8; size as usize + 1024];
        let r = GetExtendedTcpTable(buf.as_mut_ptr() as *mut c_void, &mut size, 0, AF_INET, TCP_TABLE_OWNER_PID_ALL, 0);
        if r != 0 {
            return out;
        }
        let count = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        let rows = buf.as_ptr().add(4) as *const MibTcpRowOwnerPid;
        for i in 0..count {
            let row = *rows.add(i);
            if !pids.contains(&row.dw_owning_pid) || row.dw_state != MIB_TCP_STATE_ESTAB {
                continue;
            }
            // Adresses en ordre réseau dans un u32 natif : on les lit en big-endian.
            let remote_ip = u32::from_be(row.dw_remote_addr);
            let remote_port = (row.dw_remote_port as u16).to_be();
            let local_port = (row.dw_local_port as u16).to_be();
            // Les connexions HTTPS du client (boutique, télémétrie) sont du TLS : inutiles ici.
            if remote_port == 443 {
                continue;
            }
            out.insert((remote_ip, remote_port, local_port));
        }
    }
    out
}

/// Bip du haut-parleur système (bloquant pendant `ms`).
pub fn beep(freq: u32, ms: u32) {
    unsafe {
        Beep(freq, ms);
    }
}

/// Niveau d'intégrité et élévation d'un processus, en texte, pour le diagnostic.
/// Lecture du jeton seulement (droit `PROCESS_QUERY_LIMITED_INFORMATION`).
pub fn integrity(pid: u32) -> String {
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_ELEVATION: i32 = 20;
    const TOKEN_INTEGRITY_LEVEL: i32 = 25;
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return "processus inaccessible (protégé)".into();
        }
        let mut tok: *mut c_void = std::ptr::null_mut();
        if OpenProcessToken(h, TOKEN_QUERY, &mut tok) == 0 {
            CloseHandle(h);
            return "jeton inaccessible (processus protégé ou plus privilégié)".into();
        }
        let mut buf = [0u8; 256];
        let mut ret = 0u32;
        let mut elevated = "?".to_string();
        if GetTokenInformation(tok, TOKEN_ELEVATION, buf.as_mut_ptr() as *mut c_void, 4, &mut ret) != 0 {
            elevated = if u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) != 0 { "oui".into() } else { "non".into() };
        }
        let mut level = "?".to_string();
        if GetTokenInformation(tok, TOKEN_INTEGRITY_LEVEL, buf.as_mut_ptr() as *mut c_void, buf.len() as u32, &mut ret) != 0 {
            // Le tampon d'octets n'est pas aligné sur 8 : lecture non alignée du pointeur.
            let sid = std::ptr::read_unaligned(buf.as_ptr() as *const *mut c_void);
            let n = *GetSidSubAuthorityCount(sid) as u32;
            let rid = *GetSidSubAuthority(sid, n - 1);
            level = match rid {
                0x0000..=0x0FFF => format!("basse (0x{rid:X})"),
                0x1000..=0x1FFF => format!("basse (0x{rid:X})"),
                0x2000..=0x2FFF => format!("moyenne (0x{rid:X})"),
                0x3000..=0x3FFF => format!("haute = admin (0x{rid:X})"),
                _ => format!("système (0x{rid:X})"),
            };
        }
        CloseHandle(tok);
        CloseHandle(h);
        format!("élevé : {elevated}, intégrité : {level}")
    }
}

pub fn foreground_pid() -> u32 {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return 0;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        pid
    }
}

pub fn key_down(vkey: i32) -> bool {
    unsafe { (GetAsyncKeyState(vkey) as u16) & 0x8000 != 0 }
}

/// (curseur visible, x, y)
pub fn cursor() -> (bool, i32, i32) {
    unsafe {
        let mut info = CursorInfo { cb_size: std::mem::size_of::<CursorInfo>() as u32, flags: 0, h_cursor: std::ptr::null_mut(), pt_screen_pos: Point { x: 0, y: 0 } };
        if GetCursorInfo(&mut info) == 0 {
            return (false, 0, 0);
        }
        (info.flags & CURSOR_SHOWING != 0, info.pt_screen_pos.x, info.pt_screen_pos.y)
    }
}
