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
    lista: "Limpa a fila e põe músicas novas das pastas, uma de cada, nesta ordem, sem repetir. Vale até a próxima lista. Também encerra um link.",
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
  const variasPastas = $derived(edicao?.tipo === "comercial" || edicao?.tipo === "ducking" || edicao?.tipo === "lista");
  // "Todos / Só um" só faz sentido para comercial e ducking.
  const modoPorPasta = $derived(edicao?.tipo === "comercial" || edicao?.tipo === "ducking");

  onMount(async () => {
    lista = await api.agendamentos();
  });

  const nomePasta = (c: string) => c.split(/[\\/]/).filter(Boolean).pop() ?? c;
  const tipoInfo = (t: TipoAgenda) => TIPOS.find((x) => x.tipo === t)!;

  /** Dias ativos por escrito: "Todos os dias", "Seg a Sex" ou "Seg · Qua". */
  function textoDias(dias: boolean[]) {
    if (dias.every(Boolean)) return "Todos os dias";
    if (dias.join() === "true,true,true,true,true,false,false") return "Seg a Sex";
    return DIAS_NOME.filter((_, i) => dias[i]).join(" · ");
  }

  /** Horários numa linha só: esconde os do fim que não cabem e mostra "+N". */
  function umaLinha(node: HTMLElement, _horarios: string[]) {
    const ajustar = () => {
      const itens = [...node.querySelectorAll<HTMLElement>(".h")];
      const mais = node.querySelector<HTMLElement>(".mais");
      if (!mais) return;
      itens.forEach((i) => (i.hidden = false));
      mais.textContent = "";
      for (let k = itens.length - 1; k > 0 && node.scrollWidth > node.clientWidth; k--) {
        itens[k].hidden = true;
        mais.textContent = `+${itens.length - k}`;
      }
    };
    const observador = new ResizeObserver(ajustar);
    observador.observe(node);
    ajustar();
    return { update: ajustar, destroy: () => observador.disconnect() };
  }

  const pastaHora = (a: Agendamento, k: ParteHora) => (k === "hora" ? (a.pastas[0]?.caminho ?? null) : (a[k] ?? null));

  function nomesDe(a: Agendamento) {
    const caminhos =
      a.tipo === "hora_certa"
        ? PARTES_HORA.map((p) => pastaHora(a, p.chave)).filter((c): c is string => !!c)
        : a.pastas.map((p) => p.caminho);
    if (a.tipo === "link") {
      const nomes = [a.abertura ? nomePasta(a.abertura) : null, a.url].filter(Boolean).join(" + ");
      return a.volume_db ? `${nomes} (${textoVolume(a.volume_db)})` : nomes;
    }
    return caminhos.map(nomePasta).join(" + ");
  }

  /** Ajuste de volume do link: de -30 a +12 dB, de 1 em 1. */
  function mudarVolume(passo: number) {
    if (!edicao) return;
    edicao.volume_db = Math.min(12, Math.max(-30, (edicao.volume_db ?? 0) + passo));
  }
  const textoVolume = (db: number) => (db === 0 ? "0 dB (como chega)" : `${db > 0 ? "+" : "−"}${Math.abs(db)} dB`);

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
    edicao = { id: 0, horarios: ["08:00"], dias: [true, true, true, true, true, true, true], tipo: "comercial", pastas: [], abertura: null, comercial_hora: null, intervalo: null, modo_lista: "depois_desta", url: null, volume_db: 0, ativo: true, pula_feriado: false, outras_datas: [] };
  }

  /** Nova data sem tocar, em branco: escolhe no calendário. Em branco não é salva. */
  function adicionarData() {
    if (!edicao) return;
    edicao.outras_datas = [...(edicao.outras_datas ?? []), ""];
  }

  /** Novo horário: uma hora depois do último (sem repetir um que já existe). */
  function adicionarHorario() {
    if (!edicao) return;
    const minutos = (h: string) => {
      const [hh, mm] = h.split(":").map(Number);
      return (hh || 0) * 60 + (mm || 0);
    };
    let m = edicao.horarios.length ? minutos(edicao.horarios[edicao.horarios.length - 1]) + 60 : 8 * 60;
    let novo = "";
    for (let i = 0; i < 24; i++, m += 60) {
      const t = ((m % 1440) + 1440) % 1440;
      novo = `${String(Math.floor(t / 60)).padStart(2, "0")}:${String(t % 60).padStart(2, "0")}`;
      if (!edicao.horarios.includes(novo)) break;
    }
    edicao.horarios.push(novo);
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
    // Hora certa usa uma pasta só.
    if (t === "hora_certa") edicao.pastas = edicao.pastas.slice(0, 1);
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
        <button class="painel linha ag" class:sel={edicao?.id === a.id} class:desativado={!a.ativo} onclick={() => editar(a)}>
          <span class="tag etiqueta {a.tipo}" title={t.nome}><Icone nome={t.icone} tamanho={18} /></span>
          <span class="meio">
            <span class="reticencias pastas">{nomesDe(a)}</span>
            {#if a.tipo === "hora_certa"}
              <span class="hora">A cada {a.intervalo ?? "–"} min</span>
            {:else}
              <span class="hora" title={a.horarios.join(" · ")} use:umaLinha={a.horarios}>
                {#each a.horarios as h, i}<span class="h">{i ? " · " : ""}{h}</span>{/each}<span class="mais"></span>
              </span>
            {/if}
            <span class="dias">{textoDias(a.dias)}</span>
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
      <div class="ativo">
        <span>Ativo</span>
        <button class="chave" class:on={edicao.ativo} onclick={() => (edicao!.ativo = !edicao!.ativo)} title={edicao.ativo ? "Ativado (clique para desativar)" : "Desativado (clique para ativar)"} aria-label="Ativar ou desativar">
          <span></span>
        </button>
      </div>

      {#if edicao.tipo === "hora_certa"}
        <div class="campo">Quando toca</div>
        <div class="intervalos">
          {#each INTERVALOS as i}
            <button class="intervalo" class:on={edicao.intervalo === i.minutos} onclick={() => (edicao!.intervalo = i.minutos)}>{i.nome}</button>
          {/each}
        </div>
      {:else}
        <div class="campo">Horários</div>
        <div class="horarios">
          {#each edicao.horarios as _, i}
            <div class="horario">
              <input type="time" bind:value={edicao.horarios[i]} />
              {#if edicao.horarios.length > 1}
                <button class="tirar" onclick={() => edicao!.horarios.splice(i, 1)} title="Tirar este horário" aria-label="Tirar este horário">
                  <Icone nome="x" tamanho={16} />
                </button>
              {/if}
            </div>
          {/each}
          <button class="quadrado mais-horario" onclick={adicionarHorario} title="Mais um horário" aria-label="Mais um horário">
            <Icone nome="plus" tamanho={20} />
          </button>
        </div>
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
        <div class="campo">Volume do link</div>
        <div class="volume">
          <button class="quadrado" onclick={() => mudarVolume(-1)} aria-label="Abaixar o volume" title="Abaixar 1 dB">
            <Icone nome="menos" tamanho={20} />
          </button>
          <span class="mono valor">{textoVolume(edicao.volume_db ?? 0)}</span>
          <button class="quadrado" onclick={() => mudarVolume(1)} aria-label="Aumentar o volume" title="Aumentar 1 dB">
            <Icone nome="plus" tamanho={20} />
          </button>
        </div>
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
      <div class="campo">{edicao.tipo === "lista" ? "Pastas (uma música de cada, nesta ordem)" : variasPastas ? "Pastas (tocam nesta ordem)" : "Pasta"}</div>
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
          {#if modoPorPasta}
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

      <div class="ativo feriado">
        <span>Não toca em feriado</span>
        <button class="chave" class:on={edicao.pula_feriado} onclick={() => (edicao!.pula_feriado = !edicao!.pula_feriado)} title={edicao.pula_feriado ? "Não toca em feriado (clique para tocar)" : "Toca em feriado (clique para não tocar)"} aria-label="Não tocar em feriado">
          <span></span>
        </button>
      </div>
      {#if edicao.pula_feriado}
        <div class="dica feriados">Já pula 1/1, 21/4, 1/5, 7/9, 12/10, 2/11, 15/11, 20/11 e 25/12.</div>
        <div class="campo">Outras datas</div>
        <div class="horarios">
          {#each edicao.outras_datas ?? [] as _, i}
            <div class="horario">
              <input type="date" class="data" bind:value={edicao.outras_datas![i]} />
              <button class="tirar" onclick={() => edicao!.outras_datas!.splice(i, 1)} title="Tirar esta data" aria-label="Tirar esta data">
                <Icone nome="x" tamanho={16} />
              </button>
            </div>
          {/each}
          <button class="quadrado mais-horario" onclick={adicionarData} title="Mais uma data" aria-label="Mais uma data">
            <Icone nome="plus" tamanho={20} />
          </button>
        </div>
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
    grid-template-columns: 34px minmax(0, 1fr);
  }
  .ativo {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 14px;
    font-size: 15px;
  }
  .ag.desativado {
    opacity: 0.45;
  }
  .ag:hover {
    background: var(--painel-2);
  }
  .ag.sel {
    background: #222836;
    border-color: #4a6aa5;
  }
  /* Fonte do projeto; números com a mesma largura para os horários alinharem. */
  .meio {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .hora {
    font-size: 14px;
    font-weight: 500;
    color: #fff;
    font-variant-numeric: tabular-nums;
    line-height: 1.4;
    white-space: nowrap;
    overflow: hidden;
  }
  .hora .mais:not(:empty) {
    margin-left: 6px;
    padding: 0 6px;
    border-radius: 6px;
    background: var(--painel-2);
    color: var(--texto-2);
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
    font-size: 13px;
    color: var(--texto-3);
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
  .horarios {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-bottom: 14px;
  }
  .horario {
    display: flex;
    align-items: center;
    height: 40px;
    border: 0.5px solid var(--borda-forte);
    border-radius: 8px;
    overflow: hidden;
  }
  .horario input {
    font-variant-numeric: tabular-nums;
    width: 110px;
    height: 100%;
    font-size: 16px;
    border: none;
    border-radius: 0;
  }
  .horario .tirar {
    width: 32px;
    height: 100%;
    padding: 0;
    border: none;
    border-left: 0.5px solid var(--borda-forte);
    border-radius: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--texto-3);
  }
  .horario .tirar:hover {
    color: #fff;
    background: var(--no-ar);
  }
  .horario input.data {
    width: 150px;
  }
  .ativo.feriado {
    margin-top: 18px;
  }
  .dica.feriados {
    margin: -6px 0 10px;
  }
  .mais-horario {
    width: 40px;
    height: 40px;
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
  .volume {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }
  .volume .quadrado {
    width: 40px;
    height: 40px;
  }
  .volume .valor {
    min-width: 150px;
    text-align: center;
    font-size: 17px;
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
