// Mode démo : la bulle ouverte dans un navigateur avec `?demo` (hors Tauri),
// avec les joueurs fictifs de la maquette. Sert aux captures d'écran et à
// vérifier le rendu sans le jeu. Jamais actif dans l'app.
import type { Kind, Row, Status } from "./api";

export const DEMO = !("__TAURI_INTERNALS__" in window) && new URLSearchParams(location.search).has("demo");

const now = Date.now();
const d: [string, string, number, Kind, number, number, number][] = [
  ["Kaelthorn", "Gladiateur", 70, "mort", 2783, 170700, 1],
  ["Nyxara", "Assassin", 75, "combat", 2610, 152300, 1],
  ["Vandrel", "Templier", 190, "combat", 2488, 139900, 1],
  ["Ilyane", "Clerc", 360, "groupe", 2552, 146000, 1],
  ["Ostrya", "Rôdeur", 370, "groupe", 2701, 161400, 3],
  ["Telkhar", "Sorcier", 380, "groupe", 2395, 128700, 1],
  ["Mirelda", "Aède", 840, "buff", 2630, 150100, 1],
  ["Draugmir", "Gladiateur", 1320, "kill", 2240, 117500, 2],
  ["Feyrith", "Spiritualiste", 1860, "cible", 2580, 144800, 1],
  ["Zephal", "Assassin", 2880, "cible", 2815, 176200, 1],
  ["Corvane", "Templier", 4320, "combat", 2466, 136000, 1],
  ["Sombrelame", "Rôdeur", 7500, "cible", 2350, 124400, 1],
  ["Elwyrn", "Clerc", 9000, "cible", 2290, 119800, 1],
];

export const demoRows: Row[] = d.map(([name, cls, ago, kind, gs, cp, count], i) => ({
  id: i + 1,
  name,
  server: 1303,
  pc_id: 0,
  class: cls,
  level: 45,
  gs,
  cp,
  guild: "",
  abyss_rank: 0,
  character_id: null,
  last_seen_ms: now - ago * 1000,
  last_kind: kind,
  pinned: i === 2,
  count,
}));

export const demoStatus: Status = {
  capture: { etat: "active", serveur: "démo" },
  joueurs_en_memoire: 42,
  moi: { key: 1, confirmed: true, name: "Moi", server: 1303 },
  reference: { source: "paquet", cp: 145000, gs: 2600 },
};
