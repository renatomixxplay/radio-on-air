<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Agendamento, type TipoAgenda } from "./api";
  import Icone, { type NomeIcone } from "./Icone.svelte";

  const DIAS = ["S", "T", "Q", "Q", "S", "S", "D"];
  const DIAS_NOME = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];
  const TIPOS: { tipo: TipoAgenda; nome: string; icone: NomeIcone }[] = [
    { tipo: "lista", nome: "Lista", icone: "music" },
    { tipo: "comercial", nome: "Comercial", icone: "megafone" },
    { tipo: "ducking", nome: "Com ducking", icone: "ducking" },
    { tipo: "hora_certa", nome: "Hora certa", icone: "relogio" },
    { tipo: "link", nome: "Link", icone: "aovivo" },
  ];
  const DICA: Record<TipoAgenda, string> = {
    lista: "Limpa a fila e põe músicas novas desta pasta. Vale até a próxima lista. Também encerra um link.",
    comercial: "Espera a música acabar e toca as pastas, nesta ordem.",
    ducking: "Entra por cima da música, que abaixa enquanto tem voz.",
    hora_certa: "Só existe uma. Na hora cheia (ou a cada 30 min), espera a música acabar e toca a abertura, a hora (HRS08 + MIN10) e o comercial. Todos opcionais.",
    link: "No horário, a música faz fade e entra o link (MP3/AAC), depois da abertura. Toca até a próxima lista ou link. Se cair, fica em silêncio tentando reconectar. Comercial e hora certa no meio são descartados.",
  };
  const INTERVALOS = [
    { minutos: 60, nome: "Hora cheia" },
    { minutos: 30, nome: "30 min" },
  ];
  type ParteHora = "abertura" | "hora" | "comercial_hora";
  const PARTES_HORA: { chave: ParteHora; nome: string }[] = [
    { chave: "abertura", nome: "Abertura da hora" },
    { chave: "hora", nome: "Hora" },
    { chave: "comercial_hora", nome: "Comercial da hora" },
  ];

  let lista = $state<Agendamento[]>([]);
  let filtro = $state(-1); // -1 = todos os dias
  let edicao = $state<Agendamento | null>(null);
  let erro = $state("");
  let arrastando = $state<number | null>(null);

  const visiveis = $derived(filtro < 0 ? lista : lista.filter((a) => a.dias[filtro]));
  const variasPastas = $derived(edicao?.tipo === "comercial" || edicao?.tipo === "ducking");

  onMount(async () => {
    lista = await api.agendamentos();
  });

  const nomePasta = (c: string) => c.split("/").filter(Boolean).pop() ?? c;
  const tipoInfo = (t: TipoAgenda) => TIPOS.find((x) => x.tipo === t)!;

  const pastaHora = (a: Agendamento, k: ParteHora) => (k === "hora" ? (a.pastas[0]?.caminho ?? null) : (a[k] ?? null));

  function nomesDe(a: Agendamento) {
    const caminhos =
      a.tipo === "hora_certa"
        ? PARTES_HORA.map((p) => pastaHora(a, p.chave)).filter((c): c is string => !!c)
        : a.pastas.map((p) => p.caminho);
    if (a.tipo === "link") return [a.abertura ? nomePasta(a.abertura) : null, a.url].filter(Boolean).join(" + ");
    return caminhos.map(nomePasta).join(" + ");
  }

  function definirHora(k: ParteHora, caminho: string | null) {
    if (!edicao) return;
    if (k === "hora") edicao.pastas = caminho ? [{ caminho, modo: "todos" }] : [];
    else edicao[k] = caminho;
  }

  async function escolherHora(k: ParteHora) {
    const p = await open({ directory: true, multiple: false, title: "Escolher pasta" });
    if (typeof p === "string") definirHora(k, p);
  }

  function editar(a: Agendamento) {
    erro = "";
    edicao = structuredClone($state.snapshot(a)) as Agendamento;
  }

  function novo() {
    erro = "";
    edicao = { id: 0, horario: "08:00", dias: [true, true, true, true, true, true, true], tipo: "comercial", pastas: [], abertura: null, comercial_hora: null, intervalo: null, modo_lista: "depois_desta", url: null };
  }

  async function anexar() {
    if (!edicao) return;
    const p = await open({ directory: true, multiple: false, title: "Escolher pasta" });
    if (typeof p !== "string") return;
    if (variasPastas) edicao.pastas.push({ caminho: p, modo: "todos" });
    else edicao.pastas = [{ caminho: p, modo: "todos" }];
  }

  function trocarTipo(t: TipoAgenda) {
    if (!edicao) return;
    edicao.tipo = t;
    // Lista e hora certa usam uma pasta só.
    if (t === "lista" || t === "hora_certa") edicao.pastas = edicao.pastas.slice(0, 1);
    if (t === "hora_certa" && !INTERVALOS.some((i) => i.minutos === edicao!.intervalo)) edicao.intervalo = 60;
    if (t === "link") edicao.pastas = [];
  }

  async function salvar() {
    if (!edicao) return;
    try {
      lista = await api.agendamentoSalvar($state.snapshot(edicao) as Agendamento);
      edicao = null;
      erro = "";
    } catch (e) {
      erro = String(e);
    }
  }

  async function remover() {
    if (!edicao || edicao.id === 0) return (edicao = null);
    lista = await api.agendamentoRemover(edicao.id);
    edicao = null;
  }

  function soltarPasta(i: number) {
    if (!edicao || arrastando == null || arrastando === i) return;
    const [p] = edicao.pastas.splice(arrastando, 1);
    edicao.pastas.splice(i, 0, p);
    arrastando = null;
  }
