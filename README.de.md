<div align="center">

# claude-view

**Leitstand für Claude Code**

Du hast 10 KI-Agenten laufen. Einer ist vor 12 Minuten fertig geworden. Ein anderer hat sein Kontextlimit erreicht. Ein dritter braucht eine Tool-Freigabe. Du springst mit <kbd>Cmd</kbd>+<kbd>Tab</kbd> durch Terminals und verbrennst blind 200 $/Monat.

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

**Ein Befehl. Jede Sitzung sichtbar. In Echtzeit.**

</div>

---

## Was ist claude-view?

claude-view ist ein Open-Source-Dashboard, das jede Claude Code-Sitzung auf deinem Rechner überwacht — laufende Agenten, vergangene Konversationen, Kosten, Sub-Agenten, Hooks, Tool-Aufrufe — alles an einem Ort. Rust-Backend, React-Frontend, ~10 MB Binary. Keine Konfiguration, keine Konten, 100 % lokal.

**30 Releases. 86 MCP-Tools. 9 Skills. Ein `npx claude-view`.**

---

## Live Monitor

Sieh jede laufende Sitzung auf einen Blick. Kein Terminal-Tab-Wechsel mehr.

| Feature | Beschreibung |
|---------|-------------|
| **Session-Karten** | Jede Karte zeigt die letzte Nachricht, das Modell, die Kosten und den Status — sofort wissen, woran jeder Agent arbeitet |
| **Multi-Session-Chat** | Sitzungen nebeneinander in VS Code-ähnlichen Tabs öffnen (dockview). Zum horizontalen oder vertikalen Teilen ziehen |
| **Kontextanzeige** | Echtzeit-Kontextfenster-Füllstand pro Sitzung — erkenne, welche Agenten in der Gefahrenzone sind, bevor sie das Limit erreichen |
| **Cache-Countdown** | Wisse genau, wann der Prompt-Cache abläuft, damit du Nachrichten zeitlich abstimmen kannst, um Tokens zu sparen |
| **Kostenverfolgung** | Ausgaben pro Sitzung und aggregiert mit Token-Aufschlüsselung — hovere für die Aufteilung in Input/Output/Cache nach Modell |
| **Sub-Agent-Baum** | Sieh den vollständigen Baum gestarteter Agenten, ihren Status, Kosten und welche Tools sie aufrufen |
| **Benachrichtigungstöne** | Werde benachrichtigt, wenn eine Sitzung endet, fehlschlägt oder deine Eingabe braucht — kein Terminal-Polling mehr |
| **Mehrere Ansichten** | Grid, Liste, Kanban oder Monitor-Modus — wähle, was zu deinem Workflow passt |
| **Kanban-Swimlanes** | Sitzungen nach Projekt oder Branch gruppieren — visuelles Swimlane-Layout für Multi-Projekt-Workflows |
| **Kürzlich geschlossen** | Beendete Sitzungen erscheinen unter „Kürzlich geschlossen" statt zu verschwinden — bleibt auch nach Server-Neustarts erhalten |
| **Warteschlangen-Nachrichten** | Nachrichten in der Warteschlange werden als ausstehende Blasen mit einem „Queued"-Badge angezeigt |
| **SSE-getrieben** | Alle Live-Daten per Server-Sent Events gepusht — eliminiert Stale-Cache-Risiken vollständig |

---

## Chat & Konversation

Lese, durchsuche und interagiere mit jeder Sitzung — live oder historisch.

