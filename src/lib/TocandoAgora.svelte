<script lang="ts">
  import { api, tempo, type Item, type Retrato } from "./api";
  import Icone from "./Icone.svelte";

  let {
    item,
    decorrido,
    noAr,
    pausado,
    repetir,
    ducking,
    porCima,
    aguardando,
    repetirAuto,
    proximo,
  }: {
    item: Item | null;
    decorrido: number;
    noAr: boolean;
    pausado: boolean;
    repetir: boolean;
    ducking: boolean;
    porCima: string | null;
    aguardando: string | null;
    repetirAuto: boolean;
    proximo: Retrato["proximo_agendamento"];
  } = $props();

  const duracao = $derived(item?.duracao ?? null);
  const progresso = $derived(duracao ? Math.min(1, decorrido / duracao) : 0);
  const falta = $derived(duracao != null ? duracao - decorrido : null);
  /** Comercial e hora certa não podem ser pulados. */
  const agendado = $derived(item?.tipo === "comercial" || item?.tipo === "hora_certa");
  /** Últimos 10 segundos: a barra fica vermelha. */
  const acabando = $derived(item != null && falta != null && falta <= 10);
</script>

<section class="painel tocando" class:ativo={noAr && item}>
  <div class="rotulo">
    <span class="ponto" class:on={noAr && item && !pausado}><Icone nome="play" tamanho={14} /></span>
    Tocando agora
    {#if pausado}<span class="pausado">pausado</span>{/if}
    <span class="avisos">
      {#if !aguardando && !porCima && proximo}
        <span class="aviso-ag contorno apagado" title="Próximo agendamento">
          <Icone nome={proximo.tipo === "hora_certa" ? "relogio" : proximo.tipo === "ducking" ? "ducking" : "megafone"} tamanho={14} />
          <span class="mono">{proximo.horario}</span><span class="texto">{proximo.nome}</span>
        </span>
      {/if}
      {#if aguardando}
        <span class="aviso-ag contorno" title="Entra quando esta música acabar">
          <Icone nome="megafone" tamanho={14} /><span class="texto">depois desta: {aguardando}</span>
        </span>
      {/if}
      {#if porCima}
        <span class="aviso-ag" title="Comercial tocando por cima da música">
          <Icone nome="ducking" tamanho={14} /><span class="texto">{porCima}</span>
        </span>
      {/if}
    </span>
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
      <button class:ligado={ducking} onclick={api.alternarDucking} title="Ducking: abaixa a música para o locutor falar" aria-label="Ducking">
        <Icone nome="ducking" tamanho={20} />
      </button>
      {#if noAr && !pausado}
        <button onclick={api.pausar} title="Pausar" aria-label="Pausar"><Icone nome="pause" tamanho={20} /></button>
      {:else}
        <button class="principal" onclick={api.tocar} title="Tocar" aria-label="Tocar"><Icone nome="play" tamanho={20} /></button>
      {/if}
      <button
        class:ligado={repetir || repetirAuto}
        onclick={api.alternarRepetir}
        title={repetirAuto ? "Repetindo sozinho enquanto o comercial fala" : "Repetir a música atual"}
        aria-label="Repetir"
      >
        <Icone nome="repetir" tamanho={20} />
      </button>
      <button
        onclick={api.proxima}
        title={agendado ? "Agendamento não pode ser pulado" : "Passar para a próxima"}
        aria-label="Passar"
        disabled={!noAr || agendado}
      >
        <Icone nome="skip" tamanho={20} />
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
    font-size: 13px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--laranja-bg);
    color: var(--laranja);
  }
  .avisos {
    margin-left: auto;
    display: flex;
    align-items: center;
  }
  /* Tamanho fixo: nome comprido vira "…" em vez de esticar o aviso. */
  .aviso-ag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    width: 300px;
    height: 28px;
    font-size: 13px;
    line-height: 1;
    padding: 0 10px;
    border-radius: 6px;
    background: var(--tag-comercial-bg);
    color: var(--tag-comercial);
    border: 0.5px solid transparent;
  }
  .aviso-ag .texto {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .aviso-ag.contorno {
    background: transparent;
    border-color: var(--tag-comercial);
  }
  .aviso-ag.apagado {
    opacity: 0.75;
  }
  .corpo {
    flex: 1;
    padding: 0 16px 14px;
    display: flex;
    flex-direction: column;
    justify-content: center;
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
    justify-content: center;
    gap: 10px;
    margin-top: 14px;
  }
  .botoes button {
    min-width: 48px;
    height: 40px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }
  .botoes button.ligado {
    background: var(--laranja-bg);
    border-color: var(--pasta);
    color: var(--laranja);
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
