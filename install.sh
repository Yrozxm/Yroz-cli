#!/bin/sh
set -e

# Detect architecture
ARCH=$(uname -m)
if [ "$ARCH" = "x86_64" ]; then
    BINARY_URL="https://github.com/Yrozxm/Yroz-cli/releases/latest/download/yroz-x86_64"
elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    BINARY_URL="https://github.com/Yrozxm/Yroz-cli/releases/latest/download/yroz-aarch64"
else
    echo "Arquitetura não suportada: $ARCH"
    exit 1
fi

echo "Baixando o Yroz pré-compilado para $ARCH..."
curl -L -o yroz "$BINARY_URL"
chmod +x yroz

echo "Instalando o Yroz em /usr/local/bin (pode solicitar a senha do sudo)..."
sudo mv yroz /usr/local/bin/yroz

echo "Yroz instalado com sucesso! Rode 'yroz status' para verificar."
