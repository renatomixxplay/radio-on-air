import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Tipo = "musica" | "programa" | "vinheta" | "comercial" | "hora_certa" | "link";

/** Arquivo de link de rádio online (.m3u/.pls). */
export const ehLink = (caminho: string) => /\.(m3u|pls)$/i.test(caminho);

export interface Item {
  id: number;
  caminho: string;
  artista: string;
  titulo: string;
  tipo: Tipo;
  duracao: number | null;
  travado: boolean;
}

/** Cartucheira com data de início e fim (incluindo os dois dias). */
export type CartucheiraAgendada = {
  id: number;
  nome: string;
  inicio: string;
  fim: string;
  botoes: ({ nome: string; caminho: string } | null)[];
  /** Vale hoje: está no ar no lugar da padrão. */
  hoje: boolean;
  /** O período já acabou. */
  passou: boolean;
};

export interface Retrato {
  no_ar: boolean;
  pausado: boolean;
  tocando: { item: Item; decorrido: number } | null;
  fila: Item[];
  total_musicas: number;
  carregando: boolean;
  /** Ao abrir: procurando músicas novas na pasta (a rádio espera, até 15 s). */
  procurando_musicas: boolean;
  pasta_musicas: string | null;
  pastas: Pasta[];
  tocar_ao_abrir: boolean;
  abrir_tela_cheia: boolean;
  vinheta_entre_musicas: boolean;
  protegido: boolean;
  bloqueios: Bloqueios;
  /** Os 8 botões no ar hoje (da agendada do período ou da padrão). */
  cartucheira: ({ nome: string; caminho: string } | null)[];
  cartucheira_padrao: ({ nome: string; caminho: string } | null)[];
  cartucheiras_agendadas: CartucheiraAgendada[];
  motor_ok: boolean;
  iniciando_motor: boolean;
  erro_motor: string | null;
  repetir: boolean;
  ducking: boolean;
  previa: Previa | null;
  lista_ativa: string | null;
  por_cima: string | null;
  aguardando: string | null;
  repetir_auto: boolean;
  /** A fala do ducking acabou e a troca para a próxima está marcada. */
  pos_ducking: boolean;
  nome_radio: string;
  audio: Audio;
  gravacao: Gravacao;
  transmissao: Transmissao;
  /** "desligada", "conectando", "conectado", "caiu" ou "erro: …". */
  transmissao_estado: string;
  erro_captura: string | null;
  /** A gravação está ligada e funcionando. */
  gravando: boolean;
  /** A entrada da placa está chegando só com chiado. */
  chiado: boolean;
  proximo_agendamento: { horario: string; tipo: TipoAgenda; nome: string } | null;
  /** Endereço do link ao vivo no ar. */
  link: string | null;
  /** O link no ar veio da fila: o Próximo pode tirar. */
  link_da_fila: boolean;
}

/** Placas escolhidas (null = padrão do sistema). */
export interface Audio {
  saida: string | null;
  pre_escuta: string | null;
  entrada: string | null;
  /** Canal da entrada usado na gravação e na transmissão. */
  canal: "estereo" | "esquerdo" | "direito";
}

export interface Gravacao {
  ligada: boolean;
  pasta: string | null;
  bitrate: number;
  minutos: number;
}

export interface Transmissao {
  ligada: boolean;
  servidor: string;
  porta: number;
  senha: string;
  ponto: string;
  bitrate: number;
  samplerate: number;
  estereo: boolean;
}

export interface Placa {
  id: string;
  nome: string;
}

export interface Placas {
  saidas: Placa[];
  entradas: Placa[];
}

export type TipoAgenda = "lista" | "comercial" | "ducking" | "hora_certa" | "link";

export interface PastaAnexada {
  caminho: string;
  modo: "todos" | "um";
}

export interface Agendamento {
  id: number;
  /** "HH:MM", em ordem. A hora certa não usa. */
  horarios: string[];
  /** Segunda a domingo. */
  dias: boolean[];
  tipo: TipoAgenda;
  /** Na hora certa, a pasta dos arquivos de hora. */
  pastas: PastaAnexada[];
  /** Hora certa: pasta da abertura (um arquivo antes da hora). */
  abertura?: string | null;
  /** Hora certa: pasta do comercial (um arquivo depois da hora). */
  comercial_hora?: string | null;
  /** Hora certa: toca a cada tantos minutos (o horário não é usado). */
  intervalo?: number | null;
  /** Lista: espera a música atual acabar ou faz fade e já entra. */
  modo_lista?: "depois_desta" | "fade";
  /** Desativado: fica na lista mas não toca. */
  ativo: boolean;
  /** Não toca nos feriados de data fixa nem nas outras datas. */
  pula_feriado?: boolean;
  /** Datas a mais em que não toca ("AAAA-MM-DD"). */
  outras_datas?: string[];
  /** Link: endereço do áudio ao vivo (MP3/AAC). */
  url?: string | null;
  /** Link: ajuste de volume em dB (0 = como chega; -6 = abaixa). */
  volume_db?: number;
}

