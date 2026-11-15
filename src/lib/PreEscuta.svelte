<script lang="ts">
  // Player de pré-escuta: toca numa saída separada, não interfere no que está no ar.
  import { api, tempo, type Previa } from "./api";
  import Icone from "./Icone.svelte";

  let { previa, decorrido }: { previa: Previa; decorrido: number } = $props();

  const progresso = $derived(previa.duracao ? Math.min(1, decorrido / previa.duracao) : 0);

  function irPara(ev: MouseEvent) {
    if (!previa.duracao) return;
    const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
    api.previaIr(((ev.clientX - r.left) / r.width) * previa.duracao);
  }
</script>

<div class="previa">
  <div class="topo">
    <span class="fone"><Icone nome="fone" tamanho={16} /></span>
    <div class="textos">
      <span class="reticencias titulo">{previa.titulo}</span>
      {#if previa.artista}<span class="reticencias suave">{previa.artista}</span>{/if}
    </div>
    <button class="parar" onclick={api.previaParar} title="Parar pré-escuta" aria-label="Parar pré-escuta">
      <Icone nome="stop" tamanho={16} />
    </button>
  </div>
  <button class="barra" onclick={irPara} title="Clique para pular para esse ponto" aria-label="Posição da pré-escuta">
    <span class="preenchido" style:width="{progresso * 100}%"></span>
  </button>
  <div class="tempos mono">
    <span>{tempo(decorrido)}</span>
    <span>-{tempo((previa.duracao ?? 0) - decorrido)}</span>
  </div>
</div>

<style>
  .previa {
    border-top: 0.5px solid var(--borda);
    background: #172033;
    padding: 8px 10px 6px;
    border-radius: 0 0 8px 8px;
  }
  .topo {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .fone {
    color: var(--tag-programa);
    display: flex;
  }
  .textos {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .textos .suave {
    font-size: 13px;
  }
  .parar {
    width: 32px;
    height: 32px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
  }
  .barra {
    display: block;
    width: 100%;
    height: 10px;
    margin: 8px 0 4px;
    padding: 0;
    border: none;
    border-radius: 5px;
    background: #2a2e37;
    overflow: hidden;
    cursor: pointer;
  }
  .barra:active {
    transform: none;
  }
  .preenchido {
    display: block;
    height: 100%;
    background: var(--tag-programa);
  }
  .tempos {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
    color: var(--texto-3);
  }
</style>