| Feature | Beschreibung |
|---------|-------------|
| **Einheitlicher Live-Chat** | Verlauf und Echtzeit-Nachrichten in einer scrollbaren Konversation — kein Tab-Wechsel |
| **Entwicklermodus** | Zwischen Chat- und Entwickler-Ansicht pro Sitzung umschalten. Der Entwicklermodus zeigt Tool-Karten, Event-Karten, Hook-Metadaten und die vollständige Ausführungsspur mit Filter-Chips |
| **Vollständiger Konversations-Browser** | Jede Sitzung, jede Nachricht, vollständig gerendert mit Markdown und Code-Blöcken |
| **Tool-Aufruf-Visualisierung** | Sieh Datei-Lesevorgänge, Bearbeitungen, Bash-Befehle, MCP-Aufrufe, Skill-Aufrufe — nicht nur Text |
| **Kompakt-/Detailansicht** | Überflieg die Konversation oder tauche in jeden Tool-Aufruf ein |
| **Thread-Ansicht** | Folge Agent-Konversationen mit Sub-Agent-Hierarchien und eingerücktem Threading |
| **Hook-Events inline** | Pre-/Post-Tool-Hooks als Konversationsblöcke gerendert — sieh Hooks neben der Konversation feuern |
| **Export** | Markdown-Export für Kontext-Wiederaufnahme oder zum Teilen |
| **Mehrfachauswahl & Archivierung** | Mehrere Sitzungen für Batch-Archivierung mit persistentem Filterstatus auswählen |
| **Verschlüsseltes Teilen** | Teile jede Sitzung über einen E2E-verschlüsselten Link — AES-256-GCM, kein Server-Vertrauen nötig, der Schlüssel lebt nur im URL-Fragment |

---

## Agent-Interna

Claude Code macht hinter `"thinking..."` eine Menge, die in deinem Terminal nie sichtbar wird. claude-view legt alles offen.

| Feature | Beschreibung |
|---------|-------------|
| **Sub-Agent-Konversationen** | Vollständiger Baum gestarteter Agenten, ihre Prompts, Ausgaben und Agent-bezogene Kosten-/Token-Aufschlüsselung |
| **MCP-Server-Aufrufe** | Welche MCP-Tools aufgerufen werden und ihre Ergebnisse |
| **Skill-/Hook-/Plugin-Tracking** | Welche Skills gefeuert wurden, welche Hooks liefen, welche Plugins aktiv sind |
| **Hook-Event-Aufzeichnung** | Dual-Channel-Hook-Erfassung (Live-WebSocket + JSONL-Backfill) — jedes Event aufgezeichnet und durchsuchbar, auch für vergangene Sitzungen |
| **Session-Quellen-Badges** | Jede Sitzung zeigt, wie sie gestartet wurde: Terminal, VS Code, Agent SDK oder andere Einstiegspunkte |
| **Worktree-Branch-Drift** | Erkennt, wenn git-Worktree-Branches auseinanderlaufen — im Live-Monitor und in der Historie angezeigt |
| **@File-Erwähnungs-Chips** | `@filename`-Referenzen werden als Chips extrahiert und angezeigt — hovere für den vollständigen Pfad |
| **Tool-Nutzungs-Timeline** | Aktionsprotokoll jedes tool_use/tool_result-Paares mit Zeitangaben |
| **Fehler-Anzeige** | Fehler werden auf die Session-Karte hochgehoben — keine versteckten Ausfälle |
| **Raw-Message-Inspektor** | Tauche in das rohe JSON jeder Nachricht ein, wenn du das vollständige Bild brauchst |

---

## Suche

| Feature | Beschreibung |
|---------|-------------|
| **Volltextsuche** | Suche über alle Sitzungen — Nachrichten, Tool-Aufrufe, Dateipfade. Betrieben von Tantivy (Rust-nativ, Lucene-Klasse) |
| **Einheitliche Suchmaschine** | Tantivy-Volltext + SQLite-Vorfilter laufen parallel — ein Endpunkt, Ergebnisse unter 50 ms |
| **Projekt- & Branch-Filter** | Auf das Projekt oder den Branch eingrenzen, an dem du gerade arbeitest |
| **Befehlspalette** | <kbd>Cmd</kbd>+<kbd>K</kbd> zum Wechseln zwischen Sitzungen, Ansichten ändern, alles finden |

---

## Analytik

Eine vollständige Analytik-Suite für deine Claude Code-Nutzung. Wie Cursors Dashboard, aber tiefgehender.

