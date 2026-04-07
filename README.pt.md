<div align="center">

# claude-view

**Centro de Controle para o Claude Code**

Voce tem 10 agentes de IA rodando. Um terminou 12 minutos atras. Outro atingiu o limite de contexto. Um terceiro precisa de aprovacao de ferramenta. Voce esta alternando entre terminais com <kbd>Cmd</kbd>+<kbd>Tab</kbd>, gastando $200/mes no escuro.

<p>
  <a href="https://www.npmjs.com/package/claude-view"><img src="https://img.shields.io/npm/v/claude-view.svg" alt="npm version"></a>
  <a href="https://claudeview.ai"><img src="https://img.shields.io/badge/docs-claudeview.ai-orange" alt="Website"></a>
  <a href="https://www.npmjs.com/package/@claude-view/plugin"><img src="https://img.shields.io/npm/v/@claude-view/plugin.svg?label=plugin" alt="plugin version"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://discord.gg/G7wdZTpRfu"><img src="https://img.shields.io/discord/1325420051266592859?color=5865F2&logo=discord&logoColor=white&label=Discord" alt="Discord"></a>
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

<p>
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

```bash
curl -fsSL https://get.claudeview.ai/install.sh | sh
```

**Um comando. Todas as sessoes visiveis. Em tempo real.**

</div>

---

## O que e o claude-view?

claude-view e um painel open-source que monitora todas as sessoes do Claude Code na sua maquina — agentes ativos, conversas anteriores, custos, sub-agentes, hooks, chamadas de ferramentas — em um so lugar. Backend em Rust, frontend em React, binario de ~10 MB. Zero configuracao, zero contas, 100% local.

**50+ releases. 85 ferramentas MCP. 9 skills. Um unico `npx claude-view`.**

---

## Monitor ao Vivo

Veja todas as sessoes em execucao de relance. Chega de alternar entre abas de terminal.

| Recurso | O que faz |
|---------|-----------|
| **Cards de sessao** | Cada card mostra a ultima mensagem, modelo, custo e status — saiba instantaneamente no que cada agente esta trabalhando |
| **Chat multi-sessao** | Abra sessoes lado a lado em abas estilo VS Code (dockview). Arraste para dividir horizontal ou verticalmente |
| **Medidor de contexto** | Preenchimento da janela de contexto em tempo real por sessao — veja quais agentes estao na zona de perigo antes de atingirem o limite |
| **Contagem regressiva do cache** | Saiba exatamente quando o cache de prompt expira para cronometrar mensagens e economizar tokens |
| **Rastreamento de custos** | Gasto por sessao e agregado com detalhamento de tokens — passe o mouse para ver a divisao de entrada/saida/cache por modelo |
| **Arvore de sub-agentes** | Veja a arvore completa de agentes gerados, seus status, custos e quais ferramentas estao chamando |
| **Sons de notificacao** | Receba alertas quando uma sessao terminar, der erro ou precisar da sua atencao — pare de verificar terminais manualmente |
| **Multiplas visualizacoes** | Grid, Lista, Kanban ou modo Monitor — escolha o que se adapta ao seu fluxo de trabalho |
| **Raias Kanban** | Agrupe sessoes por projeto ou branch — layout visual em raias para fluxos de trabalho multi-projeto |
| **Encerradas recentemente** | Sessoes que terminam aparecem em "Encerradas Recentemente" em vez de desaparecer — persiste entre reinicializacoes do servidor |
| **Mensagens na fila** | Mensagens aguardando na fila aparecem como bolhas pendentes com um badge "Na Fila" |
| **Baseado em SSE** | Todos os dados ao vivo sao enviados via Server-Sent Events — elimina completamente riscos de cache desatualizado |

---

## Chat e Conversa

Leia, pesquise e interaja com qualquer sessao — ao vivo ou historica.

