//! Découpage du flux du jeu en paquets, adapté d'A2Tools DPS Meter
//! (`src-tauri/src/capture/framing.rs` et `read_varint` de
//! `stream_processor.rs`, GPL-3.0, © taengu et contributeurs).
//!
//! Forme du flux (serveur → client), telle que le parser A2Tools la lit :
//!
//! ```text
//! 00 ...                                 bourrage, ignoré
//! 17 03 03 <u16 len> ...                 un enregistrement TLS d'une autre connexion, ignoré
//! <varint len> <payload>                 un paquet ; len = taille du payload + 4
//! <varint len> FF FF <u32 size> <lz4>    un lot compressé de paquets
//! ```
//!
//! Une trame occupe `len - 4 + (octets du varint)`.

const MAX_PACKET_BYTES: usize = 65535;
const MAX_FRAGMENT_WAIT_BYTES: usize = 16384;
const MAX_BUNDLE_WAIT_BYTES: usize = MAX_PACKET_BYTES;
const MAX_DECOMPRESSED_BYTES: usize = 1_000_000;
pub const MAX_BUNDLE_DEPTH: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct VarInt {
    pub value: i32,
    pub length: i32,
}

pub fn read_varint(bytes: &[u8], offset: usize) -> VarInt {
    let mut value: i32 = 0;
    let mut shift = 0;
    let mut count = 0;
    loop {
        if offset + count >= bytes.len() {
            return VarInt { value: 0, length: -1 };
        }
        let b = bytes[offset + count] as u32;
        count += 1;
        value |= ((b & 0x7F) as i32) << shift;
        if b & 0x80 == 0 {
            return VarInt { value, length: count as i32 };
        }
        shift += 7;
        if shift >= 32 {
            return VarInt { value: 0, length: -1 };
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Packet,
    Bundle,
}

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
    pub payload_start: usize,
}

#[derive(Default, Debug)]
pub struct Framing {
    pub frames: Vec<Frame>,
    pub consumed: usize,
}

pub fn frame_size(value: i32, varint_bytes: i32) -> Option<usize> {
    let size = value as i64 - 4 + varint_bytes as i64;
    (size >= varint_bytes as i64 && size > 0).then_some(size as usize)
}

fn bundle_header_at(buffer: &[u8], at: usize) -> bool {
    let Some(h) = buffer.get(at..at + 6) else { return false };
    let size = u32::from_le_bytes([h[2], h[3], h[4], h[5]]) as usize;
    h[0] == 0xFF && h[1] == 0xFF && size > 0 && size <= MAX_DECOMPRESSED_BYTES
}

fn tls_record_size(b: &[u8]) -> Option<usize> {
    if b.len() < 5 || !(0x14..=0x17).contains(&b[0]) || b[1] != 0x03 || !(0x01..=0x04).contains(&b[2]) {
        return None;
    }
    let len = u16::from_be_bytes([b[3], b[4]]) as usize;
    (len > 0 && len <= 16_384 + 256).then_some(5 + len)
}

/// Parcours d'un flux TCP (peut commencer au milieu d'un paquet : resynchronise octet par octet).
pub fn walk(buffer: &[u8]) -> Framing {
    let mut out = Framing::default();
    let mut offset = 0usize;
    while offset < buffer.len() {
        if buffer[offset] == 0x00 {
            offset += 1;
            continue;
        }
        if let Some(size) = tls_record_size(&buffer[offset..]) {
            if offset + size > buffer.len() {
                break;
            }
            offset += size;
            continue;
        }
        let li = read_varint(buffer, offset);
        if li.length <= 0 || li.value <= 0 {
            if offset + 5 > buffer.len() {
                break;
            }
            offset += 1;
            continue;
        }
        let Some(total) = frame_size(li.value, li.length) else {
            offset += 1;
            continue;
        };
        if total > MAX_PACKET_BYTES {
            offset += 1;
            continue;
        }
        if offset + total > buffer.len() {
            let limit = if bundle_header_at(buffer, offset + li.length as usize) { MAX_BUNDLE_WAIT_BYTES } else { MAX_FRAGMENT_WAIT_BYTES };
            if total > limit {
                offset += 1;
                continue;
            }
            break;
        }
        let payload_start = li.length as usize;
        let is_bundle = payload_start + 1 < total && buffer[offset + payload_start] == 0xFF && buffer[offset + payload_start + 1] == 0xFF;
        out.frames.push(Frame { kind: if is_bundle { Kind::Bundle } else { Kind::Packet }, start: offset, end: offset + total, payload_start });
        offset += total;
    }
    out.consumed = offset;
    out
}

/// Parcours du contenu décompressé d'un lot (cadré par le jeu : on s'arrête à la première incohérence).
pub fn walk_inner(buffer: &[u8]) -> Framing {
    let mut out = Framing::default();
    let mut offset = 0usize;
    while offset < buffer.len() {
        if buffer[offset] == 0x00 {
            offset += 1;
            continue;
        }
        let li = read_varint(buffer, offset);
        if li.length <= 0 || li.value <= 0 {
            break;
        }
        let Some(total) = frame_size(li.value, li.length) else {
            offset += 1;
            continue;
        };
        let end = offset + total;
        if end > buffer.len() {
            break;
        }
        let payload_start = li.length as usize;
        let is_bundle = total > payload_start + 1 && buffer[offset + payload_start] == 0xFF && buffer[offset + payload_start + 1] == 0xFF;
        out.frames.push(Frame { kind: if is_bundle { Kind::Bundle } else { Kind::Packet }, start: offset, end, payload_start });
        offset += total;
    }
    out.consumed = offset;
    out
}

/// `FF FF <u32 size LE> <bloc lz4>` → contenu décompressé.
pub fn decompress_bundle(payload: &[u8]) -> Option<Vec<u8>> {
    if payload.len() < 7 {
        return None;
    }
    let size = u32::from_le_bytes([payload[2], payload[3], payload[4], payload[5]]) as usize;
    if size == 0 || size > MAX_DECOMPRESSED_BYTES {
        return None;
    }
    lz4_flex::decompress(&payload[6..], size).ok()
}

/// Un paquet « plat » issu du parcours : son corps commence à l'opcode (2 octets).
#[derive(Clone, Debug)]
pub struct Packet {
    /// Offset dans le flux TCP de la trame de premier niveau qui le porte.
    pub stream_offset: usize,
    pub depth: usize,
    pub body: Vec<u8>,
}

/// Tous les paquets d'un tampon, lots ouverts récursivement. Retourne aussi
/// le nombre d'octets consommés (le reste est une trame incomplète).
pub fn packets(buffer: &[u8]) -> (Vec<Packet>, usize) {
    let mut out = Vec::new();
    let framing = walk(buffer);
    for f in &framing.frames {
        match f.kind {
            Kind::Packet => out.push(Packet { stream_offset: f.start, depth: 0, body: buffer[f.start + f.payload_start..f.end].to_vec() }),
            Kind::Bundle => open_bundle(&buffer[f.start + f.payload_start..f.end], f.start, 1, &mut out),
        }
    }
    (out, framing.consumed)
}

fn open_bundle(payload: &[u8], stream_offset: usize, depth: usize, out: &mut Vec<Packet>) {
    if depth > MAX_BUNDLE_DEPTH {
        return;
    }
    let Some(data) = decompress_bundle(payload) else { return };
    let inner = walk_inner(&data);
    for f in &inner.frames {
        match f.kind {
            Kind::Packet => out.push(Packet { stream_offset, depth, body: data[f.start + f.payload_start..f.end].to_vec() }),
            Kind::Bundle => open_bundle(&data[f.start + f.payload_start..f.end], stream_offset, depth + 1, out),
        }
    }
}
