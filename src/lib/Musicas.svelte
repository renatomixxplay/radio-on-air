<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, MIME, type Arrasto, type Faixa } from "./api";
  import Icone from "./Icone.svelte";

  let { pasta, total, carregando }: { pasta: string | null; total: number; carregando: boolean } = $props();

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

  async function escolherPasta() {
    const p = await open({ directory: true, multiple: false, title: "Pasta de músicas" });
    if (typeof p === "string") await api.definirPasta("musicas", p);
  }

  function arrastar(ev: DragEvent, f: Faixa) {
    const dado: Arrasto = { de: "acervo", caminho: f.caminho, tipo: "musica" };
    ev.dataTransfer?.setData(MIME, JSON.stringify(dado));
    if (ev.dataTransfer) ev.dataTransfer.effectAllowed = "copy";
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
      <Icone nome="search" tamanho={14} />
      <input bind:value={termo} placeholder="Música ou artista" spellcheck="false" />
    </div>
    <div class="lista">
      {#each resultado as f (f.caminho)}
        <div
          class="linha faixa"
          draggable="true"
          ondragstart={(e) => arrastar(e, f)}
          ondblclick={() => api.filaAdicionar(f.caminho, "musica")}
          role="listitem"
          title="Arraste para a fila ou clique duas vezes"
        >
          <div class="textos">
            <span class="reticencias musica">{f.musica}</span>
            <span class="reticencias suave">{f.artista}</span>
          </div>
          <button class="mais" aria-label="Pôr na fila" onclick={() => api.filaAdicionar(f.caminho, "musica")}>
            <Icone nome="plus" tamanho={14} />
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
  .total {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 11px;
  }
  .busca {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 10px 6px;
    padding-left: 8px;
    background: var(--campo);
    border: 0.5px solid var(--borda);
    border-radius: 6px;
    color: var(--texto-3);
  }
  .busca:focus-within {
    border-color: var(--azul);
  }
  .busca input {
    flex: 1;
    border: none;
    background: transparent;
    padding-left: 0;
  }
  .faixa {
    cursor: grab;
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
    font-size: 11px;
  }
  .mais {
    border: none;
    background: transparent;
    color: var(--texto-3);
    padding: 4px;
    display: flex;
    visibility: hidden;
  }
  .faixa:hover .mais {
    visibility: visible;
  }
  .acao {
    margin-top: 10px;
    padding: 6px 12px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
