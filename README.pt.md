# claude-view

<p align="center">
  <strong>Monitor em tempo real e copiloto para usuários avançados do Claude Code.</strong>
</p>

<p align="center">
  <a href="./README.md">English</a> ·
  <a href="./README.zh-TW.md">繁體中文</a> ·
  <a href="./README.zh-CN.md">简体中文</a> ·
  <a href="./README.ja.md">日本語</a> ·
  <a href="./README.es.md">Español</a> ·
  <a href="./README.fr.md">Français</a> ·
  <a href="./README.de.md">Deutsch</a> ·
  <a href="./README.pt.md">Português</a> ·
  <a href="./README.it.md">Italiano</a> ·
  <a href="./README.ko.md">한국어</a> ·
  <a href="./README.nl.md">Nederlands</a>
</p>

<p align="center">
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://discord.gg/G7wdZTpRfu"><img src="https://img.shields.io/discord/1325420051266592859?color=5865F2&logo=discord&logoColor=white&label=Discord" alt="Discord"></a>
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

---

## O Problema

Você tem 3 projetos abertos. Cada projeto tem múltiplas worktrees do git. Cada worktree tem múltiplas sessões do Claude Code rodando. Algumas estão pensando, outras esperando seu input, algumas estão prestes a atingir os limites de contexto, e uma terminou há 10 minutos mas você esqueceu.

Você faz Cmd-Tab entre 15 janelas de terminal tentando lembrar qual sessão estava fazendo o quê. Você desperdiça tokens porque um cache expirou enquanto você não estava olhando. Você perde o flow porque não há um lugar único para ver tudo. E por trás daquele spinner "pensando...", Claude está gerando sub-agentes, chamando servidores MCP, executando skills, disparando hooks — e você não consegue ver nada disso.

**O Claude Code é incrivelmente poderoso. Mas pilotar 10+ sessões simultâneas sem um painel é como dirigir sem velocímetro.**

## A Solução

**claude-view** é um painel em tempo real que funciona ao lado das suas sessões do Claude Code. Uma aba do navegador, cada sessão visível, contexto completo de relance.

```bash
npx claude-view
```

É isso. Abre no seu navegador. Todas as suas sessões — ao vivo e passadas — em um workspace.

---

## O Que Você Obtém

### Monitor em Tempo Real

| Recurso | Por que importa |
|---------|---------------|
| **Cards de sessão com última mensagem** | Lembre-se instantaneamente no que cada sessão de longa duração está trabalhando |
| **Sons de notificação** | Receba um aviso quando uma sessão termina ou precisa do seu input — pare de verificar terminais |
| **Medidor de contexto** | Uso da janela de contexto em tempo real por sessão — veja quais estão na zona de perigo |
| **Contagem regressiva do cache** | Saiba exatamente quando o cache de prompts expira para cronometrar sua próxima mensagem e economizar tokens |
| **Rastreamento de custos** | Gasto por sessão e agregado com detalhamento da economia de cache |
| **Visualização de sub-agentes** | Veja a árvore completa de agentes — sub-agentes, seus status e quais ferramentas estão chamando |
| **Múltiplas visualizações** | Grid, Lista ou modo Monitor (grid de chat ao vivo) — escolha o que se adapta ao seu workflow |

### Histórico de Chat Rico

| Recurso | Por que importa |
|---------|---------------|
| **Navegador de conversa completo** | Cada sessão, cada mensagem, totalmente renderizado com markdown e blocos de código |
| **Visualização de chamadas de ferramentas** | Veja leituras de arquivo, edições, comandos bash, chamadas MCP, invocações de skills — não apenas texto |
| **Toggle compacto / detalhado** | Folheie a conversa ou mergulhe em cada chamada de ferramenta |
| **Visualização de threads** | Acompanhe conversas de agentes com hierarquias de sub-agentes |
| **Exportar** | Exportação Markdown para retomada de contexto ou compartilhamento |

### Busca Avançada

| Recurso | Por que importa |
|---------|---------------|
| **Busca de texto completo** | Busque em todas as sessões — mensagens, chamadas de ferramentas, caminhos de arquivos |
| **Filtros de projeto e branch** | Limite o escopo ao projeto no qual você está trabalhando agora |
| **Paleta de comandos** | Cmd+K para pular entre sessões, trocar visualizações, encontrar qualquer coisa |