<details>
<summary><strong>Dashboard</strong></summary>
<br>

| Feature | Beschreibung |
|---------|-------------|
| **Wochen-Vergleich** | Sitzungsanzahl, Token-Verbrauch, Kosten — verglichen mit dem vorherigen Zeitraum |
| **Aktivitäts-Heatmap** | 90-Tage-GitHub-ähnliches Raster, das die tägliche Nutzungsintensität zeigt |
| **Top-Skills / Befehle / MCP-Tools / Agenten** | Ranglisten der meistgenutzten Aufrufe — klicke auf einen, um passende Sitzungen zu suchen |
| **Aktivste Projekte** | Balkendiagramm der Projekte nach Sitzungsanzahl sortiert |
| **Tool-Nutzungs-Aufschlüsselung** | Gesamtzahl der Bearbeitungen, Lesevorgänge und Bash-Befehle über alle Sitzungen |
| **Längste Sitzungen** | Schnellzugriff auf deine Marathon-Sitzungen mit Dauer |

</details>

<details>
<summary><strong>KI-Beiträge</strong></summary>
<br>

| Feature | Beschreibung |
|---------|-------------|
| **Code-Output-Tracking** | Hinzugefügte/entfernte Zeilen, bearbeitete Dateien, Commit-Anzahl — über alle Sitzungen |
| **Kosten-ROI-Metriken** | Kosten pro Commit, pro Sitzung, pro Zeile KI-Output — mit Trenddiagrammen |
| **Modellvergleich** | Seite-an-Seite-Aufschlüsselung von Output und Effizienz nach Modell (Opus, Sonnet, Haiku) |
| **Lernkurve** | Re-Edit-Rate über die Zeit — beobachte, wie du beim Prompting besser wirst |
| **Branch-Aufschlüsselung** | Einklappbare Ansicht pro Branch mit Sitzungs-Drill-Down |
| **Skill-Effektivität** | Welche Skills deinen Output tatsächlich verbessern und welche nicht |

</details>

<details>
<summary><strong>Erkenntnisse</strong> <em>(experimentell)</em></summary>
<br>

| Feature | Beschreibung |
|---------|-------------|
| **Mustererkennung** | Verhaltensmuster, die aus deiner Sitzungshistorie entdeckt wurden |
| **Damals-vs-Jetzt-Benchmarks** | Vergleiche deinen ersten Monat mit der aktuellen Nutzung |
| **Kategorie-Aufschlüsselung** | Treemap, wofür du Claude nutzt — Refactoring, Features, Debugging usw. |
| **AI Fluency Score** | Eine einzelne Zahl von 0-100, die deine Gesamteffektivität verfolgt |

> Erkenntnisse und Fluency Score sind experimentell. Als Richtungswert zu verstehen, nicht als definitive Aussage.

</details>

---

## Pläne, Prompts & Teams

| Feature | Beschreibung |
|---------|-------------|
| **Plan-Browser** | Sieh deine `.claude/plans/` direkt in der Sitzungsdetailansicht — kein Durchsuchen von Dateien mehr |
| **Prompt-Verlauf** | Volltextsuche über alle gesendeten Prompts mit Template-Clustering und Intent-Klassifikation |
| **Teams-Dashboard** | Sieh Team-Leads, Posteingangsnachrichten, Team-Aufgaben und Dateiänderungen aller Teammitglieder |
| **Prompt-Analytik** | Ranglisten von Prompt-Templates, Intent-Verteilung und Nutzungsstatistiken |

---

## Systemmonitor

| Feature | Beschreibung |
|---------|-------------|
| **Live-CPU-/RAM-/Disk-Anzeigen** | Echtzeit-Systemmetriken per SSE gestreamt mit flüssigen animierten Übergängen |
| **Komponenten-Dashboard** | Sieh Sidecar- und On-Device-KI-Metriken: VRAM-Nutzung, CPU, RAM und Sitzungsanzahl pro Komponente |
| **Prozessliste** | Prozesse nach Name gruppiert, nach CPU sortiert — sieh, was dein Rechner tatsächlich tut, während Agenten laufen |

