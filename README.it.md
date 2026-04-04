<div align="center">

# claude-view

**Centro di Controllo per Claude Code**

Hai 10 agenti IA in esecuzione. Uno ha finito 12 minuti fa. Un altro ha raggiunto il limite di contesto. Un terzo necessita dell'approvazione di uno strumento. Stai facendo <kbd>Cmd</kbd>+<kbd>Tab</kbd> tra i terminali, spendendo 200$/mese alla cieca.

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

**Un solo comando. Ogni sessione visibile. In tempo reale.**

</div>

---

## Cos'e claude-view?

claude-view e una dashboard open-source che monitora ogni sessione di Claude Code sulla tua macchina -- agenti attivi, conversazioni passate, costi, sotto-agenti, hook, chiamate agli strumenti -- tutto in un unico posto. Backend in Rust, frontend in React, binario di ~10 MB. Zero configurazione, zero account, 100% locale.

**30 release. 86 strumenti MCP. 9 skill. Un solo `npx claude-view`.**

---

## Monitor in Tempo Reale

Visualizza ogni sessione in esecuzione a colpo d'occhio. Basta cambiare scheda del terminale.

| Funzionalita | Cosa fa |
|---------|-------------|
| **Schede sessione** | Ogni scheda mostra l'ultimo messaggio, il modello, il costo e lo stato -- sai istantaneamente su cosa sta lavorando ogni agente |
| **Chat multi-sessione** | Apri sessioni affiancate in schede stile VS Code (dockview). Trascina per dividere orizzontalmente o verticalmente |
| **Indicatore di contesto** | Riempimento della finestra di contesto in tempo reale per sessione -- vedi quali agenti sono nella zona critica prima che raggiungano il limite |
| **Conto alla rovescia della cache** | Sai esattamente quando scade la cache dei prompt, cosi puoi temporizzare i messaggi per risparmiare token |
| **Tracciamento dei costi** | Spesa per sessione e aggregata con dettaglio token -- passa il mouse per vedere la suddivisione input/output/cache per modello |
| **Albero dei sotto-agenti** | Visualizza l'intero albero degli agenti generati, il loro stato, i costi e quali strumenti stanno chiamando |
| **Notifiche sonore** | Ricevi un avviso quando una sessione termina, va in errore o richiede il tuo input -- smetti di controllare i terminali |
| **Viste multiple** | Griglia, Lista, Kanban o modalita Monitor -- scegli quella che si adatta al tuo flusso di lavoro |
| **Corsie Kanban** | Raggruppa le sessioni per progetto o branch -- layout a corsie per flussi di lavoro multi-progetto |
| **Chiuse di recente** | Le sessioni che terminano appaiono in "Chiuse di recente" invece di scomparire -- persistono tra i riavvii del server |
| **Messaggi in coda** | I messaggi in attesa nella coda appaiono come bolle in sospeso con un badge "In coda" |
| **Basato su SSE** | Tutti i dati live inviati tramite Server-Sent Events -- elimina completamente il rischio di cache obsoleta |

---

## Chat e Conversazione

Leggi, cerca e interagisci con qualsiasi sessione -- attiva o storica.

| Funzionalita | Cosa fa |
|---------|-------------|
| **Chat live unificata** | Cronologia e messaggi in tempo reale in un'unica conversazione scorrevole -- nessun cambio di scheda |
| **Modalita sviluppatore** | Alterna tra le viste Chat e Sviluppatore per ogni sessione. La modalita sviluppatore mostra schede strumenti, schede eventi, metadati degli hook e la traccia di esecuzione completa con chip di filtro |
| **Browser completo delle conversazioni** | Ogni sessione, ogni messaggio, completamente renderizzato con markdown e blocchi di codice |
| **Visualizzazione delle chiamate agli strumenti** | Vedi letture file, modifiche, comandi bash, chiamate MCP, invocazioni di skill -- non solo testo |
| **Interruttore compatto/dettagliato** | Scorri la conversazione o approfondisci ogni chiamata agli strumenti |
| **Vista thread** | Segui le conversazioni degli agenti con gerarchie di sotto-agenti e indentazione |
| **Eventi hook inline** | Gli hook pre/post strumento renderizzati come blocchi di conversazione -- vedi gli hook attivarsi insieme alla conversazione |
| **Esportazione** | Esportazione in Markdown per riprendere il contesto o condividere |
| **Selezione multipla e archiviazione** | Seleziona piu sessioni per l'archiviazione in batch con stato dei filtri persistente |
| **Condivisione crittografata** | Condividi qualsiasi sessione tramite link crittografato end-to-end -- AES-256-GCM, zero fiducia nel server, la chiave vive solo nel frammento URL |

