//! Liaison minimale avec Npcap (`wpcap.dll`), chargée à l'exécution : pas de
//! SDK à l'édition de liens. Approche reprise d'A2Tools DPS Meter
//! (`src-tauri/src/capture/pcap_capturer.rs`, GPL-3.0, © taengu et
//! contributeurs), réécrite au minimum nécessaire.

use std::ffi::{c_char, c_int, c_long, c_uint, CStr, CString};
use std::ptr;

use libloading::os::windows::{Library, Symbol};

type PcapT = *mut std::ffi::c_void;

#[repr(C)]
struct PcapIf {
    next: *mut PcapIf,
    name: *const c_char,
    description: *const c_char,
    addresses: *mut PcapAddr,
    flags: c_uint,
}

#[repr(C)]
struct PcapAddr {
    next: *mut PcapAddr,
    addr: *mut std::ffi::c_void,
    netmask: *mut std::ffi::c_void,
    broadaddr: *mut std::ffi::c_void,
    dstaddr: *mut std::ffi::c_void,
}

/// `struct pcap_pkthdr` : timeval = deux `long` (32 bits sur Windows).
#[repr(C)]
pub struct PcapPkthdr {
    pub ts_sec: c_long,
    pub ts_usec: c_long,
    pub caplen: c_uint,
    pub len: c_uint,
}

#[repr(C)]
struct BpfProgram {
    bf_len: c_uint,
    bf_insns: *mut std::ffi::c_void,
}

const PCAP_IF_LOOPBACK: c_uint = 0x0000_0001;
const PCAP_NETMASK_UNKNOWN: c_uint = 0xffff_ffff;
const PCAP_ERRBUF_SIZE: usize = 256;
/// `LOAD_WITH_ALTERED_SEARCH_PATH` : Packet.dll est cherchée à côté de wpcap.dll.
const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;

#[derive(Clone, Debug)]
pub struct Device {
    pub name: String,
    pub description: String,
    pub is_loopback: bool,
    pub has_addresses: bool,
}

type FindAllDevs = unsafe extern "C" fn(*mut *mut PcapIf, *mut c_char) -> c_int;
type FreeAllDevs = unsafe extern "C" fn(*mut PcapIf);
type OpenLive = unsafe extern "C" fn(*const c_char, c_int, c_int, c_int, *mut c_char) -> PcapT;
type Close = unsafe extern "C" fn(PcapT);
type NextEx = unsafe extern "C" fn(PcapT, *mut *mut PcapPkthdr, *mut *const u8) -> c_int;
type Datalink = unsafe extern "C" fn(PcapT) -> c_int;
type Compile = unsafe extern "C" fn(PcapT, *mut BpfProgram, *const c_char, c_int, c_uint) -> c_int;
type SetFilter = unsafe extern "C" fn(PcapT, *mut BpfProgram) -> c_int;
type FreeCode = unsafe extern "C" fn(*mut BpfProgram);
type GetErr = unsafe extern "C" fn(PcapT) -> *const c_char;

/// SAFETY: le type `T` doit correspondre à la signature C du symbole.
unsafe fn sym<T>(lib: &Library, name: &[u8]) -> Result<Symbol<T>, String> {
    lib.get(name).map_err(|e| format!("{}: {e}", String::from_utf8_lossy(name)))
}

pub struct Pcap {
    _lib: Library,
    findalldevs: Symbol<FindAllDevs>,
    freealldevs: Symbol<FreeAllDevs>,
    open_live: Symbol<OpenLive>,
    close: Symbol<Close>,
    next_ex: Symbol<NextEx>,
    datalink: Symbol<Datalink>,
    compile: Symbol<Compile>,
    setfilter: Symbol<SetFilter>,
    freecode: Symbol<FreeCode>,
    geterr: Symbol<GetErr>,
}

unsafe impl Send for Pcap {}
unsafe impl Sync for Pcap {}

