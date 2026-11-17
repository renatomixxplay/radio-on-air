<script lang="ts">
  import { tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Cantor, type Faixa, type Previa } from "./api";
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

  type Tipo = "cantor" | "texto";
  /** Pesquisa por cantor (padrão) ou por texto (música, artista ou pasta). */
  let tipo = $state<Tipo>("cantor");
  /** O que foi digitado em cada tipo de pesquisa (cada um guarda o seu). */
  let termos = $state({ cantor: "", texto: "" });
  let resultado = $state<Faixa[]>([]);
  let cantores = $state<Cantor[]>([]);
  /** Cantor clicado: o nome fica na busca e a lista mostra as músicas dele. */
  let escolhido = $state<Cantor | null>(null);
  let musicasCantor = $state<Faixa[]>([]);
  let espera: ReturnType<typeof setTimeout> | undefined;

  let lista = $state<HTMLDivElement>();
  let rolagem = $state(0);
  let alturaLista = $state(0);
  /** Onde estava a lista de cantores antes de abrir um (para voltar no mesmo ponto). */
  let rolagemGuardada = 0;

  async function pesquisar() {
    resultado = await api.buscar(termos.texto);
  }

  // Pesquisa por texto: refaz quando o texto muda (com pequena espera) ou o acervo muda.
  $effect(() => {
    if (tipo !== "texto") return;
    termos.texto;
    total;
    clearTimeout(espera);
    espera = setTimeout(pesquisar, 120);
  });

  // Pesquisa por cantor: a lista vem inteira e o filtro é feito aqui.
  $effect(() => {
    if (tipo !== "cantor") return;
    total;
    api.cantores().then((c) => (cantores = c));
  });

  // Músicas do cantor escolhido (de novo se o acervo mudar).
  $effect(() => {
    const c = escolhido?.chave;
    total;
    if (!c) return;
    api.musicasDoCantor(c).then((m) => {
      if (escolhido?.chave === c) musicasCantor = m;
    });
  });

  /** Sem acento e minúsculo, separado em palavras (igual à chave do cantor). */
  function palavras(t: string): string[] {
    return t
      .normalize("NFD")
      .replace(/\p{M}/gu, "")
      .toLowerCase()
      .split(/[^\p{L}\p{N}]+/u)
      .filter(Boolean);
  }

  const filtrados = $derived.by(() => {
    const p = palavras(termos.cantor);
    return p.length ? cantores.filter((c) => p.every((x) => c.chave.includes(x))) : cantores;
  });

  // A lista de cantores pode ter milhares: só as linhas visíveis são desenhadas.
  const ALTURA_CANTOR = 36;
  const inicio = $derived(Math.max(0, Math.floor(rolagem / ALTURA_CANTOR) - 8));
  const fim = $derived(Math.min(filtrados.length, Math.ceil((rolagem + alturaLista) / ALTURA_CANTOR) + 8));

  async function rolarPara(y: number) {
    await tick();
    if (lista) lista.scrollTop = y;
    rolagem = y;
  }

  function trocarTipo(t: Tipo) {
    if (t === tipo && !escolhido) return;
    tipo = t;
    escolhido = null;
    rolarPara(0);
  }

  function escolher(c: Cantor) {
    rolagemGuardada = lista?.scrollTop ?? 0;
    musicasCantor = [];
    escolhido = c;
    rolarPara(0);
  }

  /** Clicar na busca com um cantor escolhido volta para a lista de cantores. */
  function voltarAosCantores() {
    if (!escolhido) return;
    escolhido = null;
    rolarPara(rolagemGuardada);
  }

  function digitar(valor: string) {
    termos[tipo] = valor;
    escolhido = null;
    rolarPara(0);
  }

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

  const contagem = $derived(
    carregando
      ? "lendo pastas…"
      : tipo === "texto"
        ? total.toLocaleString("pt-BR")
        : escolhido
          ? `${escolhido.qtd.toLocaleString("pt-BR")} músicas`
          : `${cantores.length.toLocaleString("pt-BR")} cantores`,
  );
</script>