---

## Dettagli Interni dell'Agente

Claude Code fa molte cose dietro `"thinking..."` che non appaiono mai nel tuo terminale. claude-view espone tutto.

| Funzionalita | Cosa fa |
|---------|-------------|
| **Conversazioni dei sotto-agenti** | Albero completo degli agenti generati, i loro prompt, output e dettaglio costi/token per agente |
| **Chiamate ai server MCP** | Quali strumenti MCP vengono invocati e i loro risultati |
| **Tracciamento skill/hook/plugin** | Quali skill si sono attivate, quali hook sono stati eseguiti, quali plugin sono attivi |
| **Registrazione eventi hook** | Cattura hook a doppio canale (WebSocket live + backfill JSONL) -- ogni evento registrato e navigabile, anche per sessioni passate |
| **Badge sorgente sessione** | Ogni sessione mostra come e stata avviata: Terminal, VS Code, Agent SDK o altri punti di ingresso |
| **Divergenza branch worktree** | Rileva quando i branch dei git worktree divergono -- mostrato nel monitor live e nella cronologia |
| **Chip menzioni @File** | I riferimenti `@filename` estratti e mostrati come chip -- passa il mouse per il percorso completo |
| **Timeline uso strumenti** | Log di azione di ogni coppia tool_use/tool_result con tempistica |
| **Emersione errori** | Gli errori emergono nella scheda sessione -- nessun errore nascosto |
| **Ispettore messaggi grezzi** | Analizza il JSON grezzo di qualsiasi messaggio quando ti serve il quadro completo |

---

## Ricerca

| Funzionalita | Cosa fa |
|---------|-------------|
| **Ricerca full-text** | Cerca in tutte le sessioni -- messaggi, chiamate agli strumenti, percorsi file. Alimentata da Tantivy (nativa in Rust, classe Lucene) |
| **Motore di ricerca unificato** | Tantivy full-text + pre-filtro SQLite eseguiti in parallelo -- un solo endpoint, risultati in meno di 50ms |
| **Filtri per progetto e branch** | Limita al progetto o branch su cui stai lavorando adesso |
| **Palette comandi** | <kbd>Cmd</kbd>+<kbd>K</kbd> per saltare tra sessioni, cambiare vista, trovare qualsiasi cosa |

---

## Analitica

Una suite analitica completa per il tuo utilizzo di Claude Code. Come la dashboard di Cursor, ma piu approfondita.

<details>
<summary><strong>Dashboard</strong></summary>
<br>

| Funzionalita | Descrizione |
|---------|-------------|
| **Metriche settimana su settimana** | Conteggio sessioni, utilizzo token, costi -- confrontati con il periodo precedente |
| **Mappa di attivita** | Griglia a 90 giorni in stile GitHub che mostra l'intensita di utilizzo giornaliero |
| **Top skill/comandi/strumenti MCP/agenti** | Classifiche dei tuoi invocabili piu utilizzati -- clicca su uno qualsiasi per cercare le sessioni corrispondenti |
| **Progetti piu attivi** | Grafico a barre dei progetti classificati per numero di sessioni |
| **Dettaglio utilizzo strumenti** | Totale modifiche, letture e comandi bash in tutte le sessioni |
| **Sessioni piu lunghe** | Accesso rapido alle tue sessioni maratona con durata |

</details>

<details>
<summary><strong>Contributi IA</strong></summary>
<br>

