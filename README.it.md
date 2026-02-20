# claude-view

<p align="center">
  <strong>Monitor in tempo reale e copilota per utenti avanzati di Claude Code.</strong>
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

## Il Problema

Hai 3 progetti aperti. Ogni progetto ha molteplici worktree git. Ogni worktree ha diverse sessioni Claude Code in esecuzione. Alcune stanno pensando, altre aspettano il tuo input, alcune stanno per raggiungere i limiti di contesto, e una è finita 10 minuti fa ma te ne sei dimenticato.

Fai Cmd-Tab tra 15 finestre del terminale cercando di ricordare quale sessione stava facendo cosa. Sprechi token perché una cache è scaduta mentre non guardavi. Perdi il flow perché non c'è un singolo posto dove vedere tutto. E dietro quello spinner "sto pensando...", Claude sta generando sub-agenti, chiamando server MCP, eseguendo skill, attivando hook — e tu non puoi vedere niente di tutto questo.

**Claude Code è incredibilmente potente. Ma gestire 10+ sessioni simultanee senza una dashboard è come guidare senza tachimetro.**

## La Soluzione

**claude-view** è una dashboard in tempo reale che funziona affianco alle tue sessioni Claude Code. Una scheda del browser, ogni sessione visibile, contesto completo a colpo d'occhio.

```bash
npx claude-view
```

Tutto qui. Si apre nel browser. Tutte le tue sessioni — live e passate — in un unico workspace.

---

## Cosa Ottieni

### Monitor in Tempo Reale

| Funzionalità | Perché è importante |
|---------|---------------|
| **Card di sessione con ultimo messaggio** | Ricorda istantaneamente su cosa sta lavorando ogni sessione di lunga durata |
| **Suoni di notifica** | Ricevi un avviso quando una sessione finisce o richiede il tuo input — smetti di controllare i terminali |
| **Indicatore di contesto** | Utilizzo della finestra di contesto in tempo reale per sessione — vedi quali sono in zona di pericolo |
| **Conto alla rovescia della cache** | Sappi esattamente quando scade la cache dei prompt per programmare il prossimo messaggio e risparmiare token |
| **Tracciamento costi** | Spesa per sessione e aggregata con dettaglio del risparmio cache |
| **Visualizzazione sub-agenti** | Vedi l'intero albero degli agenti — sub-agenti, il loro stato e quali strumenti stanno chiamando |
| **Viste multiple** | Griglia, Lista o modalità Monitor (griglia chat dal vivo) — scegli quello che si adatta al tuo workflow |

### Cronologia Chat Ricca

| Funzionalità | Perché è importante |
|---------|---------------|
| **Browser di conversazione completo** | Ogni sessione, ogni messaggio, completamente renderizzato con markdown e blocchi di codice |
| **Visualizzazione chiamate strumenti** | Vedi letture file, modifiche, comandi bash, chiamate MCP, invocazioni skill — non solo testo |
| **Toggle compatto / dettagliato** | Scorri la conversazione o approfondisci ogni chiamata strumento |
| **Vista thread** | Segui conversazioni degli agenti con gerarchie di sub-agenti |
| **Esportazione** | Esportazione Markdown per ripresa contesto o condivisione |

### Ricerca Avanzata

| Funzionalità | Perché è importante |
|---------|---------------|
| **Ricerca full-text** | Cerca in tutte le sessioni — messaggi, chiamate strumenti, percorsi file |
| **Filtri progetto e branch** | Limita l'ambito al progetto su cui stai lavorando adesso |
| **Palette comandi** | Cmd+K per saltare tra sessioni, cambiare vista, trovare qualsiasi cosa |

### Interni dell'Agente — Vedi Cosa È Nascosto

Claude Code fa molto dietro "sto pensando..." che non appare mai nel tuo terminale. claude-view espone tutto.

