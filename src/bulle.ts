// Bulle — squelette de la section 3 : affiche l'état et la liste brute.
// Le design (replié / déroulé, survol, opacité) est l'objet de la section 5.
import { etat, KIND_LABEL, liste, onEtat, onListe, type Row, type Status } from "./lib/api";

const elEtat = document.getElementById("etat")!;
const elListe = document.getElementById("liste")!;

function texteEtat(s: Status): string {
  switch (s.capture.etat) {
    case "npcap_manquant":
      return "Npcap manquant";
    case "attente_jeu":
      return "En attente du jeu";
    case "attente_trafic":
      return "Jeu détecté, en attente de trafic";
    case "active":
      return `Capture active · ${s.joueurs_en_memoire} joueurs autour`;
  }
}

function ligne(r: Row): HTMLLIElement {
  const li = document.createElement("li");
  // textContent uniquement : aucun pseudo n'est injecté comme HTML.
  li.textContent = `${KIND_LABEL[r.last_kind]} · ${r.name} · ${r.class ?? "?"} · GS ${r.gs} · CP ${r.cp.toLocaleString("fr-FR")}${r.count >= 2 ? ` ×${r.count}` : ""}`;
  return li;
}

async function rafraichir() {
  const rows = await liste();
  elListe.replaceChildren(...rows.slice(0, 20).map(ligne));
}

async function main() {
  elEtat.textContent = texteEtat(await etat());
  await onEtat((s) => (elEtat.textContent = texteEtat(s)));
  await onListe(rafraichir);
  await rafraichir();
}

main().catch((e) => (elEtat.textContent = String(e)));