| Funzionalita | Descrizione |
|---------|-------------|
| **Tracciamento output codice** | Righe aggiunte/rimosse, file toccati, conteggio commit -- in tutte le sessioni |
| **Metriche ROI dei costi** | Costo per commit, costo per sessione, costo per riga di output IA -- con grafici di tendenza |
| **Confronto modelli** | Dettaglio affiancato di output ed efficienza per modello (Opus, Sonnet, Haiku) |
| **Curva di apprendimento** | Tasso di ri-modifica nel tempo -- osserva come migliori nel prompting |
| **Dettaglio per branch** | Vista espandibile per branch con drill-down nelle sessioni |
| **Efficacia delle skill** | Quali skill migliorano effettivamente il tuo output e quali no |

</details>

<details>
<summary><strong>Approfondimenti</strong> <em>(sperimentale)</em></summary>
<br>

| Funzionalita | Descrizione |
|---------|-------------|
| **Rilevamento pattern** | Pattern comportamentali scoperti dalla tua cronologia sessioni |
| **Benchmark prima vs dopo** | Confronta il tuo primo mese con l'utilizzo recente |
| **Suddivisione per categoria** | Treemap di come usi Claude -- refactoring, funzionalita, debugging, ecc. |
| **Punteggio Fluenza IA** | Un singolo numero 0-100 che traccia la tua efficacia complessiva |

> Approfondimenti e Punteggio Fluenza sono sperimentali. Da considerare come indicativi, non definitivi.

</details>

---

## Piani, Prompt e Team

| Funzionalita | Cosa fa |
|---------|-------------|
| **Browser dei piani** | Visualizza i tuoi `.claude/plans/` direttamente nel dettaglio sessione -- basta cercare tra i file |
| **Cronologia prompt** | Ricerca full-text in tutti i prompt che hai inviato con clustering di template e classificazione dell'intento |
| **Dashboard team** | Visualizza i team leader, i messaggi in arrivo, le attivita del team e le modifiche ai file di tutti i membri del team |
| **Analitica prompt** | Classifiche dei template di prompt, distribuzione degli intenti e statistiche di utilizzo |

---

## Monitor di Sistema

| Funzionalita | Cosa fa |
|---------|-------------|
| **Indicatori live CPU/RAM/Disco** | Metriche di sistema in tempo reale in streaming via SSE con transizioni animate fluide |
| **Dashboard componenti** | Visualizza le metriche del sidecar e dell'IA on-device: utilizzo VRAM, CPU, RAM e conteggio sessioni per componente |
| **Lista processi** | Processi raggruppati per nome, ordinati per CPU -- vedi cosa sta realmente facendo la tua macchina mentre gli agenti lavorano |

---

## IA On-Device

Esegui un LLM locale per la classificazione delle fasi della sessione -- nessuna chiamata API, nessun costo aggiuntivo.

| Funzionalita | Cosa fa |
|---------|-------------|
| **Agnostico rispetto al provider** | Connettiti a qualsiasi endpoint compatibile con OpenAI -- oMLX, Ollama, LM Studio o il tuo server |
| **Selettore modello** | Scegli da un registro curato di modelli con i requisiti RAM visualizzati |
| **Classificazione delle fasi** | Le sessioni etichettate con la loro fase attuale (codifica, debugging, pianificazione, ecc.) usando una visualizzazione con soglia di confidenza |
| **Gestione intelligente delle risorse** | Classificazione stabilizzata con EMA e backoff esponenziale -- 93% di riduzione dello spreco GPU rispetto al polling ingenuo |

---

## Plugin

`@claude-view/plugin` offre a Claude accesso nativo ai dati della tua dashboard -- 86 strumenti MCP, 9 skill e avvio automatico.

```bash
claude plugin add @claude-view/plugin
```

### Avvio Automatico

Ogni sessione di Claude Code avvia automaticamente la dashboard. Nessun `npx claude-view` manuale necessario.

### 86 strumenti MCP

8 strumenti realizzati a mano con output ottimizzato per Claude:

| Strumento | Descrizione |
|------|-------------|
| `list_sessions` | Esplora le sessioni con filtri |
| `get_session` | Dettaglio completo della sessione con messaggi e metriche |
| `search_sessions` | Ricerca full-text in tutte le conversazioni |
| `get_stats` | Panoramica della dashboard -- sessioni totali, costi, tendenze |
| `get_fluency_score` | Punteggio Fluenza IA (0-100) con dettaglio |
| `get_token_stats` | Utilizzo token con rapporto di hit della cache |
| `list_live_sessions` | Agenti attualmente in esecuzione (tempo reale) |
| `get_live_summary` | Costi aggregati e stato per oggi |