| Recurso | O que faz |
|---------|-----------|
| **Chat ao vivo unificado** | Historico e mensagens em tempo real em uma unica conversa rolavel — sem trocar de aba |
| **Modo desenvolvedor** | Alterne entre as visualizacoes Chat e Desenvolvedor por sessao. O modo desenvolvedor mostra cards de ferramentas, cards de eventos, metadados de hooks e o rastreamento completo de execucao com chips de filtro |
| **Navegador completo de conversas** | Todas as sessoes, todas as mensagens, totalmente renderizadas com markdown e blocos de codigo |
| **Visualizacao de chamadas de ferramentas** | Veja leituras de arquivos, edicoes, comandos bash, chamadas MCP, invocacoes de skills — nao apenas texto |
| **Alternancia compacto/detalhado** | Navegue pela conversa rapidamente ou aprofunde-se em cada chamada de ferramenta |
| **Visualizacao em thread** | Acompanhe conversas de agentes com hierarquias de sub-agentes e threads indentadas |
| **Eventos de hooks inline** | Hooks pre/pos ferramenta renderizados como blocos de conversa — veja hooks disparando junto com a conversa |
| **Exportacao** | Exportacao em Markdown para retomada de contexto ou compartilhamento |
| **Selecao em massa e arquivamento** | Selecione multiplas sessoes para arquivamento em lote com estado de filtro persistente |
| **Compartilhamento criptografado** | Compartilhe qualquer sessao via link criptografado de ponta a ponta — AES-256-GCM, zero confianca no servidor, a chave existe apenas no fragmento da URL |

---

## Internos do Agente

O Claude Code faz muita coisa por tras do `"thinking..."` que nunca aparece no seu terminal. O claude-view expoe tudo isso.

| Recurso | O que faz |
|---------|-----------|
| **Conversas de sub-agentes** | Arvore completa de agentes gerados, seus prompts, saidas e detalhamento de custo/tokens por agente |
| **Chamadas de servidor MCP** | Quais ferramentas MCP estao sendo invocadas e seus resultados |
| **Rastreamento de skills/hooks/plugins** | Quais skills dispararam, quais hooks executaram, quais plugins estao ativos |
| **Gravacao de eventos de hooks** | Captura de hooks em canal duplo (WebSocket ao vivo + backfill JSONL) — cada evento gravado e navegavel, mesmo para sessoes passadas |
| **Badges de origem da sessao** | Cada sessao mostra como foi iniciada: Terminal, VS Code, Agent SDK ou outros pontos de entrada |
| **Divergencia de branch em worktree** | Detecta quando branches de git worktree divergem — exibido no monitor ao vivo e no historico |
| **Chips de mencao @File** | Referencias `@filename` sao extraidas e exibidas como chips — passe o mouse para ver o caminho completo |
| **Timeline de uso de ferramentas** | Log de acoes de cada par tool_use/tool_result com cronometragem |
| **Exibicao de erros** | Erros sobem para o card da sessao — sem falhas enterradas |
| **Inspetor de mensagem bruta** | Aprofunde-se no JSON bruto de qualquer mensagem quando precisar da visao completa |

---

## Busca

| Recurso | O que faz |
|---------|-----------|
| **Busca full-text** | Pesquise em todas as sessoes — mensagens, chamadas de ferramentas, caminhos de arquivos. Alimentado por Tantivy (nativo em Rust, classe Lucene) |
| **Motor de busca unificado** | Tantivy full-text + pre-filtro SQLite executam em paralelo — um endpoint, resultados em menos de 50ms |
| **Filtros de projeto e branch** | Delimite ao projeto ou branch em que voce esta trabalhando agora |
| **Paleta de comandos** | <kbd>Cmd</kbd>+<kbd>K</kbd> para navegar entre sessoes, trocar visualizacoes, encontrar qualquer coisa |

---

## Analiticos

Uma suite completa de analiticos para seu uso do Claude Code. Pense no painel do Cursor, porem mais profundo.

<details>
<summary><strong>Painel</strong></summary>
<br>

| Recurso | Descricao |
|---------|-----------|
| **Metricas semana a semana** | Contagem de sessoes, uso de tokens, custo — comparado ao periodo anterior |
| **Mapa de calor de atividade** | Grade estilo GitHub de 90 dias mostrando intensidade de uso diario |
| **Top skills/comandos/ferramentas MCP/agentes** | Rankings dos seus invocaveis mais usados — clique em qualquer um para buscar sessoes correspondentes |
| **Projetos mais ativos** | Grafico de barras de projetos ranqueados por contagem de sessoes |
| **Detalhamento de uso de ferramentas** | Total de edicoes, leituras e comandos bash em todas as sessoes |
| **Sessoes mais longas** | Acesso rapido as suas sessoes maratona com duracao |

