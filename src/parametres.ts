// Paramètres (section 8). Chaque changement est enregistré et appliqué tout
// de suite côté Rust (`parametres_ecrire`) : bulle, liste, moteur, raccourcis.
import "@fontsource/noto-sans/400.css";
import "@fontsource/noto-sans/500.css";
import "@fontsource/noto-sans/600.css";
import "@fontsource/noto-sans/700.css";
import { appel, ecoute, KIND_LABEL, type Kind } from "./lib/api";

interface Parametres {
  retention_h: number;
  max_joueurs: number;
  lignes: number;
  sources: Record<Kind, boolean>;
  tags: boolean;
  compteur: boolean;
  opacite_repos: number;
  opacite_survol: number;
  masquer_hors_jeu: boolean;
  verrouillee: boolean;
  raccourci_bulle: string;
  raccourci_fiche: string;
  raccourci_deplier: string;
  mon_perso: string;
  mon_serveur: number;
  interface: string;
  ecran_fiche: string;
  cache_api_min: number;
  dossier_png: string;
  demarrer_windows: boolean;
  maj_auto: boolean;
  diagnostic: boolean;
}

interface Choix {
  valeur: string;
  libelle: string;
}

interface Vue {
  parametres: Parametres;
  interfaces: Choix[];
  ecrans: Choix[];
  serveurs: Choix[];
  dossier_png_defaut: string;
  demarrage_actif: boolean;
  erreurs_raccourcis: Record<string, string>;
  version: string;
  essai: boolean;
}

type Raccourci = "raccourci_bulle" | "raccourci_fiche" | "raccourci_deplier";

window.addEventListener("error", (e) => void appel("journal_front", { fenetre: "parametres", message: `${e.message} (${e.filename}:${e.lineno})` }));
window.addEventListener("unhandledrejection", (e) => void appel("journal_front", { fenetre: "parametres", message: String(e.reason) }));

const racine = document.getElementById("parametres")!;
const etat = document.getElementById("etat")!;
let vue: Vue;
let p: Parametres;
let erreurs: Record<string, string> = {};

// ---------- Utilitaires ----------
function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

let minuteur = 0;
let messageMinuteur = 0;
function annoncer(texte: string, erreur = false) {
  etat.textContent = texte;
  etat.classList.toggle("erreur", erreur);
  etat.classList.add("visible");
  clearTimeout(messageMinuteur);
  messageMinuteur = window.setTimeout(() => etat.classList.remove("visible"), 1800);
}

/** Enregistre (regroupé sur 200 ms pour les curseurs). */
function enregistrer(tout_de_suite = false) {
  clearTimeout(minuteur);
  const go = async () => {
    try {
      erreurs = await appel<Record<string, string>>("parametres_ecrire", { parametres: p });
      majErreurs();
      annoncer(Object.keys(erreurs).length ? "Enregistré, avec un raccourci refusé" : "Enregistré", Object.keys(erreurs).length > 0);
    } catch (e) {
      annoncer(`Non enregistré : ${String(e)}`, true);
    }
  };
  if (tout_de_suite) void go();
  else minuteur = window.setTimeout(() => void go(), 200);
}

function section(titre: string, ...lignes: HTMLElement[]): HTMLElement {
  const s = el("section", "groupe");
  s.append(el("h2", undefined, titre), ...lignes);
  return s;
}

function ligne(libelle: string, controle: HTMLElement, aide?: string): HTMLElement {
  const l = el("div", "ligne");
  const t = el("div", "texte");
  const lab = el("label", "libelle", libelle);
  if (controle.id) lab.htmlFor = controle.id;
  t.append(lab);
  if (aide) t.append(el("span", "aide", aide));
  l.append(t, controle);
  return l;
}

let compteurId = 0;
function interrupteur<K extends keyof Parametres>(cle: K): HTMLInputElement {
  const c = el("input", "interrupteur");
  c.type = "checkbox";
  c.id = `c${++compteurId}`;
  c.setAttribute("role", "switch");
  c.checked = Boolean(p[cle]);
  c.addEventListener("change", () => {
    (p[cle] as boolean) = c.checked;
    enregistrer(true);
  });
  return c;
}

function choix<K extends keyof Parametres>(cle: K, options: [string | number, string][], nombre = typeof p[cle] === "number"): HTMLSelectElement {
  const s = el("select");
  s.id = `c${++compteurId}`;
  for (const [v, t] of options) {
    const o = el("option", undefined, t);
    o.value = String(v);
    s.append(o);
  }
  s.value = String(p[cle]);
  // Valeur hors liste (réglée à la main dans une version précédente) : ajoutée telle quelle.
  if (s.value !== String(p[cle])) {
    const o = el("option", undefined, String(p[cle]));
    o.value = String(p[cle]);
    s.append(o);
    s.value = String(p[cle]);
  }
  s.addEventListener("change", () => {
    (p[cle] as string | number) = nombre ? Number(s.value) : s.value;
    enregistrer(true);
  });
  return s;
}