Piu **78 strumenti auto-generati** dalla specifica OpenAPI in 27 categorie (contributi, approfondimenti, coaching, esportazioni, workflow e altro).

### 9 Skill

| Skill | Cosa fa |
|-------|-------------|
| `/session-recap` | Riassumi una sessione specifica -- commit, metriche, durata |
| `/daily-cost` | Spesa odierna, sessioni in corso, utilizzo token |
| `/standup` | Log di lavoro multi-sessione per aggiornamenti standup |
| `/coaching` | Suggerimenti di coaching IA e gestione regole personalizzate |
| `/insights` | Analisi dei pattern comportamentali |
| `/project-overview` | Riepilogo del progetto attraverso le sessioni |
| `/search` | Ricerca in linguaggio naturale |
| `/export-data` | Esporta sessioni in CSV/JSON |
| `/team-status` | Panoramica dell'attivita del team |

---

## Workflow

| Funzionalita | Cosa fa |
|---------|-------------|
| **Costruttore di workflow** | Crea workflow multi-fase con layout stile VS Code, anteprima diagramma Mermaid ed editor YAML |
| **Chat rail LLM in streaming** | Genera definizioni di workflow in tempo reale tramite chat integrata |
| **Esecutore delle fasi** | Visualizza colonne di fase, schede dei tentativi e barra di avanzamento durante l'esecuzione del workflow |
| **Workflow seed integrati** | Plan Polisher e Plan Executor inclusi nella dotazione |

---

## Apri nell'IDE

| Funzionalita | Cosa fa |
|---------|-------------|
| **Apertura file con un clic** | I file referenziati nelle sessioni si aprono direttamente nel tuo editor |
| **Rilevamento automatico dell'editor** | VS Code, Cursor, Zed e altri -- nessuna configurazione necessaria |
| **Ovunque sia utile** | Il pulsante appare nella scheda Modifiche, nelle intestazioni dei file e nelle intestazioni dei progetti Kanban |
| **Memoria delle preferenze** | Il tuo editor preferito viene ricordato tra le sessioni |

---

## Come e Costruito

| | |
|---|---|
| **Veloce** | Backend in Rust con parsing JSONL accelerato da SIMD, I/O memory-mapped -- indicizza migliaia di sessioni in secondi |
| **Tempo reale** | File watcher + SSE + WebSocket multiplexato con heartbeat, replay degli eventi e recupero dai crash |
| **Leggero** | ~10 MB di download, ~27 MB su disco. Nessuna dipendenza runtime, nessun demone in background |
| **100% locale** | Tutti i dati restano sulla tua macchina. Zero telemetria di default, zero account richiesti |
| **Zero configurazione** | `npx claude-view` e hai finito. Nessuna chiave API, nessun setup, nessun account |
| **Guidato da FSM** | Le sessioni di chat funzionano su una macchina a stati finiti con fasi esplicite ed eventi tipizzati -- deterministico, senza race condition |

<details>
<summary><strong>I Numeri</strong></summary>
<br>

Misurato su un Mac serie M con 1.493 sessioni in 26 progetti:

| Metrica | claude-view | Dashboard Electron tipica |
|--------|:-----------:|:--------------------------:|
| **Download** | **~10 MB** | 150-300 MB |
| **Su disco** | **~27 MB** | 300-500 MB |
| **Avvio** | **< 500 ms** | 3-8 s |
| **RAM (indice completo)** | **~50 MB** | 300-800 MB |
| **Indicizzazione 1.500 sessioni** | **< 1 s** | N/A |
| **Dipendenze runtime** | **0** | Node.js + Chromium |

Tecniche chiave: pre-filtro SIMD (`memchr`), parsing JSONL memory-mapped, ricerca full-text Tantivy, slice zero-copy dalla mmap attraverso il parsing fino alla risposta.

</details>

---

## Confronto

