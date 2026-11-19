<script lang="ts">
  import { api, MIME, tempo, type Arrasto, type Item } from "./api";
  import Icone from "./Icone.svelte";

  let { fila, lista }: { fila: Item[]; lista: string | null } = $props();

  const ETIQUETAS = {
    musica: { nome: "Música", icone: "music" },
    vinheta: { nome: "Vinheta", icone: "vinheta" },
    programa: { nome: "Programa", icone: "microphone" },
    comercial: { nome: "Comercial", icone: "megafone" },
    hora_certa: { nome: "Hora certa", icone: "relogio" },
    link: { nome: "Link ao vivo", icone: "aovivo" },
  } as const;

  const total = $derived(fila.reduce((s, i) => s + (i.duracao ?? 0), 0));

  let alvo = $state<number | null>(null);
  let arrastandoId = $state<number | null>(null);

  function iniciar(ev: DragEvent, item: Item) {
    if (item.travado) return ev.preventDefault();
    arrastandoId = item.id;
    const dado: Arrasto = { de: "fila", id: item.id };
    ev.dataTransfer?.setData(MIME, JSON.stringify(dado));
    if (ev.dataTransfer) ev.dataTransfer.effectAllowed = "move";
  }

  function sobreLinha(ev: DragEvent, i: number) {
    ev.preventDefault();
    ev.stopPropagation();
    const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
    alvo = ev.clientY < r.top + r.height / 2 ? i : i + 1;
  }

  function sobreLista(ev: DragEvent) {
    ev.preventDefault();
    alvo = fila.length;
  }

  async function soltar(ev: DragEvent) {
    ev.preventDefault();
    const pos = alvo ?? fila.length;
    alvo = null;
    arrastandoId = null;
    const txt = ev.dataTransfer?.getData(MIME);
    if (!txt) return;
    const dado = JSON.parse(txt) as Arrasto;
    if (dado.de === "acervo") {
      await api.filaAdicionar(dado.caminho, dado.tipo, pos).catch(alert);
    } else {
      const de = fila.findIndex((i) => i.id === dado.id);
      await api.filaMover(dado.id, de >= 0 && de < pos ? pos - 1 : pos);
    }
  }
</script>

<section class="painel">
  <div class="rotulo">
    <Icone nome="list" tamanho={14} />
    <!-- Tudo numa linha de texto só: o "Playlist", menor, fica na mesma base do "Fila". -->
    <span class="titulo reticencias"
      >Fila{#if lista}<span class="ponto">·</span><span class="lista-ativa" title="Lista de músicas valendo agora">Playlist {lista}</span>{/if}</span
    >
    <span class="info mono">{fila.length}/25 · {tempo(total)}</span>
  </div>
  <div
    class="lista"
    ondragover={sobreLista}
    ondragleave={(e) => {
      if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) alvo = null;
    }}
    ondrop={soltar}
    ondragend={() => {
      alvo = null;
      arrastandoId = null;
    }}
    role="list"
  >
    {#each fila as item, i (item.id)}
      <div
        class="linha item"
        class:travado={item.travado}
        class:arrastando={arrastandoId === item.id}
        class:antes={alvo === i}
        class:depois={alvo === fila.length && i === fila.length - 1}
        draggable={!item.travado}
        ondragstart={(e) => iniciar(e, item)}
        ondragover={(e) => sobreLinha(e, i)}
        role="listitem"
      >
        <span class="pega" class:oculta={item.travado}><Icone nome="grip" tamanho={22} /></span>
        <span class="tag etiqueta {item.tipo}" title={ETIQUETAS[item.tipo].nome} aria-label={ETIQUETAS[item.tipo].nome}>
          <Icone nome={ETIQUETAS[item.tipo].icone} tamanho={18} />
        </span>
        <span class="reticencias nome">
          {item.titulo}{#if item.artista}<span class="suave">&nbsp;·&nbsp;{item.artista}</span>{/if}
        </span>
        <span class="dur mono">{tempo(item.duracao)}</span>
        {#if item.travado}
          <span class="entrando" title="Já foi entregue ao motor">entrando</span>
        {:else}
          <button class="tirar" aria-label="Tirar da fila" title="Tirar da fila" onclick={() => api.filaRemover(item.id)}>
            <Icone nome="lixeira" tamanho={18} />
          </button>
        {/if}
      </div>
    {:else}
      <div class="vazio">A fila está vazia.<br />Arraste músicas ou programas para cá.</div>
    {/each}
  </div>
</section>

<style>
  .titulo {
    min-width: 0;
  }
  .ponto {
    margin: 0 6px;
  }
  .lista-ativa {
    color: var(--tag-musica);
    font-size: 13px;
  }
  .info {
    margin-left: auto;
    font-size: 13px;
  }
  .item {
    padding-left: 4px;
    border-top: 2px solid transparent;
    border-bottom: 0.5px solid var(--linha);
  }
  .item.antes {
    border-top-color: var(--azul);
  }
  .item.depois {
    border-bottom: 2px solid var(--azul);
  }
  .item.arrastando {
    opacity: 0.4;
  }
  .item:not(.travado) {
    cursor: grab;
  }
  .item:hover {
    background: var(--painel-2);
  }
  .item.travado {
    background: #1b231e;
  }
  .pega {
    color: var(--texto-3);
    display: flex;
  }
  .pega.oculta {
    visibility: hidden;
  }
  .etiqueta {
    width: 34px;
    height: 34px;
    flex: none;
    padding: 0;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .nome {
    flex: 1;
  }
  .dur {
    color: var(--texto-3);
    font-size: 14px;
  }
  .tirar {
    width: 34px;
    height: 34px;
    flex: none;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    color: var(--texto-2);
  }
  .tirar:hover {
    color: #fff;
    background: var(--no-ar);
    border-color: var(--no-ar);
  }
  .entrando {
    font-size: 13px;
    color: var(--verde);
    width: 64px;
    text-align: center;
  }
</style>
