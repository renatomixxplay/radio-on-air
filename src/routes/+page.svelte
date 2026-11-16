<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Retrato } from "$lib/api";
  import Icone from "$lib/Icone.svelte";
  import TocandoAgora from "$lib/TocandoAgora.svelte";
  import Cartucheira from "$lib/Cartucheira.svelte";
  import Musicas from "$lib/Musicas.svelte";
  import Fila from "$lib/Fila.svelte";
  import PastasNoAr from "$lib/PastasNoAr.svelte";
  import Agendamentos from "$lib/Agendamentos.svelte";
  import AbaPastas from "$lib/AbaPastas.svelte";
  import AbaCartucheira from "$lib/AbaCartucheira.svelte";
  import AbaConfiguracoes from "$lib/AbaConfiguracoes.svelte";
  import PedirSenha from "$lib/PedirSenha.svelte";
  import Abertura from "$lib/Abertura.svelte";
  import type { Bloqueios } from "$lib/api";

  let r = $state<Retrato | null>(null);
  let recebidoEm = $state(Date.now());
  let agora = $state(new Date());
  type Aba = "noar" | "agenda" | "pastas" | "cart" | "config";
  let aba = $state<Aba>("noar");
  const ABAS: { id: Aba; nome: string; trava?: keyof Bloqueios }[] = [
    { id: "noar", nome: "NO AR" },
    { id: "agenda", nome: "Agendamentos", trava: "agendamentos" },
    { id: "pastas", nome: "Pastas", trava: "pastas" },
    { id: "cart", nome: "Cartucheira", trava: "cartucheira" },
    { id: "config", nome: "Configurações", trava: "configuracoes" },
  ];

  // Senha: depois de desbloquear, vale até apertar o cadeado de novo.
  let liberado = $state(false);
  let senhaSessao = $state("");
  const abaAtual = $derived(ABAS.find((a) => a.id === aba)!);
  const travada = $derived(!!r?.protegido && !liberado && !!abaAtual.trava && r.bloqueios[abaAtual.trava]);

  function receber(novo: Retrato) {
    r = novo;
    recebidoEm = Date.now();
  }

  // Tela cheia: botão do topo ou F11. O ícone acompanha a janela (ela também
  // pode sair da tela cheia pelo próprio sistema).
  let telaCheia = $state(false);
  const conferirTelaCheia = () => api.telaCheiaAgora().then((v) => (telaCheia = v));
  async function alternarTelaCheia() {
    telaCheia = await api.alternarTelaCheia();
  }
  function tecla(ev: KeyboardEvent) {
    if (ev.key === "F11") {
      ev.preventDefault();
      alternarTelaCheia();
    }
  }

  onMount(() => {
    api.estado().then(receber);
    const parar = api.aoMudar(receber);
    conferirTelaCheia();
    window.addEventListener("resize", conferirTelaCheia);
    window.addEventListener("keydown", tecla);
    // Só o relógio e o tempo da faixa mudam sozinhos; o resto vem do programa.
    const t = setInterval(() => (agora = new Date()), 500);
    return () => {
      clearInterval(t);
      window.removeEventListener("resize", conferirTelaCheia);
      window.removeEventListener("keydown", tecla);
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
    const passou = (agora.getTime() - recebidoEm) / 1000 - p.espera;
    return p.decorrido + (p.iniciada ? Math.max(0, passou) : 0);
  });
  const previaPrograma = $derived(
    // No Windows a barra do caminho é a invertida.
    !!r?.previa && r.pastas.some((p) => [p.caminho + "/", p.caminho + "\\"].some((c) => r!.previa!.caminho.startsWith(c))),
  );
  const relogio = $derived(agora.toLocaleTimeString("pt-BR", { hour12: false }));
</script>

<Abertura {r} />

<div class="app">
  <header>
    <div class="marca reticencias" title={r?.nome_radio}>{r?.nome_radio ?? ""}</div>
    <nav>
      {#each ABAS as a}
        <button class:on={aba === a.id} onclick={() => (aba = a.id)}>
          {#if a.id === "noar"}<Icone nome="aovivo" tamanho={15} />{/if}{a.nome}
        </button>
      {/each}
    </nav>
    <div class="direita">
      {#if r?.protegido && liberado}
        <!-- Aparece só com as abas desbloqueadas: o texto é a ação do clique. -->
        <button class="etiqueta desbloqueado" title="Bloquear as abas de novo" onclick={() => ((liberado = false), (senhaSessao = ""))}>
          <Icone nome="cadeado" tamanho={14} />BLOQUEAR
        </button>
      {/if}
      {#if r?.iniciando_motor}
        <span class="ligando">ligando o áudio…</span>
      {/if}
      <!-- Gravação e transmissão: só aparecem com a chave ligada; laranja = problema. -->
      {#if r?.gravacao.ligada}
        {#if r.gravando}
          <span class="etiqueta gravando" title="Gravando a entrada"><Icone nome="gravar" tamanho={14} />GRAVANDO</span>
        {:else}
          <span class="etiqueta problema" title={r.erro_captura ?? "A gravação não está funcionando"}><Icone nome="gravar" tamanho={14} />SEM GRAVAÇÃO</span>
        {/if}
      {/if}
      {#if r?.transmissao.ligada}
        {#if r.transmissao_estado === "conectado"}
          <span class="etiqueta transmitindo" title="Transmitindo pela internet"><Icone nome="transmitir" tamanho={14} />TRANSMITINDO</span>
        {:else if r.transmissao_estado === "conectando"}
          <span class="etiqueta problema" title="Conectando ao servidor"><Icone nome="transmitir" tamanho={14} />CONECTANDO…</span>
        {:else}
          <span class="etiqueta problema" title={r.transmissao_estado}><Icone nome="transmitir" tamanho={14} />SEM TRANSMISSÃO</span>
        {/if}
      {/if}
      <span class="relogio mono">{relogio}</span>
      <button class="tela-cheia" title={telaCheia ? "Sair da tela cheia (F11)" : "Tela cheia (F11)"} onclick={alternarTelaCheia}>
        <Icone nome={telaCheia ? "sairTelaCheia" : "telaCheia"} tamanho={20} />
      </button>
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
          posDucking={r.pos_ducking}
          link={r.link}
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
        <PastasNoAr pastas={r.pastas} previa={previaPrograma ? r.previa : null} {previaDecorrido} />
      </div>
    </main>
  {:else if travada}
    <PedirSenha area={abaAtual.nome} aoLiberar={(senha) => ((liberado = true), (senhaSessao = senha))} />
  {:else if aba === "agenda"}
    <Agendamentos />
  {:else if aba === "pastas"}
    <AbaPastas pastas={r.pastas} />
  {:else if aba === "cart"}
    <AbaCartucheira slots={r.cartucheira} />
  {:else}
    <AbaConfiguracoes {r} {senhaSessao} aoMudarSenha={(s) => ((senhaSessao = s), (liberado = s !== ""))} />
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
    font-weight: 500;
    font-size: 16px;
    max-width: 260px;
  }
  nav {
    display: flex;
    gap: 4px;
  }
  nav button {
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: 5px;
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
  /* Etiquetas do topo (Gravando / Transmitindo / Bloquear): ícone + texto. */
  .etiqueta {
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    padding: 3px 9px;
    border: none;
    border-radius: 4px;
    letter-spacing: 0.04em;
  }
  .gravando {
    background: var(--no-ar);
    color: #fff;
  }
  .transmitindo {
    background: var(--tag-musica-bg);
    color: var(--tag-musica);
  }
  .problema {
    background: var(--laranja-bg);
    color: var(--laranja);
  }
  .desbloqueado {
    background: var(--painel-2);
    color: var(--texto-2);
  }
  .desbloqueado:hover {
    color: var(--texto);
    background: var(--borda-forte);
  }
  .ligando {
    font-size: 13px;
    color: var(--texto-3);
  }
  .tela-cheia {
    display: inline-flex;
    padding: 5px;
    background: transparent;
    border: none;
    color: var(--texto-3);
  }
  .tela-cheia:hover {
    color: var(--texto);
    background: var(--painel-2);
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
</style>
