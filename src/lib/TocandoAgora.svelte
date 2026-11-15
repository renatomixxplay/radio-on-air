<script lang="ts">
  import { api, tempo, type Item } from "./api";
  import Icone from "./Icone.svelte";

  let {
    item,
    decorrido,
    noAr,
    pausado,
  }: { item: Item | null; decorrido: number; noAr: boolean; pausado: boolean } = $props();

  const duracao = $derived(item?.duracao ?? null);
  const progresso = $derived(duracao ? Math.min(1, decorrido / duracao) : 0);
  const falta = $derived(duracao != null ? duracao - decorrido : null);
  /** Últimos 10 segundos: a barra fica vermelha. */
  const acabando = $derived(item != null && falta != null && falta <= 10);
</script>

<section class="painel tocando" class:ativo={noAr && item}>
  <div class="rotulo">
    <span class="ponto" class:on={noAr && item && !pausado}><Icone nome="play" tamanho={14} /></span>
    Tocando agora
    {#if pausado}<span class="pausado">pausado</span>{/if}
  </div>

  <div class="corpo">
    {#if item}
      <div class="titulo reticencias">{item.titulo}</div>
      <div class="artista reticencias">{item.artista || " "}</div>
    {:else}
      <div class="titulo suave">{noAr ? "Aguardando…" : "Parado"}</div>
      <div class="artista">&nbsp;</div>
    {/if}

    <div class="barra" aria-hidden="true">
      <div class="preenchido" class:acabando style:width="{progresso * 100}%"></div>
    </div>

    <div class="tempos mono">
      <span>{item ? tempo(decorrido) : "--:--"}</span>
      <span class="falta">{falta != null ? "-" + tempo(falta) : "--:--"}</span>
    </div>

    <div class="botoes">
      {#if noAr && !pausado}
        <button onclick={api.pausar} title="Pausar" aria-label="Pausar"><Icone nome="pause" tamanho={18} /></button>
      {:else}
        <button class="principal" onclick={api.tocar} title="Tocar" aria-label="Tocar"><Icone nome="play" tamanho={18} /></button>
      {/if}
      <button onclick={api.parar} title="Parar" aria-label="Parar"><Icone nome="stop" tamanho={18} /></button>
      <button onclick={api.proxima} title="Próxima" aria-label="Próxima" disabled={!noAr}>
        <Icone nome="skip" tamanho={18} />
      </button>
    </div>
  </div>
</section>

<style>
  .tocando.ativo {
    border-color: var(--verde-borda);
  }
  .ponto {
    display: inline-flex;
    color: var(--texto-3);
  }
  .ponto.on {
    color: var(--verde);
  }
  .pausado {
    margin-left: auto;
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--tag-vinheta-bg);
    color: var(--tag-vinheta);
  }
  .corpo {
    padding: 0 12px 12px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .titulo {
    font-size: 20px;
    font-weight: 500;
    color: #fff;
  }
  .artista {
    color: var(--texto-3);
    font-size: 14px;
    margin: 2px 0 10px;
  }
  .barra {
    height: 14px;
    border-radius: 7px;
    background: #2a2e37;
    overflow: hidden;
    margin: 8px 0;
  }
  .preenchido {
    height: 100%;
    background: var(--verde);
    border-radius: 7px;
  }
  .preenchido.acabando {
    background: var(--no-ar);
  }
  .tempos {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .falta {
    color: var(--verde);
    font-size: 22px;
    font-weight: 500;
  }
  .botoes {
    display: flex;
    gap: 6px;
    margin-top: 10px;
  }
  .botoes button {
    width: 44px;
    height: 36px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .botoes button.principal {
    background: #1f3a2a;
    border-color: var(--verde-borda);
    color: var(--tag-musica);
  }
  .botoes button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