| Funzionalità | Perché è importante |
|---------|---------------|
| **Conversazioni sub-agenti** | Vedi l'intero albero degli agenti generati, i loro prompt e i loro output |
| **Chiamate server MCP** | Vedi quali strumenti MCP vengono invocati e i loro risultati |
| **Tracciamento skill / hook / plugin** | Sappi quali skill si sono attivate, quali hook sono stati eseguiti, quali plugin sono attivi |
| **Registrazione eventi hook** | Ogni evento hook è catturato e navigabile — torna a controllare cosa si è attivato e quando. *(Richiede che claude-view sia in esecuzione mentre le sessioni sono attive; non può tracciare eventi storici retroattivamente)* |
| **Timeline utilizzo strumenti** | Log di azioni di ogni coppia tool_use/tool_result con tempistica |
| **Emersione errori** | Gli errori emergono sulla card della sessione — niente più fallimenti nascosti |
| **Ispettore messaggi raw** | Approfondisci il JSON raw di qualsiasi messaggio quando hai bisogno del quadro completo |

### Analytics

Una ricca suite analitica per il tuo uso di Claude Code. Pensa alla dashboard di Cursor, ma più profonda.

**Panoramica Dashboard**

| Funzionalità | Descrizione |
|---------|-------------|
| **Metriche settimana per settimana** | Conteggio sessioni, utilizzo token, costo — confrontato con il periodo precedente |
| **Mappa di calore attività** | Griglia stile GitHub di 90 giorni che mostra l'intensità giornaliera del tuo uso di Claude Code |
| **Top skill / comandi / strumenti MCP / agenti** | Classifiche dei tuoi invocabili più usati — clicca su qualsiasi per cercare sessioni corrispondenti |
| **Progetti più attivi** | Grafico a barre dei progetti classificati per conteggio sessioni |
| **Dettaglio utilizzo strumenti** | Totale modifiche, letture e comandi bash in tutte le sessioni |
| **Sessioni più lunghe** | Accesso rapido alle tue sessioni maratona con durata |

**Contributi IA**

| Funzionalità | Descrizione |
|---------|-------------|
| **Tracciamento output codice** | Righe aggiunte/rimosse, file toccati, conteggio commit — in tutte le sessioni |
| **Metriche ROI costo** | Costo per commit, costo per sessione, costo per riga di output IA — con grafici di tendenza |
| **Confronto modelli** | Dettaglio affiancato di output ed efficienza per modello (Opus, Sonnet, Haiku) |
| **Curva di apprendimento** | Tasso di ri-modifica nel tempo — vedi te stesso migliorare nel prompting |
| **Dettaglio per branch** | Vista collassabile per branch con drill-down sessioni |
| **Efficacia skill** | Quali skill migliorano realmente il tuo output vs quali no |

**Insights** *(sperimentale)*

| Funzionalità | Descrizione |
|---------|-------------|
| **Rilevamento pattern** | Pattern comportamentali scoperti dalla tua cronologia sessioni |
| **Benchmark allora vs adesso** | Confronta il tuo primo mese con l'uso recente |
| **Dettaglio per categoria** | Treemap di come usi Claude — refactoring, feature, debugging, ecc. |
| **Punteggio Fluenza IA** | Un singolo numero 0-100 che traccia la tua efficacia complessiva |

> **Nota:** Insights e Punteggio Fluenza sono in fase sperimentale iniziale. Considerali come direzionali, non definitivi.

---

## Progettato Per il Flow

claude-view è progettato per lo sviluppatore che:

- Esegue **3+ progetti simultaneamente**, ognuno con molteplici worktree
- Ha **10-20 sessioni Claude Code** aperte in qualsiasi momento
- Ha bisogno di cambiare contesto velocemente senza perdere traccia di cosa è in esecuzione
- Vuole **ottimizzare la spesa di token** programmando messaggi attorno alle finestre di cache
- È frustrato dal Cmd-Tab tra terminali per controllare gli agenti

Una scheda del browser. Tutte le sessioni. Resta nel flow.

---

## Come È Costruito

| | |
|---|---|
| **Ultra veloce** | Backend Rust con parsing JSONL accelerato SIMD, I/O mappato in memoria — indicizza migliaia di sessioni in secondi |
| **Tempo reale** | File watcher + SSE + WebSocket per aggiornamenti live sub-secondo in tutte le sessioni |
| **Impronta minima** | Singolo binario di ~15 MB. Nessuna dipendenza runtime, nessun daemon in background |
| **100% locale** | Tutti i dati restano sulla tua macchina. Zero telemetria, zero cloud, zero richieste di rete |
| **Zero configurazione** | `npx claude-view` e fatto. Nessuna API key, nessun setup, nessun account |

