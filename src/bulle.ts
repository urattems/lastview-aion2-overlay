// Bulle (section 5) : replié / déroulé, d'après la maquette validée par Gio.
// Le survol (opacité, clics traversants) est piloté côté Rust (overlay.rs) :
// ici on applique seulement l'opacité reçue avec l'événement « survol ».
import "@fontsource/barlow/400.css";
import "@fontsource/barlow/500.css";
import "@fontsource/barlow/600.css";
import "@fontsource/barlow-condensed/500.css";
import "@fontsource/barlow-condensed/600.css";
import "@fontsource/barlow-condensed/700.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { appel, ecoute, KIND_LABEL, liste, onListe, type Row } from "./lib/api";
import { DEMO } from "./lib/demo";
import { brancherRecherche } from "./lib/recherche";

interface Reglages {
  opacite_repos: number;
  opacite_survol: number;
  verrouillee: boolean;
  tags: boolean;
  compteur: boolean;
  lignes: number;
}

// Toute erreur du script part dans le journal de l'app.
window.addEventListener("error", (e) => void appel("journal_front", { fenetre: "bulle", message: `${e.message} (${e.filename}:${e.lineno})` }));
window.addEventListener("unhandledrejection", (e) => void appel("journal_front", { fenetre: "bulle", message: String(e.reason) }));

const racine = document.getElementById("bulle")!;
let rows: Row[] = [];
let reglages: Reglages = { opacite_repos: 30, opacite_survol: 100, verrouillee: false, tags: true, compteur: true, lignes: 5 };
let ouvert = lireOuvert() || new URLSearchParams(location.search).get("demo") === "deroule";
let filtreEpingles = false;
const vus = new Map<number, number>(); // id → last_seen_ms déjà affiché

// ---------- Icônes (SVG statiques, traits fins) ----------
const SVG = {
  oeil: (t: number) =>
    `<svg aria-hidden="true" width="${t}" height="${t}" viewBox="0 0 24 24" fill="none" stroke="#F2A541" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/></svg>`,
  chevronBas: `<svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="#9AA4B2" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9l6 6 6-6"/></svg>`,
  chevronHaut: `<svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 15l6-6 6 6"/></svg>`,
  plus: `<svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>`,
  etoile: (pleine: boolean) =>
    `<svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="${pleine ? "currentColor" : "none"}" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z"/></svg>`,
  reglages: `<svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>`,
  epingle: `<svg aria-hidden="true" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z"/></svg>`,
};

// ---------- Utilitaires ----------
function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  // textContent seulement : aucun pseudo n'est jamais injecté comme HTML.
  if (text !== undefined) e.textContent = text;
  return e;
}

function icone(svg: string): HTMLSpanElement {
  const s = el("span");
  s.style.display = "inline-flex";
  s.innerHTML = svg; // SVG statiques définis ci-dessus, aucune donnée externe.
  return s;
}

/** 170 700 → « 170,7k » (comme la maquette). */
function cpCourt(cp: number): string {
  if (cp <= 0) return "—";
  if (cp < 1000) return String(cp);
  return (cp / 1000).toLocaleString("fr-FR", { minimumFractionDigits: 1, maximumFractionDigits: 1 }) + "k";
}