### Internos do Agente — Veja O Que Está Oculto

O Claude Code faz muita coisa por trás do "pensando..." que nunca aparece no seu terminal. O claude-view expõe tudo.

| Recurso | Por que importa |
|---------|---------------|
| **Conversas de sub-agentes** | Veja a árvore completa de agentes gerados, seus prompts e seus resultados |
| **Chamadas a servidores MCP** | Veja quais ferramentas MCP estão sendo invocadas e seus resultados |
| **Rastreamento de skills / hooks / plugins** | Saiba quais skills dispararam, quais hooks rodaram, quais plugins estão ativos |
| **Registro de eventos de hooks** | Cada evento de hook é capturado e navegável — volte e verifique o que disparou e quando. *(Requer que o claude-view esteja rodando enquanto as sessões estão ativas; não pode rastrear eventos históricos retroativamente)* |
| **Timeline de uso de ferramentas** | Log de ações de cada par tool_use/tool_result com temporização |
| **Surfacing de erros** | Erros aparecem no card da sessão — sem mais falhas enterradas |
| **Inspetor de mensagens raw** | Mergulhe no JSON raw de qualquer mensagem quando precisar da visão completa |

### Análises

Uma suíte rica de análises para seu uso do Claude Code. Pense no painel do Cursor, mas mais profundo.

**Visão Geral do Painel**

| Recurso | Descrição |
|---------|-------------|
| **Métricas semana a semana** | Contagem de sessões, uso de tokens, custo — comparado com seu período anterior |
| **Mapa de calor de atividade** | Grid estilo GitHub de 90 dias mostrando a intensidade diária de uso do Claude Code |
| **Top skills / comandos / ferramentas MCP / agentes** | Rankings dos seus invocáveis mais usados — clique em qualquer um para buscar sessões correspondentes |
| **Projetos mais ativos** | Gráfico de barras de projetos ordenados por contagem de sessões |
| **Detalhamento de uso de ferramentas** | Total de edições, leituras e comandos bash em todas as sessões |
| **Sessões mais longas** | Acesso rápido às suas sessões maratona com duração |

**Contribuições de IA**

| Recurso | Descrição |
|---------|-------------|
| **Rastreamento de output de código** | Linhas adicionadas/removidas, arquivos tocados, contagem de commits — em todas as sessões |
| **Métricas de ROI de custo** | Custo por commit, custo por sessão, custo por linha de output de IA — com gráficos de tendência |
| **Comparação de modelos** | Detalhamento lado a lado de output e eficiência por modelo (Opus, Sonnet, Haiku) |
| **Curva de aprendizado** | Taxa de re-edição ao longo do tempo — veja-se melhorando no prompting |
| **Detalhamento por branch** | Visualização colapsável por branch com drill-down de sessões |
| **Eficácia de skills** | Quais skills realmente melhoram seu output vs quais não |

**Insights** *(experimental)*

| Recurso | Descrição |
|---------|-------------|
| **Detecção de padrões** | Padrões comportamentais descobertos do seu histórico de sessões |
| **Benchmarks antes vs agora** | Compare seu primeiro mês com o uso recente |
| **Detalhamento por categoria** | Treemap de para que você usa o Claude — refatoração, features, debugging, etc. |
| **Score de Fluência IA** | Um único número 0-100 rastreando sua eficácia geral |

> **Nota:** Insights e Score de Fluência estão em estágio experimental inicial. Considere como direcional, não definitivo.

---

## Feito Para o Flow

O claude-view é projetado para o desenvolvedor que:

- Roda **3+ projetos simultaneamente**, cada um com múltiplas worktrees
- Tem **10-20 sessões do Claude Code** abertas a qualquer momento
- Precisa trocar de contexto rápido sem perder o controle do que está rodando
- Quer **otimizar gastos com tokens** cronometrando mensagens ao redor das janelas de cache
- Se frustra com Cmd-Tab entre terminais para verificar agentes

Uma aba do navegador. Todas as sessões. Mantenha o flow.

---

## Como Foi Construído

