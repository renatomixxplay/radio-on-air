<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Pasta, type Previa } from "./api";
  import Icone from "./Icone.svelte";

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

  /** Durante a pré-escuta, clicar na linha pula para aquele ponto da música. */
  function pular(ev: MouseEvent, caminho: string) {
    if (previa?.caminho !== caminho || !previa.duracao) return;
    const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
    api.previaIr(((ev.clientX - r.left) / r.width) * previa.duracao);
  }

  async function escolherPasta() {
    const p = await open({ directory: true, multiple: false, title: "Pasta dos programas" });
    if (typeof p === "string") await api.definirPasta("programas", p);
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
        <!-- Clicar para pular só faz sentido com mouse; os botões da linha já são acessíveis pelo teclado. -->
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
        <div
          class="linha item"
          class:ouvindo-linha={previa?.caminho === a.caminho}
          onclick={(e) => pular(e, a.caminho)}
          role="listitem"
        >
          <span class="reticencias" style="flex:1">{a.nome}</span>
          <button
            class="acao-linha ouvir"
            class:ouvindo={previa?.caminho === a.caminho}
            aria-label={previa?.caminho === a.caminho ? "Parar pré-escuta" : "Pré-escuta"}
            title={previa?.caminho === a.caminho ? "Parar pré-escuta" : "Pré-escuta"}
            onclick={(e) => {
              e.stopPropagation();
              previa?.caminho === a.caminho ? api.previaParar() : api.previaTocar(a.caminho);
            }}
          >
            <Icone nome={previa?.caminho === a.caminho ? "stop" : "fone"} tamanho={18} />
          </button>
          {#if previa?.caminho === a.caminho}
            <span class="mini" style:width="{previa.duracao ? Math.min(100, (previaDecorrido / previa.duracao) * 100) : 0}%"></span>
          {/if}
          <button class="acao-linha adicionar" aria-label="Pôr na fila" title="Pôr na fila" onclick={(e) => {
              e.stopPropagation();
              api.filaAdicionar(a.caminho, "programa");
            }}>
            <Icone nome="plus" tamanho={18} />
          </button>
        </div>
      {:else}
        <div class="vazio">Pasta sem arquivos de áudio</div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .ouvindo-linha {
    cursor: pointer;
    background: #172033;
  }
  .mini {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 4px;
    pointer-events: none;
    background: var(--azul);
  }
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
    font-size: 14px;
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
    position: relative;
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