---

## On-Device-KI

Betreibe ein lokales LLM für die Sitzungsphasen-Klassifikation — keine API-Aufrufe, keine Zusatzkosten.

| Feature | Beschreibung |
|---------|-------------|
| **Provider-unabhängig** | Verbinde dich mit jedem OpenAI-kompatiblen Endpunkt — oMLX, Ollama, LM Studio oder deinem eigenen Server |
| **Modellauswahl** | Wähle aus einer kuratierten Modellregistrierung mit angezeigten RAM-Anforderungen |
| **Phasenklassifikation** | Sitzungen werden mit ihrer aktuellen Phase getaggt (Coding, Debugging, Planung usw.) mittels konfidenzgesteuerter Anzeige |
| **Intelligentes Ressourcenmanagement** | EMA-stabilisierte Klassifikation mit exponentiellem Backoff — 93 % GPU-Verschwendungsreduktion gegenüber naivem Polling |

---

## Plugin

`@claude-view/plugin` gibt Claude nativen Zugriff auf deine Dashboard-Daten — 86 MCP-Tools, 9 Skills und Auto-Start.

```bash
claude plugin add @claude-view/plugin
```

### Auto-Start

Jede Claude Code-Sitzung startet automatisch das Dashboard. Kein manuelles `npx claude-view` nötig.

### 86 MCP-Tools

8 handgefertigte Tools mit optimierter Ausgabe für Claude:

| Tool | Beschreibung |
|------|-------------|
| `list_sessions` | Sitzungen mit Filtern durchsuchen |
| `get_session` | Vollständige Sitzungsdetails mit Nachrichten und Metriken |
| `search_sessions` | Volltextsuche über alle Konversationen |
| `get_stats` | Dashboard-Überblick — Gesamtsitzungen, Kosten, Trends |
| `get_fluency_score` | AI Fluency Score (0-100) mit Aufschlüsselung |
| `get_token_stats` | Token-Verbrauch mit Cache-Trefferquote |
| `list_live_sessions` | Aktuell laufende Agenten (Echtzeit) |
| `get_live_summary` | Aggregierte Kosten und Status für heute |

Plus **78 automatisch generierte Tools** aus der OpenAPI-Spezifikation in 27 Kategorien (Beiträge, Erkenntnisse, Coaching, Exporte, Workflows und mehr).

### 9 Skills

| Skill | Beschreibung |
|-------|-------------|
| `/session-recap` | Fasse eine bestimmte Sitzung zusammen — Commits, Metriken, Dauer |
| `/daily-cost` | Heutige Ausgaben, laufende Sitzungen, Token-Verbrauch |
| `/standup` | Multi-Sitzungs-Arbeitsprotokoll für Standup-Updates |
| `/coaching` | KI-Coaching-Tipps und benutzerdefinierte Regelverwaltung |
| `/insights` | Analyse von Verhaltensmustern |
| `/project-overview` | Projektübersicht über Sitzungen hinweg |
| `/search` | Suche in natürlicher Sprache |
| `/export-data` | Sitzungen als CSV/JSON exportieren |
| `/team-status` | Team-Aktivitätsübersicht |

---

## Workflows

| Feature | Beschreibung |
|---------|-------------|
| **Workflow-Builder** | Erstelle mehrstufige Workflows mit VS Code-ähnlichem Layout, Mermaid-Diagramm-Vorschau und YAML-Editor |
| **Streaming-LLM-Chat-Schiene** | Generiere Workflow-Definitionen in Echtzeit über eingebetteten Chat |
| **Stage-Runner** | Visualisiere Stufen-Spalten, Versuchs-Karten und Fortschrittsbalken während dein Workflow ausgeführt wird |
| **Mitgelieferte Seed-Workflows** | Plan Polisher und Plan Executor sind sofort verfügbar |

---

## In IDE öffnen