| Strumento | Categoria | Stack | Dimensione | Monitor live | Chat multi-sessione | Ricerca | Analitica | Strumenti MCP |
|------|----------|-------|:----:|:------------:|:------------------:|:------:|:---------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | Monitor + workspace | Rust | **~10 MB** | **Si** | **Si** | **Si** | **Si** | **86** |
| [opcode](https://github.com/winfunc/opcode) | GUI + gestore sessioni | Tauri 2 | ~13 MB | Parziale | No | No | Si | No |
| [ccusage](https://github.com/ryoppippi/ccusage) | Tracker utilizzo CLI | TypeScript | ~600 KB | No | No | No | CLI | No |
| [CodePilot](https://github.com/op7418/CodePilot) | UI chat desktop | Electron | ~140 MB | No | No | No | No | No |
| [claude-run](https://github.com/kamranahmedse/claude-run) | Visualizzatore cronologia | TypeScript | ~500 KB | Parziale | No | Base | No | No |

> Le UI di chat (CodePilot, CUI, claude-code-webui) sono interfacce *per* Claude Code. claude-view e una dashboard che osserva le tue sessioni terminal esistenti. Sono complementari.

---

## Installazione

| Metodo | Comando |
|--------|---------|
| **Shell** (consigliato) | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |
| **Plugin** (avvio automatico) | `claude plugin add @claude-view/plugin` |

L'installer shell scarica un binario pre-compilato (~10 MB), lo installa in `~/.claude-view/bin` e lo aggiunge al tuo PATH. Poi basta eseguire `claude-view`.

**Unico requisito:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) installato.

<details>
<summary><strong>Configurazione</strong></summary>
<br>

| Variabile d'ambiente | Default | Descrizione |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` o `PORT` | `47892` | Sovrascrive la porta predefinita |

</details>

<details>
<summary><strong>Self-Hosting e Sviluppo Locale</strong></summary>
<br>

Il binario pre-compilato include autenticazione, condivisione e relay mobile integrati. Compili dal sorgente? Queste funzionalita sono **opt-in tramite variabili d'ambiente** -- omettine una qualsiasi e quella funzionalita viene semplicemente disabilitata.

| Variabile d'ambiente | Funzionalita | Senza di essa |
|-------------|---------|------------|
| `SUPABASE_URL` | Login / autenticazione | Autenticazione disabilitata -- modalita completamente locale, zero account |
| `RELAY_URL` | Pairing mobile | Pairing QR non disponibile |
| `SHARE_WORKER_URL` + `SHARE_VIEWER_URL` | Condivisione crittografata | Pulsante condivisione nascosto |

```bash
bun dev    # completamente locale, nessuna dipendenza cloud
```

</details>

<details>
<summary><strong>Enterprise / Ambienti Sandbox</strong></summary>
<br>

Se la tua macchina limita le scritture (DataCloak, CrowdStrike, DLP aziendale):

```bash
cp crates/server/.env.example .env
# Decommenta CLAUDE_VIEW_DATA_DIR
```

Questo mantiene database, indice di ricerca e file di lock all'interno del repository. Imposta `CLAUDE_VIEW_SKIP_HOOKS=1` per saltare la registrazione degli hook in ambienti di sola lettura.

</details>

---

## FAQ

<details>
<summary><strong>Il banner "Non autenticato" appare anche se ho effettuato l'accesso</strong></summary>
<br>

claude-view verifica le tue credenziali Claude leggendo `~/.claude/.credentials.json` (con fallback al Portachiavi macOS). Prova questi passaggi:

1. **Verifica l'autenticazione CLI di Claude:** `claude auth status`
2. **Controlla il file delle credenziali:** `cat ~/.claude/.credentials.json` -- dovrebbe avere una sezione `claudeAiOauth` con un `accessToken`
3. **Controlla il Portachiavi macOS:** `security find-generic-password -s "Claude Code-credentials" -w`
4. **Controlla la scadenza del token:** Guarda `expiresAt` nel JSON delle credenziali -- se scaduto, esegui `claude auth login`
5. **Controlla HOME:** `echo $HOME` -- il server legge da `$HOME/.claude/.credentials.json`

Se tutti i controlli passano e il banner persiste, segnalalo su [Discord](https://discord.gg/G7wdZTpRfu).

</details>

<details>
<summary><strong>A quali dati accede claude-view?</strong></summary>
<br>

claude-view legge i file di sessione JSONL che Claude Code scrive in `~/.claude/projects/`. Li indicizza localmente usando SQLite e Tantivy. **Nessun dato lascia la tua macchina** a meno che tu non utilizzi esplicitamente la funzionalita di condivisione crittografata. La telemetria e opt-in e disattivata di default.

</details>

<details>
<summary><strong>Funziona con Claude Code in VS Code / Cursor / estensioni IDE?</strong></summary>
<br>

Si. claude-view monitora tutte le sessioni di Claude Code indipendentemente da come sono state avviate -- CLI del terminale, estensione VS Code, Cursor o Agent SDK. Ogni sessione mostra un badge sorgente (Terminal, VS Code, SDK) cosi puoi filtrare per metodo di avvio.

</details>

---

## Community

- **Sito web:** [claudeview.ai](https://claudeview.ai) -- documentazione, changelog, blog
- **Discord:** [Unisciti al server](https://discord.gg/G7wdZTpRfu) -- supporto, richieste di funzionalita, discussione
- **Plugin:** [`@claude-view/plugin`](https://www.npmjs.com/package/@claude-view/plugin) -- 86 strumenti MCP, 9 skill, avvio automatico

---

<details>
<summary><strong>Sviluppo</strong></summary>
<br>

Prerequisiti: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Installa tutte le dipendenze del workspace
bun dev            # Avvia lo sviluppo full-stack (Rust + Web + Sidecar con hot reload)
```

### Layout del Workspace

| Percorso | Pacchetto | Scopo |
|------|---------|---------|
| `apps/web/` | `@claude-view/web` | SPA React (Vite) -- frontend web principale |
| `apps/share/` | `@claude-view/share` | SPA del visualizzatore condivisione -- Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | App nativa Expo |
| `apps/landing/` | `@claude-view/landing` | Landing page Astro 5 (zero JavaScript lato client) |
| `packages/shared/` | `@claude-view/shared` | Tipi condivisi e token del tema |
| `packages/design-tokens/` | `@claude-view/design-tokens` | Colori, spaziature, tipografia |
| `packages/plugin/` | `@claude-view/plugin` | Plugin Claude Code (server MCP + strumenti + skill) |
| `crates/` | -- | Backend Rust (Axum) |
| `sidecar/` | -- | Sidecar Node.js (bridge Agent SDK) |
| `infra/share-worker/` | -- | Cloudflare Worker -- API di condivisione (R2 + D1) |
| `infra/install-worker/` | -- | Cloudflare Worker -- script di installazione con tracciamento download |

### Comandi di Sviluppo

| Comando | Descrizione |
|---------|-------------|
| `bun dev` | Sviluppo full-stack -- Rust + Web + Sidecar con hot reload |
| `bun run dev:web` | Solo frontend web |
| `bun run dev:server` | Solo backend Rust |
| `bun run build` | Compila tutti i workspace |
| `bun run preview` | Compila il web + servi tramite binario release |
| `bun run lint:all` | Lint JS/TS + Rust (Clippy) |
| `bun run typecheck` | Controllo tipi TypeScript |
| `bun run test` | Esegui tutti i test (Turbo) |
| `bun run test:rust` | Esegui i test Rust |
| `bun run storybook` | Avvia Storybook per lo sviluppo dei componenti |
| `bun run dist:test` | Compila + impacchetta + installa + esegui (test di distribuzione completo) |

### Rilascio

```bash
bun run release          # patch bump
bun run release:minor    # minor bump
git push origin main --tags    # attiva la CI → compila → pubblica automaticamente su npm
```

</details>

---

## Supporto Piattaforme

| Piattaforma | Stato |
|----------|--------|
| macOS (Apple Silicon) | Disponibile |
| macOS (Intel) | Disponibile |
| Linux (x64) | Pianificato |
| Windows (x64) | Pianificato |

---

## Correlati

- **[claudeview.ai](https://claudeview.ai)** -- Sito ufficiale, documentazione e changelog
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** -- Plugin Claude Code con 86 strumenti MCP e 9 skill. `claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** -- Claude Code elimina le tue sessioni dopo 30 giorni. Questo le salva. `npx claude-backup`

---

<div align="center">

Se **claude-view** ti aiuta a vedere cosa stanno facendo i tuoi agenti IA, considera di lasciare una stella.

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT &copy; 2026

</div>
