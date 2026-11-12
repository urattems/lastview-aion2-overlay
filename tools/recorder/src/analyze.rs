//! Analyse : quel paquet suit chaque clic de ciblage ?
//!
//! 1. Remise en ordre des segments TCP par flux, parcours des trames (lots
//!    LZ4 ouverts), chaque paquet daté par le segment qui porte sa trame.
//! 2. Annuaire des entités nommées (spawn joueur `45 36`/`44 36`, soi `33 36`).
//! 3. Pour chaque opcode : part des clics (curseur visible, jeu au premier
//!    plan) suivis d'au moins un paquet dans la fenêtre, contre sa fréquence
//!    hors fenêtres. Les meilleurs candidats sont détaillés, avec les ids
//!    d'entités connus trouvés dans leurs octets.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

use crate::framing;

struct Seg {
    ts: i64,
    seq: u32,
    data: Vec<u8>,
}

#[derive(Clone)]
struct Pkt {
    ts: i64,
    stream: usize,
    body: Vec<u8>,
    depth: usize,
}

struct Click {
    ts: i64,
    visible: bool,
    game_fg: bool,
    alt: bool,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

fn decode_hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).filter_map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()).collect()
}

pub fn run(dir: &str, window_ms: Option<i64>) -> Result<(), String> {
    let capture = std::fs::read_to_string(format!("{dir}/capture.txt")).map_err(|e| format!("capture.txt: {e}"))?;
    let clicks_txt = std::fs::read_to_string(format!("{dir}/clicks.txt")).map_err(|e| format!("clicks.txt: {e}"))?;

    // --- Segments par flux ---
    let mut streams: BTreeMap<String, Vec<Seg>> = BTreeMap::new();
    for line in capture.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(4, '|');
        let (Some(ts), Some(key), Some(seq), Some(hexdata)) = (parts.next(), parts.next(), parts.next(), parts.next()) else { continue };
        let (Ok(ts), Ok(seq)) = (ts.parse::<i64>(), seq.parse::<u32>()) else { continue };
        streams.entry(key.to_string()).or_default().push(Seg { ts, seq, data: decode_hex(hexdata) });
    }
    if streams.is_empty() {
        return Err("aucun segment dans capture.txt".into());
    }

    // --- Remise en ordre + parcours des trames ---
    let mut packets: Vec<Pkt> = Vec::new();
    let mut report = String::new();
    let _ = writeln!(report, "# Analyse du test de ciblage\n");
    for (si, (key, segs)) in streams.iter_mut().enumerate() {
        let base = segs.first().map(|s| s.seq).unwrap_or(0);
        let rel = |seq: u32| (seq.wrapping_sub(base) as i32) as i64;
        segs.sort_by_key(|s| (rel(s.seq), s.ts));
        let mut bytes: Vec<u8> = Vec::new();
        let mut marks: Vec<(usize, i64)> = Vec::new();
        let mut expected: i64 = 0;
        let (mut dups, mut gaps) = (0usize, 0usize);
        for s in segs.iter() {
            let r = rel(s.seq);
            let end = r + s.data.len() as i64;
            if end <= expected {
                dups += 1;
                continue;
            }
            let skip = if r < expected { (expected - r) as usize } else { 0 };
            if r > expected {
                gaps += 1;
            }
            marks.push((bytes.len(), s.ts));
            bytes.extend_from_slice(&s.data[skip..]);
            expected = end;
        }
        let (pkts, consumed) = framing::packets(&bytes);
        let ts_at = |offset: usize| -> i64 {
            match marks.binary_search_by_key(&offset, |m| m.0) {
                Ok(i) => marks[i].1,
                Err(0) => marks.first().map(|m| m.1).unwrap_or(0),
                Err(i) => marks[i - 1].1,
            }
        };
        let n = pkts.len();
        for p in pkts {
            packets.push(Pkt { ts: ts_at(p.stream_offset), stream: si, body: p.body, depth: p.depth });
        }
        let _ = writeln!(
            report,
            "- Flux `{key}` : {} segments, {} octets, {} paquets décodés, {} octets non consommés, {dups} doublons, {gaps} trous",
            segs.len(),
            bytes.len(),
            n,
            bytes.len() - consumed
        );
    }
    packets.sort_by_key(|p| p.ts);
    let t0 = packets.first().map(|p| p.ts).unwrap_or(0);
    let t1 = packets.last().map(|p| p.ts).unwrap_or(0);
    let _ = writeln!(report, "\nTotal : {} paquets sur {:.1} s\n", packets.len(), (t1 - t0) as f64 / 1000.0);

    // --- Annuaire des entités nommées ---
    let mut names: HashMap<i32, String> = HashMap::new();
    let mut self_id: Option<i32> = None;
    for p in &packets {
        for (id, name, is_self) in masked_records(&p.body) {
            names.entry(id).or_insert(name);
            if is_self {
                self_id = Some(id);
            }
        }
    }
    let _ = writeln!(report, "## Entités nommées ({})\n", names.len());
    if let Some(id) = self_id {
        let _ = writeln!(report, "- Joueur local (33 36) : id {id} = {}", names.get(&id).cloned().unwrap_or_default());
    }
    let mut listed: Vec<_> = names.iter().collect();
    listed.sort_by_key(|(id, _)| **id);
    for (id, name) in listed.iter().take(60) {
        let _ = writeln!(report, "- {id} = {name}");
    }
    if listed.len() > 60 {
        let _ = writeln!(report, "- … {} autres", listed.len() - 60);
    }

    // --- Fréquence des opcodes (toujours utile, même sans clic) ---
    let mut freq: HashMap<[u8; 2], (usize, usize)> = HashMap::new();
    for p in &packets {
        if p.body.len() >= 2 {
            let e = freq.entry([p.body[0], p.body[1]]).or_default();
            e.0 += 1;
            e.1 += p.body.len();
        }
    }
    let mut freq: Vec<_> = freq.into_iter().collect();
    freq.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    let _ = writeln!(report, "\n## Opcodes les plus fréquents ({} distincts)\n", freq.len());
    let _ = writeln!(report, "| opcode | paquets | /s | taille moy. |");
    let _ = writeln!(report, "|---|---|---|---|");
    let dur_s = ((t1 - t0).max(1)) as f64 / 1000.0;
    for (op, (n, bytes)) in freq.iter().take(25) {
        let _ = writeln!(report, "| `{:02x} {:02x}` | {} | {:.2} | {:.0} |", op[0], op[1], n, *n as f64 / dur_s, *bytes as f64 / *n as f64);
    }

    // --- Événements : clics, bips du métronome, phases, curseur, infos ---
    struct Event {
        ts: i64,
        label: String,
    }
    let mut clicks: Vec<Click> = Vec::new();
    let mut markers: Vec<i64> = Vec::new();
    let mut beeps: Vec<Event> = Vec::new();
    let mut phases: HashMap<String, i64> = HashMap::new();
    let mut infos: Vec<String> = Vec::new();
    let (mut cur_fg, mut cur_fg_visible, mut cur_fg_toggles) = (0usize, 0usize, 0usize);
    let mut last_vis_fg: Option<bool> = None;
    for line in clicks_txt.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('|').collect();
        if f.len() < 3 {
            continue;
        }
        let ts: i64 = f[0].parse().unwrap_or(0);
        match f[1] {
            "down" if f.len() >= 7 => clicks.push(Click { ts, visible: f[2] == "1", game_fg: f[5] == "1", alt: f[6] == "1" }),
            "marker" => markers.push(ts),
            "beep" => beeps.push(Event { ts, label: f[2].to_string() }),
            "phase" => {
                phases.entry(f[2].to_string()).or_insert(ts);
            }
            "info" => infos.push(f[2].to_string()),
            "cursor" if f.len() >= 6 => {
                if f[5] == "1" {
                    cur_fg += 1;
                    let vis = f[2] == "1";
                    if vis {
                        cur_fg_visible += 1;
                    }
                    if last_vis_fg.is_some_and(|v| v != vis) {
                        cur_fg_toggles += 1;
                    }
                    last_vis_fg = Some(vis);
                }
            }
            _ => {}
        }
    }
    if !infos.is_empty() {
        let _ = writeln!(report, "\n## Privilèges\n");
        for i in &infos {
            let _ = writeln!(report, "- {i}");
        }
    }
    let _ = writeln!(report, "\n## Curseur pendant que le jeu a le focus\n");
    let _ = writeln!(
        report,
        "- {cur_fg} relevés avec le jeu au premier plan, dont {cur_fg_visible} avec curseur visible, {cur_fg_toggles} bascules visible/caché.\n- Lecture de l'état du curseur en jeu : **{}**",
        if cur_fg_toggles >= 2 { "OK (les Alt se voient)" } else if cur_fg > 0 { "aucune bascule vue : à vérifier" } else { "jamais vu le jeu au premier plan" }
    );
    let useful: Vec<&Click> = clicks.iter().filter(|c| c.visible && c.game_fg).collect();
    let _ = writeln!(
        report,
        "\n## Clics et bips\n\n- {} clics gauche au total, {} avec curseur visible et jeu au premier plan, {} marqueurs F8, {} bips du métronome",
        clicks.len(),
        useful.len(),
        markers.len(),
        beeps.len()
    );

    // Référence : les bips s'il y en a (fenêtre large, temps de réaction), sinon les clics.
    let (events, window_ms): (Vec<Event>, i64) = if beeps.len() >= 4 {
        (beeps, window_ms.unwrap_or(3000))
    } else if !useful.is_empty() {
        (useful.iter().map(|c| Event { ts: c.ts, label: "?".into() }).collect(), window_ms.unwrap_or(600))
    } else {
        let _ = writeln!(report, "\n**Ni bip ni clic exploitable** : refaire le test avec le métronome.");
        finish(dir, &report)?;
        return Ok(());
    };
    let _ = writeln!(report, "- Référence utilisée : {} événements, fenêtre de {window_ms} ms après chacun", events.len());
    let baseline_end = phases.get("test").copied().or(markers.first().copied()).unwrap_or(events[0].ts);
    let baseline_start = phases.get("reference").copied().unwrap_or(t0);
    let ev_ts: Vec<i64> = events.iter().map(|e| e.ts).collect();
    let in_window = |ts: i64| ev_ts.iter().any(|&c| ts >= c && ts < c + window_ms);
    let windows_total_ms = (ev_ts.len() as i64) * window_ms;
    let outside_ms = ((t1 - t0) - windows_total_ms).max(1);
    let baseline_s = ((baseline_end - baseline_start).max(1)) as f64 / 1000.0;

    // --- Statistiques par opcode ---
    #[derive(Default)]
    struct Stat {
        total: usize,
        baseline: usize,
        outside: usize,
        hit: HashSet<usize>,
        in_windows: usize,
        sizes: Vec<usize>,
        /// Par événement : ids nommés trouvés dans les paquets de la fenêtre.
        ids: HashMap<usize, HashSet<i32>>,
    }
    let mut stats: HashMap<[u8; 2], Stat> = HashMap::new();
    for p in &packets {
        if p.body.len() < 2 {
            continue;
        }
        let op = [p.body[0], p.body[1]];
        let s = stats.entry(op).or_default();
        s.total += 1;
        s.sizes.push(p.body.len());
        if p.ts >= baseline_start && p.ts < baseline_end {
            s.baseline += 1;
        }
        if in_window(p.ts) {
            s.in_windows += 1;
            let mut ids = named_ids_in(&p.body[2..], &names);
            // L'id en tête compte aussi, même sans pseudo connu : un joueur déjà à
            // l'écran avant l'enregistrement n'a pas de spawn dans la capture.
            // `34 38` met 3 octets nuls avant l'id : on saute les zéros de tête.
            let mut at = 2;
            while at < p.body.len() && p.body[at] == 0 && at < 5 {
                at += 1;
            }
            let head = framing::read_varint(&p.body, at);
            if head.length > 0 && head.value >= 100 {
                ids.insert(head.value);
            }
            for (ei, &c) in ev_ts.iter().enumerate() {
                if p.ts >= c && p.ts < c + window_ms {
                    s.hit.insert(ei);
                    s.ids.entry(ei).or_default().extend(ids.iter().copied());
                }
            }
        } else {
            s.outside += 1;
        }
    }
    let n_ev = ev_ts.len() as f64;
    let mut rows: Vec<(f64, f64, [u8; 2], &Stat)> = stats
        .iter()
        .map(|(op, s)| (s.hit.len() as f64 / n_ev, s.outside as f64 / (outside_ms as f64 / 1000.0), *op, s))
        .collect();
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));
    let _ = writeln!(report, "\n## Opcodes classés (part des événements suivis, puis rareté hors fenêtre)\n");
    let _ = writeln!(report, "| opcode | suivis | par événement | hors fenêtre /s | pendant la référence /s | total | taille moy. |");
    let _ = writeln!(report, "|---|---|---|---|---|---|---|");
    for (hit, outside_rate, op, s) in rows.iter().take(30) {
        let size = s.sizes.iter().sum::<usize>() as f64 / s.sizes.len().max(1) as f64;
        let _ = writeln!(
            report,
            "| `{:02x} {:02x}` | {:.0}% ({}/{}) | {:.2} | {:.2} | {:.2} | {} | {:.0} |",
            op[0],
            op[1],
            hit * 100.0,
            s.hit.len(),
            ev_ts.len(),
            s.in_windows as f64 / n_ev,
            outside_rate,
            s.baseline as f64 / baseline_s,
            s.total,
            size
        );
    }

    // --- Cohérence A/B : un même joueur après les bips A, un autre après les bips B ---
    let labelled = events.iter().any(|e| e.label == "A") && events.iter().any(|e| e.label == "B");
    let mut ab: Vec<(f64, [u8; 2], i32, i32, usize, usize)> = Vec::new();
    if labelled {
        for (op, s) in &stats {
            let best_for = |lab: &str| -> Option<(i32, usize, usize)> {
                let idx: Vec<usize> = events.iter().enumerate().filter(|(_, e)| e.label == lab).map(|(i, _)| i).collect();
                let mut count: HashMap<i32, usize> = HashMap::new();
                for i in &idx {
                    for id in s.ids.get(i).into_iter().flatten() {
                        *count.entry(*id).or_default() += 1;
                    }
                }
                count.into_iter().max_by_key(|(id, n)| (*n, -*id)).map(|(id, n)| (id, n, idx.len()))
            };
            if let (Some((ia, na, ta)), Some((ib, nb, tb))) = (best_for("A"), best_for("B")) {
                if ia != ib {
                    // l'id de A ne doit pas apparaître après B et inversement
                    let cross = events
                        .iter()
                        .enumerate()
                        .filter(|(i, e)| {
                            let ids = s.ids.get(i);
                            (e.label == "A" && ids.is_some_and(|x| x.contains(&ib))) || (e.label == "B" && ids.is_some_and(|x| x.contains(&ia)))
                        })
                        .count();
                    let score = (na + nb).saturating_sub(cross) as f64 / (ta + tb) as f64;
                    ab.push((score, *op, ia, ib, na + nb, ta + tb));
                }
            }
        }
        ab.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let _ = writeln!(report, "\n## Cohérence A/B (id nommé après les bips A ≠ id après les bips B)\n");
        let _ = writeln!(report, "| opcode | score | joueur A | joueur B | fenêtres avec le bon id |");
        let _ = writeln!(report, "|---|---|---|---|---|");
        for (score, op, ia, ib, n, t) in ab.iter().take(12) {
            let _ = writeln!(
                report,
                "| `{:02x} {:02x}` | {:.0}% | {} ({ia}) | {} ({ib}) | {n}/{t} |",
                op[0],
                op[1],
                score * 100.0,
                names.get(ia).map(String::as_str).unwrap_or("?"),
                names.get(ib).map(String::as_str).unwrap_or("?")
            );
        }
        if ab.is_empty() {
            let _ = writeln!(report, "Aucun opcode ne porte un id nommé différent après A et après B.");
        }
    }

    // --- Changement de cible : à chaque événement, l'id porté change ---
    // Robuste aux décalages A/B (un bip raté inverse les rôles). Par fenêtre,
    // on garde le dernier id de tête vu ; score = part des événements consécutifs
    // où il change, sur les fenêtres où l'opcode est présent.
    let mut switch_rows: Vec<(f64, [u8; 2], usize, usize, f64, f64)> = Vec::new();
    for (op, s) in &stats {
        let present = s.hit.len();
        if present < (ev_ts.len() * 3).div_ceil(4) {
            continue;
        }
        let mut last_ids: Vec<Option<i32>> = vec![None; ev_ts.len()];
        for p in packets.iter().filter(|p| p.body.len() >= 3 && [p.body[0], p.body[1]] == *op) {
            let mut at = 2;
            while at < p.body.len() && p.body[at] == 0 && at < 5 {
                at += 1;
            }
            let head = framing::read_varint(&p.body, at);
            if head.length <= 0 || head.value < 100 {
                continue;
            }
            for (ei, &c) in ev_ts.iter().enumerate() {
                if p.ts >= c && p.ts < c + window_ms {
                    last_ids[ei] = Some(head.value);
                }
            }
        }
        let (mut pairs, mut changes) = (0usize, 0usize);
        for w in last_ids.windows(2) {
            if let (Some(a), Some(b)) = (w[0], w[1]) {
                pairs += 1;
                if a != b {
                    changes += 1;
                }
            }
        }
        if pairs == 0 {
            continue;
        }
        let outside_rate = s.outside as f64 / (outside_ms as f64 / 1000.0);
        let ref_rate = s.baseline as f64 / baseline_s;
        switch_rows.push((changes as f64 / pairs as f64, *op, present, pairs, outside_rate, ref_rate));
    }
    switch_rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.4.partial_cmp(&b.4).unwrap()));
    let _ = writeln!(report, "\n## Changement d'id à chaque événement (robuste aux inversions A/B)\n");
    let _ = writeln!(report, "| opcode | id change | fenêtres présentes | hors fenêtre /s | pendant la référence /s |");
    let _ = writeln!(report, "|---|---|---|---|---|");
    for (score, op, present, pairs, out, rf) in switch_rows.iter().take(10) {
        let _ = writeln!(report, "| `{:02x} {:02x}` | {:.0}% (sur {pairs}) | {present}/{} | {out:.2} | {rf:.2} |", op[0], op[1], score * 100.0, ev_ts.len());
    }
    let strong_switch: Vec<[u8; 2]> = switch_rows.iter().filter(|r| r.0 >= 0.75 && r.4 < 0.2 && r.5 < 0.2).map(|r| r.1).collect();

    // --- Détail des meilleurs candidats ---
    let mut candidates: Vec<[u8; 2]> = strong_switch.clone();
    for r in ab.iter().filter(|r| r.0 >= 0.5).take(4) {
        if !candidates.contains(&r.1) {
            candidates.push(r.1);
        }
    }
    for r in rows.iter().filter(|r| r.0 >= 0.5 && r.1 < 2.0).take(4) {
        if !candidates.contains(&r.2) {
            candidates.push(r.2);
        }
    }
    let _ = writeln!(report, "\n## Détail des candidats\n");
    if candidates.is_empty() {
        let _ = writeln!(report, "Aucun opcode ne suit les événements de façon nette.");
    }
    for op in &candidates {
        let _ = writeln!(report, "### `{:02x} {:02x}`\n", op[0], op[1]);
        for (ei, e) in events.iter().enumerate().take(14) {
            let _ = writeln!(report, "- événement {} ({}) à +{:.1} s", ei + 1, e.label, (e.ts - t0) as f64 / 1000.0);
            for p in packets.iter().filter(|p| p.body.len() >= 2 && [p.body[0], p.body[1]] == *op && p.ts >= e.ts && p.ts < e.ts + window_ms) {
                let ids = entity_ids_in(&p.body[2..], &names);
                let ids_txt = if ids.is_empty() { String::new() } else { format!(" → ids connus : {}", ids.join(", ")) };
                let _ = writeln!(
                    report,
                    "  - +{} ms, {} octets, prof. {} : `{}`{}",
                    p.ts - e.ts,
                    p.body.len(),
                    p.depth,
                    hex(&p.body[..p.body.len().min(48)]),
                    ids_txt
                );
            }
        }
    }

    // --- Verdict automatique ---
    let _ = writeln!(report, "\n## Verdict automatique\n");
    let strong_ab = ab.iter().find(|r| r.0 >= 0.75).and_then(|r| {
        let s = &stats[&r.1];
        let outside_rate = s.outside as f64 / (outside_ms as f64 / 1000.0);
        (outside_rate < 2.0).then_some((r, outside_rate))
    });
    if !strong_switch.is_empty() {
        let list: Vec<String> = strong_switch.iter().map(|op| format!("`{:02x} {:02x}`", op[0], op[1])).collect();
        let _ = writeln!(
            report,
            "**Ciblage détecté** : {} suit au moins 75 % des événements, l'id porté change à chaque événement dans au moins 75 % des cas, et le paquet est quasi absent hors des fenêtres et pendant la référence.",
            list.join(", ")
        );
        return finish(dir, &report);
    }
    match strong_ab {
        Some(((score, op, ia, ib, _, _), outside_rate)) => {
            let _ = writeln!(
                report,
                "**Candidat fort** : `{:02x} {:02x}` porte {} après les bips A et {} après les bips B ({:.0} % des fenêtres), {:.2} paquet/s en dehors. Le ciblage est probablement détectable passivement.",
                op[0],
                op[1],
                names.get(ia).map(String::as_str).unwrap_or("?"),
                names.get(ib).map(String::as_str).unwrap_or("?"),
                score * 100.0,
                outside_rate
            );
        }
        None => {
            let _ = writeln!(
                report,
                "Aucun opcode ne porte de façon cohérente l'id de A après les bips A et celui de B après les bips B. Si le curseur et les bips étaient bien suivis, verdict : **pas de paquet de ciblage serveur → client**, passer au plan B (OCR de la bannière)."
            );
        }
    }
    finish(dir, &report)
}