</details>

<details>
<summary><strong>Contribuicoes de IA</strong></summary>
<br>

| Recurso | Descricao |
|---------|-----------|
| **Rastreamento de saida de codigo** | Linhas adicionadas/removidas, arquivos tocados, contagem de commits — em todas as sessoes |
| **Metricas de ROI de custo** | Custo por commit, custo por sessao, custo por linha de saida de IA — com graficos de tendencia |
| **Comparacao de modelos** | Detalhamento lado a lado de saida e eficiencia por modelo (Opus, Sonnet, Haiku) |
| **Curva de aprendizado** | Taxa de re-edicao ao longo do tempo — veja voce mesmo melhorando nos prompts |
| **Detalhamento por branch** | Visualizacao colapsavel por branch com drill-down de sessao |
| **Eficacia de skills** | Quais skills realmente melhoram sua saida vs quais nao melhoram |

</details>

<details>
<summary><strong>Insights</strong> <em>(experimental)</em></summary>
<br>

| Recurso | Descricao |
|---------|-----------|
| **Deteccao de padroes** | Padroes comportamentais descobertos a partir do seu historico de sessoes |
| **Benchmarks Antes vs Agora** | Compare seu primeiro mes com o uso recente |
| **Detalhamento por categoria** | Treemap do que voce usa o Claude — refatoracao, funcionalidades, depuracao, etc. |
| **AI Fluency Score** | Um unico numero de 0-100 rastreando sua eficacia geral |

> Insights e Fluency Score sao experimentais. Trate como direcional, nao definitivo.

</details>

---

## Planos, Prompts e Equipes

| Recurso | O que faz |
|---------|-----------|
| **Navegador de planos** | Visualize seus `.claude/plans/` diretamente no detalhe da sessao — sem mais cacar entre arquivos |
| **Historico de prompts** | Busca full-text em todos os prompts que voce enviou com agrupamento por template e classificacao de intencao |
| **Painel de equipes** | Veja lideres de equipe, mensagens da caixa de entrada, tarefas da equipe e alteracoes de arquivos de todos os membros |
| **Analiticos de prompts** | Rankings de templates de prompts, distribuicao de intencoes e estatisticas de uso |

---

## Monitor do Sistema

| Recurso | O que faz |
|---------|-----------|
| **Medidores de CPU/RAM/Disco ao vivo** | Metricas do sistema em tempo real via SSE com transicoes animadas suaves |
| **Painel de componentes** | Veja metricas do sidecar e IA on-device: uso de VRAM, CPU, RAM e contagem de sessoes por componente |
| **Lista de processos** | Processos agrupados por nome, ordenados por CPU — veja o que sua maquina esta realmente fazendo enquanto os agentes rodam |

---

## IA On-Device

Execute um LLM local para classificacao de fase de sessao — sem chamadas de API, sem custo extra.

| Recurso | O que faz |
|---------|-----------|
| **Agnostico de provedor** | Conecte a qualquer endpoint compativel com OpenAI — oMLX, Ollama, LM Studio ou seu proprio servidor |
| **Seletor de modelo** | Escolha de um registro curado de modelos com requisitos de RAM exibidos |
| **Classificacao de fase** | Sessoes marcadas com sua fase atual (codificacao, depuracao, planejamento, etc.) usando exibicao com controle de confianca |
| **Gerenciamento inteligente de recursos** | Classificacao estabilizada por EMA com backoff exponencial — 93% de reducao de desperdicio de GPU vs polling ingenuo |

---

## Plugin

`@claude-view/plugin` da ao Claude acesso nativo aos dados do seu painel — 85 ferramentas MCP, 9 skills e auto-start.

```bash
claude plugin add @claude-view/plugin
```

### Auto-start

Cada sessao do Claude Code inicia automaticamente o painel. Sem necessidade de `npx claude-view` manual.

### 85 ferramentas MCP

8 ferramentas elaboradas manualmente com saida otimizada para o Claude:

