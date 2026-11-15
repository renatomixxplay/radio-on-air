import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Tipo = "musica" | "programa" | "vinheta";

export interface Item {
  id: number;
  caminho: string;
  artista: string;
  titulo: string;
  tipo: Tipo;
  duracao: number | null;
  travado: boolean;
}

export interface Retrato {
  no_ar: boolean;
  pausado: boolean;
  tocando: { item: Item; decorrido: number } | null;
  fila: Item[];
  total_musicas: number;
  carregando: boolean;
  pasta_musicas: string | null;
  pasta_programas: string | null;
  cartucheira: ({ nome: string; caminho: string } | null)[];
  motor_ok: boolean;
  erro_motor: string | null;
  repetir: boolean;
  ducking: boolean;
  previa: Previa | null;
}

export interface Previa {
  caminho: string;
  titulo: string;
  artista: string;
  duracao: number | null;
  decorrido: number;
  iniciada: boolean;
}

export interface Faixa {
  caminho: string;
  artista: string;
  musica: string;
}

export interface Pasta {
  nome: string;
  caminho: string;
}

export const api = {
  estado: () => invoke<Retrato>("estado"),
  buscar: (termo: string) => invoke<Faixa[]>("buscar", { termo }),
  programas: () => invoke<Pasta[]>("programas"),
  arquivosDaPasta: (caminho: string) => invoke<Pasta[]>("arquivos_da_pasta", { caminho }),
  filaAdicionar: (caminho: string, tipo: Tipo, posicao?: number) =>
    invoke<void>("fila_adicionar", { caminho, tipo, posicao: posicao ?? null }),
  filaRemover: (id: number) => invoke<void>("fila_remover", { id }),
  filaMover: (id: number, posicao: number) => invoke<void>("fila_mover", { id, posicao }),
  tocar: () => invoke<void>("tocar"),
  pausar: () => invoke<void>("pausar"),
  parar: () => invoke<void>("parar"),
  proxima: () => invoke<void>("proxima"),
  cartucheiraTocar: (posicao: number) => invoke<void>("cartucheira_tocar", { posicao }),
  cartucheiraDefinir: (posicao: number, caminho: string | null) =>
    invoke<void>("cartucheira_definir", { posicao, caminho }),
  definirPasta: (qual: "musicas" | "programas", caminho: string) =>
    invoke<void>("definir_pasta", { qual, caminho }),
  atualizarMusicas: () => invoke<void>("atualizar_musicas"),
  alternarRepetir: () => invoke<void>("alternar_repetir"),
  alternarDucking: () => invoke<void>("alternar_ducking"),
  previaTocar: (caminho: string) => invoke<void>("previa_tocar", { caminho }),
  previaParar: () => invoke<void>("previa_parar"),
  previaIr: (segundos: number) => invoke<void>("previa_ir", { segundos }),
  aoMudar: (fn: (r: Retrato) => void) => listen<Retrato>("estado", (e) => fn(e.payload)),
};

/** Formata segundos como m:ss (ou h:mm:ss). */
export function tempo(seg: number | null | undefined): string {
  if (seg == null || !isFinite(seg)) return "--:--";
  const s = Math.max(0, Math.round(seg));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${r}` : `${m}:${r}`;
}

/** Arrastar e soltar: o que está sendo arrastado. */
export type Arrasto =
  | { de: "acervo"; caminho: string; tipo: Tipo }
  | { de: "fila"; id: number };

export const MIME = "application/x-radio-on-air";