export interface Previa {
  caminho: string;
  titulo: string;
  artista: string;
  duracao: number | null;
  decorrido: number;
  iniciada: boolean;
  /** Segundos até o som chegar à saída (a barra só anda depois disso). */
  espera: number;
}

export interface Faixa {
  caminho: string;
  artista: string;
  musica: string;
}

/** Cantor da pesquisa por cantor (grafias que só mudam acento/maiúscula juntas). */
export interface Cantor {
  nome: string;
  /** Sem acento e minúsculo: identifica o cantor e serve para filtrar. */
  chave: string;
  qtd: number;
}

export interface Bloqueios {
  agendamentos: boolean;
  pastas: boolean;
  cartucheira: boolean;
  configuracoes: boolean;
}

export interface Pasta {
  nome: string;
  caminho: string;
}

export const api = {
  estado: () => invoke<Retrato>("estado"),
  buscar: (termo: string) => invoke<Faixa[]>("buscar", { termo }),
  cantores: () => invoke<Cantor[]>("cantores"),
  musicasDoCantor: (chave: string) => invoke<Faixa[]>("musicas_do_cantor", { chave }),
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
  /** `agendada` = id da cartucheira agendada; null = a padrão. */
  cartucheiraDefinir: (posicao: number, caminho: string | null, agendada: number | null = null) =>
    invoke<void>("cartucheira_definir", { posicao, caminho, agendada }),
  /** id 0 = nova. Devolve o id. Datas "AAAA-MM-DD". */
  cartucheiraAgendadaSalvar: (id: number, nome: string, inicio: string, fim: string) =>
    invoke<number>("cartucheira_agendada_salvar", { id, nome, inicio, fim }),
  cartucheiraAgendadaApagar: (id: number) => invoke<void>("cartucheira_agendada_apagar", { id }),
  definirPasta: (qual: "musicas" | "programas", caminho: string) =>
    invoke<void>("definir_pasta", { qual, caminho }),
  atualizarMusicas: () => invoke<void>("atualizar_musicas"),
  pastaAdicionar: (caminho: string) => invoke<void>("pasta_adicionar", { caminho }),
  pastaRemover: (caminho: string) => invoke<void>("pasta_remover", { caminho }),
  definirTocarAoAbrir: (valor: boolean) => invoke<void>("definir_tocar_ao_abrir", { valor }),
  definirAbrirTelaCheia: (valor: boolean) => invoke<void>("definir_abrir_tela_cheia", { valor }),
  alternarTelaCheia: () => invoke<boolean>("alternar_tela_cheia"),
  /** Fecha e abre o programa de novo. */
  reiniciar: () => invoke<void>("reiniciar_programa"),
  /** Fecha o programa. */
  fechar: () => invoke<void>("fechar_programa"),
  telaCheiaAgora: () => invoke<boolean>("tela_cheia_agora"),
  definirVinhetaEntreMusicas: (valor: boolean) => invoke<void>("definir_vinheta_entre_musicas", { valor }),
  nomeRadioDefinir: (nome: string) => invoke<void>("nome_radio_definir", { nome }),
  placas: () => invoke<Placas>("placas"),
  audioDefinir: (audio: Audio) => invoke<void>("audio_definir", { audio }),
  gravacaoDefinir: (gravacao: Gravacao) => invoke<void>("gravacao_definir", { gravacao }),
  transmissaoDefinir: (transmissao: Transmissao) => invoke<void>("transmissao_definir", { transmissao }),
  senhaConferir: (senha: string) => invoke<boolean>("senha_conferir", { senha }),
  senhaDefinir: (atual: string, nova: string) => invoke<void>("senha_definir", { atual, nova }),
  bloqueiosDefinir: (bloqueios: Bloqueios, senha: string) => invoke<void>("bloqueios_definir", { bloqueios, senha }),
  alternarRepetir: () => invoke<void>("alternar_repetir"),
  alternarDucking: () => invoke<void>("alternar_ducking"),
  previaTocar: (caminho: string) => invoke<void>("previa_tocar", { caminho }),
  previaParar: () => invoke<void>("previa_parar"),
  previaIr: (segundos: number) => invoke<void>("previa_ir", { segundos }),
  agendamentos: () => invoke<Agendamento[]>("agendamentos"),
  agendamentoSalvar: (ag: Agendamento) => invoke<Agendamento[]>("agendamento_salvar", { ag }),
  agendamentoRemover: (id: number) => invoke<Agendamento[]>("agendamento_remover", { id }),
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