| Feature | Beschreibung |
|---------|-------------|
| **Ein-Klick-Dateiöffnung** | In Sitzungen referenzierte Dateien öffnen sich direkt in deinem Editor |
| **Automatische Editor-Erkennung** | VS Code, Cursor, Zed und andere — keine Konfiguration nötig |
| **Überall, wo es zählt** | Button erscheint im Changes-Tab, in Datei-Headern und Kanban-Projekt-Headern |
| **Präferenz-Speicher** | Dein bevorzugter Editor wird sitzungsübergreifend gespeichert |

---

## Wie es gebaut ist

| | |
|---|---|
| **Schnell** | Rust-Backend mit SIMD-beschleunigtem JSONL-Parsing, Memory-Mapped I/O — indexiert tausende Sitzungen in Sekunden |
| **Echtzeit** | File-Watcher + SSE + multiplexierter WebSocket mit Heartbeat, Event-Replay und Crash-Recovery |
| **Winzig** | ~10 MB Download, ~27 MB auf der Festplatte. Keine Laufzeitabhängigkeiten, keine Hintergrund-Daemons |
| **100 % lokal** | Alle Daten bleiben auf deinem Rechner. Standardmäßig null Telemetrie, keine erforderlichen Konten |
| **Null Konfiguration** | `npx claude-view` und fertig. Keine API-Keys, kein Setup, keine Konten |
| **FSM-gesteuert** | Chat-Sitzungen laufen auf einer endlichen Zustandsmaschine mit expliziten Phasen und typisierten Events — deterministisch, frei von Race-Conditions |

<details>
<summary><strong>Die Zahlen</strong></summary>
<br>

Gemessen auf einem M-Series-Mac mit 1.493 Sitzungen über 26 Projekte:

| Metrik | claude-view | Typisches Electron-Dashboard |
|--------|:-----------:|:--------------------------:|
| **Download** | **~10 MB** | 150-300 MB |
| **Auf der Festplatte** | **~27 MB** | 300-500 MB |
| **Startzeit** | **< 500 ms** | 3-8 s |
| **RAM (vollständiger Index)** | **~50 MB** | 300-800 MB |
| **1.500 Sitzungen indexieren** | **< 1 s** | N/A |
| **Laufzeitabhängigkeiten** | **0** | Node.js + Chromium |

Schlüsseltechniken: SIMD-Vorfilter (`memchr`), Memory-Mapped JSONL-Parsing, Tantivy-Volltextsuche, Zero-Copy-Slices von mmap über das Parsing bis zur Response.

</details>

---

## Im Vergleich

