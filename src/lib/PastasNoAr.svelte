<script lang="ts">
  // Painel "Pastas" da tela No ar: as pastas configuradas na aba Pastas.
  import { api, type Pasta, type Previa } from "./api";
  import Icone from "./Icone.svelte";

  let {
    pastas,
    previa,
    previaDecorrido,
  }: { pastas: Pasta[]; previa: Previa | null; previaDecorrido: number } = $props();

  let escolhida = $state("");
  let arquivos = $state<Pasta[]>([]);

  // Mantém a escolhida se ela ainda existir.
  $effect(() => {
    if (!pastas.some((x) => x.caminho === escolhida)) escolhida = pastas[0]?.caminho ?? "";
  });

  // Arquivos da pasta escolhida no seletor.
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
</script>

<section class="painel">
  <div class="rotulo">
    <Icone nome="folder" tamanho={14} />Pastas
  </div>

  {#if pastas.length === 0}
    <div class="vazio">Nenhuma pasta ainda.<br />Adicione na aba Pastas.</div>
  {:else}
    <div class="seletor">
      <select bind:value={escolhida} aria-label="Pasta">
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
</style>
