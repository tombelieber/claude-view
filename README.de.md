# claude-view

<p align="center">
  <strong>Live-Monitor & Copilot für Claude Code Power-User.</strong>
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

## Das Problem

Du hast 3 Projekte offen. Jedes Projekt hat mehrere Git-Worktrees. Jeder Worktree hat mehrere Claude Code Sessions laufen. Manche denken nach, manche warten auf dich, manche erreichen gleich die Kontextlimits, und eine ist vor 10 Minuten fertig geworden, aber du hast sie vergessen.

Du wechselst mit Cmd-Tab durch 15 Terminalfenster und versuchst dich zu erinnern, welche Session was gemacht hat. Du verschwendest Tokens, weil ein Cache abgelaufen ist, während du nicht hingeschaut hast. Du verlierst deinen Flow, weil es keinen einzelnen Ort gibt, um alles zu sehen. Und hinter dem "denke nach..."-Spinner erzeugt Claude Sub-Agents, ruft MCP-Server auf, führt Skills aus, löst Hooks aus — und du kannst nichts davon sehen.

**Claude Code ist unglaublich leistungsfähig. Aber 10+ gleichzeitige Sessions ohne Dashboard zu fliegen, ist wie Autofahren ohne Tachometer.**

## Die Lösung

**claude-view** ist ein Echtzeit-Dashboard, das neben deinen Claude Code Sessions läuft. Ein Browser-Tab, jede Session sichtbar, voller Kontext auf einen Blick.

```bash
npx claude-view
```

Das war's. Öffnet sich im Browser. Alle deine Sessions — live und vergangene — in einem Workspace.

---

## Was Du Bekommst

### Live-Monitor

| Feature | Warum es wichtig ist |
|---------|---------------|
| **Sessionkarten mit letzter Nachricht** | Erinnere dich sofort, woran jede langlaufende Session arbeitet |
| **Benachrichtigungstöne** | Werde benachrichtigt, wenn eine Session fertig ist oder deine Eingabe braucht — hör auf, Terminals zu pollen |
| **Kontextanzeige** | Echtzeit-Kontextfenster-Nutzung pro Session — sieh, welche in der Gefahrenzone sind |
| **Cache-Warm-Countdown** | Wisse genau, wann der Prompt-Cache abläuft, um deine nächste Nachricht zu timen und Tokens zu sparen |
| **Kostenverfolgung** | Ausgaben pro Session und aggregiert mit Aufschlüsselung der Cache-Einsparungen |
| **Sub-Agent-Visualisierung** | Sieh den vollständigen Agent-Baum — Sub-Agents, ihren Status und welche Tools sie aufrufen |
| **Mehrere Ansichten** | Grid, Liste oder Monitor-Modus (Live-Chat-Grid) — wähle, was zu deinem Workflow passt |

### Reichhaltiger Chat-Verlauf

| Feature | Warum es wichtig ist |
|---------|---------------|
| **Vollständiger Konversationsbrowser** | Jede Session, jede Nachricht, vollständig gerendert mit Markdown und Codeblöcken |
| **Tool-Aufruf-Visualisierung** | Sieh Datei-Lese, Bearbeitungen, Bash-Befehle, MCP-Aufrufe, Skill-Invokationen — nicht nur Text |
| **Kompakt-/Detail-Toggle** | Überfliege die Konversation oder tauche in jeden Tool-Aufruf ein |
| **Thread-Ansicht** | Verfolge Agent-Konversationen mit Sub-Agent-Hierarchien |
| **Export** | Markdown-Export für Kontextwiederaufnahme oder Teilen |

### Erweiterte Suche

| Feature | Warum es wichtig ist |
|---------|---------------|
| **Volltextsuche** | Suche über alle Sessions — Nachrichten, Tool-Aufrufe, Dateipfade |
| **Projekt- & Branch-Filter** | Beschränke auf das Projekt, an dem du gerade arbeitest |
| **Befehlspalette** | Cmd+K zum Springen zwischen Sessions, Ansichten wechseln, alles finden |

### Agent-Interna — Sieh Was Verborgen Ist

Claude Code tut viel hinter "denke nach...", das nie in deinem Terminal erscheint. claude-view legt alles offen.

