# Yroz

Yroz e um gerenciador universal de software para Linux escrito em Rust. Ele unifica a gestao de pacotes sob um unico comando, detectando dinamicamente o gerenciador nativo da sua distribuicao e integrando suporte a formatos universais.

O objetivo e reduzir a fragmentacao de comandos no ecossistema Linux, oferecendo uma interface de linha de comando rapida, consistente e facil de utilizar.

---

## Caracteristicas

*   **Deteccao automatica:** Identifica o gerenciador de pacotes da distribuicao em tempo de execucao.
*   **Suporte nativo e universal:**
    *   Nativos: APT, Pacman, DNF, Portage, XBPS, Zypper e APK.
    *   Universais: Flatpak, Snap e Nix (nix-env).
    *   Repositórios comunitários: AUR (via helper yay).
*   **Ordem de prioridade inteligente:** Prioriza instalacoes via gerenciador nativo da distribuicao, recorrendo aos gerenciadores universais caso o pacote nao seja encontrado.
*   **Busca paralela:** Realiza pesquisas concorrentes em todas as fontes ativas para entregar resultados instantaneos.
*   **Altamente configuravel:** Suporta desativacao de fontes e alteracao na ordem de prioridades de instalacao atraves de um arquivo TOML simples.
*   **Auto-atualizacao:** Atualiza seu proprio binario diretamente a partir das releases do GitHub.

---

## Como Compilar e Instalar

### Requisitos
*   Rust toolchain (cargo)

### Compilacao
Para gerar o binario otimizado de producao, clone o repositorio e execute:

```bash
cargo build --release
```

### Instalacao Global
Mova o binario compilado para o PATH do seu sistema para executa-lo de qualquer lugar:

```bash
sudo cp target/release/yroz /usr/local/bin/yroz
```

---

## Interface de Comando (CLI)

*   **Verificar estado:** Mostra quais gerenciadores estao ativos e configurados no sistema.
    ```bash
    yroz status
    ```
*   **Buscar pacotes:** Pesquisa concorrente em todas as fontes habilitadas.
    ```bash
    yroz search <termo>
    ```
*   **Instalar pacote:** Segue a ordem de prioridades com deteccao inteligente de App IDs.
    ```bash
    yroz install <pacote>
    ```
*   **Remover pacote:** Deteta de onde o pacote veio e o remove.
    ```bash
    yroz remove <pacote>
    ```
*   **Atualizar fontes:** Atualiza a base de dados de todos os gerenciadores ativos.
    ```bash
    yroz update
    ```
*   **Detalhes do pacote:** Mostra metadados e estado de instalacao local.
    ```bash
    yroz info <pacote>
    ```
*   **Listar instalados:** Exibe os pacotes instalados por fonte.
    ```bash
    yroz list
    ```
*   **Auto-atualizar:**
    ```bash
    yroz self-update
    ```

---

## Configuracao

Você pode personalizar o comportamento do Yroz criando o arquivo em `~/.config/yroz/config.toml`.

### Exemplo de configuracao:

```toml
# Lista de gerenciadores a serem ignorados completamente
disabled_backends = ["Snap"]

# Ordem customizada de prioridade para o comando de instalacao
priority = ["Nix", "Flatpak", "APT"]
```

---

## Licenca

Este projeto e open-source e licenciado sob a licenca MIT.