| | |
|---|---|
| **Ultra rápido** | Backend Rust com parsing JSONL acelerado por SIMD, I/O mapeado em memória — indexa milhares de sessões em segundos |
| **Tempo real** | File watcher + SSE + WebSocket para atualizações ao vivo sub-segundo em todas as sessões |
| **Pegada mínima** | Binário único de ~15 MB. Sem dependências de runtime, sem daemons em segundo plano |
| **100% local** | Todos os dados ficam na sua máquina. Zero telemetria, zero nuvem, zero requisições de rede |
| **Zero configuração** | `npx claude-view` e pronto. Sem API keys, sem setup, sem contas |

---

## Início Rápido

```bash
npx claude-view
```

Abre em `http://localhost:47892`.

### Configuração

| Variável de Ambiente | Padrão | Descrição |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` ou `PORT` | `47892` | Sobrescrever a porta padrão |

---

## Instalação

| Método | Comando |
|--------|---------|
| **npx** (recomendado) | `npx claude-view` |
| **Script shell** (Node não necessário) | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

### Requisitos

- **Claude Code** instalado ([obtenha aqui](https://docs.anthropic.com/en/docs/claude-code)) — isso cria os arquivos de sessão que monitoramos

---

## Comparativo

Outras ferramentas são visualizadores (navegar histórico) ou monitores simples. Nenhuma combina monitoramento em tempo real, histórico de chat rico, ferramentas de debugging e busca avançada em um único workspace.

```
                    Passivo ←————————————→ Ativo
                         |                  |
            Apenas       |  ccusage         |
            visualizar   |  History Viewer  |
                         |  clog            |
                         |                  |
            Apenas       |  claude-code-ui  |
            monitor      |  Agent Sessions  |
                         |                  |
            Workspace    |  ★ claude-view   |
            completo     |                  |
```

---

## Comunidade

Junte-se ao [servidor Discord](https://discord.gg/G7wdZTpRfu) para suporte, pedidos de funcionalidades e discussão.

---

## Gostou deste projeto?

Se o **claude-view** ajuda você a dominar o Claude Code, considere dar uma estrela. Isso ajuda outros a descobrir esta ferramenta.

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## Desenvolvimento

Pré-requisitos: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Instalar dependências do frontend
bun dev            # Iniciar desenvolvimento full-stack (Rust + Vite com hot reload)
```

| Comando | Descrição |
|---------|-------------|
| `bun dev` | Desenvolvimento full-stack — Rust reinicia automaticamente nas mudanças, Vite HMR |
| `bun dev:server` | Apenas backend Rust (com cargo-watch) |
| `bun dev:client` | Apenas frontend Vite (assume backend rodando) |
| `bun run build` | Compilar frontend para produção |
| `bun run preview` | Compilar + servir via binário de release |
| `bun run lint` | Lint de frontend (ESLint) e backend (Clippy) |
| `bun run fmt` | Formatar código Rust |
| `bun run check` | Typecheck + lint + test (gate de pré-commit) |
| `bun test` | Executar suíte de testes Rust (`cargo test --workspace`) |
| `bun test:client` | Executar testes de frontend (vitest) |
| `bun run test:e2e` | Executar testes end-to-end do Playwright |

### Testando a Distribuição de Produção

```bash
bun run dist:test    # Um comando: build → pack → install → run
```

Ou passo a passo:

| Comando | Descrição |
|---------|-------------|
| `bun run dist:pack` | Empacotar binário + frontend em tarball no `/tmp/` |
| `bun run dist:install` | Extrair tarball para `~/.cache/claude-view/` (simula download da primeira execução) |
| `bun run dist:run` | Executar o wrapper npx usando o binário em cache |
| `bun run dist:test` | Tudo acima em um único comando |
| `bun run dist:clean` | Remover todos os arquivos de cache dist e temporários |

### Lançamento

```bash
bun run release          # bump de patch: 0.1.0 → 0.1.1
bun run release:minor    # bump minor: 0.1.0 → 0.2.0
bun run release:major    # bump major: 0.1.0 → 1.0.0
```

Isso incrementa a versão no `npx-cli/package.json`, faz commit e cria uma tag git. Depois:

```bash
git push origin main --tags    # dispara CI → compila todas as plataformas → auto-publica no npm
```

---

## Suporte de Plataformas

| Plataforma | Status |
|----------|--------|
| macOS (Apple Silicon) | Disponível |
| macOS (Intel) | Disponível |
| Linux (x64) | Planejado |
| Windows (x64) | Planejado |

---

## Licença

MIT © 2026
