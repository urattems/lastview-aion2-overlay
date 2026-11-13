// Recherche par pseudo (section 7), commune à la bulle et à la fiche.
// Suggestions à partir de 2 caractères : d'abord la liste et la mémoire
// silencieuse (instantané), puis l'API NCSoft, région EU, après 300 ms sans
// frappe. Navigation au clavier : ↑ ↓ Entrée Échap. Les noms sont toujours
// insérés en texte (textContent) : aucune balise venue de l'API n'est injectée.
import { appel } from "./api";

export interface Suggestion {
  nom: string;
  serveur_id: number;
  serveur: string;
  classe: string | null;
  niveau: number;
  source: "liste" | "memoire" | "api";
}

const SOURCE: Record<Suggestion["source"], string> = { liste: "Liste", memoire: "Autour", api: "NCSoft" };

export interface Recherche {
  vider(): void;
}

/**
 * Branche la recherche sur un champ. `liste` reçoit les suggestions ;
 * `choisir` est appelé à l'Entrée ou au clic ; `apres` après chaque rendu
 * (la bulle y ajuste la taille de sa fenêtre).
 */
export function brancherRecherche(input: HTMLInputElement, liste: HTMLElement, choisir: (s: Suggestion) => void, apres: () => void = () => {}, echap: () => void = () => {}): Recherche {
  let suggestions: Suggestion[] = [];
  let active = -1;
  let enCours = false;
  let minuteur = 0;
  let generation = 0;

  const cle = (s: Suggestion) => `${s.nom.toLowerCase()}|${s.serveur_id}`;

  function rendre() {
    liste.replaceChildren();
    const texte = input.value.trim();
    if (texte.length < 2) {
      liste.hidden = true;
      apres();
      return;
    }
    liste.hidden = false;
    suggestions.forEach((s, i) => {
      const b = document.createElement("button");
      b.type = "button";
      b.className = `suggestion${i === active ? " active" : ""}`;
      const nom = document.createElement("span");
      nom.className = "s-nom";
      nom.textContent = s.nom;
      const sous = document.createElement("span");
      sous.className = "s-sous";
      sous.textContent = [s.classe, s.niveau ? `niv. ${s.niveau}` : "", s.serveur].filter(Boolean).join(" · ");
      const src = document.createElement("span");
      src.className = `s-source ${s.source}`;
      src.textContent = SOURCE[s.source];
      b.append(nom, src, sous);
      // mousedown : on garde le focus du champ jusqu'au choix.
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => choisir(s));
      liste.append(b);
    });
    if (enCours) {
      const d = document.createElement("div");
      d.className = "s-info";
      d.textContent = "Recherche NCSoft…";
      liste.append(d);
    } else if (!suggestions.length) {
      const d = document.createElement("div");
      d.className = "s-info";
      d.textContent = "Aucun joueur trouvé";
      liste.append(d);
    }
    apres();
  }

  async function chercher() {
    const texte = input.value.trim();
    const g = ++generation;
    clearTimeout(minuteur);
    if (texte.length < 2) {
      suggestions = [];
      enCours = false;
      rendre();
      return;
    }
    // 1. Local, instantané.
    try {
      const locales = await appel<Suggestion[]>("recherche_locale", { texte });
      if (g !== generation) return;
      suggestions = locales;
      active = suggestions.length ? 0 : -1;
    } catch {
      suggestions = [];
    }
    enCours = true;
    rendre();
    // 2. API, après 300 ms sans frappe.
    minuteur = window.setTimeout(async () => {
      try {
        const api = await appel<Suggestion[]>("recherche_api", { texte });
        if (g !== generation) return;
        const vus = new Set(suggestions.map(cle));
        suggestions = [...suggestions, ...api.filter((s) => !vus.has(cle(s)))].slice(0, 12);
        if (active < 0 && suggestions.length) active = 0;
      } catch {
        /* API indisponible : on garde les suggestions locales */
      }
      if (g !== generation) return;
      enCours = false;
      rendre();
    }, 300);
  }

  input.addEventListener("input", () => void chercher());
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!suggestions.length) return;
      active = (active + (e.key === "ArrowDown" ? 1 : -1) + suggestions.length) % suggestions.length;
      rendre();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const s = suggestions[active] ?? suggestions[0];
      if (s) choisir(s);
    } else if (e.key === "Escape") {
      e.preventDefault();
      vider();
      echap();
    }
  });

  function vider() {
    generation++;
    clearTimeout(minuteur);
    input.value = "";
    suggestions = [];
    active = -1;
    enCours = false;
    rendre();
  }

  return { vider };
}