function curseur<K extends keyof Parametres>(cle: K, min: number, max: number, pas: number, unite: string): HTMLElement {
  const bloc = el("div", "curseur");
  const r = el("input");
  r.type = "range";
  r.id = `c${++compteurId}`;
  r.min = String(min);
  r.max = String(max);
  r.step = String(pas);
  r.value = String(p[cle]);
  const v = el("output", "valeur", `${p[cle]}${unite}`);
  r.addEventListener("input", () => {
    (p[cle] as number) = Number(r.value);
    v.textContent = `${r.value}${unite}`;
    enregistrer();
  });
  bloc.append(r, v);
  bloc.id = r.id;
  return bloc;
}

function bouton(texte: string, action: () => void, cls = "bouton"): HTMLButtonElement {
  const b = el("button", cls, texte);
  b.type = "button";
  b.addEventListener("click", action);
  return b;
}

// ---------- Raccourcis ----------
const CODES: Record<string, string> = {
  Insert: "Inser",
  Delete: "Suppr",
  Home: "Début",
  End: "Fin",
  PageUp: "PgPrec",
  PageDown: "PgSuiv",
  Pause: "Pause",
  ScrollLock: "ArrêtDéfil",
};

function nomTouche(code: string): string | null {
  let m: RegExpMatchArray | null;
  if ((m = code.match(/^Key([A-Z])$/))) return m[1];
  if ((m = code.match(/^Digit(\d)$/))) return m[1];
  if ((m = code.match(/^Numpad(\d)$/))) return `PavNum${m[1]}`;
  if ((m = code.match(/^F(\d{1,2})$/)) && Number(m[1]) <= 24) return `F${m[1]}`;
  return CODES[code] ?? null;
}

const champsRaccourci = new Map<Raccourci, { bouton: HTMLButtonElement; erreur: HTMLElement }>();
let enSaisie: Raccourci | null = null;

function afficherRaccourci(cle: Raccourci) {
  const c = champsRaccourci.get(cle)!;
  c.bouton.textContent = enSaisie === cle ? "Appuyez sur la combinaison…" : p[cle] || "Aucun";
  c.bouton.classList.toggle("saisie", enSaisie === cle);
  c.bouton.classList.toggle("vide", !p[cle] && enSaisie !== cle);
}

function majErreurs() {
  for (const [cle, c] of champsRaccourci) {
    const e = erreurs[cle];
    c.erreur.textContent = e ? `Refusé : ${e}.` : "";
    c.erreur.hidden = !e;
  }
}

function finSaisie(valeur: string | null) {
  const cle = enSaisie;
  if (!cle) return;
  enSaisie = null;
  window.removeEventListener("keydown", saisie, true);
  if (valeur !== null && valeur !== p[cle]) {
    p[cle] = valeur;
    afficherRaccourci(cle);
    enregistrer(true);
  } else {
    afficherRaccourci(cle);
    // Rien de changé : les raccourcis suspendus reprennent.
    void appel("raccourcis_pause", { actif: false });
  }
}

function saisie(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();
  if (["Control", "Alt", "Shift", "Meta", "AltGraph"].includes(e.key)) return;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Maj", e.metaKey && "Win"].filter(Boolean) as string[];
  if (e.key === "Escape" && !mods.length) return finSaisie(null);
  if ((e.key === "Backspace" || e.key === "Delete") && !mods.length) return finSaisie("");
  const touche = nomTouche(e.code);
  const cle = enSaisie!;
  const erreur = champsRaccourci.get(cle)!.erreur;
  if (!touche) {
    erreur.textContent = "Touche non prise en charge : lettres, chiffres, F1 à F24, pavé numérique, Inser, Suppr, Début, Fin, PgPrec, PgSuiv, Pause.";
    erreur.hidden = false;
    return;
  }
  if (!mods.length && !/^F\d+$/.test(touche)) {
    erreur.textContent = "Ajoutez Ctrl, Alt ou Maj : une touche seule gênerait le jeu (sauf F1 à F24).";
    erreur.hidden = false;
    return;
  }
  erreur.hidden = true;
  finSaisie([...mods, touche].join("+"));
}

