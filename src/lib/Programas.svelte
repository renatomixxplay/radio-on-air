<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, MIME, type Arrasto, type Pasta, type Previa } from "./api";
  import Icone from "./Icone.svelte";
  import PreEscuta from "./PreEscuta.svelte";

  let {
    pasta,
    previa,
    previaDecorrido,
  }: { pasta: string | null; previa: Previa | null; previaDecorrido: number } = $props();

  let pastas = $state<Pasta[]>([]);
  let aberta = $state<Pasta | null>(null);
  let arquivos = $state<Pasta[]>([]);

  $effect(() => {
    pasta;
    aberta = null;
    api.programas().then((p) => (pastas = p));
  });

  async function abrir(p: Pasta) {
    aberta = p;
    arquivos = await api.arquivosDaPasta(p.caminho);
  }

  async function escolherPasta() {
    const p = await open({ directory: true, multiple: false, title: "Pasta dos programas" });
    if (typeof p === "string") await api.definirPasta("programas", p);
  }

  function arrastar(ev: DragEvent, a: Pasta) {
    const dado: Arrasto = { de: "acervo", caminho: a.caminho, tipo: "programa" };
    ev.dataTransfer?.setData(MIME, JSON.stringify(dado));
    if (ev.dataTransfer) ev.dataTransfer.effectAllowed = "copy";
  }
</script>

<section class="painel">
  <div class="rotulo">
    {#if aberta}
      <button class="voltar" onclick={() => (aberta = null)} aria-label="Voltar"><Icone nome="voltar" tamanho={14} /></button>
      <span class="reticencias">{aberta.nome}</span>
    {:else}
      <Icone nome="microphone" tamanho={14} />Programas
      {#if pasta}
        <button class="trocar" onclick={escolherPasta} title={`Pasta: ${pasta}\nClique para trocar`} aria-label="Trocar pasta de programas">
          <Icone nome="folderOpen" tamanho={14} />
        </button>
      {/if}
    {/if}
  </div>

  {#if !pasta}
    <div class="vazio">
      Escolha a pasta dos programas<br />(uma subpasta por locutor).<br />
      <button class="acao" onclick={escolherPasta}><Icone nome="folderOpen" tamanho={14} /> Escolher pasta</button>
    </div>
  {:else if aberta}
    <div class="lista">
      {#each arquivos as a (a.caminho)}
        <div
          class="linha item"
          draggable="true"
          ondragstart={(e) => arrastar(e, a)}
          ondblclick={() => api.filaAdicionar(a.caminho, "programa")}
          role="listitem"
          title="Arraste para a fila ou clique duas vezes"
        >
          <span class="reticencias" style="flex:1">{a.nome}</span>
          <button
            class="acao-linha ouvir"
            class:ouvindo={previa?.caminho === a.caminho}
            aria-label="Pré-escuta"
            title="Pré-escuta"
            onclick={() => (previa?.caminho === a.caminho ? api.previaParar() : api.previaTocar(a.caminho))}
          >
            <Icone nome="fone" tamanho={18} />
          </button>
          <button class="acao-linha adicionar" aria-label="Pôr na fila" title="Pôr na fila" onclick={() => api.filaAdicionar(a.caminho, "programa")}>
            <Icone nome="plus" tamanho={18} />
          </button>
        </div>
      {:else}
        <div class="vazio">Pasta sem arquivos de áudio</div>
      {/each}
    </div>
  {:else}
    <div class="lista">
      {#each pastas as p (p.caminho)}
        <button class="linha pasta" onclick={() => abrir(p)}>
          <span class="cor-pasta"><Icone nome="folder" tamanho={16} /></span>
          <span class="reticencias">{p.nome}</span>
        </button>
      {:else}
        <div class="vazio">Nenhum programa ainda</div>
      {/each}
    </div>
  {/if}
  {#if pasta && previa}<PreEscuta {previa} decorrido={previaDecorrido} />{/if}
</section>

<style>
  .trocar {
    margin-left: auto;
    border: none;
    background: transparent;
    padding: 0 2px;
    display: flex;
    color: var(--texto-3);
  }
  .trocar:hover {
    color: var(--texto);
  }
  .voltar {
    border: none;
    background: transparent;
    padding: 0;
    display: flex;
    color: var(--texto-2);
  }
  .pasta {
    width: 100%;
    border: none;
    border-bottom: 0.5px solid var(--linha);
    border-radius: 0;
    background: transparent;
    text-align: left;
  }
  .pasta:hover,
  .item:hover {
    background: var(--painel-2);
  }
  .cor-pasta {
    color: var(--pasta);
    display: flex;
  }
  .item {
    cursor: grab;
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
  .item:hover .acao-linha,
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
