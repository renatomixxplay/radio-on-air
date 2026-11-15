<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Faixa, type Previa } from "./api";
  import Icone from "./Icone.svelte";

  let {
    pasta,
    total,
    carregando,
    previa,
    previaDecorrido,
  }: {
    pasta: string | null;
    total: number;
    carregando: boolean;
    previa: Previa | null;
    previaDecorrido: number;
  } = $props();

  let termo = $state("");
  let resultado = $state<Faixa[]>([]);
  let espera: ReturnType<typeof setTimeout> | undefined;

  async function pesquisar() {
    resultado = await api.buscar(termo);
  }

  // Refaz a busca quando o texto muda (com pequena espera) ou o acervo muda.
  $effect(() => {
    termo;
    total;
    clearTimeout(espera);
    espera = setTimeout(pesquisar, 120);
  });

  /** Durante a pré-escuta, clicar na linha pula para aquele ponto da música. */
  function pular(ev: MouseEvent, caminho: string) {
    if (previa?.caminho !== caminho || !previa.duracao) return;
    const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
    api.previaIr(((ev.clientX - r.left) / r.width) * previa.duracao);
  }

  async function escolherPasta() {
    const p = await open({ directory: true, multiple: false, title: "Pasta de músicas" });
    if (typeof p === "string") await api.definirPasta("musicas", p);
  }

</script>

<section class="painel">
  <div class="rotulo">
    <Icone nome="music" tamanho={14} />Músicas
    <span class="total">{carregando ? "lendo pastas…" : total.toLocaleString("pt-BR")}</span>
  </div>

  {#if !pasta}
    <div class="vazio">
      Escolha a pasta onde ficam as músicas.<br />
      <button class="acao" onclick={escolherPasta}><Icone nome="folderOpen" tamanho={14} /> Escolher pasta</button>
    </div>
  {:else}
    <div class="busca">
      <Icone nome="search" tamanho={16} />
      <input bind:value={termo} placeholder="Música ou artista" spellcheck="false" />
    </div>
    <div class="lista">
      {#each resultado as f (f.caminho)}
        <!-- Clicar para pular só faz sentido com mouse; os botões da linha já são acessíveis pelo teclado. -->
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
        <div
          class="linha faixa"
          class:ouvindo-linha={previa?.caminho === f.caminho}
          onclick={(e) => pular(e, f.caminho)}
          role="listitem"
        >
          <div class="textos">
            <span class="reticencias musica">{f.musica}</span>
            <span class="reticencias suave">{f.artista}</span>
          </div>
          <button
            class="acao-linha ouvir"
            class:ouvindo={previa?.caminho === f.caminho}
            aria-label={previa?.caminho === f.caminho ? "Parar pré-escuta" : "Pré-escuta"}
            title={previa?.caminho === f.caminho ? "Parar pré-escuta" : "Pré-escuta"}
            onclick={(e) => {
              e.stopPropagation();
              previa?.caminho === f.caminho ? api.previaParar() : api.previaTocar(f.caminho);
            }}
          >
            <Icone nome={previa?.caminho === f.caminho ? "stop" : "fone"} tamanho={18} />
          </button>
          {#if previa?.caminho === f.caminho}
            <span class="mini" style:width="{previa.duracao ? Math.min(100, (previaDecorrido / previa.duracao) * 100) : 0}%"></span>
          {/if}
          <button class="acao-linha adicionar" aria-label="Pôr na fila" title="Pôr na fila" onclick={(e) => {
              e.stopPropagation();
              api.filaAdicionar(f.caminho, "musica");
            }}>
            <Icone nome="plus" tamanho={18} />
          </button>
        </div>
      {:else}
        <div class="vazio">{carregando ? "Lendo as pastas…" : "Nenhuma música encontrada"}</div>
      {/each}
      {#if resultado.length === 200}
        <div class="vazio">Mostrando as primeiras 200. Refine a busca.</div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .ouvindo-linha {
    cursor: pointer;
    background: #172033;
  }
  /* Desliza entre as atualizações do tempo (a cada 0,5 s). */
  .mini {
    transition: width 0.5s linear;
    position: absolute;
    left: 0;
    bottom: 0;
    height: 4px;
    pointer-events: none;
    background: var(--azul);
  }
  .total {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
    font-size: 13px;
  }
  .busca {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    margin: 0 10px 8px;
    padding-left: 12px;
    background: var(--campo);
    border: 0.5px solid var(--borda-forte);
    border-radius: 8px;
    color: var(--texto-3);
  }
  .busca:hover {
    border-color: #4a5060;
  }
  .busca:focus-within {
    border-color: var(--azul);
  }
  .busca input {
    flex: 1;
    height: 100%;
    font-size: 14px;
    border: none;
    background: transparent;
    padding-left: 0;
  }
  .faixa {
    position: relative;
  }
  .faixa:hover {
    background: var(--painel-2);
  }
  .textos {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .textos .suave {
    font-size: 13px;
  }
  .acao-linha {
    width: 34px;
    height: 34px;
    flex: none;
    padding: 0;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--texto-2);
    visibility: hidden;
  }
  .faixa:hover .acao-linha,
  .acao-linha.ouvindo {
    visibility: visible;
  }
  .acao-linha.ouvindo {
    color: var(--azul-claro);
    border-color: #4a6aa5;
    background: #1e2533;
  }
  .acao-linha.ouvir:hover {
    color: #fff;
    background: var(--azul);
    border-color: var(--azul);
  }
  .acao-linha.adicionar:hover {
    color: #fff;
    background: #238636;
    border-color: #2ea043;
  }
  .acao {
    margin-top: 10px;
    padding: 6px 12px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
