// Fiche joueur (section 6) : calque de la page personnage de shugo.gg.
// Étape 1 de la porte 6 : en-tête et onglet Équipement avec ses infobulles.
// Les autres onglets suivront une fois le calque validé.
import "@fontsource/noto-sans/400.css";
import "@fontsource/noto-sans/500.css";
import "@fontsource/noto-sans/600.css";
import "@fontsource/noto-sans/700.css";
import { toBlob } from "html-to-image";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { appel, ecoute, KIND_LABEL, type Row } from "./lib/api";

// ---------- Types (réponses de l'API NCSoft : tout champ peut manquer) ----------
type J = Record<string, any>;

interface SearchHit {
  character_id: string;
  name: string;
  server_id: number;
  server_name: string;
  level: number;
  pc_id: number;
}

interface FicheReponse {
  joueur: Row | null;
  candidats: SearchHit[];
  character_id: string | null;
  serveur: number | null;
  info: J | null;
  equipement: J | null;
  donnees_ms: number | null;
  erreur: string | null;
  pause_s: number | null;
}

interface Demande {
  id?: number;
  nom?: string;
  serveur?: number | null;
  /** Ouvert depuis la recherche : badge « Recherche ». */
  recherche?: boolean;
}

window.addEventListener("error", (e) => void appel("journal_front", { fenetre: "fiche", message: `${e.message} (${e.filename}:${e.lineno})` }));
window.addEventListener("unhandledrejection", (e) => void appel("journal_front", { fenetre: "fiche", message: String(e.reason) }));

const racine = document.getElementById("fiche")!;
const bulle = document.getElementById("infobulle")!;

let demande: Demande | null = null;
let rep: FicheReponse | null = null;
let chargement = false;
let onglet = 0;
let plateau: number | null = null;
let pleinEcran = false;
let roueTexte = false;
const plateaux = new Map<string, Promise<J>>();
const historique: Demande[] = [];
let position = -1;
const detailsPieces = new Map<string, Promise<J>>();

// ---------- Couleurs (annexe B) ----------
const GRADES: Record<string, { c: string; bord: string; lueur: string }> = {
  Unique: { c: "#FFD700", bord: "rgba(255, 215, 0, 0.4)", lueur: "rgba(255, 215, 0, 0.08)" },
  Legend: { c: "#6699FF", bord: "rgba(51, 102, 255, 0.4)", lueur: "rgba(51, 102, 255, 0.10)" },
  Special: { c: "#00BFFF", bord: "rgba(0, 191, 255, 0.4)", lueur: "rgba(0, 191, 255, 0.08)" },
  Epic: { c: "#B36BFF", bord: "rgba(179, 107, 255, 0.4)", lueur: "rgba(179, 107, 255, 0.08)" },
  Rare: { c: "#4ADE80", bord: "rgba(74, 222, 128, 0.4)", lueur: "rgba(74, 222, 128, 0.08)" },
  Common: { c: "#C9D1D9", bord: "rgba(201, 209, 217, 0.25)", lueur: "rgba(201, 209, 217, 0.05)" },
};
const FAMILIER = { c: "#FFFFFF", bord: "rgba(99, 189, 255, 0.35)", lueur: "rgba(99, 189, 255, 0.08)" };
const grade = (g?: string) => GRADES[g ?? ""] ?? GRADES.Common;
/** Pierres de mana : couleur selon leur grade. */
const PIERRE: Record<string, string> = { Unique: "#FFD700", Legend: "#8CC8FF", Special: "#00BFFF", Epic: "#B36BFF", Rare: "#4ADE80", Common: "#C9D1D9" };

// ---------- Emplacements, dans l'ordre de la section 6.3 ----------
type Emp = { pos: number; nom: string };
const SECTIONS: { titre: string; emps: Emp[] }[] = [
  { titre: "Armes", emps: [{ pos: 1, nom: "Main principale" }, { pos: 2, nom: "Main secondaire" }] },
  {
    titre: "Armures",
    emps: [
      { pos: 5, nom: "Torse" },
      { pos: 6, nom: "Bas" },
      { pos: 3, nom: "Casque" },
      { pos: 4, nom: "Épaulettes" },
      { pos: 7, nom: "Gants" },
      { pos: 8, nom: "Chaussures" },
      { pos: 19, nom: "Cape" },
    ],
  },
  {
    titre: "Accessoires",
    emps: [
      { pos: 17, nom: "Ceinture" },
      { pos: 10, nom: "Collier" },
      { pos: 11, nom: "Boucle d’oreille" },
      { pos: 12, nom: "Boucle d’oreille" },
      { pos: 13, nom: "Anneau" },
      { pos: 14, nom: "Anneau" },
      { pos: 15, nom: "Bracelet" },
      { pos: 16, nom: "Bracelet" },
      { pos: 22, nom: "Amulette" },
    ],
  },
  { titre: "Runes", emps: [{ pos: 23, nom: "Emplacement de rune 1" }, { pos: 24, nom: "Emplacement de rune 2" }] },
  {
    titre: "Arcanes",
    emps: [41, 42, 43, 44, 45].map((pos, i) => ({ pos, nom: `Emplacement d’arcane ${i + 1}` })),
  },
];

// ---------- Utilitaires ----------
function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  // textContent uniquement : rien venant de l'API n'est injecté comme HTML.
  if (text !== undefined) e.textContent = text;
  return e;
}

function svg(chemin: string, taille = 13, epaisseur = 2.2): HTMLSpanElement {
  const s = el("span");
  s.style.display = "inline-flex";
  // SVG statiques définis dans ce fichier, aucune donnée externe.
  s.innerHTML = `<svg aria-hidden="true" width="${taille}" height="${taille}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="${epaisseur}" stroke-linecap="round" stroke-linejoin="round"><path d="${chemin}"/></svg>`;
  return s;
}

const ICONES = {
  actualiser: "M21 12a9 9 0 1 1-3-6.7L21 8M21 3v5h-5",
  externe: "M7 17L17 7M9 7h8v8",
  fermer: "M6 6l12 12M18 6L6 18",
  precedent: "M15 18l-6-6 6-6",
  copier: "M9 9h11v11H9zM5 15H4V4h11v1",
  suivant: "M9 18l6-6-6-6",
  pc: "M14.5 17.5L3 6V3h3l11.5 11.5M13 19l6-6M16 16l4 4M19 21l2-2",
  attaque: "M14.5 17.5L3 6V3h3l11.5 11.5M13 19l6-6M16 16l4 4M19 21l2-2",
  defense: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z",
  autre: "M12 2l3 7h7l-5.5 4.5 2 7.5-6.5-4.5L5.5 21l2-7.5L2 9h7z",
};