| Tool | Kategorie | Stack | Größe | Live-Monitor | Multi-Session-Chat | Suche | Analytik | MCP-Tools |
|------|----------|-------|:----:|:------------:|:------------------:|:------:|:---------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | Monitor + Workspace | Rust | **~10 MB** | **Ja** | **Ja** | **Ja** | **Ja** | **86** |
| [opcode](https://github.com/winfunc/opcode) | GUI + Session-Manager | Tauri 2 | ~13 MB | Teilweise | Nein | Nein | Ja | Nein |
| [ccusage](https://github.com/ryoppippi/ccusage) | CLI-Nutzungstracker | TypeScript | ~600 KB | Nein | Nein | Nein | CLI | Nein |
| [CodePilot](https://github.com/op7418/CodePilot) | Desktop-Chat-UI | Electron | ~140 MB | Nein | Nein | Nein | Nein | Nein |
| [claude-run](https://github.com/kamranahmedse/claude-run) | Verlaufs-Viewer | TypeScript | ~500 KB | Teilweise | Nein | Einfach | Nein | Nein |

> Chat-UIs (CodePilot, CUI, claude-code-webui) sind Oberflächen *für* Claude Code. claude-view ist ein Dashboard, das deine bestehenden Terminal-Sitzungen überwacht. Sie ergänzen sich.

---

## Installation

| Methode | Befehl |
|--------|---------|
| **Shell** (empfohlen) | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |
| **Plugin** (Auto-Start) | `claude plugin add @claude-view/plugin` |

Der Shell-Installer lädt ein vorkompiliertes Binary (~10 MB) herunter, installiert es nach `~/.claude-view/bin` und fügt es deinem PATH hinzu. Dann einfach `claude-view` ausführen.

**Einzige Voraussetzung:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) installiert.

<details>
<summary><strong>Konfiguration</strong></summary>
<br>

| Umgebungsvariable | Standard | Beschreibung |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` oder `PORT` | `47892` | Standard-Port überschreiben |

</details>

<details>
<summary><strong>Self-Hosting & lokale Entwicklung</strong></summary>
<br>

Das vorkompilierte Binary enthält Auth, Sharing und Mobile-Relay. Beim Bauen aus dem Quellcode? Diese Features sind **opt-in über Umgebungsvariablen** — weglassen und das Feature ist einfach deaktiviert.

| Umgebungsvariable | Feature | Ohne diese Variable |
|-------------|---------|------------|
| `SUPABASE_URL` | Login / Auth | Auth deaktiviert — vollständig lokal, Null-Konto-Modus |
| `RELAY_URL` | Mobile Kopplung | QR-Kopplung nicht verfügbar |
| `SHARE_WORKER_URL` + `SHARE_VIEWER_URL` | Verschlüsseltes Teilen | Teilen-Button ausgeblendet |

```bash
bun dev    # vollständig lokal, keine Cloud-Abhängigkeiten
```

</details>

<details>
<summary><strong>Enterprise / Sandbox-Umgebungen</strong></summary>
<br>

Falls dein Rechner Schreibzugriffe einschränkt (DataCloak, CrowdStrike, Unternehmens-DLP):

```bash
cp crates/server/.env.example .env
# CLAUDE_VIEW_DATA_DIR auskommentieren
```

Dies hält Datenbank, Suchindex und Lock-Dateien innerhalb des Repos. Setze `CLAUDE_VIEW_SKIP_HOOKS=1`, um die Hook-Registrierung in schreibgeschützten Umgebungen zu überspringen.

</details>

---

## FAQ

<details>
<summary><strong>Banner „Nicht angemeldet" wird angezeigt, obwohl ich eingeloggt bin</strong></summary>
<br>

claude-view prüft deine Claude-Anmeldedaten, indem es `~/.claude/.credentials.json` liest (mit macOS-Keychain-Fallback). Versuche diese Schritte:

1. **Claude CLI-Auth prüfen:** `claude auth status`
2. **Anmeldedaten-Datei prüfen:** `cat ~/.claude/.credentials.json` — sollte einen `claudeAiOauth`-Abschnitt mit einem `accessToken` enthalten
3. **macOS-Keychain prüfen:** `security find-generic-password -s "Claude Code-credentials" -w`
4. **Token-Ablauf prüfen:** Schau dir `expiresAt` in der Credentials-JSON an — falls abgelaufen, führe `claude auth login` aus
5. **HOME prüfen:** `echo $HOME` — der Server liest aus `$HOME/.claude/.credentials.json`

Falls alle Prüfungen bestanden und das Banner weiterhin angezeigt wird, melde es auf [Discord](https://discord.gg/G7wdZTpRfu).

</details>

<details>
<summary><strong>Auf welche Daten greift claude-view zu?</strong></summary>
<br>

claude-view liest die JSONL-Sitzungsdateien, die Claude Code nach `~/.claude/projects/` schreibt. Es indexiert sie lokal mit SQLite und Tantivy. **Keine Daten verlassen deinen Rechner**, es sei denn, du nutzt explizit die verschlüsselte Teilen-Funktion. Telemetrie ist opt-in und standardmäßig deaktiviert.

</details>

<details>
<summary><strong>Funktioniert es mit Claude Code in VS Code / Cursor / IDE-Erweiterungen?</strong></summary>
<br>

Ja. claude-view überwacht alle Claude Code-Sitzungen, unabhängig davon, wie sie gestartet wurden — Terminal-CLI, VS Code-Erweiterung, Cursor oder Agent SDK. Jede Sitzung zeigt ein Quellen-Badge (Terminal, VS Code, SDK), damit du nach Startmethode filtern kannst.

</details>

---

## Community

- **Website:** [claudeview.ai](https://claudeview.ai) — Dokumentation, Changelog, Blog
- **Discord:** [Server beitreten](https://discord.gg/G7wdZTpRfu) — Support, Feature-Wünsche, Diskussion
- **Plugin:** [`@claude-view/plugin`](https://www.npmjs.com/package/@claude-view/plugin) — 86 MCP-Tools, 9 Skills, Auto-Start

---

<details>
<summary><strong>Entwicklung</strong></summary>
<br>

Voraussetzungen: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Alle Workspace-Abhängigkeiten installieren
bun dev            # Full-Stack-Dev starten (Rust + Web + Sidecar mit Hot Reload)
```

### Workspace-Struktur

| Pfad | Paket | Zweck |
|------|---------|---------|
| `apps/web/` | `@claude-view/web` | React SPA (Vite) — Haupt-Web-Frontend |
| `apps/share/` | `@claude-view/share` | Share-Viewer SPA — Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | Expo Native App |
| `apps/landing/` | `@claude-view/landing` | Astro 5 Landingpage (kein clientseitiges JS) |
| `packages/shared/` | `@claude-view/shared` | Gemeinsame Typen & Theme-Tokens |
| `packages/design-tokens/` | `@claude-view/design-tokens` | Farben, Abstände, Typografie |
| `packages/plugin/` | `@claude-view/plugin` | Claude Code Plugin (MCP-Server + Tools + Skills) |
| `crates/` | — | Rust-Backend (Axum) |
| `sidecar/` | — | Node.js Sidecar (Agent SDK Bridge) |
| `infra/share-worker/` | — | Cloudflare Worker — Share-API (R2 + D1) |
| `infra/install-worker/` | — | Cloudflare Worker — Installations-Skript mit Download-Tracking |

### Dev-Befehle

| Befehl | Beschreibung |
|---------|-------------|
| `bun dev` | Full-Stack-Dev — Rust + Web + Sidecar mit Hot Reload |
| `bun run dev:web` | Nur Web-Frontend |
| `bun run dev:server` | Nur Rust-Backend |
| `bun run build` | Alle Workspaces bauen |
| `bun run preview` | Web bauen + über Release-Binary bereitstellen |
| `bun run lint:all` | JS/TS + Rust (Clippy) linten |
| `bun run typecheck` | TypeScript-Typ-Prüfung |
| `bun run test` | Alle Tests ausführen (Turbo) |
| `bun run test:rust` | Rust-Tests ausführen |
| `bun run storybook` | Storybook für Komponentenentwicklung starten |
| `bun run dist:test` | Bauen + packen + installieren + ausführen (vollständiger Dist-Test) |

### Releases

```bash
bun run release          # Patch-Bump
bun run release:minor    # Minor-Bump
git push origin main --tags    # löst CI aus → baut → veröffentlicht automatisch auf npm
```

</details>

---

## Plattform-Unterstützung

| Plattform | Status |
|----------|--------|
| macOS (Apple Silicon) | Verfügbar |
| macOS (Intel) | Verfügbar |
| Linux (x64) | Geplant |
| Windows (x64) | Geplant |

---

## Verwandte Projekte

- **[claudeview.ai](https://claudeview.ai)** — Offizielle Website, Dokumentation und Changelog
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** — Claude Code Plugin mit 86 MCP-Tools und 9 Skills. `claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** — Claude Code löscht deine Sitzungen nach 30 Tagen. Dieses Tool sichert sie. `npx claude-backup`

---

<div align="center">

Wenn **claude-view** dir hilft zu sehen, was deine KI-Agenten tun, erwäge einen Stern zu vergeben.

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT &copy; 2026

</div>
