<script lang="ts">
  import { tick } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Cantor, type Faixa, type Previa, type VideoYT } from "./api";
  import Icone from "./Icone.svelte";

  let {
    pasta,
    pastaYoutube,
    total,
    carregando,
    previa,
    previaDecorrido,
  }: {
    pasta: string | null;
    pastaYoutube: string | null;
    total: number;
    carregando: boolean;
    previa: Previa | null;
    previaDecorrido: number;
  } = $props();

  type Tipo = "cantor" | "texto" | "youtube";
  /** Pesquisa por cantor (padrão), por texto (música, artista ou pasta) ou no YouTube. */
  let tipo = $state<Tipo>("cantor");
  /** O que foi digitado em cada tipo de pesquisa (cada um guarda o seu). */
  let termos = $state({ cantor: "", texto: "", youtube: "" });
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

  // ─── YouTube ───
  let videos = $state<VideoYT[]>([]);
  let pesquisandoYT = $state(false);
  /** Já apertou Enter: a lista mostra os vídeos (apagar o texto volta para a pasta). */
  let pesquisouYT = $state(false);
  /** Músicas já baixadas na pasta do YouTube (A a Z): aparecem antes de pesquisar. */
  let musicasYT = $state<Faixa[]>([]);
  /** Muda depois de cada download, para reler a pasta. */
  let releituraYT = $state(0);
  const verVideos = $derived(pesquisandoYT || pesquisouYT);

  $effect(() => {
    if (tipo !== "youtube" || !pastaYoutube) return;
    total;
    releituraYT;
    api.youtubeMusicas().then((m) => (musicasYT = m));
  });
  let erroBusca = $state("");
  /** Baixando agora: id → andamento (0 a 100; 100 = convertendo). */
  let baixando = $state<Record<string, number>>({});
  /** Baixados nesta sessão: id → arquivo. */
  let prontos = $state<Record<string, string>>({});
  let errosYT = $state<Record<string, string>>({});
  /** Vídeo da lista que pôs a pré-escuta para tocar: só a linha clicada acende. */
  let ouvindoYT = $state<string | null>(null);

  /** A pré-escuta está tocando este arquivo (e, nos vídeos, a partir desta linha). */
  function tocandoAqui(caminho: string, id?: string): boolean {
    return previa?.caminho === caminho && (!id || ouvindoYT === id);
  }



  $effect(() => {
    const parar = api.aoProgressoYoutube((id, pct) => {
      if (baixando[id] !== undefined) baixando[id] = pct;
    });
    return () => void parar.then((f) => f());
  });

  /** A pesquisa no YouTube só sai com Enter (cada uma leva alguns segundos). */
  async function pesquisarYT() {
    const termo = termos.youtube.trim();
    if (!termo || pesquisandoYT) return;
    pesquisandoYT = true;
    pesquisouYT = true;
    erroBusca = "";
    try {
      videos = await api.youtubePesquisar(termo);
    } catch (e) {
      videos = [];
      erroBusca = String(e);
    } finally {
      pesquisandoYT = false;
    }
    rolarPara(0);
  }

  /** 🎧 e ⬇ fazem o mesmo: baixam, convertem e salvam. O 🎧 já toca na pré-escuta. */
  async function baixarVideo(v: VideoYT, ouvir: boolean) {
    if (baixando[v.id] !== undefined) return;
    delete errosYT[v.id];
    baixando[v.id] = 0;
    try {
      const caminho = await api.youtubeBaixar(v);
      prontos[v.id] = caminho;
      releituraYT++;
      if (ouvir) {
        ouvindoYT = v.id;
        api.previaTocar(caminho);
      }
    } catch (e) {
      errosYT[v.id] = String(e);
    } finally {
      delete baixando[v.id];
    }
  }

  async function escolherPastaYoutube() {
    const p = await open({ directory: true, multiple: false, title: "Pasta das músicas do YouTube" });
    if (typeof p === "string") await api.definirPasta("youtube", p);
  }

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

  // As músicas da pasta do YouTube também podem ser milhares: mesma coisa.
  const ALTURA_FAIXA = 56;
  const inicioYT = $derived(Math.max(0, Math.floor(rolagem / ALTURA_FAIXA) - 8));
  const fimYT = $derived(Math.min(musicasYT.length, Math.ceil((rolagem + alturaLista) / ALTURA_FAIXA) + 8));

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
    // Apagou o texto do YouTube: volta para as músicas da pasta.
    if (tipo === "youtube" && !valor.trim()) {
      pesquisouYT = false;
      videos = [];
      erroBusca = "";
    }
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
    tipo === "youtube"
      ? !verVideos
        ? `${musicasYT.length.toLocaleString("pt-BR")} músicas`
        : !pesquisandoYT && videos.length
          ? `${videos.length} vídeos`
          : ""
      : carregando
      ? "lendo pastas…"
      : tipo === "texto"
        ? total.toLocaleString("pt-BR")
        : escolhido
          ? `${escolhido.qtd.toLocaleString("pt-BR")} músicas`
          : `${cantores.length.toLocaleString("pt-BR")} cantores`,
  );
