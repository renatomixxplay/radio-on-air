<script lang="ts">
  // Campo de data com calendário próprio, no lugar do calendário do sistema
  // (que no Mac fica pequeno e apertado). O valor é "AAAA-MM-DD", como o
  // <input type="date">.
  import Icone from "./Icone.svelte";

  let {
    value = $bindable(""),
    largura = 160,
    embutido = false,
  }: {
    value?: string;
    largura?: number;
    /** Sem borda própria, ocupando a altura de quem está em volta. */
    embutido?: boolean;
  } = $props();

  const MESES = ["Janeiro", "Fevereiro", "Março", "Abril", "Maio", "Junho", "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro"];
  const SEMANA = ["D", "S", "T", "Q", "Q", "S", "S"];

  const doisDigitos = (n: number) => String(n).padStart(2, "0");
  const iso = (a: number, m: number, d: number) => `${a}-${doisDigitos(m + 1)}-${doisDigitos(d)}`;
  const hojeIso = () => {
    const h = new Date();
    return iso(h.getFullYear(), h.getMonth(), h.getDate());
  };

  let aberto = $state(false);
  let ano = $state(0);
  let mes = $state(0);
  let campo: HTMLButtonElement;
  let caixa = $state<HTMLDivElement>();
  let pos = $state({ top: 0, left: 0 });

  /** Dias do mês mostrado, com espaços vazios antes do dia 1. */
  const dias = $derived.by(() => {
    const vazios = new Date(ano, mes, 1).getDay();
    const total = new Date(ano, mes + 1, 0).getDate();
    return [...Array(vazios).fill(0), ...Array.from({ length: total }, (_, i) => i + 1)] as number[];
  });

  function abrir() {
    if (aberto) return fechar();
    const [a, m] = (value || hojeIso()).split("-").map(Number);
    ano = a;
    mes = m - 1;
    // Abre embaixo do campo; se não couber, abre em cima.
    const r = campo.getBoundingClientRect();
    const altura = 380;
    const top = r.bottom + 6 + altura > window.innerHeight ? Math.max(8, r.top - 6 - altura) : r.bottom + 6;
    const left = Math.min(r.left, window.innerWidth - 340);
    pos = { top, left: Math.max(8, left) };
    aberto = true;
  }

  function fechar() {
    aberto = false;
  }

  function mudarMes(passo: number) {
    const d = new Date(ano, mes + passo, 1);
    ano = d.getFullYear();
    mes = d.getMonth();
  }

  function escolher(dia: number) {
    value = iso(ano, mes, dia);
    fechar();
  }

  function cliqueFora(ev: PointerEvent) {
    const alvo = ev.target as Node;
    if (!caixa?.contains(alvo) && !campo.contains(alvo)) fechar();
  }

  function tecla(ev: KeyboardEvent) {
    if (ev.key === "Escape") fechar();
  }

  $effect(() => {
    if (!aberto) return;
    window.addEventListener("pointerdown", cliqueFora, true);
    window.addEventListener("keydown", tecla);
    window.addEventListener("resize", fechar);
    return () => {
      window.removeEventListener("pointerdown", cliqueFora, true);
      window.removeEventListener("keydown", tecla);
      window.removeEventListener("resize", fechar);
    };
  });

  const hoje = $derived(aberto ? hojeIso() : "");
</script>

<button type="button" class="campo" class:aberto class:embutido style:width="{largura}px" bind:this={campo} onclick={abrir}>
  {#if value}
    <span class="mono">{value.split("-").reverse().join("/")}</span>
  {:else}
    <span class="vazio">Escolher</span>
  {/if}
  <Icone nome="calendar" tamanho={18} />
</button>

{#if aberto}
  <div class="painel calendario" bind:this={caixa} style:top="{pos.top}px" style:left="{pos.left}px">
    <div class="topo">
      <button type="button" class="seta" onclick={() => mudarMes(-1)} title="Mês anterior" aria-label="Mês anterior">
        <Icone nome="voltar" tamanho={22} />
      </button>
      <span class="titulo">{MESES[mes]} {ano}</span>
      <button type="button" class="seta" onclick={() => mudarMes(1)} title="Próximo mês" aria-label="Próximo mês">
        <Icone nome="avancar" tamanho={22} />
      </button>
    </div>
    <div class="grade">
      {#each SEMANA as s}
        <span class="semana">{s}</span>
      {/each}
      {#each dias as dia}
        {#if dia}
          {@const d = iso(ano, mes, dia)}
          <button type="button" class="dia" class:hoje={d === hoje} class:sel={d === value} onclick={() => escolher(dia)}>{dia}</button>
        {:else}
          <span></span>
        {/if}
      {/each}
    </div>
  </div>
{/if}

<style>
  .campo {
    height: 38px;
    padding: 0 10px 0 12px;
    border-radius: 8px;
    background: var(--campo);
    border-color: var(--borda);
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: 15px;
    color: var(--texto);
  }
  .campo :global(.icone) {
    color: var(--texto-2);
  }
  .campo.aberto {
    border-color: var(--azul);
  }
  .campo.embutido {
    height: 100%;
    border: none;
    border-radius: 0;
    font-size: 16px;
  }
  .vazio {
    color: var(--texto-3);
  }
  .calendario {
    position: fixed;
    z-index: 50;
    width: 332px;
    padding: 12px;
    background: var(--painel);
    border-color: var(--borda-forte);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  }
  .topo {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .titulo {
    font-size: 17px;
    font-weight: 600;
  }
  .seta {
    width: 40px;
    height: 40px;
    padding: 0;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--texto-2);
  }
  .grade {
    display: grid;
    grid-template-columns: repeat(7, 40px);
    gap: 4px;
    justify-content: space-between;
  }
  .semana {
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 13px;
    color: var(--texto-3);
  }
  .dia {
    width: 40px;
    height: 40px;
    padding: 0;
    border-radius: 8px;
    font-size: 16px;
    background: transparent;
    border-color: transparent;
  }
  .dia:hover {
    background: var(--painel-2);
    border-color: var(--borda-forte);
  }
  .dia.hoje {
    border-color: var(--azul);
  }
  .dia.sel {
    background: var(--azul);
    border-color: var(--azul);
    color: #fff;
    font-weight: 600;
  }
</style>