| Ferramenta | Descricao |
|------------|-----------|
| `list_sessions` | Navegue sessoes com filtros |
| `get_session` | Detalhe completo da sessao com mensagens e metricas |
| `search_sessions` | Busca full-text em todas as conversas |
| `get_stats` | Visao geral do painel — total de sessoes, custos, tendencias |
| `get_fluency_score` | AI Fluency Score (0-100) com detalhamento |
| `get_token_stats` | Uso de tokens com taxa de acerto de cache |
| `list_live_sessions` | Agentes em execucao no momento (tempo real) |
| `get_live_summary` | Custo agregado e status do dia |

Mais **78 ferramentas geradas automaticamente** a partir da especificacao OpenAPI em 26 categorias (contribuicoes, insights, coaching, exportacoes, workflows e mais).

### 9 Skills

| Skill | O que faz |
|-------|-----------|
| `/session-recap` | Resuma uma sessao especifica — commits, metricas, duracao |
| `/daily-cost` | Gastos de hoje, sessoes em execucao, uso de tokens |
| `/standup` | Log de trabalho multi-sessao para atualizacoes de standup |
| `/coaching` | Dicas de coaching com IA e gerenciamento de regras personalizadas |
| `/insights` | Analise de padroes comportamentais |
| `/project-overview` | Resumo do projeto entre sessoes |
| `/search` | Busca em linguagem natural |
| `/export-data` | Exporte sessoes para CSV/JSON |
| `/team-status` | Visao geral da atividade da equipe |

---

## Workflows

| Recurso | O que faz |
|---------|-----------|
| **Construtor de workflows** | Crie workflows multi-etapa com layout estilo VS Code, preview de diagrama Mermaid e editor YAML |
| **Chat com streaming LLM** | Gere definicoes de workflow em tempo real via chat integrado |
| **Executor de etapas** | Visualize colunas de etapas, cards de tentativas e barra de progresso enquanto seu workflow executa |
| **Workflows iniciais inclusos** | Plan Polisher e Plan Executor ja vem inclusos |

---

## Abrir no IDE

| Recurso | O que faz |
|---------|-----------|
| **Abertura de arquivo com um clique** | Arquivos referenciados em sessoes abrem diretamente no seu editor |
| **Detecta seu editor automaticamente** | VS Code, Cursor, Zed e outros — sem necessidade de configuracao |
| **Em todos os lugares que importam** | O botao aparece na aba de Alteracoes, cabecalhos de arquivos e cabecalhos de projeto no Kanban |
| **Memoria de preferencia** | Seu editor preferido e lembrado entre sessoes |

---

## Como e Construido

| | |
|---|---|
| **Rapido** | Backend em Rust com parsing de JSONL acelerado por SIMD, I/O com mapeamento de memoria — indexa milhares de sessoes em segundos |
| **Tempo real** | File watcher + SSE + WebSocket multiplexado com heartbeat, replay de eventos e recuperacao de falhas |
| **Compacto** | ~10 MB de download, ~27 MB em disco. Sem dependencias de runtime, sem daemons em background |
| **100% local** | Todos os dados ficam na sua maquina. Zero telemetria por padrao, zero contas obrigatorias |
| **Zero configuracao** | `npx claude-view` e pronto. Sem chaves de API, sem setup, sem contas |
| **Orientado por FSM** | Sessoes de chat rodam em uma maquina de estados finita com fases explicitas e eventos tipados — deterministico, sem race conditions |

<details>
<summary><strong>Os Numeros</strong></summary>
<br>

Medido em um Mac serie M com 1.493 sessoes em 26 projetos:

| Metrica | claude-view | Painel Electron tipico |
|---------|:-----------:|:----------------------:|
| **Download** | **~10 MB** | 150-300 MB |
| **Em disco** | **~27 MB** | 300-500 MB |
| **Inicializacao** | **< 500 ms** | 3-8 s |
| **RAM (indice completo)** | **~50 MB** | 300-800 MB |
| **Indexar 1.500 sessoes** | **< 1 s** | N/A |
| **Deps de runtime** | **0** | Node.js + Chromium |

Tecnicas-chave: pre-filtro SIMD (`memchr`), parsing JSONL com mapeamento de memoria, busca full-text Tantivy, fatias zero-copy do mmap passando pelo parse ate a resposta.

</details>

---

## Como se Compara