</script>

<div class="tela">
  <div class="coluna">
  <div class="cabeca">
    <h3 class="grupo">Agendamentos</h3>
    <button class="botao" onclick={novo}><Icone nome="plus" tamanho={16} /> Novo</button>
  </div>
  <div class="filtro">
      <button class="chip" class:on={filtro < 0} onclick={() => (filtro = -1)}>Todos</button>
      {#each DIAS_NOME as d, i}
        <button class="chip" class:on={filtro === i} onclick={() => (filtro = i)}>{d}</button>
      {/each}
  </div>
  <div class="lista cartoes">
      {#each visiveis as a (a.id)}
        {@const t = tipoInfo(a.tipo)}
        <button class="painel linha ag" class:sel={edicao?.id === a.id} onclick={() => editar(a)}>
          <span class="hora">{a.tipo === "hora_certa" ? `${a.intervalo ?? "–"} min` : a.horario}</span>
          <span class="tag etiqueta {a.tipo}" title={t.nome}><Icone nome={t.icone} tamanho={18} /></span>
          <span class="reticencias pastas">{nomesDe(a)}</span>
          <span class="dias">
            {#each DIAS as d, i}<span class="d" class:on={a.dias[i]}>{d}</span>{/each}
          </span>
        </button>
      {:else}
        <div class="painel vazio">Nenhum agendamento {filtro >= 0 ? "neste dia" : "ainda"}.</div>
      {/each}
  </div>
  </div>

  <div class="coluna">
  <div class="cabeca">
    <h3 class="grupo">{edicao ? (edicao.id === 0 ? "Novo agendamento" : "Editar agendamento") : "Detalhes"}</h3>
    {#if edicao}
      <button class="quadrado lixo" onclick={remover} title="Excluir" aria-label="Excluir">
        <Icone nome="lixeira" tamanho={18} />
      </button>
    {/if}
  </div>
  <section class="painel cartao editor">
    {#if edicao}

      {#if edicao.tipo === "hora_certa"}
        <div class="campo">Quando toca</div>
        <div class="intervalos">
          {#each INTERVALOS as i}
            <button class="intervalo" class:on={edicao.intervalo === i.minutos} onclick={() => (edicao!.intervalo = i.minutos)}>{i.nome}</button>
          {/each}
        </div>
      {:else}
        <div class="campo">Horário</div>
        <input class="horario" type="time" bind:value={edicao.horario} />
      {/if}

      <div class="campo">Dias</div>
      <div class="dias-grandes">
        {#each DIAS as d, i}
          <button class="dia" class:on={edicao.dias[i]} title={DIAS_NOME[i]} onclick={() => (edicao!.dias[i] = !edicao!.dias[i])}>
            {d}
          </button>
        {/each}
      </div>

      <div class="campo">Tipo</div>
      <div class="tipos">
        {#each TIPOS as t}
          <button class="tipo" class:on={edicao.tipo === t.tipo} onclick={() => trocarTipo(t.tipo)}>
            <span class="tag etiqueta {t.tipo}"><Icone nome={t.icone} tamanho={18} /></span>
            {t.nome}
          </button>
        {/each}
      </div>
      <div class="dica">{DICA[edicao.tipo]}</div>

      {#if edicao.tipo === "link"}
        <div class="campo">Endereço do link (MP3 ou AAC)</div>
        <input class="url" type="url" placeholder="https://…" bind:value={edicao.url} spellcheck="false" />
        <div class="campo">Abertura</div>
        {#if edicao.abertura}
          <div class="pasta">
            <span class="cor-pasta"><Icone nome="folder" tamanho={18} /></span>
            <span class="reticencias nome" title={edicao.abertura}>{nomePasta(edicao.abertura)}</span>
            <button class="quadrado pequeno" onclick={() => (edicao!.abertura = null)} aria-label="Tirar pasta">
              <Icone nome="x" tamanho={16} />
            </button>
          </div>
        {:else}
          <button class="anexar parte" onclick={() => escolherHora("abertura")}>
            <Icone nome="plus" tamanho={16} /> Escolher pasta
          </button>
        {/if}
      {:else if edicao.tipo === "hora_certa"}
        {#each PARTES_HORA as parte (parte.chave)}
          {@const caminho = pastaHora(edicao, parte.chave)}
          <div class="campo">{parte.nome}</div>
          {#if caminho}
            <div class="pasta">
              <span class="cor-pasta"><Icone nome="folder" tamanho={18} /></span>
              <span class="reticencias nome" title={caminho}>{nomePasta(caminho)}</span>
              <button class="quadrado pequeno" onclick={() => definirHora(parte.chave, null)} aria-label="Tirar pasta">
                <Icone nome="x" tamanho={16} />
              </button>
            </div>
          {:else}
            <button class="anexar parte" onclick={() => escolherHora(parte.chave)}>
              <Icone nome="plus" tamanho={16} /> Escolher pasta
            </button>
          {/if}
        {/each}
      {:else}
      <div class="campo">{variasPastas ? "Pastas (tocam nesta ordem)" : "Pasta"}</div>
      {#each edicao.pastas as p, i (p.caminho + i)}
        <div
          class="pasta"
          class:arrastando={arrastando === i}
          draggable={variasPastas}
          ondragstart={() => (arrastando = i)}
          ondragover={(e) => e.preventDefault()}
          ondrop={() => soltarPasta(i)}
          ondragend={() => (arrastando = null)}
          role="listitem"
        >
          {#if variasPastas}<span class="pega"><Icone nome="grip" tamanho={18} /></span>{/if}
          <span class="cor-pasta"><Icone nome="folder" tamanho={18} /></span>
          <span class="reticencias nome" title={p.caminho}>{nomePasta(p.caminho)}</span>
          {#if variasPastas}
            <span class="seg">
              <button class:on={p.modo === "todos"} onclick={() => (p.modo = "todos")}>Todos</button>
              <button class:on={p.modo === "um"} onclick={() => (p.modo = "um")} title="Um arquivo por vez: com data primeiro, depois rodízio">Só um</button>
            </span>
          {/if}
          <button class="quadrado pequeno" onclick={() => edicao!.pastas.splice(i, 1)} aria-label="Tirar pasta">
            <Icone nome="x" tamanho={16} />
          </button>
        </div>
      {/each}
      {#if variasPastas || edicao.pastas.length === 0}
        <button class="anexar" onclick={anexar}><Icone nome="plus" tamanho={16} /> Anexar pasta</button>
      {:else}
        <button class="anexar" onclick={anexar}><Icone nome="folderOpen" tamanho={16} /> Trocar pasta</button>
      {/if}
      {#if edicao.tipo === "lista"}
        <div class="campo espaco">Ao entrar</div>
        <div class="modos">
          <button class="modo" class:on={(edicao.modo_lista ?? "depois_desta") === "depois_desta"} onclick={() => (edicao!.modo_lista = "depois_desta")} title="Espera a música atual acabar">
            Aguardar
          </button>
          <button class="modo" class:on={edicao.modo_lista === "fade"} onclick={() => (edicao!.modo_lista = "fade")} title="Fade de 3 s na música atual e já começa">Imediato</button>
        </div>
      {/if}
      {/if}

      {#if erro}<div class="erro">{erro}</div>{/if}

      <div class="acoes">
        <button class="botao" onclick={() => (edicao = null)}>Cancelar</button>
        <button class="botao salvar" onclick={salvar}>Salvar</button>
      </div>
    {:else}
      <div class="vazio vazio-editor">
        <Icone nome="calendar" tamanho={32} />
        <p>Escolha um agendamento na lista<br />ou crie um novo.</p>
      </div>
    {/if}
  </section>
  </div>
</div>

<style>
  .tela {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
    gap: 14px;
    padding: 10px 10px 14px;
  }
  .coluna {
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  /* Título fora do cartão, como em Configurações. */
  .cabeca {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 38px;
    margin: 0 4px 8px;
  }
  .grupo {
    margin: 0;
    font-size: 15px;
    font-weight: 500;
  }
  .cartao {
    flex: 1;
    overflow: hidden;
  }
  .filtro {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0 0 10px;
  }
  .cartoes {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-bottom: 4px;
  }
  .chip {
    padding: 5px 12px;
    border-radius: 999px;
    color: var(--texto-2);
  }
  .chip.on {
    background: #2b3a55;
    border-color: #4a6aa5;
    color: #cfe0ff;
  }
  .ag {
    width: 100%;
    flex: none;
    flex-direction: row;
    padding: 10px 12px;
    border: 0.5px solid var(--borda);
    border-radius: 8px;
    background: var(--painel);
    text-align: left;
    display: grid;
    grid-template-columns: 56px 34px minmax(0, 1fr) auto;
  }
  .ag:hover {
    background: var(--painel-2);
  }
  .ag.sel {
    background: #222836;
    border-color: #4a6aa5;
  }
  /* Fonte do projeto; números com a mesma largura para os horários alinharem. */
  .hora {
    font-size: 15px;
    font-weight: 500;
    color: #fff;
    font-variant-numeric: tabular-nums;
  }
  .etiqueta {
    width: 34px;
    height: 34px;
    padding: 0;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
  }
  .tag.lista {
    background: var(--tag-musica-bg);
    color: var(--tag-musica);
  }
  .tag.ducking {
    background: var(--tag-comercial-bg);
    color: var(--tag-comercial);
  }
  .dias {
    display: inline-flex;
    gap: 4px;
  }
  .d {
    width: 26px;
    height: 26px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    border-radius: 6px;
    border: 0.5px solid var(--borda-forte);
    color: var(--apagado);
  }
  .d.on {
    background: #2b3a55;
    border-color: #4a6aa5;
    color: #cfe0ff;
  }
  .botao {
    height: 38px;
    padding: 0 14px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .editor {
    padding: 14px;
    overflow-y: auto;
  }
  .quadrado {
    width: 34px;
    height: 34px;
    padding: 0;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--texto-2);
    flex: none;
  }
  .quadrado.pequeno {
    width: 30px;
    height: 30px;
  }
  .lixo:hover {
    color: #fff;
    background: var(--no-ar);
    border-color: var(--no-ar);
  }
  .campo {
    color: var(--texto-3);
    font-size: 13px;
    margin: 0 0 6px;
  }
  .horario {
    font-variant-numeric: tabular-nums;
    width: 130px;
    height: 40px;
    font-size: 16px;
    margin-bottom: 14px;
    border-color: var(--borda-forte);
    border-radius: 8px;
  }
  .intervalos {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 14px;
  }
  .intervalo {
    height: 36px;
    padding: 0 12px;
    border-radius: 8px;
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }
  .intervalo.on {
    background: #2b3a55;
    border-color: #4a6aa5;
    color: #cfe0ff;
  }
  .dias-grandes {
    display: flex;
    gap: 6px;
    margin-bottom: 14px;
  }
  .dia {
    width: 36px;
    height: 36px;
    border-radius: 8px;
    color: var(--apagado);
  }
  .dia.on {
    background: #2b3a55;
    border-color: #4a6aa5;
    color: #cfe0ff;
  }
  .tipos {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 6px;
  }
  .tipo {
    padding: 8px 4px;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    font-size: 13px;
    color: var(--texto-2);
  }
  .tipo.on {
    border-color: #4a6aa5;
    background: #1e2533;
    color: #fff;
  }
  .dica {
    color: var(--texto-3);
    font-size: 13px;
    margin: 8px 0 14px;
  }
  .pasta {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--campo);
    border: 0.5px solid var(--borda-forte);
    border-radius: 8px;
    padding: 6px 8px;
    margin-bottom: 6px;
  }
  .pasta.arrastando {
    opacity: 0.4;
  }
  .pega {
    color: var(--texto-3);
    display: flex;
    cursor: grab;
  }
  .cor-pasta {
    color: var(--pasta);
    display: flex;
  }
  .nome {
    flex: 1;
  }
  .seg {
    display: flex;
    border: 0.5px solid var(--borda-forte);
    border-radius: 6px;
    overflow: hidden;
    flex: none;
  }
  .seg button {
    border: none;
    border-radius: 0;
    background: transparent;
    font-size: 12px;
    padding: 4px 9px;
    color: var(--texto-3);
  }
  .seg button.on {
    background: #2b3a55;
    color: #cfe0ff;
  }
  .anexar {
    width: 100%;
    height: 38px;
    border-style: dashed;
    border-radius: 8px;
    background: transparent;
    color: var(--texto-2);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }
  .anexar.parte {
    margin-bottom: 14px;
  }
  .pasta + .campo {
    margin-top: 8px;
  }
  .tag.link {
    background: #4a1f24;
    color: #ff9b9b;
  }
  .url {
    width: 100%;
    height: 40px;
    font-size: 15px;
    margin-bottom: 14px;
    border-color: var(--borda-forte);
    border-radius: 8px;
  }
  .espaco {
    margin-top: 14px;
  }
  .modos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
  .modo {
    height: 38px;
    border-radius: 8px;
    color: var(--texto-2);
  }
  .modo.on {
    border-color: #4a6aa5;
    background: #1e2533;
    color: #fff;
  }
  .erro {
    margin-top: 10px;
    color: #ff9b9b;
    font-size: 13px;
  }
  .acoes {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  .salvar {
    background: #238636;
    border-color: #2ea043;
    color: #fff;
  }
  .vazio-editor {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }
</style>