/// Écrit `<dossier>/packets.tsv` : `ts_ms\tflux\tprofondeur\topcode\thex`, un paquet décodé par ligne,
/// et `<dossier>/names.tsv` : `id\tnom\tsoi`. Pour des analyses ad hoc.
pub fn dump(dir: &str) -> Result<(), String> {
    let capture = std::fs::read_to_string(format!("{dir}/capture.txt")).map_err(|e| format!("capture.txt: {e}"))?;
    let mut streams: BTreeMap<String, Vec<Seg>> = BTreeMap::new();
    for line in capture.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(4, '|');
        let (Some(ts), Some(key), Some(seq), Some(hexdata)) = (parts.next(), parts.next(), parts.next(), parts.next()) else { continue };
        let (Ok(ts), Ok(seq)) = (ts.parse::<i64>(), seq.parse::<u32>()) else { continue };
        streams.entry(key.to_string()).or_default().push(Seg { ts, seq, data: decode_hex(hexdata) });
    }
    let mut out = String::new();
    let mut names: HashMap<i32, (String, bool)> = HashMap::new();
    for (si, (_key, segs)) in streams.iter_mut().enumerate() {
        let base = segs.first().map(|s| s.seq).unwrap_or(0);
        let rel = |seq: u32| (seq.wrapping_sub(base) as i32) as i64;
        segs.sort_by_key(|s| (rel(s.seq), s.ts));
        let mut bytes: Vec<u8> = Vec::new();
        let mut marks: Vec<(usize, i64)> = Vec::new();
        let mut expected: i64 = 0;
        for s in segs.iter() {
            let r = rel(s.seq);
            let end = r + s.data.len() as i64;
            if end <= expected {
                continue;
            }
            let skip = if r < expected { (expected - r) as usize } else { 0 };
            marks.push((bytes.len(), s.ts));
            bytes.extend_from_slice(&s.data[skip..]);
            expected = end;
        }
        let (pkts, _) = framing::packets(&bytes);
        for p in pkts {
            let ts = match marks.binary_search_by_key(&p.stream_offset, |m| m.0) {
                Ok(i) => marks[i].1,
                Err(0) => marks.first().map(|m| m.1).unwrap_or(0),
                Err(i) => marks[i - 1].1,
            };
            for (id, name, is_self) in masked_records(&p.body) {
                names.entry(id).or_insert((name, is_self));
            }
            if p.body.len() < 2 {
                continue;
            }
            let h: String = p.body.iter().map(|b| format!("{b:02x}")).collect();
            let _ = writeln!(out, "{ts}\t{si}\t{}\t{:02x}{:02x}\t{h}", p.depth, p.body[0], p.body[1]);
        }
    }
    std::fs::write(format!("{dir}/packets.tsv"), out).map_err(|e| e.to_string())?;
    let mut n = String::new();
    for (id, (name, s)) in &names {
        let _ = writeln!(n, "{id}\t{name}\t{}", *s as u8);
    }
    std::fs::write(format!("{dir}/names.tsv"), n).map_err(|e| e.to_string())?;
    println!("Écrit : {dir}/packets.tsv et {dir}/names.tsv");
    Ok(())
}