---

## Avvio Rapido

```bash
npx claude-view
```

Si apre su `http://localhost:47892`.

### Configurazione

| Variabile d'Ambiente | Predefinito | Descrizione |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` o `PORT` | `47892` | Sovrascrivere la porta predefinita |

---

## Installazione

| Metodo | Comando |
|--------|---------|
| **npx** (raccomandato) | `npx claude-view` |
| **Script shell** (Node non richiesto) | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

### Requisiti

- **Claude Code** installato ([scaricalo qui](https://docs.anthropic.com/en/docs/claude-code)) — questo crea i file di sessione che monitoriamo

---

## Confronto

Gli altri strumenti sono visualizzatori (navigazione cronologia) o semplici monitor. Nessuno combina monitoraggio in tempo reale, cronologia chat ricca, strumenti di debugging e ricerca avanzata in un singolo workspace.

```
                    Passivo ←————————————→ Attivo
                         |                  |
            Solo         |  ccusage         |
            visualizzare |  History Viewer  |
                         |  clog            |
                         |                  |
            Solo         |  claude-code-ui  |
            monitor      |  Agent Sessions  |
                         |                  |
            Workspace    |  ★ claude-view   |
            completo     |                  |
```

---

## Community

Unisciti al [server Discord](https://discord.gg/G7wdZTpRfu) per supporto, richieste di funzionalità e discussione.

---

## Ti piace questo progetto?

Se **claude-view** ti aiuta a padroneggiare Claude Code, considera di dargli una stella. Aiuta gli altri a scoprire questo strumento.

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## Sviluppo

Prerequisiti: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Installare dipendenze frontend
bun dev            # Avviare sviluppo full-stack (Rust + Vite con hot reload)
```

| Comando | Descrizione |
|---------|-------------|
| `bun dev` | Sviluppo full-stack — Rust si riavvia automaticamente alle modifiche, Vite HMR |
| `bun dev:server` | Solo backend Rust (con cargo-watch) |
| `bun dev:client` | Solo frontend Vite (assume backend in esecuzione) |
| `bun run build` | Compilare frontend per produzione |
| `bun run preview` | Compilare + servire via binario release |
| `bun run lint` | Lint frontend (ESLint) e backend (Clippy) |
| `bun run fmt` | Formattare codice Rust |
| `bun run check` | Typecheck + lint + test (gate pre-commit) |
| `bun test` | Eseguire suite test Rust (`cargo test --workspace`) |
| `bun test:client` | Eseguire test frontend (vitest) |
| `bun run test:e2e` | Eseguire test end-to-end Playwright |

### Test della Distribuzione di Produzione

```bash
bun run dist:test    # Un comando: build → pack → install → run
```

O passo per passo:

| Comando | Descrizione |
|---------|-------------|
| `bun run dist:pack` | Impacchettare binario + frontend in tarball su `/tmp/` |
| `bun run dist:install` | Estrarre tarball in `~/.cache/claude-view/` (simula primo download) |
| `bun run dist:run` | Eseguire wrapper npx usando binario in cache |
| `bun run dist:test` | Tutto quanto sopra in un solo comando |
| `bun run dist:clean` | Rimuovere tutti i file cache dist e temporanei |

### Rilascio

```bash
bun run release          # bump patch: 0.1.0 → 0.1.1
bun run release:minor    # bump minor: 0.1.0 → 0.2.0
bun run release:major    # bump major: 0.1.0 → 1.0.0
```

Questo incrementa la versione in `npx-cli/package.json`, fa commit e crea un tag git. Poi:

```bash
git push origin main --tags    # attiva CI → compila tutte le piattaforme → auto-pubblica su npm
```

---

## Supporto Piattaforme

| Piattaforma | Stato |
|----------|--------|
| macOS (Apple Silicon) | Disponibile |
| macOS (Intel) | Disponibile |
| Linux (x64) | Pianificato |
| Windows (x64) | Pianificato |

---

## Licenza

MIT © 2026
