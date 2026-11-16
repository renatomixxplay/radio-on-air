# Radio On Air — Escopo

Software de automação de rádio (rádio DJ) para Linux, open source, feito para **rádios comunitárias**.

> Documento vivo. Cada recurso só entra depois de aprovado.
> Legenda: ✅ feito · 💡 sugestão (aguardando aprovação) · ❓ pendente / a decidir · 🧪 feito, falta testar

**Situação em 15/11/2026:** Fases 1 e 2 prontas e rodando no Mac. Próximo passo: **testar tudo com calma** (seção 12) antes da Fase 3.

---

## 1. Princípios

- ✅ **Leve** — precisa rodar bem em PCs modestos e antigos.
- ✅ **Programa de desktop** — janela própria, instalado no computador (não é site/navegador).
- ✅ **Arquivos locais** — músicas e áudios ficam em pastas do próprio computador.
- ✅ **Sem banco de dados** — tudo em pastas + arquivos de texto. Backup = copiar a pasta.
- ✅ **Open source, licença GPL-3.0.**
- ✅ **Tema escuro**, visual simples: ícones no lugar de texto, letras grandes, uma fonte só (a do sistema).
- ✅ Interface só em **português**.
- ✅ Microfone fica na **mesa de som** (o programa não captura microfone).

## 2. Tecnologia

| Parte | Escolha | Por quê |
|---|---|---|
| Janela do programa | **Tauri 2** (Rust) | Instalador pequeno, usa pouca memória |
| Tela | **Svelte 5** (SvelteKit estático) | Telas pequenas e rápidas |
| Motor de áudio | **Liquidsoap 2.4** (roda escondido) | Feito para rádio: ducking, Icecast, gravação, 24h estável |
| Dados | Arquivos de texto/JSON | Fácil de ler, editar e fazer backup |

- ✅ Alvo: Linux (Ubuntu/Debian) — `.deb` e AppImage.
- 💡 Possivelmente **Windows** no futuro (`.exe`/`.msi`; o Liquidsoap tem versão Windows).
- ✅ Desenvolvimento no **macOS** (Intel): Liquidsoap compilado isolado em `~/.opam` (sem Homebrew mexer no sistema).
- ✅ O motor de áudio **liga sozinho ao abrir o programa**, em segundo plano (no Mac leva ~4 s).
- ✅ **Motor que se religa sozinho:** se o motor cair (placa USB solta, driver, arquivo estragado, antivírus…), o aviso vermelho mostra o motivo com "religando…" e em **3 s** ele volta. Se a rádio estava no ar, continua no ar com a **próxima** da fila (a que tocava não volta, porque pode ter sido ela que derrubou o motor); link volta a conectar e agendados pendentes seguem. Se o motor não conseguir ligar com a rádio no ar, tenta de novo **a cada 10 s** sem tirar do ar. Estava pausada: volta parada. Testado no Mac derrubando o motor duas vezes seguidas.
- ✅ **Fechou o programa, fecha o motor** e a gravação/transmissão.
- ✅ **Tela cheia:** botão (só ícone) no topo, depois do relógio, e tecla **F11** (no Mac o sistema usa o F11; lá só o botão). Em Configurações, chave **"Abrir em tela cheia"** (desligada por padrão).
- ✅ No Mac a saída tem uma folga de **1 s de áudio** contra travadinhas (no Linux não precisa).

### Arquivos de dados

