// Contrat entre le cœur Rust et l'interface : commandes et événements.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { DEMO, demoRows, demoStatus } from "./demo";

export type Kind = "mort" | "kill" | "combat" | "cible" | "groupe" | "buff";

export const KIND_LABEL: Record<Kind, string> = {
  mort: "Mort",
  kill: "Kill",
  combat: "Combat",
  cible: "Ciblé",
  groupe: "Groupe",
  buff: "Buff",
};

/** Une ligne de la liste (table `players`). */
export interface Row {
  id: number;
  name: string;
  server: number;
  pc_id: number;
  class: string | null;
  level: number;
  gs: number;
  cp: number;
  guild: string;
  abyss_rank: number;
  character_id: string | null;
  last_seen_ms: number;
  last_kind: Kind;
  pinned: boolean;
  count: number;
}

export type CaptureStatus =
  | { etat: "npcap_manquant"; message: string }
  | { etat: "attente_jeu" }
  | { etat: "attente_trafic" }
  | { etat: "active"; serveur: string };

export interface Status {
  capture: CaptureStatus;
  joueurs_en_memoire: number;
  moi: { key: number | null; confirmed: boolean; name: string; server: number };
  /** Référence des flèches ▲▼ : source « paquet », « api » ou « aucune » (pas de flèche). */
  reference: { source: "paquet" | "api" | "aucune"; cp: number; gs: number };
}

/** Appel d'une commande Rust (en mode démo : réponses fictives, sans effet). */
export async function appel<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (DEMO) {
    if (cmd === "liste") return demoRows as T;
    if (cmd === "etat") return demoStatus as T;
    if (cmd === "reglages_bulle") return { opacite_repos: 100, opacite_survol: 100, verrouillee: false, tags: true, compteur: true } as T;
    return undefined as T;
  }
  return invoke<T>(cmd, args);
}

/** Écoute d'un événement Rust (rien en mode démo). */
export async function ecoute<T>(event: string, f: (payload: T) => void): Promise<UnlistenFn> {
  if (DEMO) return () => {};
  return listen<T>(event, (e) => f(e.payload));
}

export const liste = () => appel<Row[]>("liste");
export const etat = () => appel<Status>("etat");

/** La liste a changé (nouvelle rencontre, purge…). */
export const onListe = (f: () => void) => ecoute<null>("liste-modifiee", () => f());
export const onEtat = (f: (s: Status) => void) => ecoute<Status>("etat", f);