/** « à l'instant », « il y a 20 s », « il y a 3 min », « il y a 1 h 12 ». */
export function ilYa(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.floor((now - ms) / 1000));
  if (s < 10) return "à l'instant";
  if (s < 60) return `il y a ${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `il y a ${m} min`;
  const h = Math.floor(m / 60);
  return `il y a ${h} h ${String(m % 60).padStart(2, "0")}`;
}

/** CP d'une seule couleur, sans flèche ▲▼ (choix de Gio, 12/11/2026).
 *  CP normalisé (Abysses) : le dernier CP réel connu avec un petit « N », ou « — ». */
function cpElement(cp: number, taille: number, normalise = false): HTMLSpanElement {
  const s = el("span", "cp");
  s.style.fontSize = `${taille}px`;
  s.append(el("span", undefined, cpCourt(cp)));
  if (normalise) {
    const n = el("span", "normalise", "N");
    s.title = cp > 0 ? "CP normalisé (Abysses) : dernier CP réel connu affiché" : "CP normalisé (Abysses) : aucun CP réel connu";
    s.prepend(n);
  }
  return s;
}

function lireOuvert(): boolean {
  try {
    return localStorage.getItem("bulle-ouverte") === "1";
  } catch {
    return false;
  }
}

function basculer() {
  ouvert = !ouvert;
  try {
    localStorage.setItem("bulle-ouverte", ouvert ? "1" : "0");
  } catch {
    /* stockage indisponible : sans importance */
  }
  rendre();
}

function journalClic(cible: string) {
  void appel("clic_bulle", { cible });
}

// ---------- Rendu ----------
function rendre() {
  racine.classList.toggle("verrouillee", reglages.verrouillee);
  racine.replaceChildren(ouvert ? deroule() : replie());
  ajusterTaille();
}

function replie(): HTMLElement {
  const b = el("button", "pastille");
  b.type = "button";
  // Clic simple : dérouler. Glisser (plus de 4 px) : déplacer la bulle repliée.
  let depart: { x: number; y: number } | null = null;
  let glisse = false;
  b.addEventListener("mousedown", (e) => {
    if (e.button === 0) {
      depart = { x: e.screenX, y: e.screenY };
      glisse = false;
    }
  });
  b.addEventListener("mousemove", (e) => {
    if (!depart || glisse || reglages.verrouillee || DEMO) return;
    if (Math.abs(e.screenX - depart.x) + Math.abs(e.screenY - depart.y) > 4) {
      glisse = true;
      depart = null;
      void getCurrentWindow().startDragging();
    }
  });
  b.addEventListener("click", () => {
    depart = null;
    if (glisse) {
      glisse = false;
      return;
    }
    journalClic("pastille");
    basculer();
  });
  b.append(icone(SVG.oeil(17)));
  const dernier = rows[0];
  if (!dernier) {
    b.setAttribute("aria-label", "Dérouler la liste, aucune rencontre");
    b.append(el("span", "vide", "Aucune rencontre"));
  } else {
    b.setAttribute("aria-label", `Dérouler la liste, dernier joueur : ${dernier.name}`);
    b.append(el("span", "pseudo", dernier.name), el("span", "classe", dernier.class ?? ""), cpElement(dernier.cp, 19, dernier.cp_normalise));
  }
  b.append(icone(SVG.chevronBas));
  return b;
}

let listeEl: HTMLDivElement | null = null;

function deroule(): HTMLElement {
  const p = el("section", "panneau");
  p.setAttribute("aria-label", "Last View");

  const entete = el("div", "entete");
  const titre = el("span", "titre cond", "LAST VIEW");
  const poignee = el("div", "poignee");
  // Poignée de déplacement (5.4), sauf si la position est verrouillée.
  poignee.addEventListener("mousedown", (e) => {
    if (e.button === 0 && !reglages.verrouillee && !DEMO) void getCurrentWindow().startDragging();
  });
  const recherche = el("input", "recherche");
  recherche.type = "search";
  recherche.placeholder = "Rechercher";
  recherche.setAttribute("aria-label", "Rechercher un joueur par pseudo");
  recherche.autocomplete = "off";
  recherche.spellcheck = false;
  // Le champ prend le clavier seulement pendant la saisie (5.3), puis le rend au jeu.
  recherche.addEventListener("focus", () => void appel("recherche_focus", { actif: true }));
  recherche.addEventListener("blur", () => {
    rech.vider();
    void appel("recherche_focus", { actif: false });
  });
  const suggestions = el("div", "suggestions");
  suggestions.hidden = true;
  const rech = brancherRecherche(
    recherche,
    suggestions,
    (s) => {
      journalClic("suggestion");
      void appel("ouvrir_fiche_nom", { nom: s.nom, serveur: s.serveur_id || null });
      recherche.blur();
    },
    ajusterTaille,
    () => recherche.blur(),
  );
  const epingles = el("button", `icone${filtreEpingles ? " actif" : ""}`);
  epingles.type = "button";
  epingles.setAttribute("aria-label", "Épinglés seulement");
  epingles.setAttribute("aria-pressed", String(filtreEpingles));
  epingles.append(icone(SVG.epingle));
  epingles.addEventListener("click", () => {
    journalClic("filtre épinglés");
    filtreEpingles = !filtreEpingles;
    rendre();
  });
  const replier = el("button", "icone");
  replier.type = "button";
  replier.setAttribute("aria-label", "Replier la liste");
  replier.append(icone(SVG.chevronHaut));
  replier.addEventListener("click", () => {
    journalClic("replier");
    basculer();
  });
  const parametres = el("button", "icone");
  parametres.type = "button";
  parametres.setAttribute("aria-label", "Paramètres");
  parametres.title = "Paramètres";
  parametres.append(icone(SVG.reglages));
  parametres.addEventListener("click", () => {
    journalClic("paramètres");
    void appel("ouvrir_parametres");
  });
  entete.append(icone(SVG.oeil(18)), titre, poignee, recherche, epingles, parametres, replier);

  const colonnes = el("div", "colonnes cond");
  colonnes.append(el("span", "joueur", "JOUEUR"), el("span", "col-gs", "GS"), el("span", "col-cp", "CP"), el("span", "col-plus"));

  listeEl = el("div", "liste");
  p.append(entete, suggestions, colonnes, listeEl);
  remplirListe();
  return p;
}

function ligne(r: Row, nouvelle: boolean): HTMLElement {
  const l = el("div", `ligne${nouvelle ? " nouvelle" : ""}`);
  l.dataset.id = String(r.id);
  const infos = el("div", "infos");
  const haut = el("div", "haut");
  haut.append(el("span", "pseudo", r.name));
  if (reglages.tags) haut.append(el("span", `tag cond ${r.last_kind}`, KIND_LABEL[r.last_kind]));
  if (reglages.compteur && r.count >= 2) haut.append(el("span", "compteur cond", `×${r.count}`));
  const etoile = el("button", `etoile${r.pinned ? " pleine" : ""}`);
  etoile.type = "button";
  etoile.setAttribute("aria-label", r.pinned ? `Désépingler ${r.name}` : `Épingler ${r.name}`);
  etoile.append(icone(SVG.etoile(r.pinned)));
  etoile.addEventListener("click", () => {
    journalClic("étoile");
    void appel("epingler", { id: r.id, oui: !r.pinned });
  });
  haut.append(etoile);
  const sous = el("span", "sous", `${r.class ?? "?"} · ${ilYa(r.last_seen_ms)}`);
  sous.dataset.ts = String(r.last_seen_ms);
  sous.dataset.classe = r.class ?? "?";
  infos.append(haut, sous);

  const gs = el("span", "col-gs chiffre", r.gs > 0 ? String(r.gs) : "—");
  const cp = cpElement(r.cp, 18, r.cp_normalise);
  cp.classList.add("col-cp");
  const plus = el("button", "plus");
  plus.type = "button";
  plus.setAttribute("aria-label", `Ouvrir la fiche de ${r.name}`);
  plus.append(icone(SVG.plus));
  plus.addEventListener("click", () => {
    journalClic("+");
    void appel("ouvrir_fiche", { id: r.id });
  });
  l.append(infos, gs, cp, plus);
  return l;
}

function remplirListe() {
  if (!listeEl) return;
  // Ancre de défilement : si Gio a défilé, la vue ne saute pas quand une ligne arrive en haut.
  let ancre: { id: string; decalage: number } | null = null;
  if (listeEl.scrollTop > 0) {
    const top = listeEl.getBoundingClientRect().top;
    for (const c of Array.from(listeEl.children) as HTMLElement[]) {
      const r = c.getBoundingClientRect();
      if (r.bottom > top && c.dataset.id) {
        ancre = { id: c.dataset.id, decalage: r.top - top };
        break;
      }
    }
  }
  const visibles = rows.filter((r) => !filtreEpingles || r.pinned);
  const enfants: HTMLElement[] = visibles.map((r) => {
    const avant = vus.get(r.id);
    const nouvelle = avant !== undefined && r.last_seen_ms > avant;
    return ligne(r, nouvelle);
  });
  if (enfants.length === 0) {
    enfants.push(el("div", "aucune", filtreEpingles ? "Aucun joueur épinglé" : "Aucune rencontre"));
  }
  listeEl.replaceChildren(...enfants);
  // Fin de la mise en évidence (1 s).
  requestAnimationFrame(() => {
    for (const n of Array.from(listeEl!.querySelectorAll(".ligne.nouvelle"))) {
      requestAnimationFrame(() => n.classList.remove("nouvelle"));
    }
  });
  if (ancre) {
    const cible = listeEl.querySelector<HTMLElement>(`[data-id="${ancre.id}"]`);
    if (cible) listeEl.scrollTop += cible.getBoundingClientRect().top - listeEl.getBoundingClientRect().top - ancre.decalage;
  }
}

/** La fenêtre colle au contenu : la zone de survol est exactement la bulle. */
let derniereTaille = "";
function ajusterTaille() {
  requestAnimationFrame(() => {
    const r = racine.getBoundingClientRect();
    const t = `${Math.ceil(r.width)}x${Math.ceil(r.height)}`;
    if (t !== derniereTaille && r.width > 0) {
      derniereTaille = t;
      void appel("taille_bulle", { largeur: Math.ceil(r.width), hauteur: Math.ceil(r.height) });
    }
  });
}

// « il y a … » mis à jour en continu, sans reconstruire la liste.
setInterval(() => {
  const now = Date.now();
  for (const s of Array.from(document.querySelectorAll<HTMLElement>(".sous[data-ts]"))) {
    s.textContent = `${s.dataset.classe} · ${ilYa(Number(s.dataset.ts), now)}`;
  }
}, 1000);

async function rafraichir() {
  rows = await liste();
  if (ouvert && listeEl && racine.contains(listeEl)) remplirListe();
  else rendre();
  ajusterTaille();
  for (const r of rows) vus.set(r.id, r.last_seen_ms);
}

async function lireReglages() {
  try {
    reglages = await appel<Reglages>("reglages_bulle");
  } catch {
    /* valeurs par défaut */
  }
  const repos = racine.classList.contains("repos");
  document.documentElement.style.setProperty("--opacite", String((repos ? reglages.opacite_repos : reglages.opacite_survol) / 100));
  document.documentElement.style.setProperty("--lignes", String(Math.min(20, Math.max(3, reglages.lignes))));
}

async function main() {
  if (DEMO) {
    // Fond de la maquette, pour comparer les captures.
    document.body.style.background = "#0B0E12";
    document.body.style.padding = "20px";
  }
  await lireReglages();
  // Paramètres modifiés (section 8) : appliqués tout de suite.
  await ecoute<null>("parametres-modifies", () => void lireReglages().then(rendre));
  // Raccourci « déplier / replier la bulle ».
  await ecoute<null>("basculer", () => basculer());
  await ecoute<{ actif: boolean; opacite: number }>("survol", (p) => {
    document.documentElement.style.setProperty("--opacite", String(p.opacite / 100));
    racine.classList.toggle("repos", !p.actif);
  });
  await onListe(() => void rafraichir());
  rows = await liste();
  for (const r of rows) vus.set(r.id, r.last_seen_ms);
  rendre();
}

void main();