</script>

{#snippet linhaFaixa(f: Faixa, altura?: number)}
  <!-- Clicar para pular só faz sentido com mouse; os botões da linha já são acessíveis pelo teclado. -->
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <div
    class="linha faixa"
    class:ouvindo-linha={tocandoAqui(f.caminho)}
    style:height={altura ? `${altura}px` : undefined}
    onclick={(e) => pular(e, f.caminho)}
    role="listitem"
  >
    <div class="textos">
      <span class="reticencias musica">{f.musica}</span>
      {#if f.artista}<span class="reticencias suave">{f.artista}</span>{/if}
    </div>
    {@render botoesArquivo(f.caminho)}
  </div>
{/snippet}

<!-- 🎧 (pré-escuta, com a barrinha do tempo) e ➕ (pôr na fila) de um arquivo do computador. -->
{#snippet botoesArquivo(caminho: string, id?: string)}
  {@const aqui = tocandoAqui(caminho, id)}
  <button
    class="acao-linha ouvir"
    class:ouvindo={aqui}
    aria-label={aqui ? "Parar pré-escuta" : "Pré-escuta"}
    title={aqui ? "Parar pré-escuta" : "Pré-escuta"}
    onclick={(e) => {
      e.stopPropagation();
      if (aqui) {
        api.previaParar();
      } else {
        ouvindoYT = id ?? null;
        api.previaTocar(caminho);
      }
    }}
  >
    <Icone nome={aqui ? "stop" : "fone"} tamanho={18} />
  </button>
  {#if aqui && previa}
    <span class="mini" style:width="{previa.duracao ? Math.min(100, (previaDecorrido / previa.duracao) * 100) : 0}%"></span>
  {/if}
  <button class="acao-linha adicionar" aria-label="Pôr na fila" title="Pôr na fila" onclick={(e) => {
      e.stopPropagation();
      api.filaAdicionar(caminho, "musica");
    }}>
    <Icone nome="plus" tamanho={18} />
  </button>
{/snippet}

{#snippet linhaVideo(v: VideoYT)}
  {@const caminho = prontos[v.id] ?? v.caminho}
  {@const pct = baixando[v.id]}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <div
    class="linha faixa"
    class:ouvindo-linha={!!caminho && tocandoAqui(caminho, v.id)}
    onclick={(e) => caminho && pular(e, caminho)}
    role="listitem"
    title={caminho ? undefined : `Salva como: ${v.nome}`}
  >
    <div class="textos">
      <span class="reticencias musica">{v.titulo}</span>
      {#if errosYT[v.id]}
        <span class="reticencias erro" title={errosYT[v.id]}>{errosYT[v.id]}</span>
      {:else if v.canal}
        <span class="reticencias suave">{v.canal}</span>
      {/if}
    </div>
    {#if pct !== undefined}
      <span class="andamento">{pct < 100 ? `${pct}%` : "convertendo…"}</span>
      <span class="mini" style:width="{pct}%"></span>
    {:else if caminho}
      {@render botoesArquivo(caminho, v.id)}
    {:else}
      <button class="acao-linha ouvir" aria-label="Baixar e ouvir" title="Baixar e ouvir" onclick={(e) => {
          e.stopPropagation();
          baixarVideo(v, true);
        }}>
        <Icone nome="fone" tamanho={18} />
      </button>
      <button class="acao-linha ouvir" aria-label="Baixar" title="Baixar" onclick={(e) => {
          e.stopPropagation();
          baixarVideo(v, false);
        }}>
        <Icone nome="baixar" tamanho={18} />
      </button>
    {/if}
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
          onkeydown={(e) => tipo === "youtube" && e.key === "Enter" && pesquisarYT()}
          onfocus={voltarAosCantores}
          onclick={voltarAosCantores}
          placeholder={tipo === "cantor" ? "Cantor" : tipo === "texto" ? "Música ou artista" : "Pesquisar no YouTube e Enter"}
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
        <button
          class="tipo"
          class:ativo={tipo === "youtube"}
          aria-pressed={tipo === "youtube"}
          aria-label="Pesquisa no YouTube"
          title="Pesquisa no YouTube"
          onclick={() => trocarTipo("youtube")}
        >
          <Icone nome="youtube" tamanho={20} />
        </button>
      </div>
    </div>
    <div class="lista" bind:this={lista} bind:clientHeight={alturaLista} onscroll={() => (rolagem = lista?.scrollTop ?? 0)}>
      {#if tipo === "youtube"}
        {#if !pastaYoutube}
          <div class="vazio">
            Escolha a pasta onde as músicas do YouTube vão ficar.<br />
            <button class="acao" onclick={escolherPastaYoutube}><Icone nome="folderOpen" tamanho={14} /> Escolher pasta</button>
          </div>
        {:else if !verVideos}
          {#if musicasYT.length}
            <div class="rolo" style:height="{musicasYT.length * ALTURA_FAIXA}px">
              <div class="visiveis" style:transform="translateY({inicioYT * ALTURA_FAIXA}px)">
                {#each musicasYT.slice(inicioYT, fimYT) as f (f.caminho)}
                  {@render linhaFaixa(f, ALTURA_FAIXA)}
                {/each}
              </div>
            </div>
          {:else}
            <div class="vazio">Nenhuma música baixada ainda.<br />Digite o nome da música e aperte Enter.</div>
          {/if}
        {:else if pesquisandoYT}
          <div class="vazio">Pesquisando no YouTube…</div>
        {:else if erroBusca}
          <div class="vazio erro">{erroBusca}</div>
        {:else}
          {#each videos as v (v.id)}
            {@render linhaVideo(v)}
          {:else}
            <div class="vazio">Nenhum vídeo encontrado.</div>
          {/each}
        {/if}
      {:else if tipo === "texto"}
        {#each resultado as f (f.caminho)}
          {@render linhaFaixa(f)}
        {:else}
          <div class="vazio">{carregando ? "Lendo as pastas…" : "Nenhuma música encontrada"}</div>
        {/each}
        {#if resultado.length === 200}
          <div class="vazio">Mostrando as primeiras 200. Refine a busca.</div>
        {/if}
      {:else if escolhido}
        {#each musicasCantor as f (f.caminho)}
          {@render linhaFaixa(f)}
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
    border-radius: 0;
    margin-left: -0.5px;
  }
  .tipo:first-child {
    border-radius: 8px 0 0 8px;
    margin-left: 0;
  }
  .tipo:last-child {
    border-radius: 0 8px 8px 0;
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
  /* Mesma altura dos botões: a linha não afina enquanto baixa. */
  .andamento {
    flex: none;
    height: 34px;
    display: flex;
    align-items: center;
    color: var(--texto-2);
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .erro {
    color: #ff9b9b;
    font-size: 13px;
  }
  .acao {
    margin-top: 10px;
    padding: 6px 12px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