| Ferramenta | Categoria | Stack | Tamanho | Monitor ao vivo | Chat multi-sessao | Busca | Analiticos | Ferramentas MCP |
|------------|-----------|-------|:-------:|:---------------:|:------------------:|:-----:|:----------:|:---------------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | Monitor + workspace | Rust | **~10 MB** | **Sim** | **Sim** | **Sim** | **Sim** | **85** |
| [opcode](https://github.com/winfunc/opcode) | GUI + gerenciador de sessoes | Tauri 2 | ~13 MB | Parcial | Nao | Nao | Sim | Nao |
| [ccusage](https://github.com/ryoppippi/ccusage) | Rastreador de uso CLI | TypeScript | ~600 KB | Nao | Nao | Nao | CLI | Nao |
| [CodePilot](https://github.com/op7418/CodePilot) | UI de chat desktop | Electron | ~140 MB | Nao | Nao | Nao | Nao | Nao |
| [claude-run](https://github.com/kamranahmedse/claude-run) | Visualizador de historico | TypeScript | ~500 KB | Parcial | Nao | Basica | Nao | Nao |

> UIs de chat (CodePilot, CUI, claude-code-webui) sao interfaces *para* o Claude Code. O claude-view e um painel que observa suas sessoes de terminal existentes. Sao complementares.

---

## Instalacao

| Metodo | Comando |
|--------|---------|
| **Shell** (recomendado) | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |
| **Plugin** (auto-start) | `claude plugin add @claude-view/plugin` |

O instalador shell baixa um binario pre-compilado (~10 MB), instala em `~/.claude-view/bin` e adiciona ao seu PATH. Depois basta executar `claude-view`.

**Unico requisito:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) instalado.

<details>
<summary><strong>Configuracao</strong></summary>
<br>

| Variavel de Ambiente | Padrao | Descricao |
|---------------------|--------|-----------|
| `CLAUDE_VIEW_PORT` ou `PORT` | `47892` | Sobrescrever a porta padrao |

</details>

<details>
<summary><strong>Self-Hosting e Desenvolvimento Local</strong></summary>
<br>

O binario pre-compilado vem com autenticacao, compartilhamento e relay mobile integrados. Compilando a partir do codigo-fonte? Esses recursos sao **opt-in via variaveis de ambiente** — omita qualquer um e o recurso sera simplesmente desabilitado.

| Variavel de Ambiente | Recurso | Sem ela |
|---------------------|---------|---------|
| `SUPABASE_URL` | Login/autenticacao | Autenticacao desabilitada — totalmente local, modo zero conta |
| `RELAY_URL` | Pareamento mobile | Pareamento via QR indisponivel |
| `SHARE_WORKER_URL` + `SHARE_VIEWER_URL` | Compartilhamento criptografado | Botao de compartilhar oculto |

```bash
bun dev    # totalmente local, sem dependencias de nuvem
```

</details>

<details>
<summary><strong>Ambientes Empresariais / Sandbox</strong></summary>
<br>

Se sua maquina restringe escrita (DataCloak, CrowdStrike, DLP corporativo):

```bash
cp crates/server/.env.example .env
# Descomente CLAUDE_VIEW_DATA_DIR
```

Isso mantem banco de dados, indice de busca e arquivos de lock dentro do repositorio. Defina `CLAUDE_VIEW_SKIP_HOOKS=1` para pular o registro de hooks em ambientes somente leitura.

</details>

---

## FAQ

<details>
<summary><strong>Banner "Nao conectado" aparecendo mesmo estando logado</strong></summary>
<br>

O claude-view verifica suas credenciais do Claude lendo `~/.claude/.credentials.json` (com fallback para o macOS Keychain). Tente estes passos:

1. **Verifique a autenticacao do CLI do Claude:** `claude auth status`
2. **Verifique o arquivo de credenciais:** `cat ~/.claude/.credentials.json` — deve ter uma secao `claudeAiOauth` com um `accessToken`
3. **Verifique o macOS Keychain:** `security find-generic-password -s "Claude Code-credentials" -w`
4. **Verifique a expiracao do token:** Veja `expiresAt` no JSON de credenciais — se ja passou, execute `claude auth login`
5. **Verifique o HOME:** `echo $HOME` — o servidor le de `$HOME/.claude/.credentials.json`

Se todas as verificacoes passarem e o banner persistir, reporte no [Discord](https://discord.gg/G7wdZTpRfu).

</details>

<details>
<summary><strong>Quais dados o claude-view acessa?</strong></summary>
<br>

O claude-view le os arquivos de sessao JSONL que o Claude Code grava em `~/.claude/projects/`. Ele os indexa localmente usando SQLite e Tantivy. **Nenhum dado sai da sua maquina** a menos que voce use explicitamente o recurso de compartilhamento criptografado. A telemetria e opt-in e desativada por padrao.

</details>

<details>
<summary><strong>Funciona com o Claude Code no VS Code / Cursor / extensoes de IDE?</strong></summary>
<br>

Sim. O claude-view monitora todas as sessoes do Claude Code independentemente de como foram iniciadas — terminal CLI, extensao VS Code, Cursor ou Agent SDK. Cada sessao mostra um badge de origem (Terminal, VS Code, SDK) para que voce possa filtrar por metodo de inicio.

</details>

---

## Comunidade

- **Website:** [claudeview.ai](https://claudeview.ai) — documentacao, changelog, blog
- **Discord:** [Entre no servidor](https://discord.gg/G7wdZTpRfu) — suporte, solicitacoes de funcionalidades, discussao
- **Plugin:** [`@claude-view/plugin`](https://www.npmjs.com/package/@claude-view/plugin) — 85 ferramentas MCP, 9 skills, auto-start

---

<details>
<summary><strong>Desenvolvimento</strong></summary>
<br>

Pre-requisitos: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Instalar todas as dependencias do workspace
bun dev            # Iniciar dev full-stack (Rust + Web + Sidecar com hot reload)
```

### Layout do Workspace

| Caminho | Pacote | Proposito |
|---------|--------|-----------|
| `apps/web/` | `@claude-view/web` | SPA React (Vite) — frontend web principal |
| `apps/share/` | `@claude-view/share` | SPA do visualizador de compartilhamento — Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | App nativo Expo |
| `apps/landing/` | `@claude-view/landing` | Landing page Astro 5 (zero JS client-side) |
| `packages/shared/` | `@claude-view/shared` | Tipos compartilhados e tokens de tema |
| `packages/design-tokens/` | `@claude-view/design-tokens` | Cores, espacamento, tipografia |
| `packages/plugin/` | `@claude-view/plugin` | Plugin do Claude Code (servidor MCP + ferramentas + skills) |
| `crates/` | — | Backend Rust (Axum) |
| `sidecar/` | — | Sidecar Node.js (ponte Agent SDK) |
| `infra/share-worker/` | — | Cloudflare Worker — API de compartilhamento (R2 + D1) |
| `infra/install-worker/` | — | Cloudflare Worker — script de instalacao com rastreamento de downloads |

### Comandos de Desenvolvimento

| Comando | Descricao |
|---------|-----------|
| `bun dev` | Dev full-stack — Rust + Web + Sidecar com hot reload |
| `bun run dev:web` | Apenas frontend web |
| `bun run dev:server` | Apenas backend Rust |
| `bun run build` | Build de todos os workspaces |
| `bun run preview` | Build web + servir via binario de release |
| `bun run lint:all` | Lint JS/TS + Rust (Clippy) |
| `bun run typecheck` | Verificacao de tipos TypeScript |
| `bun run test` | Executar todos os testes (Turbo) |
| `bun run test:rust` | Executar testes Rust |
| `bun run storybook` | Iniciar Storybook para desenvolvimento de componentes |
| `bun run dist:test` | Build + pack + install + run (teste completo de distribuicao) |

### Releases

```bash
bun run release          # patch bump
bun run release:minor    # minor bump
git push origin main --tags    # aciona CI → builds → publica automaticamente no npm
```

</details>

---

## Suporte de Plataformas

| Plataforma | Status |
|------------|--------|
| macOS (Apple Silicon) | Disponivel |
| macOS (Intel) | Disponivel |
| Linux (x64) | Planejado |
| Windows (x64) | Planejado |

---

## Relacionados

- **[claudeview.ai](https://claudeview.ai)** — Website oficial, documentacao e changelog
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** — Plugin do Claude Code com 85 ferramentas MCP e 9 skills. `claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** — O Claude Code deleta suas sessoes apos 30 dias. Isso as salva. `npx claude-backup`

---

<div align="center">

Se o **claude-view** te ajuda a ver o que seus agentes de IA estao fazendo, considere dar uma estrela.

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT &copy; 2026

</div>