fn finish(dir: &str, report: &str) -> Result<(), String> {
    std::fs::write(format!("{dir}/report.md"), report).map_err(|e| e.to_string())?;
    println!("{report}");
    println!("\nRapport écrit : {dir}/report.md");
    Ok(())
}

/// Enregistrements d'identité masqués (`33 36` soi, `45 36`/`44 36` autre joueur) :
/// `<opcode> <id varint> <mask1 u32> <mask2 u8> [mask2 & 1] <len u8> <utf8>`.
/// Logique reprise d'A2Tools (`masked_records`).
fn masked_records(data: &[u8]) -> Vec<(i32, String, bool)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 8 < data.len() {
        if data[i + 1] != 0x36 || !(data[i] == 0x33 || data[i] == 0x45 || data[i] == 0x44) {
            i += 1;
            continue;
        }
        let is_self = data[i] == 0x33;
        let id = framing::read_varint(data, i + 2);
        if id.length <= 0 || !(1..=9_999_999).contains(&id.value) {
            i += 1;
            continue;
        }
        let mask2 = i + 2 + id.length as usize + 4;
        if mask2 + 1 >= data.len() || data[mask2] & 0x01 == 0 {
            i += 1;
            continue;
        }
        let len = data[mask2 + 1] as usize;
        if !(1..=36).contains(&len) || mask2 + 2 + len > data.len() {
            i += 1;
            continue;
        }
        let field = &data[mask2 + 2..mask2 + 2 + len];
        let Ok(name) = std::str::from_utf8(field) else {
            i += 1;
            continue;
        };
        if name.chars().any(|c| c.is_control() || c.is_whitespace()) || name.chars().count() > 24 {
            i += 1;
            continue;
        }
        out.push((id.value, name.to_string(), is_self));
        i = mask2 + 2 + len;
    }
    out
}