impl Pcap {
    pub fn load() -> Result<Self, String> {
        let candidates = [
            r"C:\Windows\System32\Npcap\wpcap.dll".to_string(),
            std::env::var("SystemRoot").map(|r| format!(r"{r}\System32\Npcap\wpcap.dll")).unwrap_or_default(),
            "wpcap.dll".to_string(),
        ];
        let mut errors = Vec::new();
        let mut lib = None;
        for path in candidates.iter().filter(|p| !p.is_empty()) {
            // SAFETY: chargement de la bibliothèque Npcap, sans initialisation particulière.
            match unsafe { Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH) } {
                Ok(l) => {
                    lib = Some(l);
                    break;
                }
                Err(e) => errors.push(format!("{path}: {e}")),
            }
        }
        let lib = lib.ok_or_else(|| format!("wpcap.dll introuvable ({}). Installer Npcap : https://npcap.com", errors.join("; ")))?;
        unsafe {
            Ok(Self {
                findalldevs: sym(&lib, b"pcap_findalldevs\0")?,
                freealldevs: sym(&lib, b"pcap_freealldevs\0")?,
                open_live: sym(&lib, b"pcap_open_live\0")?,
                close: sym(&lib, b"pcap_close\0")?,
                next_ex: sym(&lib, b"pcap_next_ex\0")?,
                datalink: sym(&lib, b"pcap_datalink\0")?,
                compile: sym(&lib, b"pcap_compile\0")?,
                setfilter: sym(&lib, b"pcap_setfilter\0")?,
                freecode: sym(&lib, b"pcap_freecode\0")?,
                geterr: sym(&lib, b"pcap_geterr\0")?,
                _lib: lib,
            })
        }
    }

    pub fn devices(&self) -> Vec<Device> {
        let mut out = Vec::new();
        let mut errbuf = [0 as c_char; PCAP_ERRBUF_SIZE];
        let mut list: *mut PcapIf = ptr::null_mut();
        unsafe {
            if (self.findalldevs)(&mut list, errbuf.as_mut_ptr()) != 0 {
                return out;
            }
            let mut cur = list;
            while !cur.is_null() {
                let d = &*cur;
                let name = if d.name.is_null() { String::new() } else { CStr::from_ptr(d.name).to_string_lossy().into_owned() };
                let description = if d.description.is_null() { String::new() } else { CStr::from_ptr(d.description).to_string_lossy().into_owned() };
                out.push(Device {
                    name,
                    description,
                    is_loopback: d.flags & PCAP_IF_LOOPBACK != 0,
                    has_addresses: !d.addresses.is_null(),
                });
                cur = d.next;
            }
            (self.freealldevs)(list);
        }
        out
    }

    /// Ouvre un périphérique en lecture (non promiscuous), filtre BPF `tcp`.
    pub fn open(&self, device: &str, filter: &str) -> Result<Handle<'_>, String> {
        let name = CString::new(device).map_err(|e| e.to_string())?;
        let mut errbuf = [0 as c_char; PCAP_ERRBUF_SIZE];
        let h = unsafe { (self.open_live)(name.as_ptr(), 65535, 0, 100, errbuf.as_mut_ptr()) };
        if h.is_null() {
            let msg = unsafe { CStr::from_ptr(errbuf.as_ptr()) }.to_string_lossy().into_owned();
            return Err(msg);
        }
        let datalink = unsafe { (self.datalink)(h) };
        if !filter.is_empty() {
            let prog = CString::new(filter).map_err(|e| e.to_string())?;
            let mut bpf = BpfProgram { bf_len: 0, bf_insns: ptr::null_mut() };
            unsafe {
                if (self.compile)(h, &mut bpf, prog.as_ptr(), 1, PCAP_NETMASK_UNKNOWN) != 0 {
                    let msg = CStr::from_ptr((self.geterr)(h)).to_string_lossy().into_owned();
                    (self.close)(h);
                    return Err(format!("filtre: {msg}"));
                }
                let ok = (self.setfilter)(h, &mut bpf) == 0;
                (self.freecode)(&mut bpf);
                if !ok {
                    let msg = CStr::from_ptr((self.geterr)(h)).to_string_lossy().into_owned();
                    (self.close)(h);
                    return Err(format!("setfilter: {msg}"));
                }
            }
        }
        Ok(Handle { lib: self, h, datalink })
    }
}