| Feature | Warum es wichtig ist |
|---------|---------------|
| **Sub-Agent-Konversationen** | Sieh den vollständigen Baum der erzeugten Agents, ihre Prompts und ihre Ausgaben |
| **MCP-Server-Aufrufe** | Sieh, welche MCP-Tools aufgerufen werden und ihre Ergebnisse |
| **Skill-/Hook-/Plugin-Tracking** | Wisse, welche Skills ausgelöst wurden, welche Hooks liefen, welche Plugins aktiv sind |
| **Hook-Event-Aufzeichnung** | Jedes Hook-Event wird erfasst und ist durchsuchbar — prüfe, was wann ausgelöst wurde. *(Erfordert, dass claude-view läuft, während Sessions aktiv sind; kann historische Events nicht rückwirkend nachverfolgen)* |
| **Tool-Nutzungs-Timeline** | Aktionslog jedes tool_use/tool_result-Paares mit Timing |
| **Fehler-Surfacing** | Fehler tauchen auf der Sessionkarte auf — keine vergrabenen Fehlschläge mehr |
| **Roh-Nachrichten-Inspektor** | Tauche in das rohe JSON jeder Nachricht ein, wenn du das vollständige Bild brauchst |

### Analytik

Eine umfangreiche Analyse-Suite für deine Claude Code-Nutzung. Denke an Cursors Dashboard, aber tiefer.

**Dashboard-Übersicht**

| Feature | Beschreibung |
|---------|-------------|
| **Woche-für-Woche-Metriken** | Session-Anzahl, Token-Nutzung, Kosten — verglichen mit deiner vorherigen Periode |
| **Aktivitäts-Heatmap** | 90-Tage-GitHub-Style-Grid, das deine tägliche Claude Code Nutzungsintensität zeigt |
| **Top Skills / Befehle / MCP-Tools / Agents** | Ranglisten deiner meistgenutzten Aufrufbaren — klicke auf einen, um passende Sessions zu suchen |
| **Aktivste Projekte** | Balkendiagramm der Projekte, sortiert nach Session-Anzahl |
| **Tool-Nutzungs-Aufschlüsselung** | Gesamte Bearbeitungen, Lesevorgänge und Bash-Befehle über alle Sessions |
| **Längste Sessions** | Schnellzugriff auf deine Marathon-Sessions mit Dauer |

**KI-Beiträge**

| Feature | Beschreibung |
|---------|-------------|
| **Code-Output-Tracking** | Hinzugefügte/entfernte Zeilen, bearbeitete Dateien, Commit-Anzahl — über alle Sessions |
| **Kosten-ROI-Metriken** | Kosten pro Commit, Kosten pro Session, Kosten pro KI-Output-Zeile — mit Trendcharts |
| **Modellvergleich** | Nebeneinander-Aufschlüsselung von Output und Effizienz nach Modell (Opus, Sonnet, Haiku) |
| **Lernkurve** | Re-Edit-Rate über die Zeit — sieh, wie du beim Prompting besser wirst |
| **Branch-Aufschlüsselung** | Klappbare Branch-Ansicht mit Session-Drill-Down |
| **Skill-Effektivität** | Welche Skills deinen Output tatsächlich verbessern vs welche nicht |

**Insights** *(experimentell)*

| Feature | Beschreibung |
|---------|-------------|
| **Mustererkennung** | Verhaltensmuster, die aus deinem Session-Verlauf entdeckt wurden |
| **Damals-vs-Jetzt-Benchmarks** | Vergleiche deinen ersten Monat mit der aktuellen Nutzung |
| **Kategorien-Aufschlüsselung** | Treemap, wofür du Claude nutzt — Refactoring, Features, Debugging, etc. |
| **KI-Fluenz-Score** | Eine einzige 0-100-Zahl, die deine Gesamteffektivität verfolgt |

> **Hinweis:** Insights und Fluenz-Score sind in einer frühen experimentellen Phase. Betrachte sie als richtungsweisend, nicht definitiv.

---

## Für den Flow Gebaut

claude-view ist für den Entwickler konzipiert, der:

- **3+ Projekte gleichzeitig** ausführt, jedes mit mehreren Worktrees
- Jederzeit **10-20 Claude Code Sessions** offen hat
- Schnell den Kontext wechseln muss, ohne den Überblick zu verlieren
- **Token-Ausgaben optimieren** will, indem Nachrichten um Cache-Fenster herum getimed werden
- Frustriert ist vom Cmd-Tab durch Terminals, um Agents zu prüfen

Ein Browser-Tab. Alle Sessions. Im Flow bleiben.

---

## Wie Es Gebaut Ist

| | |
|---|---|
| **Blitzschnell** | Rust-Backend mit SIMD-beschleunigtem JSONL-Parsing, Memory-Mapped I/O — indiziert tausende Sessions in Sekunden |
| **Echtzeit** | File-Watcher + SSE + WebSocket für Sub-Sekunden-Live-Updates über alle Sessions |
| **Minimaler Fußabdruck** | Einzelnes ~15 MB Binary. Keine Runtime-Abhängigkeiten, keine Hintergrund-Daemons |
| **100% lokal** | Alle Daten bleiben auf deinem Rechner. Null Telemetrie, null Cloud, null Netzwerkanfragen |
| **Null Konfiguration** | `npx claude-view` und fertig. Keine API-Keys, kein Setup, keine Konten |

