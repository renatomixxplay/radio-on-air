# Radio On Air — Escopo

Software de automação de rádio (rádio DJ) para Linux, open source, feito para **rádios comunitárias**.

> Documento vivo. Cada recurso só entra depois de aprovado.
> Legenda: ✅ decidido · 💡 sugestão (aguardando aprovação) · ❓ pendente

---

## 1. Princípios

- ✅ **Leve** — precisa rodar bem em PCs modestos e antigos.
- ✅ **Programa de desktop** — janela própria, instalado no computador (não é site/navegador).
- ✅ **Arquivos locais** — músicas e áudios ficam em pastas do próprio computador.
- ✅ **Sem banco de dados** — tudo em pastas + arquivos de texto. Backup = copiar a pasta.
- ✅ **Open source, licença GPL-3.0.**
- ✅ **Tema escuro.**

## 2. Tecnologia

| Parte | Escolha | Por quê |
|---|---|---|
| Janela do programa | **Tauri** (Rust) | Instalador ~10 MB, usa pouca memória |
| Tela | **Svelte** | Gera telas pequenas e rápidas |
| Motor de áudio | **Liquidsoap** (roda escondido) | Feito para rádio: crossfade, ducking, Icecast, gravação, 24h estável |
| Arquivos de dados | Texto (`.m3u`, `.toml`/`.json`, `.txt`) | Fácil de ler, editar e fazer backup |

- ✅ Alvo: Linux (Ubuntu/Debian), pacote `.deb` e AppImage.
- ✅ Desenvolvimento e testes também no **macOS** (Tauri e Liquidsoap rodam no Mac).

## 3. Telas

Navegação no topo: **No ar · Agendamentos · Configurações**.

### 3.1 No ar (tela principal) ✅

```
┌───────────────────────────────────────────────┐
│ Radio On Air        NO AR  AUTO   14:32:07    │
├──────────────────────────┬────────────────────┤
│ Tocando agora            │ Cartucheira (8)    │
├────────────┬─────────────┴──────┬─────────────┤
│ Músicas    │ Fila (10)          │ Programas   │
└────────────┴────────────────────┴─────────────┘
```

- **Barra do topo:** nome da rádio, indicador NO AR, modo AUTO, relógio grande.
- **Tocando agora:** música, artista, forma de onda, tempo decorrido e restante, botões tocar / pausar / parar / próxima. Sem controle de volume.
- ✅ **Botão "Abaixar música"** — o microfone fica na mesa de som; o operador aperta para baixar a música enquanto fala e aperta de novo para voltar.
- **Cartucheira:** 8 botões **fixos** (sem páginas) para vinhetas e efeitos.
- **Músicas** (esquerda): busca por música ou artista; lista sem duração.
- **Fila** (meio): até 10 itens, com horário previsto, tipo (música, vinheta, comercial, programa) e duração.
- **Programas** (direita): pastas dos locutores.

### 3.2 Agendamentos ✅ (esboço feito, aguardando aprovação do visual)

- Lista de agendamentos com horário, tipo, pasta/arquivo e dias da semana.
- Filtro por dia da semana.
- Painel lateral para criar/editar.

### 3.3 Configurações ✅ (esboço feito, aguardando aprovação do visual)

Seções: Pastas · Áudio · Transmissão · Gravação · Fila · Cartucheira · Rádio.

## 4. Músicas e índice

- ✅ Acervo de ~**100 mil músicas**, organizadas em **subpastas**.
- ✅ Padrão de nome: `Artista - Música.mp3` (ex.: `Renato - Fogo de Chão.mp3`). Artista e título vêm do nome do arquivo.
- ✅ Busca lê só os **nomes** dos arquivos e fica em memória (rápida mesmo com 100 mil).
- ✅ Lista de nomes salva em `indice.txt` para o programa abrir instantaneamente.
- ✅ Atualização **automática**: música adicionada/removida/renomeada aparece na hora (inotify no Linux).
- ✅ Botão **"Atualizar músicas"** nas Configurações, para reler manualmente se der erro.
- ✅ **Sem** varredura ao abrir o programa.
- ✅ A duração só é lida quando o arquivo entra na fila.
- ❓ Arquivo fora do padrão (sem ` - `): mostrar o nome inteiro como música e artista em branco?
- ❓ Muitas subpastas podem exigir aumentar o limite de pastas vigiadas do Linux (`fs.inotify.max_user_watches`) na instalação.

## 5. Fila

- ✅ Capacidade de **10 itens**; o operador pode adicionar à mão.
- ✅ O automático garante um **mínimo de 3**: quando a fila cai para 3, puxa mais 1.
- ✅ O automático puxa da **Lista de músicas** ativa no momento (ver Agendamentos).
- 💡 Não repetir a mesma música por X horas.
- 💡 Não repetir o mesmo artista por X minutos.
- 💡 Crossfade entre músicas (segundos configuráveis).

## 6. Agendamentos