/// Les ids d'entités connus présents dans `body` (varint à chaque offset, et u32 LE).
fn entity_ids_in(body: &[u8], names: &HashMap<i32, String>) -> Vec<String> {
    let mut found: Vec<(usize, i32, &str)> = Vec::new();
    for off in 0..body.len() {
        let v = framing::read_varint(body, off);
        if v.length > 0 && v.value >= 100 {
            if let Some(n) = names.get(&v.value) {
                found.push((off, v.value, "varint"));
                let _ = n;
            }
        }
        if off + 4 <= body.len() {
            let u = u32::from_le_bytes([body[off], body[off + 1], body[off + 2], body[off + 3]]) as i32;
            if u >= 100 && names.contains_key(&u) {
                found.push((off, u, "u32"));
            }
        }
    }
    found.sort();
    found.dedup_by_key(|f| (f.0, f.1));
    found
        .iter()
        .map(|(off, id, kind)| format!("{id} ({}) @{off} {kind}", names.get(id).map(String::as_str).unwrap_or("?")))
        .collect()
}

/// Les ids nommés présents dans `body` (varint qui ne prolonge pas l'octet précédent, ou u32 LE).
fn named_ids_in(body: &[u8], names: &HashMap<i32, String>) -> HashSet<i32> {
    let mut out = HashSet::new();
    for off in 0..body.len() {
        if off == 0 || body[off - 1] & 0x80 == 0 {
            let v = framing::read_varint(body, off);
            if v.length > 0 && names.contains_key(&v.value) {
                out.insert(v.value);
            }
        }
        if off + 4 <= body.len() {
            let u = u32::from_le_bytes([body[off], body[off + 1], body[off + 2], body[off + 3]]) as i32;
            if u >= 100 && names.contains_key(&u) {
                out.insert(u);
            }
        }
    }
    out
}
