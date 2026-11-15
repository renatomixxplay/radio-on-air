<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Retrato } from "$lib/api";
  import Icone from "$lib/Icone.svelte";
  import TocandoAgora from "$lib/TocandoAgora.svelte";
  import Cartucheira from "$lib/Cartucheira.svelte";
  import Musicas from "$lib/Musicas.svelte";
  import Fila from "$lib/Fila.svelte";
  import Programas from "$lib/Programas.svelte";
  import Agendamentos from "$lib/Agendamentos.svelte";

  let r = $state<Retrato | null>(null);
  let recebidoEm = $state(Date.now());
  let agora = $state(new Date());
  let aba = $state<"noar" | "agenda" | "config">("noar");

  function receber(novo: Retrato) {
    r = novo;
    recebidoEm = Date.now();
  }

  onMount(() => {
    api.estado().then(receber);
    const parar = api.aoMudar(receber);
    // Só o relógio e o tempo da faixa mudam sozinhos; o resto vem do programa.
    const t = setInterval(() => (agora = new Date()), 500);
    return () => {
      clearInterval(t);
      parar.then((f) => f());
    };
  });

  const decorrido = $derived.by(() => {
    if (!r?.tocando) return 0;
    const extra = r.no_ar && !r.pausado ? (agora.getTime() - recebidoEm) / 1000 : 0;
    return r.tocando.decorrido + Math.max(0, extra);
  });
  /** Pré-escuta: aparece no painel de onde o arquivo veio. */
  const previaDecorrido = $derived.by(() => {
    const p = r?.previa;
    if (!p) return 0;
    return p.decorrido + (p.iniciada ? Math.max(0, (agora.getTime() - recebidoEm) / 1000) : 0);
  });
  const previaPrograma = $derived(
    !!r?.previa && !!r.pasta_programas && r.previa.caminho.startsWith(r.pasta_programas),
  );
  const relogio = $derived(agora.toLocaleTimeString("pt-BR", { hour12: false }));
</script>

<div class="app">
  <header>
    <div class="marca"><span class="antena"><Icone nome="antenna" tamanho={20} /></span>Radio On Air</div>
    <nav>
      <button class:on={aba === "noar"} onclick={() => (aba = "noar")}>No ar</button>
      <button class:on={aba === "agenda"} onclick={() => (aba = "agenda")}>Agendamentos</button>
      <button class:on={aba === "config"} onclick={() => (aba = "config")}>Configurações</button>
    </nav>
    <div class="direita">
      {#if r?.iniciando_motor}
        <span class="ligando">ligando o áudio…</span>
      {/if}
      {#if r?.no_ar}
        <span class="no-ar">NO AR</span>
      {:else}
        <span class="fora">FORA DO AR</span>
      {/if}
      <span class="relogio mono">{relogio}</span>
    </div>
  </header>

  {#if r?.erro_motor}
    <div class="aviso"><Icone nome="alert" tamanho={16} /> {r.erro_motor}</div>
  {/if}

  {#if !r}
    <div class="vazio">Abrindo…</div>
  {:else if aba === "noar"}
    <main>
      <div class="cima">
        <TocandoAgora
          item={r.tocando?.item ?? null}
          {decorrido}
          noAr={r.no_ar}
          pausado={r.pausado}
          repetir={r.repetir}
          ducking={r.ducking}
          porCima={r.por_cima}
          aguardando={r.aguardando}
          proximo={r.proximo_agendamento}
          repetirAuto={r.repetir_auto}
        />
        <Cartucheira slots={r.cartucheira} />
      </div>
      <div class="baixo">
        <Musicas
          pasta={r.pasta_musicas}
          total={r.total_musicas}
          carregando={r.carregando}
          previa={previaPrograma ? null : r.previa}
          {previaDecorrido}
        />
        <Fila fila={r.fila} lista={r.lista_ativa} />
        <Programas pasta={r.pasta_programas} previa={previaPrograma ? r.previa : null} {previaDecorrido} />
      </div>
    </main>
  {:else if aba === "agenda"}
    <Agendamentos />
  {:else}
    <div class="em-breve">
      <Icone nome="settings" tamanho={32} />
      <p>Configurações chegam nas próximas fases.</p>
    </div>
  {/if}
</div>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
  header {
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 8px 14px;
    background: var(--barra);
    border-bottom: 0.5px solid var(--borda);
  }
  .marca {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 500;
    font-size: 14px;
  }
  .antena {
    color: var(--no-ar);
    display: flex;
  }
  nav {
    display: flex;
    gap: 4px;
  }
  nav button {
    background: transparent;
    border: none;
    border-radius: 0;
    padding: 4px 8px;
    color: var(--texto-3);
    border-bottom: 2px solid transparent;
  }
  nav button.on {
    color: #fff;
    border-bottom-color: var(--no-ar);
  }
  .direita {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .no-ar,
  .fora {
    font-size: 13px;
    font-weight: 600;
    padding: 3px 9px;
    border-radius: 4px;
    letter-spacing: 0.04em;
  }
  .ligando {
    font-size: 13px;
    color: var(--texto-3);
  }
  .no-ar {
    background: var(--no-ar);
    color: #fff;
  }
  .fora {
    background: var(--painel-2);
    color: var(--texto-3);
  }
  .relogio {
    font-size: 26px;
    font-weight: 500;
    color: #fff;
  }
  .aviso {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    background: #3a1d1e;
    color: #ffb4b4;
    border-bottom: 0.5px solid #5a2a2c;
  }
  main {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 10px;
    padding: 10px;
  }
  .cima {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
    gap: 10px;
  }
  .baixo {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.4fr) minmax(0, 0.9fr);
    gap: 10px;
    min-height: 0;
  }
  .em-breve {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--texto-3);
  }
</style>