Funciona por **horário + dias da semana**. Não existe "sequência/receita": o programa só preenche a fila.

| Tipo | Comportamento |
|---|---|
| ✅ **Lista de músicas** | A partir do horário, a fila passa a puxar **aleatório** da pasta escolhida (ex.: `Músicas/Gaúchas`). Vale até a próxima lista. |
| ✅ **Comercial** | No horário, **espera a música atual acabar** e toca o comercial. |
| ✅ **Comercial com ducking** | No horário, entra **por cima** da música: abaixa a música, toca a fala e sobe a música de novo. |
| ✅ **Hora certa** | Toca o arquivo da hora atual da pasta de hora certa. ❓ Padrão de nome dos arquivos e se espera a música acabar ou entra com ducking. |

- ✅ **Programas dos locutores** entram de dois jeitos: o operador **arrasta para a fila**, ou é **agendado como comercial** (agendamento apontando para a pasta do programa).

- ✅ Cada agendamento repete nos **dias da semana** marcados.
- ✅ O agendamento de comercial aponta para uma **pasta** (não um arquivo mp3).
- ✅ Um mesmo agendamento pode ter **várias pastas** anexadas.
- ✅ As pastas tocam na **ordem em que foram anexadas**.
- ✅ Cada pasta tem a opção: tocar **todos** os arquivos ou só **um**.
- ✅ Dentro da pasta, os arquivos tocam sempre em **ordem alfabética** (ordem natural: `2` vem antes de `10`).
- ✅ Vinheta de abertura/fechamento não é recurso à parte: basta nomear para ficar em ordem (ex.: `1 Abertura.mp3` … `9 Fechamento.mp3`).
- ✅ **Ducking em bloco:** a música fica abaixada até terminar todas as pastas e só sobe no final.
- ✅ **Ducking inteligente:** o programa detecta se há **fala** no comercial — abaixa a música enquanto tem voz e sobe nos silêncios.
- ✅ Opção "só um" funciona em **rodízio**: a cada execução toca o próximo arquivo em ordem alfabética e volta ao primeiro no fim. Comercial vencido é pulado. A posição do rodízio fica salva em arquivo de texto (sobrevive a fechar/abrir o programa).

### 6.1 Validade de comerciais

- ✅ Data final no nome do arquivo: `Ofertas xxx [12-10-2025].mp3` toca **até 12/10/2025, inclusive**.
- ✅ Formato `DD-MM-AAAA`. Só data final (sem data de início).
- ✅ Vale **só para comerciais**.
- ✅ Vencido é **ignorado** (o arquivo fica na pasta, não é movido).
- ✅ Na tela, o nome aparece sem a data.

## 7. Áudio

- ✅ **Saída principal** (caixa de som / mesa).
- ✅ **Pré-escuta** — ouvir um arquivo no fone/segunda saída sem ir para o ar.
- ✅ **Transmissão pela internet** (Icecast): servidor, porta, ponto de montagem, senha, qualidade.
- ✅ **Gravação** do que vai ao ar.
- 💡 Gravação dividida em 1 arquivo por hora.
- 💡 Apagar gravações após X dias.
- ✅ Nível do ducking configurável (quanto a música abaixa).

## 8. Histórico

- 💡 Um arquivo de texto por dia com o que tocou (`historico/AAAA-MM-DD.txt`) — serve de relatório e para evitar repetição.

## 9. Leveza (meta)

O programa precisa ser **bem leve**. Metas para um PC antigo (2 núcleos, 4 GB de RAM, HD mecânico):

- Memória total (janela + motor de áudio): **abaixo de ~200 MB**.
- Processador tocando no automático: **abaixo de ~5%**.
- Abrir o programa: **poucos segundos**, mesmo com 100 mil músicas (lê o `indice.txt`).
- Forma de onda calculada só para o que está tocando/na fila, nunca para o acervo inteiro.
- Tela sem animações pesadas; atualiza só o que muda (relógio, tempo, fila).

## 10. Fases (proposta)

1. **Tocando** — programa abre, lê a pasta de músicas (índice + atualização automática), busca, fila com mínimo de 3, Tocando agora, botões, cartucheira, saída na caixa. **Medir memória/CPU aqui** para validar a meta de leveza.
2. **Agendamentos** — lista de músicas, comercial, comercial com ducking (detecta fala), hora certa, validade, rodízio, botão "Abaixar música".
3. **Saídas** — pré-escuta, transmissão Icecast, gravação.
4. **Acabamento** — tela de configurações, programas dos locutores, instalador `.deb`/AppImage.

## 11. Decisões gerais

- ✅ Microfone fica na **mesa de som** (o programa não captura microfone).
- ✅ Interface só em **português**.

## 12. Pendências

- ❓ Configurações: alinhar item a item (inclui as sugestões 💡).
- ❓ Hora certa: padrão de nome dos arquivos e comportamento (espera a música ou ducking).
- ❓ Arquivo de música fora do padrão `Artista - Música`.