Ficam em `~/.local/share/org.radioonair.app/` (Linux), `~/Library/Application Support/org.radioonair.app/` (Mac) ou `C:\ProgramData\RadioOnAir\` (Windows):

| Arquivo | Conteúdo |
|---|---|
| `config.json` | Pasta de músicas, pastas, cartucheira, tocar ao abrir, senha (hash) e bloqueios |
| `indice.txt` | Lista de nomes das músicas (o programa abre na hora mesmo com 100 mil) |
| `agendamentos.json` | Agendamentos |
| `rodizio.json` | Último arquivo tocado de cada pasta no modo "só um" |
| `motor.liq` / `motor.log` | Script e avisos do motor de áudio (diagnóstico; nível 3: músicas preparadas, trocas de fonte, fim de arquivo, erros). Recomeça a cada abertura |
| `historico.log` | O que acontece no ar, com horário até o milésimo: o que o programa manda ao motor, os avisos do motor, agendamentos, ducking, fades, botões e por que o programa não entregou a próxima. Continua entre aberturas; passando de 5 MB vira `historico.anterior.log` |
| `captura.liq` / `captura.log` | Script e avisos da gravação/transmissão (diagnóstico) |

### Windows
- Instalador `.exe` gerado pelo **GitHub Actions** (aba Actions → "Instalador Windows" → Run workflow); o arquivo fica em *Artifacts* no fim da execução.
- O **Liquidsoap 2.4.6 para Windows vai dentro do instalador** (pasta de recursos `liquidsoap/`): não precisa instalar nada além do programa.
- Som pelo **PortAudio** (saída, pré-escuta e entrada escolhidas pelo nome da placa); gravação e transmissão em MP3 pelo **FFmpeg** (o Liquidsoap do Windows não tem o codificador lame). Mesma folga de 1 s do Mac contra travadinhas.
- Liquidsoap e consultas de placas abrem **sem janela de terminal**; motor antigo preso é fechado com `tasklist`/`taskkill`.
- Instala em **Arquivos de Programas** (`C:\Program Files\Radio On Air`, pede administrador uma vez). **Todos os dados** (configurações, agenda, índice, motor, registros, cache do Liquidsoap) ficam em `C:\ProgramData\RadioOnAir`. Quem usava a versão que guardava em `AppData\Roaming\org.radioonair.app` tem tudo trazido sozinho na primeira vez.
- Fechou o programa, fecha o motor (e gravação/transmissão). No Windows o Liquidsoap fica "amarrado" ao programa (Job Object): se o programa cair ou for encerrado no Gerenciador de Tarefas, o Windows fecha o motor junto.
- Caminhos no Windows usam `\`: o código compara caminhos por partes (`Path::starts_with`), nunca procurando `/` no texto.
- **Caminhos com acento** (ex.: usuário do Windows "Rádio Cidade"): o Liquidsoap de Windows não abre arquivos com acento no caminho. Se alguém instalar numa pasta com acento, o motor roda de uma cópia em `C:\ProgramData\RadioOnAir\liquidsoap` (scripts e registros também ficam em `C:\ProgramData\RadioOnAir`), e músicas/pastas com acento vão para ele pelo **nome curto** do Windows (8.3, sem acento), traduzido de volta para a tela.
- O aviso vermelho mostra a mensagem real do motor quando ele não abre.
- Testado: instala, abre e toca som (usuário com acento).

## 3. Telas

Navegação no topo: **No ar · Agendamentos · Pastas · Cartucheira · Configurações**.
Todas as abas de configuração seguem o mesmo estilo: **título acima, cartões separados embaixo**.

**Abertura:** ao abrir o programa, uma janela "Seja bem-vindo" com o nome da rádio, uma barra que enche em 4 partes e a etapa atual escrita embaixo (configurações, músicas, procurando músicas novas, motor de áudio por último). A rádio só começa a tocar depois da procura de músicas novas (ou de 15 s, se o Drive travar: aí usa o que tem e termina em segundo plano). Some sozinha quando tudo fica pronto (fica no mínimo 1,5 s, para não piscar). Se o motor não ligar, mostra o erro, os botões **Reiniciar** (fecha e abre o programa) e **Fechar**; eles também aparecem se demorar mais de 30 s.

### 3.1 No ar ✅

```
┌──────────────────────────────────────────────────────┐
│ Radio On Air   abas…        [ligando o áudio…] NO AR 14:32:07 │
├─────────────────────────────────┬────────────────────┤
│ 📅 agendamento (atual/próximo)  │ Cartucheira (8)    │
│ Música / artista / barra / tempo│                    │
│   🎙  ▶/⏸  🔁  ⏭               │                    │
├────────────┬────────────────────┴──┬─────────────────┤
│ Músicas    │ Fila (até 10)         │ Pastas          │
└────────────┴───────────────────────┴─────────────────┘
```

- **Topo:** NO AR / FORA DO AR, relógio grande, "ligando o áudio…" enquanto o motor sobe, cadeado quando a senha está desbloqueada.
- **Tocando agora:**
  - No lugar do título, uma linha 📅 de agendamento (amarelo transparente): **Por cima:** … / **Depois desta:** … / **Próximo:** HH:MM … / "Nenhum agendamento".
  - Música e artista, **barra de progresso** (vermelha nos 10 s finais), tempo tocado e o que falta.
  - Botões só com ícone: **Ducking · Tocar/Pausar · Repetir · Passar** (sem Parar).
  - **Travados** (apagados, e o programa também ignora): com **comercial ou hora certa** tocando e durante o **ducking** (voz por cima, música repetindo e os 10 s até a troca), ficam travados **todos** — Ducking, Pausar, Repetir e Passar. Com o **link** (Voz do Brasil): Ducking, Repetir e Passar (Pausar continua). Sempre dá para **desligar** o que já estava ligado (Ducking, Repetir) e o **Tocar** de uma rádio pausada nunca trava.
  - **Ducking:** abaixa a música para 20% (rampa suave); a cartucheira não abaixa.
  - **Repetir:** a música atual toca de novo no fim; a fila espera.
  - **Passar** não funciona durante comercial/hora certa (agendamento não se pula).
- **Cartucheira:** 8 botões fixos; só toca (configuração na aba Cartucheira).
- **Músicas:** busca por música ou artista (sem acento/maiúscula), 🎧 pré-escuta e ➕ adicionar. Ícone de pasta troca a pasta de músicas.
- **Fila:** ícone colorido por tipo (🎵 verde música, 🎙 amarelo pasta/programa, ✨ roxo vinheta), duração, lixeira, arrastar pelos 6 pontinhos para reordenar. Topo mostra a lista de músicas ativa e "N/10 · total".
- **Pastas:** seletor com as pastas da aba Pastas (ordem alfabética) e os arquivos delas, com 🎧 e ➕.
- **Pré-escuta:** o 🎧 vira ■ e uma barrinha mostra o andamento; **clicar na linha pula** para aquele ponto. Toca numa saída separada e não mexe no que está no ar (por enquanto na mesma placa de som).
- Nas listas não se arrasta: adiciona-se só pelo ➕.

### 3.2 Agendamentos ✅

- Lista (cada agendamento em um cartão): ícone do tipo, pastas com os horários embaixo, numa linha só (08:00 · 12:00 · 18:00 e "+N" para os que não cabem) e, embaixo, só os dias ativos (Seg · Qua · Sex; "Todos os dias" e "Seg a Sex" quando for o caso).
- Filtro por dia da semana; botão **Novo**.
- Editor ao lado: horários (em fileira, cada um com ✕ para tirar e **+** para incluir outro; os dias e pastas valem para todos), dias, tipo (Lista · Comercial · Com ducking · Hora certa), pastas anexadas (arrastar para reordenar, **Todos / Só um**), salvar e excluir.

### 3.3 Pastas ✅
- Pastas que aparecem no painel Pastas do No ar (adicionar várias de uma vez, tirar). Ordem alfabética.

### 3.4 Cartucheira ✅
- 8 cartões (2 colunas): número, nome do áudio, **Escolher** e lixeira.

### 3.5 Configurações ✅ (parte)
- ✅ **Entrar no ar sozinho** ao abrir o programa.
- ✅ Pasta de músicas (**Trocar**) e **Atualizar** (relê o acervo).
- ✅ **Nome da rádio**: aparece no topo (sem ícone) e na transmissão.
- ✅ **Placa de som**, **Gravação** e **Transmissão** (ver seção 7).
- ✅ **Senha** (criar, trocar, remover) e **bloqueios** por área: Agendamentos, Pastas, Cartucheira, Configurações. Desbloqueia uma vez e vale até apertar o cadeado. Proteção contra mexida sem querer — não é segurança forte.
- ❓ Ainda por alinhar: saídas de áudio / placa da pré-escuta, transmissão, gravação, regras da fila.

## 4. Músicas e índice ✅

- Acervo de ~**100 mil músicas**, em **subpastas**.
- Padrão de nome `Artista - Música.mp3`; fora do padrão, o nome inteiro vira a música e o artista fica em branco.
- Busca em memória só pelos **nomes** dos arquivos. Palavras em qualquer ordem; traço e outros sinais são ignorados ("jorge - amor" = "amor jorge"). Mostra até 200, **as mais certeiras primeiro**: música ou artista que começa com o que foi digitado, depois palavra que começa com a primeira palavra digitada, depois o resto.
- `indice.txt` para abrir na hora; atualização **automática** quando entra/sai/renomeia música; botão **Atualizar** se der erro. **Ao abrir**, depois de carregar o `indice.txt`, relê a pasta inteira procurando mudanças (só os nomes, ~1–2 s) e atualiza o índice se algo mudou — no Google Drive o Windows nem sempre avisa. A rádio espera essa procura para começar a tocar, no máximo **15 s**.
- Pasta de **lista** que o índice ainda não conhece (pasta nova) é lida **direto do disco**: a lista nunca cai no acervo todo por isso.
- Duração lida do **cabeçalho** do arquivo só quando ele entra na fila (sem decodificar o áudio).
- ❓ Linux com muitas subpastas pode precisar aumentar `fs.inotify.max_user_watches` na instalação.

## 5. Fila ✅

- Até **10 itens**; o automático garante o **mínimo de 3** (quando cai para 3, puxa mais 1).
- O automático puxa da **lista de músicas ativa** (agendamento tipo Lista) ou do acervo inteiro, em **baralho**: a pasta é embaralhada e as músicas saem na ordem do baralho; só quando **todas** tocaram ela é embaralhada de novo (a última da rodada não abre a seguinte). Ou seja, **nenhuma música repete antes de todas as outras da pasta**. Um baralho por pasta, só na memória (ao abrir o programa, embaralha de novo). Música apagada depois de embaralhar é pulada; música nova entra na rodada seguinte. Música posta na fila **à mão** não segue a regra. Pasta da lista vazia ou apagada: baralho do acervo todo. Pasta fora do acervo: lida direto do disco.
- No topo da fila: **Fila · Playlist _nome da pasta_** (a lista valendo agora).
- 💡 Não repetir o mesmo artista por X minutos.
- 💡 Crossfade entre músicas.

## 6. Agendamentos ✅

Por **horário + dias da semana**. O programa só preenche/executa — não existe "sequência/receita".

| Tipo | Comportamento |
|---|---|
| 🎵 **Lista de músicas** | **Limpa a fila** e põe músicas novas (aleatório) da pasta. Duas opções: **Aguardar** (a música atual termina) ou **Imediato** (fade de 3 s e a lista já começa; comercial/hora certa tocando não levam fade). Vale até a próxima lista. Ao abrir no meio do dia, vale a última lista que já passou. Também **encerra o link** (fade de 3 s). |
| 📢 **Comercial** | Espera a música acabar e toca as pastas anexadas. |
| 🎙 **Comercial com ducking** | Entra **por cima**: a música cai para 20% quando há voz (desce em ~0,6 s) e volta **suave em ~2 s** nos silêncios, em degraus iguais de volume (antes subia num salto de ~10 dB). Entra na hora só se estiver tocando música e a próxima **ainda não foi entregue** ao motor; senão (últimos 5 s, comercial ou vinheta tocando) **espera a próxima música e entra 10 s depois** que ela começar (descartado se esperar mais de 10 min ou entrar um link). Se o bloco **não cabe** no que falta da música, ela **repete por baixo**; quando a fala acaba, a música volta ao normal, **toca mais 10 s** e sai com **fade de 3 s** para a próxima (se a música mudar nesse meio tempo, não troca). O volume do fade vale **antes** do crossfade: o fim da música que sai continua baixo enquanto a próxima entra (antes ela voltava alta por ~0,6 s — o "pulo"). No fim do fade (e no **Passar**) o programa **já manda a próxima** ao motor, sem esperar o aviso de "fiquei vazio" do motor — esse aviso às vezes não vinha e a rádio ficava muda. **Sem vinheta** durante o ducking nem logo depois dele. O botão **Repetir** não liga faltando menos de 10 s. |
| 🕐 **Hora certa** | **Só existe uma.** Toca na **hora cheia** ou **a cada 30 min** (:00 e :30), nos dias marcados. Espera a música acabar e toca, nesta ordem, três partes opcionais: **abertura da hora** (um arquivo da pasta), **hora** e **comercial da hora** (um arquivo da pasta). A hora falada é a do fim da música, não a do disparo. Arquivos de hora: separados `HRS08` + `MIN10` (hora cheia = só `HRS08`) ou um só (`HRS08_MIN10`, `08-10`, `08h10`, `0810`). |
| 📡 **Link** | Áudio ao vivo por endereço (MP3/AAC/HLS), ex.: Voz do Brasil. No horário, **fade de 3 s** na música, toca **um arquivo da pasta de abertura** (opcional) e entra o link. Toca **até a próxima lista ou outro link**. Se cair, **silêncio tentando reconectar** sozinho. Comercial, ducking e hora certa que caem durante o link (ou esperavam a música acabar) são **descartados**. Passar fica bloqueado. |

- **Agendamento não entra na fila e não se pula:** comercial e hora certa tocam numa sequência própria antes da fila. Passar não pula agendamento; se pular uma música, o comercial que estava esperando continua garantido.
- Fora do ar, comercial e hora certa **não são guardados** para depois (lista de músicas vale mesmo fora do ar).
- **Ativar/desativar:** chave "Ativo" dentro do editor, antes dos horários (vale ao salvar). Desativado fica apagadinho, **não toca** e **não ocupa o horário** (dá para criar outro no mesmo horário). Ao ativar de novo, confere conflito e avisa se o horário já está ocupado.
- **Não toca em feriado:** chave no editor. Ligada, o agendamento não toca nos feriados de data fixa (1/1, 21/4, 1/5, 7/9, 12/10, 2/11, 15/11, 20/11, 25/12) nem nas **outras datas** escolhidas no calendário (Carnaval, dispensa da Voz do Brasil, feriado da cidade). Datas que já passaram somem ao salvar.
- **Vários horários** num agendamento: toca em cada um deles, nos dias marcados.
- **Mesmo horário e dia:** comercial entra em fila e pode cair junto com qualquer outro. **Bloqueado** (salvar avisa): dois **ducking** juntos, e **lista com lista, link com link ou lista com link** (um desfaria o outro). A **hora certa** é uma só, pode cair junto com qualquer outro e toca primeiro.
- Voz da hora certa que a rádio usa: `audios/Hora Certa (Voz Fem)/` no projeto.
- Várias **pastas** por agendamento, tocando **na ordem anexada**; cada pasta **Todos** ou **Só um** (rodízio salvo).
- **Só um:** arquivo **com data** (ainda válida) tem prioridade sobre os sem data; com vários de data, rodízio entre eles. Sem nenhum com data, rodízio dos sem data. Vale também para abertura e comercial da hora certa.
- Dentro da pasta: **ordem alfabética natural** (`2` antes de `10`). Abertura/fechamento = nomear em ordem.
- **Validade** no nome: `Ofertas [12-10-2025].mp3` toca até 12/10/2025 inclusive; vencido é pulado (fica na pasta). Só comerciais. Na tela o nome aparece sem a data.
- Cores: agendamentos em **amarelo**; hora certa em azul.

- **macOS:** o motor processa o áudio em pedaços de 0,2 s (padrão 0,04 s). A saída do Mac só guarda dois pedaços; com pedaços pequenos, abrir o navegador ou outro programa fazia o som raspar.

## 7. Áudio (Fase 3)

- ✅ **Placa de som** (Configurações): saída principal, pré-escuta e entrada (mesa), cada uma com "Padrão do sistema" ou uma placa. Linux: lista do PulseAudio (`pactl`); Mac (desenvolvimento): lista do `ffmpeg`. Trocar a saída reinicia o motor (a música atual é interrompida e segue a próxima).
- ✅ **Gravação** da **entrada da placa** (o que vem da mesa) em MP3 mono (**24** ou **32** kbps, padrão 24). Um arquivo a cada **30 min** (padrão) ou **1 hora**, cortado na virada do relógio: `<pasta>/15-11-2026/14-00.mp3` (pasta do dia, arquivo com a hora do início). Chave liga/desliga: ligada, grava sempre que o programa está aberto. Pastas de dia com **mais de 30 dias são apagadas** (só pastas com nome dd-mm-aaaa).
- ✅ **Transmissão** Icecast da **entrada da placa**, em MP3: servidor, porta, senha, ponto de montagem, bitrate, samplerate, mono/estéreo. Chave liga/desliga; se cair, tenta de novo a cada 5 s. Mostra No ar / Conectando / Erro.
- Gravação e transmissão rodam num **segundo Liquidsoap** (`captura.liq` / `captura.log`), separado do que toca: mudar essas configurações não interrompe o som no ar. Se ele parar, volta sozinho em 5 s.
- 💡 Transmissão em AAC e servidor Shoutcast (hoje: MP3 e Icecast).
- ✅ **Vinheta entre as músicas** (chave em Configurações → Músicas, ligada por padrão): entre **toda** música e a seguinte toca uma vinheta **sorteada dos botões da cartucheira** (sem repetir a última), inteira e sem mistura. Só entre música e música (não perto de comercial, hora certa ou link). Passar pula direto para a próxima música, sem vinheta. A vinheta e a música seguinte vão juntas ao motor.
- ✅ **Sem análise: os arquivos já vêm preparados** (silêncio cortado e volume igualado antes, por quem monta o acervo). O motor toca cada arquivo como ele é, por isso tudo começa **na hora** (Passar sem "aguardando"). Entre **duas músicas**, **crossfade fixo de 2 s** (a que acaba desce enquanto a nova sobe). **Comercial, vinheta, hora certa, abertura, comercial com ducking e cartucheira** tocam **inteiros** e **sem mistura**. A próxima faixa é entregue ao motor **5 s antes** do fim. (Antes o motor analisava cada música — corte de silêncio, volume e mistura de 1 s — e precisava de 15 s de antecedência.)
- 💡 Nível do ducking configurável.

## 8. Histórico

- 💡 Um arquivo de texto por dia com o que tocou (`historico/AAAA-MM-DD.txt`) — relatório e base para não repetir.

## 9. Leveza

Metas para PC antigo (2 núcleos, 4 GB, HD mecânico): **< ~200 MB** de memória, **< ~5%** de CPU, abrir em poucos segundos.

Medido no Mac (versão de desenvolvimento), tocando: programa ~30 MB, tela ~55 MB, Liquidsoap ~60 MB → **~145 MB**; CPU somada ~10–12% de um núcleo. ❓ Medir de novo no Linux e na versão final.

## 10. Fases

1. ✅ **Tocando** — acervo, busca, fila, Tocando agora, cartucheira, pré-escuta.
2. ✅ **Agendamentos** — listas, comerciais, ducking que detecta fala, hora certa, validade, rodízio. Mais as abas Pastas, Cartucheira e Configurações (senha, tocar ao abrir).
3. **Saídas** — placa da pré-escuta, transmissão Icecast, gravação.
4. **Acabamento** — instalador `.deb`/AppImage (gerado pelo GitHub), teste num Linux de verdade, medir leveza.

## 11. Pendências e decisões em aberto

- ❓ Comercial que dispara fora do ar: descartar (atual) ou guardar?
- ❓ Sugestões 💡 da fila (repetição, crossfade) e do histórico.
- ❓ Limite do inotify no Linux com muitas subpastas.

## 12. O que testar (🧪)

Nada disso foi testado de ouvido ou clicando por completo:

**No ar**
- [ ] Ducking manual abaixa e sobe a música; cartucheira não abaixa junto.
- [ ] Repetir: a música toca de novo; desligar volta para a fila.
- [ ] Passar pula música, mas fica bloqueado durante comercial/hora certa.
- [ ] Pré-escuta: 🎧 vira ■, barrinha anda, clicar na linha pula, ar continua normal.
- [ ] Barra vermelha nos 10 s finais.
- [ ] Som sem acelerar/desacelerar e sem travadinha ao abrir outros apps (folga de 1 s no Mac).
- [ ] Fila: reordenar arrastando, lixeira, mínimo de 3, lista ativa aparecendo.

**Agendamentos**
- [ ] Comercial normal entra no fim da música ("depois desta") e não aparece na fila.
- [ ] Várias pastas, ordem anexada, Todos / Só um (rodízio andando e salvo depois de fechar o programa).
- [ ] Validade: arquivo vencido não toca.
- [ ] Ducking curto: música abaixa com voz, sobe no silêncio, volta ao normal.
- [ ] Ducking longo: música repete por baixo, fade de 3 s e próxima.
- [ ] Hora certa com `audios/Hora Certa (Voz Fem)` (HRS + MIN), a cada X minutos, abertura + hora + comercial da hora; hora falada certa no fim da música.
- [ ] Só dá para criar uma hora certa; opções Hora cheia e 30 min.
- [ ] Novo agendamento já vem com todos os dias marcados.
- [ ] Dois ducking (ou duas listas, ou lista e link) no mesmo horário e dia é bloqueado; dois comerciais juntos tocam em sequência; hora certa junto com comercial toca antes.
- [ ] Agendamento com vários horários toca em cada um.
- [ ] Só um: arquivo com data toca no lugar dos sem data.
- [ ] Lista de músicas trocando de pasta no horário: fila limpa com músicas novas; Aguardar e Imediato.
- [ ] Link: fade, abertura, link no ar; internet cai = silêncio e volta sozinho; lista seguinte encerra com fade; comercial/hora certa no meio não tocam.
- [ ] macOS: abrir Chrome/outros programas com a rádio no ar sem o som raspar.
- [ ] Vinheta da cartucheira entre toda música (sorteada, sem repetir), desligável em Configurações.
- [ ] Barras de tempo (verde no ar, azul na pré-escuta) andando suaves.
- [ ] Músicas sem buraco entre uma e outra (silêncio cortado), mistura de ~1 s, volumes parecidos; comercial/vinheta/hora certa inteiros e sem mistura; barra de tempo acaba junto com a música.
- [ ] Pré-escuta: clicar na linha para frente e para trás vai ao ponto certo; a barra acompanha o que se ouve (no Mac, com 1 s de atraso compensado).
- [ ] Nome da rádio muda o topo.
- [ ] Topo: aba NO AR com ícone; etiquetas GRAVANDO / TRANSMITINDO (laranja quando há problema).
- [ ] Música não aumenta de volume no fim, antes da vinheta.
- [ ] Windows: instalar, tocar, pré-escuta, escolher placas, gravar, transmitir.
- [ ] Trocar saída principal e pré-escuta de placa (o som muda de lugar).
- [ ] Gravação: arquivos em pasta do dia, cortados na virada da hora / meia hora, com o som da mesa.
- [ ] Transmissão: conecta no servidor, aparece "No ar"; senha errada mostra erro; internet cai e volta sozinha.
- [ ] Próximo agendamento correto no Tocando agora.

**Abas**
- [ ] Pastas: adicionar várias, tirar, ordem alfabética no No ar.
- [ ] Cartucheira: escolher/esvaziar os 8 botões.
- [ ] Configurações: entrar no ar sozinho (fechar e abrir o programa), Atualizar, criar/trocar/remover senha, bloqueios por aba, cadeado para bloquear de novo.
