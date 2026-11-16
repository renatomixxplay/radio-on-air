<script lang="ts">
  // Janela de boas-vindas ao abrir o programa: mostra as etapas carregando e
  // some sozinha quando tudo fica pronto. Só aparece uma vez, na abertura.
  import { api, type Retrato } from "./api";

  let { r }: { r: Retrato | null } = $props();

  type Situacao = "espera" | "andando" | "ok" | "erro";
  /** Mostra pelo menos este tempo, para não piscar quando abre rápido. */
  const MINIMO_MS = 1500;
  /** Depois disso, aparecem os botões de reiniciar e fechar. */
  const DEMORA_MS = 30000;

  let aberta = $state(true);
  let demorou = $state(false);
  const inicio = Date.now();

  const etapas = $derived<{ nome: string; situacao: Situacao }[]>([
    { nome: "Carregando configurações", situacao: r ? "ok" : "andando" },
    { nome: "Carregando músicas", situacao: !r ? "espera" : r.carregando ? "andando" : "ok" },
    r?.erro_motor
      ? { nome: "Motor de áudio não ligou", situacao: "erro" }
      : { nome: "Carregando motor de áudio", situacao: !r ? "espera" : r.iniciando_motor ? "andando" : "ok" },
  ]);
  const pronto = $derived(etapas.every((e) => e.situacao === "ok"));
  const comErro = $derived(etapas.some((e) => e.situacao === "erro"));
  /** A primeira que ainda não terminou é a que aparece escrita. */
  const atual = $derived(etapas.find((e) => e.situacao !== "ok"));
  const feitas = $derived(etapas.filter((e) => e.situacao === "ok").length);

  $effect(() => {
    if (!aberta || !pronto) return;
    // Deixa ver a última etapa marcada antes de sumir.
    const t = setTimeout(() => (aberta = false), Math.max(MINIMO_MS - (Date.now() - inicio), 0) + 500);
    return () => clearTimeout(t);
  });

  $effect(() => {
    const t = setTimeout(() => (demorou = true), DEMORA_MS);
    return () => clearTimeout(t);
  });
</script>

{#if aberta}
  <div class="fundo">
    <div class="caixa">
      <div class="titulo">Seja bem-vindo</div>
      {#if r?.nome_radio}<div class="radio">{r.nome_radio}</div>{/if}

      <div class="barra" class:erro={comErro}><span style="width: {(feitas / etapas.length) * 100}%"></span></div>
      <div class="etapa" class:erro={comErro}>{atual ? (comErro ? atual.nome : `${atual.nome}…`) : "Pronto"}</div>

      {#if comErro && r?.erro_motor}
        <div class="mensagem">{r.erro_motor}</div>
      {/if}
      {#if comErro || demorou}
        <div class="botoes">
          <button class="reiniciar" onclick={() => api.reiniciar()}>Reiniciar</button>
          <button class="fechar" onclick={() => api.fechar()}>Fechar</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .fundo {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: var(--fundo);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
  }
  .caixa {
    width: 100%;
    max-width: 420px;
    padding: 32px 28px;
    border: 0.5px solid var(--borda);
    border-radius: 12px;
    background: var(--painel);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .titulo {
    font-size: 24px;
    font-weight: 600;
  }
  .radio {
    font-size: 17px;
    color: var(--texto-2);
  }
  .barra {
    align-self: stretch;
    height: 6px;
    margin-top: 26px;
    border-radius: 3px;
    background: var(--borda-forte);
    overflow: hidden;
  }
  .barra span {
    display: block;
    height: 100%;
    background: var(--verde);
    transition: width 0.4s;
  }
  .barra.erro span {
    background: var(--no-ar);
  }
  .etapa {
    margin-top: 10px;
    font-size: 16px;
    color: var(--texto-2);
  }
  .etapa.erro {
    color: #ff9b9b;
  }
  .mensagem {
    margin-top: 14px;
    align-self: stretch;
    color: #ff9b9b;
    font-size: 13px;
    line-height: 1.4;
    word-break: break-word;
  }
  .botoes {
    margin-top: 20px;
    display: flex;
    justify-content: center;
    gap: 10px;
  }
  .reiniciar {
    height: 44px;
    padding: 0 28px;
    border-radius: 8px;
    font-size: 16px;
    font-weight: 500;
    color: #fff;
    background: var(--verde);
    border-color: var(--verde);
  }
  .fechar {
    height: 44px;
    padding: 0 28px;
    border-radius: 8px;
    font-size: 16px;
  }
</style>