/** Image NCSoft servie par le cache disque de LastView (protocole lvimg). */
function img(url?: string, alt = ""): HTMLImageElement {
  const i = el("img");
  i.alt = alt;
  i.decoding = "async";
  if (url && /^https:\/\//.test(url)) i.src = `http://lvimg.localhost/${encodeURIComponent(url)}`;
  return i;
}

function ilYa(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.floor((now - ms) / 1000));
  if (s < 10) return "à l'instant";
  if (s < 60) return `il y a ${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `il y a ${m} min`;
  const h = Math.floor(m / 60);
  return `il y a ${h} h ${String(m % 60).padStart(2, "0")}`;
}

const nombre = (n: number) => n.toLocaleString("fr-FR");
const statValeur = (info: J, type: string): number => Number((info?.stat?.statList ?? []).find((s: J) => s?.type === type)?.value ?? 0);

// ---------- Navigation ----------
async function ouvrir(d: Demande, { ajouter = true, actualiser = false } = {}) {
  if (ajouter) {
    historique.splice(position + 1);
    historique.push(d);
    position = historique.length - 1;
  }
  demande = d;
  chargement = true;
  plateau = null;
  fermerInfobulle();
  rendre();
  const r = await appel<FicheReponse>("fiche_ouvrir", { id: d.id ?? null, nom: d.nom ?? null, serveur: d.serveur ?? null, actualiser });
  if (demande !== d) return; // une autre fiche a été demandée entre-temps
  rep = r;
  chargement = false;
  rendre();
}

function naviguer(delta: number) {
  const p = position + delta;
  if (p < 0 || p >= historique.length) return;
  position = p;
  void ouvrir(historique[p], { ajouter: false });
}

// ---------- Rendu ----------
function rendre() {
  if (!demande) {
    racine.replaceChildren(el("div", "message info", "Aucune fiche ouverte. Clique sur « + » dans la bulle pour ouvrir la fiche d'un joueur."));
    return;
  }
  const enfants: HTMLElement[] = [entete()];
  if (rep?.erreur && !chargement) enfants.push(messageErreur());
  enfants.push(onglets());
  if (onglet === 0) enfants.push(...equipement());
  else if (onglet === 1) enfants.push(...competences());
  else if (onglet === 2) enfants.push(...daevanion());
  else enfants.push(...stats());
  racine.replaceChildren(...enfants);
}

function bloc(l: number, h: number, extra = ""): HTMLDivElement {
  const d = el("div", "squelette");
  d.style.width = `${l}px`;
  d.style.height = `${h}px`;
  if (extra) d.style.cssText += extra;
  return d;
}

function entete(): HTMLElement {
  const h = el("header", "entete");
  const info = chargement ? null : rep?.info;
  const p = info?.profile ?? {};
  const joueur = rep?.joueur ?? null;

  // Portrait et niveau
  const portrait = el("div", "portrait");
  if (chargement) portrait.append(bloc(96, 96, "border-radius:12px"));
  else if (p.profileImage) portrait.append(img(p.profileImage, `Portrait de ${p.characterName ?? ""}`));
  else portrait.append(el("div", "vide"));
  const niveau = p.characterLevel ?? joueur?.level;
  if (niveau && !chargement) portrait.append(el("span", "niveau", String(niveau)));

  // Identité
  const id = el("div", "identite");
  if (chargement) {
    id.append(bloc(220, 26), bloc(150, 12), bloc(120, 14), bloc(260, 22, "margin-top:6px"), bloc(130, 28, "margin-top:6px"));
  } else {
    const ligne = el("div", "ligne-nom");
    ligne.append(el("h1", undefined, p.characterName ?? demande?.nom ?? joueur?.name ?? "?"));
    const serveur = p.serverName ?? (rep?.candidats?.[0]?.server_name || "");
    if (serveur) ligne.append(el("span", "badge serveur", serveur));
    if (joueur && !demande?.recherche) ligne.append(el("span", "badge rencontre", `${KIND_LABEL[joueur.last_kind]} ${ilYa(joueur.last_seen_ms)}`));
    else ligne.append(el("span", "badge rencontre", "Recherche"));
    id.append(ligne);
    if (rep?.donnees_ms) {
      const maj = el("span", "maj", `Données API ${ilYa(rep.donnees_ms)}`);
      if (Date.now() - rep.donnees_ms > 10 * 60 * 1000) maj.append(el("span", "vieux", " · anciennes, Actualiser"));
      id.append(maj);
    }
    if (p.titleName) {
      const t = el("span", "titre-perso", p.titleName);
      t.style.color = grade(p.titleGrade).c;
      id.append(t);
    }
    const classe = el("div", "ligne-classe");
    const parts = [p.className ?? joueur?.class, p.raceName].filter(Boolean) as string[];
    parts.forEach((x, i) => {
      if (i) classe.append(el("span", "point", "•"));
      classe.append(el("span", undefined, x));
    });
    const iniv = info ? statValeur(info, "ItemLevel") : joueur?.gs ?? 0;
    if (iniv) {
      classe.append(el("span", "point", "•"));
      const pi = el("span", "pastille-iniv");
      pi.append(el("span", "l", "iNiv"), el("span", "v", String(iniv)));
      classe.append(pi);
    }
    id.append(classe);
    const cp = Number(p.combatPower ?? joueur?.cp ?? 0);
    if (cp) {
      const pc = el("span", "pastille-pc");
      const ic = svg(ICONES.pc, 14, 2);
      ic.style.color = "#78F2FB";
      pc.append(ic, el("span", "l", "PC"), el("span", "v", nombre(cp)));
      id.append(pc);
    }
  }

  // Actions et titres
  const actions = el("div", "actions");
  const boutons = el("div", "boutons");
  const prec = el("button", "rond");
  prec.setAttribute("aria-label", "Fiche précédente");
  prec.disabled = position <= 0;
  prec.append(svg(ICONES.precedent, 14));
  prec.addEventListener("click", () => naviguer(-1));
  const suiv = el("button", "rond");
  suiv.setAttribute("aria-label", "Fiche suivante");
  suiv.disabled = position >= historique.length - 1;
  suiv.append(svg(ICONES.suivant, 14));
  suiv.addEventListener("click", () => naviguer(1));
  const act = el("button", "bouton actualiser");
  act.append(svg(ICONES.actualiser), el("span", undefined, "Actualiser"));
  // « Actualiser » permis si la donnée a plus de 30 s (section 4.10).
  act.disabled = chargement || !demande || (rep?.donnees_ms != null && Date.now() - rep.donnees_ms < 30_000);
  act.addEventListener("click", () => demande && void ouvrir(demande, { ajouter: false, actualiser: true }));
  const shugo = el("button", "bouton shugo");
  shugo.append(el("span", undefined, "Shugo.GG"), svg(ICONES.externe, 12));
  shugo.disabled = !p.characterName;
  shugo.addEventListener("click", () => void appel("ouvrir_shugo", { nom: p.characterName, serveur: p.serverName ?? "" }));
  // Copier en image (6.8) : clic = presse-papiers, clic droit = enregistrer en PNG.
  const copie = el("button", "bouton copier");
  copie.append(svg(ICONES.copier), el("span", undefined, "Copier en image"));
  copie.title = "Clic droit : enregistrer en PNG";
  copie.disabled = chargement || !p.characterName;
  copie.addEventListener("click", () => void copierImage(false));
  copie.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    void copierImage(true);
  });
  const fermer = el("button", "rond");
  fermer.setAttribute("aria-label", "Fermer la fiche");
  fermer.append(svg(ICONES.fermer, 13, 2.4));
  fermer.addEventListener("click", () => void getCurrentWindow().hide());
  boutons.append(prec, suiv, act, shugo, copie, fermer);
  actions.append(boutons);

  const titres = el("ul", "titres");
  titres.setAttribute("aria-label", "Titres équipés");
  const ordre: Record<string, [number, string, string]> = { Attack: [0, ICONES.attaque, "#F2A541"], Defense: [1, ICONES.defense, "#8CC8FF"], Etc: [2, ICONES.autre, "#C0C8D2"] };
  const liste: J[] = (info?.title?.titleList ?? []).filter((t: J) => t?.equipCategory in ordre);
  liste.sort((a, b) => ordre[a.equipCategory][0] - ordre[b.equipCategory][0]);
  for (const t of liste.slice(0, 3)) {
    const li = el("li");
    const ic = svg(ordre[t.equipCategory][1], 16, 2);
    ic.style.color = ordre[t.equipCategory][2];
    li.title = [t.name, ...(t.equipStatList ?? []).map((s: J) => s?.desc)].filter(Boolean).join("\n");
    li.append(ic, el("span", undefined, t.name ?? ""));
    titres.append(li);
  }
  if (chargement) for (let i = 0; i < 3; i++) titres.append(bloc(160, 30, "border-radius:8px"));
  actions.append(titres);

  h.append(portrait, id, actions);
  return h;
}

function messageErreur(): HTMLElement {
  const m = el("div", rep?.candidats?.length && !rep?.info ? "message info" : "message");
  if (rep?.pause_s) m.textContent = `NCSoft limite les requêtes, reprise dans ${Math.ceil(rep.pause_s / 60)} min.`;
  else if (rep?.erreur === "Perso introuvable") m.textContent = "Perso introuvable chez NCSoft (pseudo changé, perso supprimé ou serveur différent).";
  else m.textContent = rep?.erreur ?? "";
  if (rep && rep.info) m.append(el("div", undefined, "Les données affichées viennent du cache."));
  // Homonymes : sélecteur de candidats (6.7).
  if (rep?.candidats && rep.candidats.length > 1) {
    const c = el("div", "candidats");
    for (const h of rep.candidats) {
      const b = el("button", undefined, `${h.name} · ${h.server_name} · niv. ${h.level}`);
      b.addEventListener("click", () => void ouvrir({ nom: h.name, serveur: h.server_id, recherche: true }));
      c.append(b);
    }
    m.append(c);
  }
  return m;
}

function onglets(): HTMLElement {
  const n = el("nav", "onglets");
  n.setAttribute("aria-label", "Onglets de la fiche");
  ["Équipement", "Compétences", "Daevanion", "Stats"].forEach((nom, i) => {
    const b = el("button", undefined, nom);
    b.type = "button";
    b.setAttribute("aria-pressed", String(i === onglet));
    b.addEventListener("click", () => {
      onglet = i;
      fermerInfobulle();
      rendre();
    });
    n.append(b);
  });
  return n;
}

// ---------- Onglet Équipement ----------
function equipement(): HTMLElement[] {
  const liste: J[] = rep?.equipement?.equipment?.equipmentList ?? [];
  const parPos = new Map<number, J>(liste.map((it) => [Number(it.slotPos), it]));
  const sections = SECTIONS.map(({ titre, emps }) => section(titre, emps.map((e) => (chargement ? squelettePiece() : carte(parPos.get(e.pos), e)))));
  const pw = rep?.equipement?.petwing ?? {};
  const familier: HTMLElement[] = chargement
    ? [squelettePiece(), squelettePiece()]
    : [carteSimple(pw.pet, pw.pet ? `Familier · Niv. ${pw.pet.level ?? "?"}` : "Familier", FAMILIER), carteSimple(pw.wing, "Ailes", grade(pw.wing?.grade))];
  sections.push(section("Familier et ailes", familier));
  return sections;
}

function section(titre: string, cartes: HTMLElement[]): HTMLElement {
  const s = el("section", "section");
  s.setAttribute("aria-label", titre);
  const g = el("div", "grille");
  g.append(...cartes);
  s.append(el("h2", undefined, titre), g);
  return s;
}

/** Section sans la grille de pièces (contenu libre). */
function sectionLibre(titre: string, corps: HTMLElement[]): HTMLElement {
  const s = el("section", "section");
  s.setAttribute("aria-label", titre);
  s.append(el("h2", undefined, titre), ...corps);
  return s;
}

function squelettePiece(): HTMLElement {
  const d = el("div", "piece");
  const t = el("div", "textes");
  t.append(bloc(150, 14), bloc(90, 11));
  d.append(bloc(44, 44), t);
  return d;
}

function vide(emplacement: string): HTMLElement {
  const d = el("div", "piece vide");
  const ic = el("span", "icone");
  const t = el("span", "textes");
  t.append(el("span", "nom", "Vide"), el("span", "emp", emplacement));
  d.append(ic, t);
  return d;
}

function carte(it: J | undefined, e: Emp): HTMLElement {
  if (!it) return vide(e.nom);
  const g = grade(it.grade);
  const b = el("button", "piece");
  b.type = "button";
  b.dataset.emplacement = String(e.pos);
  b.style.setProperty("--bord", g.bord);
  const enchant = Number(it.enchantLevel ?? 0);
  const exceed = Number(it.exceedLevel ?? 0);
  const label = `${enchant > 0 ? `+${enchant} ` : ""}${it.name ?? ""}`;
  b.setAttribute("aria-label", `${label}, ${e.nom}`);
  const ic = el("span", "icone");
  ic.append(img(it.icon));
  if (exceed > 0) ic.append(el("span", "depassement", `◆${exceed}`));
  const t = el("span", "textes");
  const nom = el("span", "nom", label);
  nom.style.color = g.c;
  t.append(nom, el("span", "emp", e.nom));
  b.append(ic, t);
  // Infobulle au survol (après un court délai) ou au clic.
  let minuteur = 0;
  b.addEventListener("mouseenter", () => (minuteur = window.setTimeout(() => montrerInfobulle(b, it, e), 250)));
  b.addEventListener("mouseleave", () => clearTimeout(minuteur));
  b.addEventListener("click", (ev) => {
    ev.stopPropagation();
    clearTimeout(minuteur);
    montrerInfobulle(b, it, e);
  });
  return b;
}

function carteSimple(x: J | undefined, sous: string, g: { c: string; bord: string }): HTMLElement {
  if (!x) return vide(sous);
  const d = el("div", "piece");
  d.style.setProperty("--bord", g.bord);
  d.style.cursor = "default";
  const ic = el("span", "icone");
  ic.append(img(x.icon));
  const t = el("span", "textes");
  const enchant = Number(x.enchantLevel ?? 0);
  const nom = el("span", "nom", `${enchant > 0 ? `+${enchant} ` : ""}${x.name ?? ""}`);
  nom.style.color = g.c;
  t.append(nom, el("span", "emp", sous));
  d.append(ic, t);
  return d;
}

// ---------- Infobulle de pièce (6.3) ----------
let ancre: HTMLElement | null = null;

function fermerInfobulle() {
  bulle.hidden = true;
  bulle.replaceChildren();
  ancre = null;
}

function placer() {
  if (!ancre || bulle.hidden) return;
  const r = ancre.getBoundingClientRect();
  const b = bulle.getBoundingClientRect();
  const marge = 8;
  // À droite de la carte, sinon à gauche ; toujours dans l'écran.
  let x = r.right + marge;
  if (x + b.width > window.innerWidth - marge) x = r.left - b.width - marge;
  x = Math.max(marge, Math.min(x, window.innerWidth - b.width - marge));
  let y = r.top;
  y = Math.max(marge, Math.min(y, window.innerHeight - b.height - marge));
  bulle.style.left = `${x}px`;
  bulle.style.top = `${y}px`;
}

function montrerInfobulle(carteEl: HTMLElement, it: J, e: Emp) {
  if (!rep?.character_id || rep.serveur == null) return;
  ancre = carteEl;
  const g = grade(it.grade);
  bulle.style.setProperty("--bord", g.bord.replace(/0\.4\)$/, "0.55)"));
  bulle.style.setProperty("--lueur", g.lueur);
  bulle.hidden = false;
  const enchant = Number(it.enchantLevel ?? 0);
  const exceed = Number(it.exceedLevel ?? 0);
  const cle = `${rep.character_id}|${it.id}|${enchant + exceed}|${it.slotPos}`;
  // En-tête tout de suite (données de la liste), le reste à l'arrivée du détail.
  bulle.replaceChildren(teteInfobulle(it, null), chargementInfobulle());
  placer();
  let p = detailsPieces.get(cle);
  if (!p) {
    p = appel<J>("fiche_piece", { id: Number(it.id), niveau: enchant + exceed, emplacement: Number(it.slotPos), characterId: rep.character_id, serveur: rep.serveur });
    detailsPieces.set(cle, p);
    p.catch(() => detailsPieces.delete(cle));
  }
  p.then(
    (d) => {
      if (ancre !== carteEl) return;
      bulle.replaceChildren(...contenuInfobulle(it, d, e));
      placer();
    },
    (err) => {
      if (ancre !== carteEl) return;
      const m = el("div", "ib-bloc ib-rouge", String(err));
      bulle.replaceChildren(teteInfobulle(it, null), m);
      placer();
    },
  );
}

function chargementInfobulle(): HTMLElement {
  const d = el("div", "ib-chargement");
  d.append(bloc(200, 12), bloc(300, 12), bloc(260, 12), bloc(280, 12));
  return d;
}

function teteInfobulle(it: J, d: J | null): HTMLElement {
  const g = grade(d?.grade ?? it.grade);
  const tete = el("div", "ib-tete");
  tete.append(img(d?.icon ?? it.icon));
  const t = el("div", "t");
  const enchant = Number(it.enchantLevel ?? 0);
  const nom = el("span", "ib-nom", `${enchant > 0 ? `+${enchant} ` : ""}${d?.name ?? it.name ?? ""}`);
  nom.style.color = g.c;
  t.append(nom);
  if (d) {
    t.append(el("span", "ib-sous", [d.gradeName, d.categoryName].filter(Boolean).join(" · ")));
    const max = Number(d.maxExceedEnchantLevel ?? 0);
    const ex = Number(it.exceedLevel ?? 0);
    if (max > 0) {
      const l = el("span", "ib-losanges", "◆".repeat(Math.min(ex, max)) + "◇".repeat(Math.max(0, max - ex)));
      l.setAttribute("aria-label", `Dépassement ${ex} sur ${max}`);
      t.append(l);
    }
  }
  const f = el("button", "ib-fermer");
  f.setAttribute("aria-label", "Fermer le détail");
  f.append(svg(ICONES.fermer, 12, 2.4));
  f.addEventListener("click", fermerInfobulle);
  tete.append(t, f);
  return tete;
}

function ligne(gauche: string, droite: string | HTMLElement, cls = ""): HTMLElement {
  const l = el("div", `ib-ligne ${cls}`);
  l.append(el("span", undefined, gauche));
  if (typeof droite === "string") l.append(el("span", undefined, droite));
  else l.append(droite);
  return l;
}

function blocTitre(titre: string): HTMLElement {
  const b = el("div", "ib-bloc");
  b.append(el("span", "ib-label", titre));
  return b;
}

const nul = (v: unknown) => v == null || v === "" || v === "0" || v === "0%" || v === 0;

function contenuInfobulle(it: J, d: J, e: Emp): HTMLElement[] {
  const blocs: HTMLElement[] = [teteInfobulle(it, d)];

  // Niveau et classe
  const niv = el("div", "ib-bloc");
  const gaucheNiv = [d.level != null ? `Niv. ${d.level}` : "", d.equipLevel != null ? `(Niv. requis ${d.equipLevel})` : ""].filter(Boolean).join(" ");
  // Comme Shugo : « Toutes les classes » quand la pièce n'en vise aucune.
  niv.append(ligne(gaucheNiv || e.nom, (d.classNames ?? []).join(", ") || "Toutes les classes"));
  blocs.push(niv);

  // Stats de base, bonus de renforcement en vert, lignes de dépassement en orange
  const stats: J[] = d.mainStats ?? [];
  if (stats.length) {
    const b = blocTitre("Stats de base");
    for (const s of stats) {
      if (s.exceed) {
        b.append(ligne(s.name ?? "", `+${s.extra ?? s.value}`, "ib-depassement"));
      } else {
        // Comme Shugo : la valeur seule ; le minimum de l'arme reste au survol.
        const v = el("span", undefined, String(s.value ?? ""));
        if (s.minValue) v.title = `Minimum ${s.minValue}`;
        if (!nul(s.extra)) v.append(el("span", "ib-plus", ` (+${s.extra})`));
        b.append(ligne(s.name ?? "", v));
      }
    }
    blocs.push(b);
  }

  // Taux de lien d'âme
  if (d.soulBindRate != null && d.soulBindRate !== "") {
    const b = el("div", "ib-bloc");
    const taux = Math.max(0, Math.min(100, Number(d.soulBindRate)));
    const l = ligne("Taux de lien d'âme", `${taux.toFixed(1)}%`);
    (l.firstChild as HTMLElement).className = "ib-lien";
    const barre = el("span", "ib-barre");
    const plein = el("span");
    plein.style.width = `${taux}%`;
    barre.append(plein);
    b.append(l, barre);
    b.style.gap = "5px";
    blocs.push(b);
  }

  // Stats additionnelles : stats puis compétences, dans un seul bloc comme Shugo.
  // Maximum = subStatCount + subSkillCountMax (bâton : 3 + 3 = « 3/6 » chez Shugo).
  const subs: J[] = d.subStats ?? [];
  const comps: J[] = d.subSkills ?? [];
  const maxAdd = Number(d.subStatCount ?? 0) + Number(d.subSkillCountMax ?? 0);
  if (subs.length || comps.length || maxAdd > 0) {
    const b = blocTitre(`Stats additionnelles (${subs.length + comps.length}/${maxAdd || subs.length + comps.length})`);
    for (const s of subs) b.append(ligne(s.name ?? "", `+${String(s.value ?? "").replace(/^\+/, "")}`, "ib-vert"));
    for (const c of comps) {
      const p = el("div", "ib-pierre");
      // Comme Shugo : nom de la compétence, « +niveau » à droite.
      const nom = el("span", "ib-competence", c.name ?? "");
      p.append(img(c.icon), nom, el("span", "ib-vert ib-droite", `+${c.level ?? "?"}`));
      b.append(p);
    }
    blocs.push(b);
  }

  // Pierres de mana
  const pierres: J[] = d.magicStoneStat ?? [];
  if (Number(d.magicStoneSlotCount) > 0) {
    // Accessoires : « pierres d'âme » (icônes SoulStone), armes et armures : « pierres de mana ».
    const ame = pierres.some((m) => String(m.icon ?? "").includes("SoulStone"));
    const b = blocTitre(`${ame ? "Pierres d'âme" : "Pierres de mana"} (${pierres.length}/${d.magicStoneSlotCount})`);
    for (const m of pierres) {
      const p = el("div", "ib-pierre");
      const t = el("span", undefined, `${m.name ?? ""} ${m.value ?? ""}`);
      t.style.color = PIERRE[m.grade] ?? "#C9D1D9";
      p.append(img(m.icon), t);
      b.append(p);
    }
    blocs.push(b);
  }

  // Pierre divine
  const divines: J[] = d.godStoneStat ?? [];
  if (Number(d.godStoneSlotCount) > 0) {
    const b = blocTitre(`Pierre divine (${divines.length}/${d.godStoneSlotCount})`);
    for (const g of divines) {
      const n = el("span", "ib-vert", g.name ?? "");
      n.style.fontWeight = "600";
      b.append(n, el("span", "ib-desc", g.desc ?? ""));
    }
    blocs.push(b);
  }

  // Set d'arcanes
  if (d.set) {
    const total = (d.set.items ?? []).length;
    const b = blocTitre(`Set : ${d.set.name ?? ""} (${d.set.equippedCount ?? 0}/${total})`);
    for (const bo of d.set.bonuses ?? []) {
      const actif = Number(d.set.equippedCount ?? 0) >= Number(bo.degree ?? 99);
      for (const txt of bo.descriptions ?? []) {
        const l = el("span", actif ? "ib-vert" : "ib-gris", `${bo.degree} pièces : ${txt}`);
        b.append(l);
      }
    }
    blocs.push(b);
  }

  // Source et échange
  const fin = el("div", "ib-bloc");
  fin.style.gap = "6px";
  if ((d.sources ?? []).length) {
    fin.append(el("span", "ib-label", "Source"));
    const s = el("div", "ib-sources");
    for (const x of d.sources) s.append(el("span", undefined, x));
    fin.append(s);
  }
  if (d.tradable === false) fin.append(el("span", "ib-rouge", "Non échangeable"));
  if (fin.childElementCount) blocs.push(fin);
  return blocs;
}


// ---------- Infobulle simple (compétences, nœuds, stats) ----------
function ouvrirBulle(a: HTMLElement, contenu: HTMLElement[], bord = "rgba(120, 170, 255, 0.45)", lueur = "rgba(120, 170, 255, 0.08)") {
  ancre = a;
  bulle.style.setProperty("--bord", bord);
  bulle.style.setProperty("--lueur", lueur);
  bulle.hidden = false;
  bulle.replaceChildren(...contenu);
  placer();
}

/** Infobulle au survol (petit délai), fermée à la sortie. */
function survol(a: HTMLElement, contenu: () => HTMLElement[], bord?: string) {
  (a as HTMLElement & { ouvrirBulle?: () => void }).ouvrirBulle = () => ouvrirBulle(a, contenu(), bord);
  let m = 0;
  a.addEventListener("mouseenter", () => (m = window.setTimeout(() => ouvrirBulle(a, contenu(), bord), 150)));
  a.addEventListener("mouseleave", () => {
    clearTimeout(m);
    if (ancre === a) fermerInfobulle();
  });
}

function teteSimple(icone: string | undefined, titre: string, sous: string, couleur = "#E6F0FF"): HTMLElement {
  const tete = el("div", "ib-tete");
  if (icone) tete.append(img(icone));
  const tt = el("div", "t");
  const n = el("span", "ib-nom", titre);
  n.style.color = couleur;
  tt.append(n);
  if (sous) tt.append(el("span", "ib-sous", sous));
  tete.append(tt);
  return tete;
}

// ---------- Onglet Compétences (6.4) ----------
const CATEGORIE: Record<string, string> = { Active: "Compétence active", Passive: "Compétence passive", Dp: "Stigma" };

/** Tuile de compétence, comme Shugo : badge de niveau toujours affiché (même 0),
 *  badge vert « Équipé » en haut à droite, « Niv. n » sous le nom si non acquise. */
function tuile(s: J, etat: "normal" | "equipe" | "non-acquise" = "normal"): HTMLElement {
  const d = el("div", `tuile ${etat}`);
  const ic = el("span", "tuile-icone");
  ic.append(img(s.icon), el("span", "tuile-niveau", String(Number(s.skillLevel) || 0)));
  d.append(ic, el("span", "tuile-nom", s.name ?? ""));
  if (etat === "equipe") d.append(el("span", "tuile-badge", "Équipé"));
  if (etat === "non-acquise") d.append(el("span", "tuile-requis", `Niv. ${s.needLevel ?? "?"}`));
  survol(d, () => {
    const b = el("div", "ib-bloc");
    b.append(ligne("Niveau", String(Number(s.skillLevel) || 0)));
    if (s.category === "Dp") b.append(ligne("État", etat === "equipe" ? "Équipé" : `Non acquise (niv. ${s.needLevel ?? "?"} requis)`));
    return [teteSimple(s.icon, s.name ?? "", CATEGORIE[s.category] ?? s.category ?? ""), b];
  });
  return d;
}

function grilleTuiles(tuiles: HTMLElement[]): HTMLElement {
  const g = el("div", "tuiles");
  g.append(...tuiles);
  return g;
}

/** Titre de section façon Shugo : « Compétences actives  (12) », complément en couleur. */
function titreSection(titre: string, compte: number, extra?: string): HTMLElement {
  const h = el("h2", "titre-compte");
  h.append(el("span", undefined, titre), el("span", "compte", `(${compte})`));
  if (extra) h.append(el("span", "extra", extra));
  return h;
}

function competences(): HTMLElement[] {
  if (chargement) return [sectionLibre("Compétences", [grilleTuiles(Array.from({ length: 12 }, () => bloc(84, 96, "border-radius:10px")))])];
  // Comme Shugo : chaque groupe trié par niveau décroissant (ordre de l'API en cas d'égalité).
  const parNiveau = (l: J[]) => l.map((s, i) => [s, i] as const).sort((x, y) => (Number(y[0].skillLevel) || 0) - (Number(x[0].skillLevel) || 0) || x[1] - y[1]).map((x) => x[0]);
  const liste: J[] = rep?.equipement?.skill?.skillList ?? [];
  const actives = parNiveau(liste.filter((s) => s.category === "Active"));
  const passives = parNiveau(liste.filter((s) => s.category === "Passive"));
  const stigmas = liste.filter((s) => s.category === "Dp");
  // Shugo classe les stigmas par le seul champ « equip » : équipé, sinon non acquise.
  const equipes = parNiveau(stigmas.filter((s) => Number(s.equip)));
  const autres = parNiveau(stigmas.filter((s) => !Number(s.equip)));
  const out: HTMLElement[] = [];
  const sec = (h: HTMLElement, corps: HTMLElement[]) => {
    const s = el("section", "section");
    s.append(h, ...corps);
    out.push(s);
  };
  sec(titreSection("Compétences actives", actives.length), [grilleTuiles(actives.map((s) => tuile(s)))]);
  const corps: HTMLElement[] = [];
  if (equipes.length) corps.push(el("h3", "sous-titre vert", `Équipé (${equipes.length})`), grilleTuiles(equipes.map((s) => tuile(s, "equipe"))));
  if (autres.length) corps.push(el("h3", "sous-titre", `Non acquise (${autres.length})`), grilleTuiles(autres.map((s) => tuile(s, "non-acquise"))));
  corps.push(el("p", "mention", "La spécialisation des compétences (éclats de spécialisation) n'est pas fournie par l'API NCSoft : seuls l'état appris et l'état équipé sont connus."));
  sec(titreSection("Compétences Stigma", stigmas.length, `${equipes.length} équipée(s)`), corps);
  if (passives.length) sec(titreSection("Compétences passives", passives.length), [grilleTuiles(passives.map((s) => tuile(s)))]);
  return out;
}

// ---------- Onglet Daevanion (6.5) ----------
function chargerPlateau(id: number): Promise<J> {
  const cle = `${rep?.character_id}|${id}`;
  let p = plateaux.get(cle);
  if (!p) {
    p = appel<J>("fiche_daevanion", { characterId: rep?.character_id, serveur: rep?.serveur, plateau: id });
    plateaux.set(cle, p);
    p.catch(() => plateaux.delete(cle));
  }
  return p;
}

function daevanion(): HTMLElement[] {
  const liste: J[] = rep?.info?.daevanion?.boardList ?? [];
  if (chargement) return [sectionLibre("Daevanion", [bloc(860, 420, "border-radius:10px")])];
  if (!liste.length) return [el("div", "section a-venir", "Aucun plateau Daevanion.")];
  if (plateau == null) plateau = Number((liste.find((b) => Number(b.open)) ?? liste[0]).id);
  const choix = el("nav", "plateaux");
  for (const b of liste) {
    const btn = el("button", "plateau");
    btn.type = "button";
    btn.setAttribute("aria-pressed", String(Number(b.id) === plateau));
    btn.disabled = !Number(b.open);
    const pct = Number(b.totalNodeCount) ? (100 * Number(b.openNodeCount)) / Number(b.totalNodeCount) : 0;
    const barre = el("span", "barre");
    const plein = el("span");
    plein.style.width = `${pct}%`;
    barre.append(plein);
    btn.append(el("span", "nom", b.name ?? ""), el("span", "compte", `${b.openNodeCount ?? 0}/${b.totalNodeCount ?? 0}`), barre);
    btn.addEventListener("click", () => {
      plateau = Number(b.id);
      fermerInfobulle();
      rendre();
    });
    choix.append(btn);
  }
  const zone = el("div", `daevanion${pleinEcran ? " plein-ecran" : ""}`);
  const grilleZone = el("div", "daev-grille-zone");
  const panneau = el("aside", "daev-effets");
  grilleZone.append(bloc(560, 560, "border-radius:10px"));
  panneau.append(bloc(240, 16), bloc(220, 12, "margin-top:10px"), bloc(200, 12, "margin-top:6px"));
  zone.append(grilleZone, panneau);
  const id = plateau;
  void chargerPlateau(id).then(
    (d) => {
      if (plateau !== id || onglet !== 2) return;
      remplirPlateau(grilleZone, panneau, d);
    },
    (e) => {
      if (plateau !== id) return;
      grilleZone.replaceChildren(el("div", "message", String(e)));
    },
  );
  const s = el("section", "section");
  const tete = el("div", "section-tete");
  const plein = el("button", "bouton-plein");
  plein.type = "button";
  plein.textContent = pleinEcran ? "Quitter le plein écran" : "Plein écran";
  plein.addEventListener("click", () => {
    pleinEcran = !pleinEcran;
    rendre();
  });
  tete.append(el("h2", undefined, "Daevanion"), plein);
  s.append(tete, choix, zone);
  return [s];
}

// Icônes Daevanion servies par NCSoft (vérifiées le 13/11/2026). Le champ `icon`
// de nodeList (UT_FWindow_Daevanion_Node_*_Sprite.png) renvoie 404 : jamais utilisé.
const DAEV = "https://assets.playnccdn.com/static-aion2/characters/img/daevanion";
/** Classe (nom français de l'API) → nom anglais de l'icône de départ. */
const CLASSE_EN: Record<string, string> = {
  Gladiateur: "gladiator",
  Templier: "templar",
  "Rôdeur": "ranger",
  Assassin: "assassin",
  Spiritualiste: "elementalist",
  Sorcier: "sorcerer",
  Clerc: "cleric",
  "Aède": "chanter",
  Combattant: "fighter",
};

/** Image d'un nœud ; en repli (image absente), la forme dessinée. */
function imagesNoeud(n: J): HTMLElement[] {
  const repli = () => el("span", "forme");
  const avecRepli = (url: string, cls: string) => {
    const i = img(url);
    i.className = cls;
    i.addEventListener("error", () => i.replaceWith(repli()));
    return i;
  };
  const ouvert = Number(n.open) ? "_open" : "";
  const g = String(n.grade ?? "").toLowerCase();
  if (n.type === "Start") {
    const cls = CLASSE_EN[rep?.info?.profile?.className ?? ""];
    return [cls ? avecRepli(`${DAEV}/board_icon_start_${cls}.png`, "fond") : repli()];
  }
  if (!["common", "rare", "legend", "unique"].includes(g)) return [repli()];
  const fond = avecRepli(`${DAEV}/board_icon_${g}${ouvert}.png`, "fond");
  if (n.type !== "SkillLevel") return [fond];
  // Nœud de compétence, comme Shugo : l'icône de la compétence en carré plein, halo du grade.
  // Nœud de compétence : l'icône de la compétence (onglet Compétences) sur le fond de son grade.
  const nomComp = String(n.name ?? "").split(" - ").pop()?.trim() ?? "";
  const comp = (rep?.equipement?.skill?.skillList ?? []).find((s: J) => s.name === nomComp);
  if (!comp?.icon) return [fond];
  const ic = img(comp.icon);
  ic.className = "competence";
  ic.addEventListener("error", () => ic.replaceWith(fond));
  return [ic];
}

function remplirPlateau(grilleZone: HTMLElement, panneau: HTMLElement, d: J) {
  const noeuds: J[] = d.nodeList ?? [];
  const lignes = Math.max(1, ...noeuds.map((n) => Number(n.row) || 0));
  const colonnes = Math.max(1, ...noeuds.map((n) => Number(n.col) || 0));
  const g = el("div", "daev-grille");
  g.style.gridTemplateColumns = `repeat(${colonnes}, var(--case))`;
  g.style.gridTemplateRows = `repeat(${lignes}, var(--case))`;
  for (const n of noeuds) {
    if (n.type === "None") continue;
    const c = el("div", `noeud ${Number(n.open) ? "ouvert" : "ferme"} ${String(n.type).toLowerCase()}`);
    c.style.gridRow = String(n.row);
    c.style.gridColumn = String(n.col);
    c.style.setProperty("--g", grade(n.grade).c);
    c.append(...imagesNoeud(n));
    survol(
      c,
      () => {
        const b = el("div", "ib-bloc");
        for (const e of n.effectList ?? []) b.append(el("span", "ib-vert", e.desc ?? ""));
        b.append(el("span", Number(n.open) ? "ib-lien" : "ib-gris", Number(n.open) ? "Ouvert" : "Fermé"));
        return [teteSimple(undefined, n.name || (n.type === "Start" ? "Départ" : "Nœud"), n.grade && n.grade !== "None" ? n.grade : "", grade(n.grade).c), b];
      },
      grade(n.grade).bord,
    );
    g.append(c);
  }
  grilleZone.replaceChildren(g);
  const comp: J[] = d.openSkillEffectList ?? [];
  const st: J[] = d.openStatEffectList ?? [];
  panneau.replaceChildren(el("h3", "daev-titre", `Effets actifs (${comp.length + st.length})`));
  if (comp.length) {
    panneau.append(el("span", "ib-label", "Améliorations de compétences"));
    const liste = el("div", "pastilles");
    for (const e of comp) {
      const p = el("span", "pastille-effet competence");
      const nomComp = String(e.desc ?? "").replace(/\s*[+-]\d+$/, "");
      const s = (rep?.equipement?.skill?.skillList ?? []).find((x: J) => x.name === nomComp);
      if (s?.icon) p.append(img(s.icon));
      p.append(el("span", undefined, e.desc ?? ""));
      liste.append(p);
    }
    panneau.append(liste);
  }
  if (st.length) {
    panneau.append(el("span", "ib-label", "Bonus de stats"));
    const liste = el("div", "pastilles");
    for (const e of st) liste.append(el("span", "pastille-effet", e.desc ?? ""));
    panneau.append(liste);
  }
}

// ---------- Onglet Stats (6.6) ----------
const BASE = ["STR", "DEX", "AGI", "CON", "INT", "WIS"];

function stats(): HTMLElement[] {
  if (chargement) return [sectionLibre("Stats", [bloc(860, 380, "border-radius:10px")])];
  const liste: J[] = (rep?.info?.stat?.statList ?? []).filter((s: J) => s.type !== "ItemLevel");
  const base = BASE.map((t) => liste.find((s) => s.type === t)).filter(Boolean) as J[];
  const divines = liste.filter((s) => !BASE.includes(s.type));
  const out: HTMLElement[] = [];

  // Stats divines : roue (image) ou liste (texte), comme Shugo.
  const sd = el("section", "section");
  const tete = el("div", "section-tete");
  const bascule = el("div", "bascule");
  for (const [nom, texte] of [["Image", false], ["Texte", true]] as const) {
    const b = el("button", undefined, nom);
    b.type = "button";
    b.setAttribute("aria-pressed", String(roueTexte === texte));
    b.addEventListener("click", () => {
      roueTexte = texte;
      rendre();
    });
    bascule.append(b);
  }
  tete.append(el("h2", undefined, "Stats divines"), bascule);
  sd.append(tete, roueTexte ? listeStats([...base, ...divines]) : roue(base, divines));
  out.push(sd);

  // Liste complète, effets secondaires en infobulle.
  out.push(sectionLibre("Stats", [listeStats(liste)]));

  // Titres équipés et possédés.
  const titre = rep?.info?.title ?? {};
  const st = el("section", "section");
  st.append(el("h2", undefined, `Titres (${titre.ownedCount ?? 0} / ${titre.totalCount ?? "?"} possédés)`));
  const cat: Record<string, string> = { Attack: "Attaque", Defense: "Défense", Etc: "Autre" };
  const g = el("div", "titres-grille");
  for (const t of titre.titleList ?? []) {
    const c = el("div", "titre-carte");
    c.style.setProperty("--bord", grade(t.grade).bord);
    const n = el("span", "titre-nom", t.name ?? "");
    n.style.color = grade(t.grade).c;
    c.append(el("span", "ib-label", cat[t.equipCategory] ?? t.equipCategory ?? ""), n);
    for (const s of t.equipStatList ?? []) c.append(el("span", "ib-vert", s.desc ?? ""));
    for (const s of t.statList ?? []) c.append(el("span", "titre-possede", `Possédé : ${s.desc ?? ""}`));
    g.append(c);
  }
  st.append(g);
  out.push(st);
  return out;
}

function listeStats(liste: J[]): HTMLElement {
  const g = el("div", "stats-grille");
  for (const s of liste) {
    const l = el("div", "stat");
    l.append(el("span", "stat-nom", s.name ?? s.type ?? ""), el("span", "stat-val", String(s.value ?? "")));
    if ((s.statSecondList ?? []).length) {
      survol(l, () => {
        const b = el("div", "ib-bloc");
        for (const x of s.statSecondList) b.append(el("span", "ib-vert", x));
        return [teteSimple(undefined, `${s.name ?? ""} ${s.value ?? ""}`, ""), b];
      });
    }
    g.append(l);
  }
  return g;
}

/** Roue des stats divines (SVG) : 6 stats de base au centre, les stats des dieux autour. */
function roue(base: J[], divines: J[]): HTMLElement {
  const NS = "http://www.w3.org/2000/svg";
  const R = 190;
  const c = 210;
  const s = document.createElementNS(NS, "svg");
  s.setAttribute("viewBox", "0 0 420 420");
  s.setAttribute("class", "roue");
  s.setAttribute("role", "img");
  s.setAttribute("aria-label", "Roue des stats divines");
  const mk = (tag: string, attrs: Record<string, string | number>, texte?: string) => {
    const e = document.createElementNS(NS, tag);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
    if (texte !== undefined) e.textContent = texte;
    s.append(e);
    return e;
  };
  mk("circle", { cx: c, cy: c, r: R, class: "roue-ext" });
  mk("circle", { cx: c, cy: c, r: 92, class: "roue-int" });
  const n = Math.max(1, divines.length);
  const pas = (2 * Math.PI) / n;
  divines.forEach((d, i) => {
    const a = -Math.PI / 2 + pas * i;
    const sep = a + pas / 2;
    mk("line", { x1: c + 92 * Math.cos(sep), y1: c + 92 * Math.sin(sep), x2: c + R * Math.cos(sep), y2: c + R * Math.sin(sep), class: "roue-sep" });
    const lx = c + 141 * Math.cos(a);
    const ly = c + 141 * Math.sin(a);
    const nom = String(d.name ?? "").replace(/\s*\[.*\]$/, "");
    const dieu = (String(d.name ?? "").match(/\[(.*)\]/) ?? [])[1] ?? "";
    mk("text", { x: lx, y: ly - 8, class: "roue-nom" }, nom);
    mk("text", { x: lx, y: ly + 9, class: "roue-val" }, String(d.value ?? ""));
    if (dieu) mk("text", { x: lx, y: ly + 23, class: "roue-dieu" }, dieu);
  });
  base.forEach((b, i) => {
    const a = -Math.PI / 2 + (Math.PI / 3) * i;
    const x = c + 56 * Math.cos(a);
    const y = c + 56 * Math.sin(a);
    mk("text", { x, y: y - 3, class: "roue-base-nom" }, String(b.name ?? ""));
    mk("text", { x, y: y + 12, class: "roue-base-val" }, String(b.value ?? ""));
  });
  const div = el("div", "roue-zone");
  div.append(s);
  return div;
}


// ---------- Copier en image (6.8) ----------
function toast(texte: string) {
  const d = el("div", "toast", texte);
  document.body.append(d);
  window.setTimeout(() => d.remove(), 2200);
}

async function copierImage(enregistrer: boolean) {
  fermerInfobulle();
  try {
    // Sans la marge de centrage de la fiche (sinon l'image est décalée et coupée à droite).
    const blob = await toBlob(racine, {
      backgroundColor: "#0a0e16",
      pixelRatio: 1,
      width: racine.offsetWidth,
      height: racine.scrollHeight,
      style: { margin: "0" },
      filter: (n) => !(n instanceof HTMLElement && n.classList.contains("boutons")),
    });
    if (!blob) throw new Error("capture vide");
    if (enregistrer) {
      const octets = Array.from(new Uint8Array(await blob.arrayBuffer()));
      const chemin = await appel<string>("enregistrer_png", { nom: rep?.info?.profile?.characterName ?? "fiche", octets });
      toast(`Enregistré : ${chemin}`);
    } else {
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      toast("Copié ✓");
    }
  } catch (e) {
    void appel("journal_front", { fenetre: "fiche", message: `copier en image : ${e}` });
    toast("Copie impossible");
  }
}

// Fermeture : Échap, clic ailleurs, défilement de la fiche.
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") fermerInfobulle();
});
document.addEventListener("click", (e) => {
  if (!bulle.hidden && !bulle.contains(e.target as Node)) fermerInfobulle();
});
window.addEventListener("resize", placer);

// « il y a … » mis à jour en continu.
setInterval(() => {
  if (!chargement && rep && document.visibilityState === "visible") {
    const ouverte = !bulle.hidden;
    if (!ouverte) rendre();
  }
}, 15_000);

async function main() {
  await ecoute<number>("fiche-demandee", (id) => void ouvrir({ id }));
  const initiale = await appel<{ nom: string; serveur: number | null; infobulle: number | null; onglet: number | null; enregistrer: boolean } | null>("fiche_initiale");
  if (initiale) {
    if (initiale.onglet != null) onglet = Math.max(0, Math.min(3, initiale.onglet));
    await ouvrir({ nom: initiale.nom, serveur: initiale.serveur, recherche: true });
    // Mode test : enregistrement en PNG (6.8), après le chargement des images.
    if (initiale.enregistrer) window.setTimeout(() => void copierImage(true), 4000);
    // Mode test : une infobulle ouverte d'office (captures de la porte 6).
    const n = initiale.infobulle;
    if (n != null) {
      if (onglet === 0) document.querySelector<HTMLElement>(`[data-emplacement="${n}"]`)?.click();
      else {
        // Compétences : n-ième tuile ; Daevanion : n-ième nœud ouvert, une fois le plateau chargé.
        const viser = () => {
          const cible = document.querySelectorAll<HTMLElement & { ouvrirBulle?: () => void }>(onglet === 1 ? ".tuile" : ".noeud.ouvert")[n];
          if (cible?.ouvrirBulle) cible.ouvrirBulle();
          else window.setTimeout(viser, 300);
        };
        // Après le redimensionnement de la fenêtre par le script de capture.
        window.setTimeout(viser, 3000);
      }
    }
  } else rendre();
}

void main();