function champRaccourci(cle: Raccourci): HTMLElement {
  const bloc = el("div", "raccourci");
  const b = bouton("", () => {
    if (enSaisie) finSaisie(null);
    enSaisie = cle;
    afficherRaccourci(cle);
    // Les raccourcis de LastView sont suspendus, sinon Windows les garde pour lui.
    void appel("raccourcis_pause", { actif: true });
    window.addEventListener("keydown", saisie, true);
  }, "bouton touche");
  b.id = `c${++compteurId}`;
  const effacer = bouton("Effacer", () => {
    if (enSaisie === cle) finSaisie("");
    else if (p[cle]) {
      p[cle] = "";
      afficherRaccourci(cle);
      enregistrer(true);
    }
  }, "lien");
  const erreur = el("span", "erreur-champ");
  erreur.hidden = true;
  const haut = el("div", "rangee");
  haut.append(b, effacer);
  bloc.append(haut, erreur);
  bloc.id = b.id;
  champsRaccourci.set(cle, { bouton: b, erreur });
  afficherRaccourci(cle);
  return bloc;
}

window.addEventListener("blur", () => finSaisie(null));

// ---------- Rendu ----------
function rendre() {
  champsRaccourci.clear();

  // Liste
  const sources = el("div", "sources");
  for (const k of ["cible", "combat", "kill", "mort", "groupe", "buff"] as Kind[]) {
    const b = bouton(KIND_LABEL[k], () => {
      p.sources[k] = !p.sources[k];
      b.classList.toggle("actif", p.sources[k]);
      b.setAttribute("aria-pressed", String(p.sources[k]));
      enregistrer(true);
    }, `source ${k}${p.sources[k] ? " actif" : ""}`);
    b.setAttribute("aria-pressed", String(p.sources[k]));
    sources.append(b);
  }
  sources.id = `c${++compteurId}`;
  let confirmer = 0;
  const vider = bouton("Vider la liste…", () => {
    if (!confirmer) {
      vider.textContent = "Confirmer : vider (les épinglés restent)";
      vider.classList.add("danger");
      confirmer = window.setTimeout(() => {
        confirmer = 0;
        vider.textContent = "Vider la liste…";
        vider.classList.remove("danger");
      }, 5000);
      return;
    }
    clearTimeout(confirmer);
    confirmer = 0;
    vider.textContent = "Vider la liste…";
    vider.classList.remove("danger");
    void appel<number>("vider_liste").then((n) => annoncer(`Liste vidée : ${n} joueur${n > 1 ? "s" : ""}`));
  });

  const heures: [number, string][] = [1, 2, 4, 8, 12, 24, 48].map((h) => [h, `${h} h`]);
  const maxima: [number, string][] = [50, 100, 150, 200, 300, 500].map((n) => [n, String(n)]);

  // Moi : liste des serveurs EU si l'API répond, sinon numéro.
  let serveur: HTMLElement;
  if (vue.serveurs.length) serveur = choix("mon_serveur", vue.serveurs.map((s) => [Number(s.valeur), s.libelle]));
  else {
    const n = el("input", "texte-court");
    n.type = "number";
    n.id = `c${++compteurId}`;
    n.value = String(p.mon_serveur);
    n.addEventListener("change", () => {
      p.mon_serveur = Number(n.value) || p.mon_serveur;
      enregistrer(true);
    });
    serveur = n;
  }
  const perso = el("input", "texte-court");
  perso.type = "text";
  perso.id = `c${++compteurId}`;
  perso.value = p.mon_perso;
  perso.spellcheck = false;
  perso.autocomplete = "off";
  perso.addEventListener("change", () => {
    p.mon_perso = perso.value.trim();
    enregistrer(true);
  });

  // Dossier des PNG
  const dossier = el("div", "dossier");
  const chemin = el("span", "chemin", p.dossier_png || vue.dossier_png_defaut);
  chemin.title = chemin.textContent ?? "";
  const actions = el("div", "rangee");
  actions.append(
    bouton("Choisir…", async () => {
      const d = await appel<string | null>("choisir_dossier_png");
      if (d) {
        p.dossier_png = d;
        chemin.textContent = d;
        chemin.title = d;
        enregistrer(true);
      }
    }),
    bouton("Ouvrir", () => void appel("ouvrir_dossier", { quel: "png" })),
    bouton("Par défaut", () => {
      p.dossier_png = "";
      chemin.textContent = vue.dossier_png_defaut;
      chemin.title = vue.dossier_png_defaut;
      enregistrer(true);
    }, "lien"),
  );
  dossier.append(chemin, actions);
  dossier.id = `c${++compteurId}`;

  const dossiers = el("div", "rangee");
  dossiers.append(bouton("Données", () => void appel("ouvrir_dossier", { quel: "donnees" })), bouton("Journaux", () => void appel("ouvrir_dossier", { quel: "logs" })));
  dossiers.id = `c${++compteurId}`;

  const position = bouton("Réinitialiser", () => {
    void appel("reinitialiser_position").then(() => annoncer("Bulle replacée sous la mini-carte"));
  });
  position.id = `c${++compteurId}`;

  racine.replaceChildren(
    section(
      "Liste",
      ligne("Durée de rétention", choix("retention_h", heures), "Les joueurs plus anciens sortent de la liste. Les épinglés restent."),
      ligne("Nombre maximum de joueurs", choix("max_joueurs", maxima)),
      ligne("Lignes visibles dans la bulle déroulée", curseur("lignes", 3, 20, 1, ""), "Le reste en faisant défiler."),
      ligne("Sources", sources, "Les rencontres qui ajoutent un joueur à la liste."),
      ligne("Afficher les tags de rencontre", interrupteur("tags")),
      ligne("Afficher le compteur de rencontres", interrupteur("compteur")),
      ligne("Vider la liste", vider, "Retire tous les joueurs, sauf les épinglés."),
    ),
    section(
      "Bulle",
      ligne("Opacité au repos", curseur("opacite_repos", 10, 100, 5, " %")),
      ligne("Opacité au survol", curseur("opacite_survol", 10, 100, 5, " %")),
      ligne("Masquer quand le jeu n'est pas au premier plan", interrupteur("masquer_hors_jeu")),
      ligne("Verrouiller la position", interrupteur("verrouillee")),
      ligne("Position", position, "Replace la bulle à droite de l'écran principal, sous la mini-carte."),
    ),
    section(
      "Raccourcis",
      ligne("Masquer / afficher la bulle", champRaccourci("raccourci_bulle")),
      ligne("Ouvrir la fiche du dernier ciblé", champRaccourci("raccourci_fiche")),
      ligne("Déplier / replier la bulle", champRaccourci("raccourci_deplier")),
      el("p", "note", "Cliquez sur un champ, puis appuyez sur la combinaison voulue. Échap annule, Retour arrière efface. Les raccourcis marchent aussi en jeu."),
    ),
    section(
      "Moi",
      ligne("Mon perso", perso, "Utilisé si la détection automatique échoue."),
      ligne("Mon serveur", serveur),
    ),
    section(
      "Capture",
      ligne("Interface réseau", choix("interface", [["", "Automatique (toutes)"], ...vue.interfaces.map((i) => [i.valeur, i.libelle] as [string, string])]), "Écoute passive seulement : rien n'est jamais envoyé au jeu."),
    ),
    section(
      "Fiche",
      ligne("Ouvrir sur l'écran", choix("ecran_fiche", [["", "Dernier utilisé"], ...vue.ecrans.map((e) => [e.valeur, e.libelle] as [string, string])])),
      ligne("Durée du cache API", choix("cache_api_min", [1, 5, 10, 15, 30, 60].map((m) => [m, `${m} min`]))),
      ligne("Dossier des images", dossier, "Bouton « Copier en image », clic droit « Enregistrer en PNG »."),
    ),
    section(
      "Système",
      ligne("Démarrer avec Windows", interrupteur("demarrer_windows"), "Pour que LastView soit prêt avant le jeu."),
      ligne("Mise à jour automatique", interrupteur("maj_auto"), "Vérifiée au démarrage, dès la première version publiée."),
      ligne("Mode diagnostic", interrupteur("diagnostic"), "Journal détaillé, pour comprendre un problème."),
      ligne("Ouvrir le dossier", dossiers),
    ),
    el("p", "version", `LastView ${vue.version}`),
  );
  majErreurs();
}

async function charger() {
  vue = await appel<Vue>("parametres_lire");
  p = vue.parametres;
  erreurs = vue.erreurs_raccourcis;
  rendre();
}

async function main() {
  await charger();
  // Mode test (`--essai`, données isolées) : trois réglages changés comme le ferait Gio.
  if (vue.essai) {
    window.setTimeout(() => {
      p.opacite_repos = 55;
      p.sources.buff = false;
      p.raccourci_bulle = "Ctrl+Alt+Maj+F24";
      p.raccourci_deplier = "Ctrl+Alt+Maj+F24";
      rendre();
      enregistrer(true);
    }, 1500);
  }
  // Modifié ailleurs (case « Mode diagnostic » de la zone de notification) : relu,
  // sauf pendant une saisie.
  await ecoute<null>("parametres-modifies", () => {
    if (enSaisie || document.activeElement?.tagName === "INPUT") return;
    void appel<Vue>("parametres_lire").then((v) => {
      if (JSON.stringify(v.parametres) !== JSON.stringify(p)) {
        vue = v;
        p = v.parametres;
        rendre();
      }
    });
  });
}

void main();
