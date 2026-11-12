// Contrat entre le cœur Rust et l'interface : commandes et événements.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
}

export const liste = () => invoke<Row[]>("liste");
export const etat = () => invoke<Status>("etat");

/** La liste a changé (nouvelle rencontre, purge…). */
export const onListe = (f: () => void): Promise<UnlistenFn> => listen("liste-modifiee", () => f());
export const onEtat = (f: (s: Status) => void): Promise<UnlistenFn> => listen<Status>("etat", (e) => f(e.payload));
