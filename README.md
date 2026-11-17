# Radio On Air

Automação de rádio (rádio DJ) **leve e open source** para Linux, feita para **rádios comunitárias**.

> Em desenvolvimento — Fases 1 (tocar, fila, busca, cartucheira, pré-escuta) e 2 (agendamentos, pastas, senha) prontas; em teste. Veja o [ESCOPO.md](ESCOPO.md), inclusive a lista do que testar.

## Como funciona

- **Tela:** [Tauri](https://tauri.app) + [Svelte](https://svelte.dev) (programa de desktop, janela própria).
- **Áudio:** [Liquidsoap](https://www.liquidsoap.info) rodando escondido; o programa entrega uma faixa por vez.
- **Sem banco de dados:** tudo em pastas e arquivos de texto.
  - Músicas no padrão `Artista - Música.mp3`, em qualquer subpasta.
  - Índice do acervo em `indice.txt`, atualizado sozinho quando entra/sai música.
  - Configurações em `config.json`, agendamentos em `agendamentos.json`, rodízio em `rodizio.json`.

Os arquivos de dados ficam em `~/.local/share/org.radioonair.app/` (Linux)
ou `~/Library/Application Support/org.radioonair.app/` (macOS).

## Instalar (Debian 13)

Num terminal do computador da rádio:

```bash
wget -qO- https://renatomixxplay.github.io/radio-on-air/instalar.sh | sudo sh
```

(Se o `sudo` não estiver configurado: `su -`, digitar a senha de administrador
e rodar o mesmo comando sem o `sudo`.)

Isso instala o programa e o Liquidsoap 2.4.6 e liga as atualizações. Para
atualizar depois: `sudo apt update && sudo apt upgrade`, ou pela tela de
atualizações do Debian.

Os registros (`historico.log`, `motor.log`) ficam em
`~/.local/share/org.radioonair.app/`.

## Publicar uma versão nova

1. GitHub → **Actions** → **Instalador Linux** → **Run workflow**.
2. Digitar o número da versão nova (ex.: `0.1.1`), maior que a anterior.
3. No fim, o `.deb` fica em *Artifacts*, e as rádios recebem a versão nova no
   próximo `apt upgrade`.

Preparação (uma vez só): repositório público; *Settings → Pages → Source:
GitHub Actions*; segredo `APT_CHAVE_PRIVADA` com a chave que assina as
atualizações (a parte pública é `src-tauri/linux/radio-on-air.asc`).

## Desenvolver

Requisitos: Node 22+, Rust (stable) e Liquidsoap 2.4+.

```bash
npm install
npm run tauri dev
```

Para o PATH do Rust instalado pelo rustup: `export PATH="$HOME/.cargo/bin:$PATH"`.

No Linux, use o Liquidsoap 2.4+ das [versões oficiais](https://github.com/savonet/liquidsoap/releases)
(o `apt install liquidsoap` do Debian/Ubuntu traz uma versão mais antiga).
No macOS, dá para compilar isolado com o [opam](https://opam.ocaml.org) em `~/.opam`
(o programa procura `~/.opam/radio/bin/liquidsoap`). Também é possível apontar
outro executável com a variável `RADIO_LIQUIDSOAP`.

Testes do núcleo:

```bash
cd src-tauri && cargo test
```

## Licença

[GPL-3.0-or-later](LICENSE).
