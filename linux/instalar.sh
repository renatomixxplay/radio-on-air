#!/bin/sh
# Instala o Radio On Air num Debian 13 e deixa as atualizações ligadas.
#
#   wget -qO- https://renatomixxplay.github.io/radio-on-air/instalar.sh | sudo sh
#
# Depois, para atualizar: sudo apt update && sudo apt upgrade
# (ou pela Central de Programas / Atualizações do Debian).
set -e

SITE="https://renatomixxplay.github.io/radio-on-air"
CHAVE="/usr/share/keyrings/radio-on-air.asc"
FONTE="/etc/apt/sources.list.d/radio-on-air.sources"

if [ "$(id -u)" != 0 ]; then
    echo "Rode como administrador: ... | sudo sh"
    exit 1
fi

if [ "$(dpkg --print-architecture)" != "amd64" ]; then
    echo "Este computador não é amd64 (64 bits Intel/AMD). Ainda não há instalador para ele."
    exit 1
fi

. /etc/os-release
if [ "$ID" != "debian" ] || [ "${VERSION_ID:-}" != "13" ]; then
    echo "Atenção: feito para Debian 13. Este sistema é: ${PRETTY_NAME:-desconhecido}."
    echo "Continuando mesmo assim em 5 segundos (Ctrl+C para parar)..."
    sleep 5
fi

baixar() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1" -o "$2"
    else
        wget -qO "$2" "$1"
    fi
}

echo "==> Chave das atualizações"
mkdir -p /usr/share/keyrings
baixar "$SITE/radio-on-air.asc" "$CHAVE"
chmod 644 "$CHAVE"

echo "==> Repositório do Radio On Air"
cat > "$FONTE" <<EOF
# Atualizações do Radio On Air (e do Liquidsoap que ele usa).
# Instalado junto com o programa: "sudo apt update && sudo apt upgrade" já traz
# a versão nova.
Types: deb
URIs: $SITE/apt/
Suites: ./
Signed-By: $CHAVE
EOF

echo "==> Instalando"
apt-get update
apt-get install -y radio-on-air

echo
echo "Pronto! O Radio On Air está no menu de programas."
echo "Para atualizar depois: sudo apt update && sudo apt upgrade"
