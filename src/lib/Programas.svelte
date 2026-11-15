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
  let escolhida = $state("");
  let arquivos = $state<Pasta[]>([]);

  // Lê as pastas dos programas; mantém a escolhida se ela ainda existir.
  $effect(() => {
    pasta;
    api.programas().then((p) => {
      pastas = p;
      if (!p.some((x) => x.caminho === escolhida)) escolhida = p[0]?.caminho ?? "";
    });
  });

  // Arquivos do programa escolhido no seletor.
  $effect(() => {
    const c = escolhida;
    if (!c) {
      arquivos = [];
      return;
    }
    api.arquivosDaPasta(c).then((a) => {
      if (escolhida === c) arquivos = a;
    });
  });

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
    <Icone nome="microphone" tamanho={14} />Programas
    {#if pasta}
      <button class="trocar" onclick={escolherPasta} title={`Pasta: ${pasta}\nClique para trocar`} aria-label="Trocar pasta de programas">
        <Icone nome="folderOpen" tamanho={14} />
      </button>
    {/if}
  </div>

  {#if !pasta}
    <div class="vazio">
      Escolha a pasta dos programas<br />(uma subpasta por locutor).<br />
      <button class="acao" onclick={escolherPasta}><Icone nome="folderOpen" tamanho={14} /> Escolher pasta</button>
    </div>
  {:else if pastas.length === 0}
    <div class="vazio">Nenhum programa ainda.<br />Crie uma subpasta para cada locutor.</div>
  {:else}
    <div class="seletor">
      <select bind:value={escolhida} aria-label="Programa">
        {#each pastas as p (p.caminho)}
          <option value={p.caminho}>{p.nome}</option>
        {/each}
      </select>
      <span class="seta"><Icone nome="abrir" tamanho={18} /></span>
    </div>
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
  {/if}
  {#if pasta && previa}<PreEscuta {previa} decorrido={previaDecorrido} />{/if}
</section>

<style>
  .seletor {
    position: relative;
    margin: 0 10px 8px;
  }
  .seta {
    position: absolute;
    right: 10px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    color: var(--texto-2);
    pointer-events: none;
  }
  select {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 40px;
    padding: 0 36px 0 12px;
    font: inherit;
    font-size: 15px;
    color: var(--texto);
    background: var(--campo);
    border: 0.5px solid var(--borda-forte);
    border-radius: 8px;
    outline: none;
    cursor: pointer;
  }
  select:hover {
    border-color: #4a5060;
  }
  select:focus {
    border-color: var(--azul);
  }
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
  .item:hover {
    background: var(--painel-2);
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