{#snippet linhaFaixa(f: Faixa, comArtista: boolean)}
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
      {#if comArtista}<span class="reticencias suave">{f.artista}</span>{/if}
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
{/snippet}

<section class="painel">
  <div class="rotulo">
    <Icone nome="music" tamanho={14} />Músicas
    <span class="total">{contagem}</span>
  </div>

  {#if !pasta}
    <div class="vazio">
      Escolha a pasta onde ficam as músicas.<br />
      <button class="acao" onclick={escolherPasta}><Icone nome="folderOpen" tamanho={14} /> Escolher pasta</button>
    </div>
  {:else}
    <div class="barra">
      <label class="busca" class:escolhido title={escolhido ? "Clique para voltar aos cantores" : undefined}>
        <Icone nome="search" tamanho={16} />
        <input
          value={escolhido ? escolhido.nome : termos[tipo]}
          oninput={(e) => digitar(e.currentTarget.value)}
          onfocus={voltarAosCantores}
          onclick={voltarAosCantores}
          placeholder={tipo === "cantor" ? "Cantor" : "Música ou artista"}
          spellcheck="false"
        />
      </label>
      <div class="tipos">
        <button
          class="tipo"
          class:ativo={tipo === "cantor"}
          aria-pressed={tipo === "cantor"}
          aria-label="Pesquisa por cantor"
          title="Pesquisa por cantor"
          onclick={() => trocarTipo("cantor")}
        >
          <Icone nome="cantor" tamanho={20} />
        </button>
        <button
          class="tipo"
          class:ativo={tipo === "texto"}
          aria-pressed={tipo === "texto"}
          aria-label="Pesquisa por texto"
          title="Pesquisa por texto"
          onclick={() => trocarTipo("texto")}
        >
          <Icone nome="texto" tamanho={20} />
        </button>
      </div>
    </div>
    <div class="lista" bind:this={lista} bind:clientHeight={alturaLista} onscroll={() => (rolagem = lista?.scrollTop ?? 0)}>
      {#if tipo === "texto"}
        {#each resultado as f (f.caminho)}
          {@render linhaFaixa(f, true)}
        {:else}
          <div class="vazio">{carregando ? "Lendo as pastas…" : "Nenhuma música encontrada"}</div>
        {/each}
        {#if resultado.length === 200}
          <div class="vazio">Mostrando as primeiras 200. Refine a busca.</div>
        {/if}
      {:else if escolhido}
        {#each musicasCantor as f (f.caminho)}
          {@render linhaFaixa(f, false)}
        {/each}
      {:else if filtrados.length}
        <div class="rolo" style:height="{filtrados.length * ALTURA_CANTOR}px">
          <div class="visiveis" style:transform="translateY({inicio * ALTURA_CANTOR}px)">
            {#each filtrados.slice(inicio, fim) as c (c.chave)}
              <button class="cantor" style:height="{ALTURA_CANTOR}px" onclick={() => escolher(c)}>
                <span class="reticencias nome">{c.nome}</span>
                <span class="qtd">{c.qtd.toLocaleString("pt-BR")}</span>
              </button>
            {/each}
          </div>
        </div>
      {:else}
        <div class="vazio">{carregando ? "Lendo as pastas…" : "Nenhum cantor encontrado"}</div>
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
  .barra {
    display: flex;
    gap: 8px;
    margin: 0 10px 8px;
  }
  .busca {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
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
  .busca.escolhido {
    background: var(--azul-claro-bg);
    border-color: #4a6aa5;
    color: var(--azul-claro);
  }
  .busca.escolhido input {
    color: var(--azul-claro);
    font-weight: 600;
    font-size: 15px;
    cursor: pointer;
  }
  .tipos {
    display: flex;
    flex: none;
  }
  .tipo {
    width: 40px;
    height: 40px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--texto-2);
    border-radius: 0 8px 8px 0;
  }
  .tipo:first-child {
    border-radius: 8px 0 0 8px;
    margin-right: -0.5px;
  }
  .tipo.ativo {
    color: var(--azul-claro);
    background: var(--azul-claro-bg);
    border-color: #4a6aa5;
    position: relative;
  }
  .cantor {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    font-size: 15px;
    text-align: left;
    background: transparent;
    border: none;
    border-bottom: 0.5px solid var(--linha);
    border-radius: 0;
  }
  .cantor:hover {
    background: var(--painel-2);
  }
  .cantor:active {
    transform: none;
  }
  .cantor .nome {
    flex: 1;
    min-width: 0;
  }
  .cantor .qtd {
    color: var(--texto-3);
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .busca input {
    min-width: 0;
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