---

## Schnellstart

```bash
npx claude-view
```

Öffnet sich unter `http://localhost:47892`.

### Konfiguration

| Umgebungsvariable | Standard | Beschreibung |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` oder `PORT` | `47892` | Standard-Port überschreiben |

---

## Installation

| Methode | Befehl |
|--------|---------|
| **npx** (empfohlen) | `npx claude-view` |
| **Shell-Skript** (kein Node erforderlich) | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

### Voraussetzungen

- **Claude Code** installiert ([hier herunterladen](https://docs.anthropic.com/en/docs/claude-code)) — dies erstellt die Session-Dateien, die wir überwachen

---

## Vergleich

Andere Tools sind entweder Viewer (Verlauf durchsuchen) oder einfache Monitore. Keines kombiniert Echtzeit-Monitoring, reichhaltigen Chat-Verlauf, Debugging-Tools und erweiterte Suche in einem einzigen Workspace.

```
                    Passiv ←————————————→ Aktiv
                         |                  |
            Nur Ansicht  |  ccusage         |
                         |  History Viewer  |
                         |  clog            |
                         |                  |
            Nur Monitor  |  claude-code-ui  |
                         |  Agent Sessions  |
                         |                  |
            Vollständiger|  ★ claude-view   |
            Workspace    |                  |
```

---

## Community

Tritt dem [Discord-Server](https://discord.gg/G7wdZTpRfu) bei für Support, Feature-Requests und Diskussion.

---

## Gefällt dir dieses Projekt?

Wenn **claude-view** dir hilft, Claude Code zu meistern, erwäge einen Stern zu vergeben. Es hilft anderen, dieses Tool zu entdecken.

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## Entwicklung

Voraussetzungen: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Frontend-Abhängigkeiten installieren
bun dev            # Full-Stack-Entwicklung starten (Rust + Vite mit Hot Reload)
```

| Befehl | Beschreibung |
|---------|-------------|
| `bun dev` | Full-Stack-Entwicklung — Rust startet bei Änderungen automatisch neu, Vite HMR |
| `bun dev:server` | Nur Rust-Backend (mit cargo-watch) |
| `bun dev:client` | Nur Vite-Frontend (setzt laufendes Backend voraus) |
| `bun run build` | Frontend für Produktion bauen |
| `bun run preview` | Bauen + über Release-Binary bereitstellen |
| `bun run lint` | Frontend (ESLint) und Backend (Clippy) linten |
| `bun run fmt` | Rust-Code formatieren |
| `bun run check` | Typecheck + Lint + Test (Pre-Commit-Gate) |
| `bun test` | Rust-Testsuite ausführen (`cargo test --workspace`) |
| `bun test:client` | Frontend-Tests ausführen (vitest) |
| `bun run test:e2e` | Playwright End-to-End-Tests ausführen |

### Produktions-Distribution Testen

```bash
bun run dist:test    # Ein Befehl: Build → Pack → Install → Run
```

Oder Schritt für Schritt:

| Befehl | Beschreibung |
|---------|-------------|
| `bun run dist:pack` | Binary + Frontend als Tarball in `/tmp/` verpacken |
| `bun run dist:install` | Tarball nach `~/.cache/claude-view/` extrahieren (simuliert Erstdownload) |
| `bun run dist:run` | npx-Wrapper mit gecachtem Binary ausführen |
| `bun run dist:test` | Alles oben in einem Befehl |
| `bun run dist:clean` | Alle Dist-Cache- und Temp-Dateien entfernen |

### Veröffentlichung

```bash
bun run release          # Patch-Bump: 0.1.0 → 0.1.1
bun run release:minor    # Minor-Bump: 0.1.0 → 0.2.0
bun run release:major    # Major-Bump: 0.1.0 → 1.0.0
```

Dies erhöht die Version in `npx-cli/package.json`, committet und erstellt einen Git-Tag. Dann:

```bash
git push origin main --tags    # triggert CI → baut alle Plattformen → veröffentlicht automatisch auf npm
```

---

## Plattform-Support

| Plattform | Status |
|----------|--------|
| macOS (Apple Silicon) | Verfügbar |
| macOS (Intel) | Verfügbar |
| Linux (x64) | Geplant |
| Windows (x64) | Geplant |

---

## Lizenz

MIT © 2026