pub struct Handle<'a> {
    lib: &'a Pcap,
    h: PcapT,
    pub datalink: c_int,
}

unsafe impl Send for Handle<'_> {}

pub enum Next<'p> {
    Packet { ts_ms: i64, data: &'p [u8] },
    Timeout,
    Error(String),
    Eof,
}

impl Handle<'_> {
    /// Lit le paquet suivant (bloque au plus `to_ms` donné à l'ouverture).
    pub fn next(&mut self) -> Next<'_> {
        let mut hdr: *mut PcapPkthdr = ptr::null_mut();
        let mut data: *const u8 = ptr::null();
        let r = unsafe { (self.lib.next_ex)(self.h, &mut hdr, &mut data) };
        match r {
            1 => {
                let hdr = unsafe { &*hdr };
                let ts_ms = hdr.ts_sec as i64 * 1000 + hdr.ts_usec as i64 / 1000;
                let data = unsafe { std::slice::from_raw_parts(data, hdr.caplen as usize) };
                Next::Packet { ts_ms, data }
            }
            0 => Next::Timeout,
            -2 => Next::Eof,
            _ => {
                let msg = unsafe { CStr::from_ptr((self.lib.geterr)(self.h)) }.to_string_lossy().into_owned();
                Next::Error(msg)
            }
        }
    }
}

impl Drop for Handle<'_> {
    fn drop(&mut self) {
        unsafe { (self.lib.close)(self.h) };
    }
}

/// Un segment TCP IPv4 extrait d'une trame, selon le type de liaison.
pub struct TcpSegment<'a> {
    pub src: u32,
    pub dst: u32,
    pub sport: u16,
    pub dport: u16,
    pub seq: u32,
    pub payload: &'a [u8],
}

pub fn parse_tcp(datalink: c_int, frame: &[u8]) -> Option<TcpSegment<'_>> {
    // DLT_EN10MB = 1, DLT_NULL = 0 (Npcap Loopback), DLT_RAW = 12, DLT_LINUX_SLL = 113
    let ip = match datalink {
        1 => {
            if frame.len() < 14 {
                return None;
            }
            let mut off = 12;
            let mut ethertype = u16::from_be_bytes([frame[off], frame[off + 1]]);
            // VLAN 802.1Q
            if ethertype == 0x8100 && frame.len() >= 18 {
                off += 4;
                ethertype = u16::from_be_bytes([frame[off], frame[off + 1]]);
            }
            if ethertype != 0x0800 {
                return None;
            }
            &frame[off + 2..]
        }
        0 => {
            if frame.len() < 4 {
                return None;
            }
            let family = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
            if family != 2 {
                return None;
            }
            &frame[4..]
        }
        12 => frame,
        113 => {
            if frame.len() < 16 {
                return None;
            }
            &frame[16..]
        }
        _ => return None,
    };
    if ip.len() < 20 || ip[0] >> 4 != 4 {
        return None;
    }
    let ihl = ((ip[0] & 0x0F) as usize) * 4;
    if ip[9] != 6 || ip.len() < ihl + 20 {
        return None;
    }
    let total_len = u16::from_be_bytes([ip[2], ip[3]]) as usize;
    let ip_end = total_len.min(ip.len()).max(ihl);
    let src = u32::from_be_bytes([ip[12], ip[13], ip[14], ip[15]]);
    let dst = u32::from_be_bytes([ip[16], ip[17], ip[18], ip[19]]);
    let tcp = &ip[ihl..ip_end];
    if tcp.len() < 20 {
        return None;
    }
    let sport = u16::from_be_bytes([tcp[0], tcp[1]]);
    let dport = u16::from_be_bytes([tcp[2], tcp[3]]);
    let seq = u32::from_be_bytes([tcp[4], tcp[5], tcp[6], tcp[7]]);
    let doff = ((tcp[12] >> 4) as usize) * 4;
    if tcp.len() < doff {
        return None;
    }
    Some(TcpSegment { src, dst, sport, dport, seq, payload: &tcp[doff..] })
}

pub fn ip_to_string(ip: u32) -> String {
    let b = ip.to_be_bytes();
    format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
}
