#!/bin/sh
# Depois de instalar: aplica já o limite de pastas vigiadas (sem reiniciar).
sysctl -q -p /etc/sysctl.d/60-radio-on-air.conf >/dev/null 2>&1 || true
exit 0
