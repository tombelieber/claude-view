<div align="center">

# claude-view

**Missiecontrole voor Claude Code**

Je hebt 10 AI-agents draaien. Eentje is 12 minuten geleden klaar. Een andere heeft zijn contextlimiet bereikt. Een derde wacht op goedkeuring voor een tool. Je zit te <kbd>Cmd</kbd>+<kbd>Tab</kbd>-ben tussen terminals en verbrandt $200/maand zonder overzicht.

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

**Een commando. Elke sessie zichtbaar. Realtime.**

</div>

---

## Wat is claude-view?

claude-view is een open-source dashboard dat elke Claude Code-sessie op je machine monitort -- actieve agents, eerdere gesprekken, kosten, sub-agents, hooks, tool-aanroepen -- op een plek. Rust-backend, React-frontend, ~10 MB binary. Geen configuratie, geen accounts, 100% lokaal.

**30 releases. 86 MCP-tools. 9 skills. Een enkele `npx claude-view`.**

---

## Live Monitor

Bekijk elke actieve sessie in een oogopslag. Geen terminal-tabbladen meer wisselen.

| Functie | Wat het doet |
|---------|-------------|
| **Sessiekaarten** | Elke kaart toont het laatste bericht, model, kosten en status -- direct weten waar elke agent mee bezig is |
| **Multi-sessie chat** | Open sessies naast elkaar in VS Code-achtige tabbladen (dockview). Sleep om horizontaal of verticaal te splitsen |
| **Contextmeter** | Realtime contextvenster-vulling per sessie -- zie welke agents in de gevarenzone zitten voordat ze de limiet bereiken |
| **Cache-afteller** | Weet precies wanneer de prompt-cache verloopt zodat je berichten kunt timen om tokens te besparen |
| **Kostenoverzicht** | Kosten per sessie en totaal met token-uitsplitsing -- hover voor input/output/cache-verdeling per model |
| **Sub-agent boom** | Bekijk de volledige boom van gestarte agents, hun status, kosten en welke tools ze aanroepen |
| **Notificatiegeluiden** | Ontvang een signaal wanneer een sessie klaar is, een fout heeft of je input nodig heeft -- stop met terminals checken |
| **Meerdere weergaven** | Grid, Lijst, Kanban of Monitor-modus -- kies wat bij je workflow past |
| **Kanban-swimlanes** | Groepeer sessies op project of branch -- visuele swimlane-indeling voor multi-projectworkflows |
| **Recent gesloten** | Sessies die eindigen verschijnen in "Recent gesloten" in plaats van te verdwijnen -- blijft bewaard na server-herstart |
| **Berichten in wachtrij** | Berichten die in de wachtrij staan worden weergegeven als wachtende bubbels met een "In wachtrij"-badge |
| **SSE-aangedreven** | Alle live data wordt gepusht via Server-Sent Events -- elimineert verouderde-cache-risico's volledig |

---

## Chat & Gesprek

Lees, zoek en communiceer met elke sessie -- live of historisch.

| Functie | Wat het doet |
|---------|-------------|
| **Geunificeerde live chat** | Geschiedenis en realtime berichten in een enkel scrollbaar gesprek -- geen tabbladen wisselen |
| **Ontwikkelaarsmodus** | Schakel tussen Chat- en Ontwikkelaarsweergave per sessie. Ontwikkelaarsmodus toont tool-kaarten, event-kaarten, hook-metadata en de volledige uitvoertrace met filterchips |
| **Volledige gespreksbrowser** | Elke sessie, elk bericht, volledig gerenderd met markdown en codeblokken |
| **Tool-aanroep visualisatie** | Bekijk bestandslezingen, bewerkingen, bash-commando's, MCP-aanroepen, skill-aanroepen -- niet alleen tekst |
| **Compact / uitgebreid toggle** | Scan het gesprek of duik in elke tool-aanroep |
| **Thread-weergave** | Volg agent-gesprekken met sub-agent hierarchieen en ingesprongen threading |
| **Hook-events inline** | Pre/post tool-hooks worden gerenderd als gespreksblokken -- zie hooks afgaan naast het gesprek |
| **Exporteren** | Markdown-export voor contexthervatting of delen |
| **Bulkselectie & archiveren** | Selecteer meerdere sessies voor batcharchivering met persistente filterstatus |
| **Versleuteld delen** | Deel elke sessie via een end-to-end versleutelde link -- AES-256-GCM, nul serververtrouwen, sleutel leeft alleen in het URL-fragment |

---

## Agent-internals

Claude Code doet veel achter `"thinking..."` dat nooit in je terminal verschijnt. claude-view legt alles bloot.

| Functie | Wat het doet |
|---------|-------------|
| **Sub-agent gesprekken** | Volledige boom van gestarte agents, hun prompts, outputs en kosten/token-uitsplitsing per agent |
| **MCP-serveraanroepen** | Welke MCP-tools worden aangeroepen en hun resultaten |
| **Skill / hook / plugin-tracking** | Welke skills zijn afgevuurd, welke hooks hebben gedraaid, welke plugins zijn actief |
| **Hook-event opname** | Dualkanaal hook-capture (live WebSocket + JSONL-backfill) -- elk event opgenomen en doorzoekbaar, ook voor eerdere sessies |
| **Sessiebron-badges** | Elke sessie toont hoe deze is gestart: Terminal, VS Code, Agent SDK of andere ingangen |
| **Worktree branch-drift** | Detecteert wanneer git worktree-branches uiteenlopen -- getoond in live monitor en geschiedenis |
| **@File-vermelding chips** | `@filename`-referenties worden geextraheerd en getoond als chips -- hover voor het volledige pad |
| **Tool-gebruik tijdlijn** | Actielogboek van elk tool_use/tool_result-paar met timing |
| **Fouten naar boven** | Fouten bubbelen omhoog naar de sessiekaart -- geen begraven foutmeldingen |
| **Ruwe bericht-inspector** | Duik in de ruwe JSON van elk bericht wanneer je het volledige beeld nodig hebt |

---

## Zoeken

| Functie | Wat het doet |
|---------|-------------|
| **Volledige-tekst zoeken** | Zoek door alle sessies -- berichten, tool-aanroepen, bestandspaden. Aangedreven door Tantivy (Rust-native, Lucene-klasse) |
| **Geunificeerde zoekmachine** | Tantivy volledige-tekst + SQLite-voorfilter draaien parallel -- een endpoint, resultaten onder 50 ms |
| **Project- & branchfilters** | Beperk tot het project of de branch waar je nu aan werkt |
| **Commandopalet** | <kbd>Cmd</kbd>+<kbd>K</kbd> om te springen tussen sessies, weergaven te wisselen, alles te vinden |

---

## Analyse

Een volledig analysepakket voor je Claude Code-gebruik. Denk aan het dashboard van Cursor, maar diepgaander.

<details>
<summary><strong>Dashboard</strong></summary>
<br>

| Functie | Beschrijving |
|---------|-------------|
| **Week-op-week statistieken** | Sessieaantal, tokenverbruik, kosten -- vergeleken met je vorige periode |
| **Activiteits-heatmap** | 90-dagen GitHub-achtig raster dat dagelijkse gebruiksintensiteit toont |
| **Top skills / commando's / MCP-tools / agents** | Ranglijsten van je meestgebruikte aanroepen -- klik er een aan om bijbehorende sessies te zoeken |
| **Meest actieve projecten** | Staafdiagram van projecten gerangschikt op sessieaantal |
| **Tool-gebruik uitsplitsing** | Totaal bewerkingen, lezingen en bash-commando's over alle sessies |
| **Langste sessies** | Snelle toegang tot je marathon-sessies met duur |

</details>

<details>
<summary><strong>AI-bijdragen</strong></summary>
<br>

| Functie | Beschrijving |
|---------|-------------|
| **Code-output tracking** | Regels toegevoegd/verwijderd, bestanden aangeraakt, commit-aantal -- over alle sessies |
| **Kosten-ROI statistieken** | Kosten per commit, kosten per sessie, kosten per regel AI-output -- met trendgrafieken |
| **Modelvergelijking** | Zij-aan-zij uitsplitsing van output en efficientie per model (Opus, Sonnet, Haiku) |
| **Leercurve** | Herbewerkingspercentage over tijd -- zie jezelf beter worden in prompting |
| **Branch-uitsplitsing** | Inklapbare weergave per branch met sessie-doordrilling |
| **Skill-effectiviteit** | Welke skills daadwerkelijk je output verbeteren versus welke niet |

</details>

<details>
<summary><strong>Inzichten</strong> <em>(experimenteel)</em></summary>
<br>

| Functie | Beschrijving |
|---------|-------------|
| **Patroondetectie** | Gedragspatronen ontdekt uit je sessiegeschiedenis |
| **Toen vs Nu benchmarks** | Vergelijk je eerste maand met recent gebruik |
| **Categorie-uitsplitsing** | Treemap van waarvoor je Claude gebruikt -- refactoring, features, debugging, etc. |
| **AI Fluency Score** | Enkel 0-100 getal dat je algehele effectiviteit bijhoudt |

> Inzichten en Fluency Score zijn experimenteel. Beschouw ze als richtinggevend, niet definitief.

</details>

---

## Plannen, Prompts & Teams

| Functie | Wat het doet |
|---------|-------------|
| **Planbrowser** | Bekijk je `.claude/plans/` direct in het sessiedetail -- niet meer door bestanden zoeken |
| **Promptgeschiedenis** | Volledige-tekst zoeken door alle prompts die je hebt verstuurd met sjabloonclustering en intentclassificatie |
| **Teams-dashboard** | Bekijk teamleiders, inboxberichten, teamtaken en bestandswijzigingen van alle teamleden |
| **Promptanalyse** | Ranglijsten van promptsjablonen, intentverdeling en gebruiksstatistieken |

---

## Systeemmonitor

| Functie | Wat het doet |
|---------|-------------|
| **Live CPU / RAM / Schijfmeters** | Realtime systeemstatistieken gestreamd via SSE met vloeiende geanimeerde overgangen |
| **Componentdashboard** | Bekijk sidecar- en on-device AI-statistieken: VRAM-gebruik, CPU, RAM en sessieaantal per component |
| **Proceslijst** | Processen gegroepeerd op naam, gesorteerd op CPU -- zie wat je machine werkelijk doet terwijl agents draaien |

---

## On-Device AI

Draai een lokaal LLM voor sessiefaseclassificatie -- geen API-aanroepen, geen extra kosten.

| Functie | Wat het doet |
|---------|-------------|
| **Provider-agnostisch** | Verbind met elk OpenAI-compatibel endpoint -- oMLX, Ollama, LM Studio of je eigen server |
| **Modelselector** | Kies uit een samengesteld modelregister met getoonde RAM-vereisten |
| **Faseclassificatie** | Sessies worden getagd met hun huidige fase (coderen, debuggen, plannen, etc.) met vertrouwensgestuurde weergave |
| **Slim resourcebeheer** | EMA-gestabiliseerde classificatie met exponential backoff -- 93% GPU-verspillingsreductie vs. naief pollen |

---

## Plugin

`@claude-view/plugin` geeft Claude native toegang tot je dashboardgegevens -- 86 MCP-tools, 9 skills en auto-start.

```bash
claude plugin add @claude-view/plugin
```

### Auto-start

Elke Claude Code-sessie start automatisch het dashboard. Geen handmatig `npx claude-view` nodig.

### 86 MCP-tools

8 handgemaakte tools met geoptimaliseerde output voor Claude:

| Tool | Beschrijving |
|------|-------------|
| `list_sessions` | Blader door sessies met filters |
| `get_session` | Volledig sessiedetail met berichten en statistieken |
| `search_sessions` | Volledige-tekst zoeken door alle gesprekken |
| `get_stats` | Dashboardoverzicht -- totaal sessies, kosten, trends |
| `get_fluency_score` | AI Fluency Score (0-100) met uitsplitsing |
| `get_token_stats` | Tokenverbruik met cache-hitratio |
| `list_live_sessions` | Momenteel actieve agents (realtime) |
| `get_live_summary` | Totale kosten en status voor vandaag |

Plus **78 automatisch gegenereerde tools** uit de OpenAPI-specificatie over 27 categorieen (bijdragen, inzichten, coaching, exports, workflows en meer).

### 9 Skills

| Skill | Wat het doet |
|-------|-------------|
| `/session-recap` | Vat een specifieke sessie samen -- commits, statistieken, duur |
| `/daily-cost` | Uitgaven van vandaag, actieve sessies, tokenverbruik |
| `/standup` | Multi-sessie werklogboek voor standup-updates |
| `/coaching` | AI-coachingtips en aangepast regelbeheer |
| `/insights` | Gedragspatroonanalyse |
| `/project-overview` | Projectsamenvatting over sessies heen |
| `/search` | Zoekopdrachten in natuurlijke taal |
| `/export-data` | Exporteer sessies naar CSV/JSON |
| `/team-status` | Teamactiviteitsoverzicht |

---

## Workflows

| Functie | Wat het doet |
|---------|-------------|
| **Workflow-bouwer** | Maak meerfasige workflows met VS Code-achtige indeling, Mermaid-diagramvoorvertoning en YAML-editor |
| **Streaming LLM-chatrails** | Genereer workflowdefinities in realtime via ingebedde chat |
| **Fase-uitvoerder** | Visualiseer fasekolommen, pogingskaarten en voortgangsbalk terwijl je workflow draait |
| **Ingebouwde seed-workflows** | Plan Polisher en Plan Executor worden standaard meegeleverd |

---

## Openen in IDE

| Functie | Wat het doet |
|---------|-------------|
| **Bestand openen met een klik** | Bestanden waarnaar verwezen wordt in sessies openen direct in je editor |
| **Automatische editordetectie** | VS Code, Cursor, Zed en andere -- geen configuratie nodig |
| **Overal waar het ertoe doet** | Knop verschijnt in het Wijzigingen-tabblad, bestandskoppen en Kanban-projectkoppen |
| **Voorkeurgeheugen** | Je voorkeurs-editor wordt onthouden tussen sessies |

---

## Hoe het gebouwd is

| | |
|---|---|
| **Snel** | Rust-backend met SIMD-versnelde JSONL-parsing, memory-mapped I/O -- indexeert duizenden sessies in seconden |
| **Realtime** | Filewatcher + SSE + gemultiplexte WebSocket met heartbeat, event-replay en crashherstel |
| **Klein** | ~10 MB download, ~27 MB op schijf. Geen runtime-afhankelijkheden, geen achtergrondprocessen |
| **100% lokaal** | Alle gegevens blijven op je machine. Standaard geen telemetrie, geen verplichte accounts |
| **Geen configuratie** | `npx claude-view` en je bent klaar. Geen API-sleutels, geen setup, geen accounts |
| **FSM-gestuurd** | Chatsessies draaien op een finite state machine met expliciete fasen en getypte events -- deterministisch, race-vrij |

<details>
<summary><strong>De cijfers</strong></summary>
<br>

Gemeten op een M-serie Mac met 1.493 sessies over 26 projecten:

| Metriek | claude-view | Typisch Electron-dashboard |
|--------|:-----------:|:--------------------------:|
| **Download** | **~10 MB** | 150-300 MB |
| **Op schijf** | **~27 MB** | 300-500 MB |
| **Opstarttijd** | **< 500 ms** | 3-8 s |
| **RAM (volledige index)** | **~50 MB** | 300-800 MB |
| **1.500 sessies indexeren** | **< 1 s** | N.v.t. |
| **Runtime-afhankelijkheden** | **0** | Node.js + Chromium |

Belangrijke technieken: SIMD-voorfilter (`memchr`), memory-mapped JSONL-parsing, Tantivy volledige-tekst zoeken, zero-copy slices van mmap door parse tot response.

</details>

---

## Vergelijking

| Tool | Categorie | Stack | Grootte | Live monitor | Multi-sessie chat | Zoeken | Analyse | MCP-tools |
|------|----------|-------|:----:|:------------:|:------------------:|:------:|:---------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | Monitor + werkruimte | Rust | **~10 MB** | **Ja** | **Ja** | **Ja** | **Ja** | **86** |
| [opcode](https://github.com/winfunc/opcode) | GUI + sessiebeheer | Tauri 2 | ~13 MB | Gedeeltelijk | Nee | Nee | Ja | Nee |
| [ccusage](https://github.com/ryoppippi/ccusage) | CLI-gebruikstracker | TypeScript | ~600 KB | Nee | Nee | Nee | CLI | Nee |
| [CodePilot](https://github.com/op7418/CodePilot) | Desktop chat-UI | Electron | ~140 MB | Nee | Nee | Nee | Nee | Nee |
| [claude-run](https://github.com/kamranahmedse/claude-run) | Geschiedenisviewer | TypeScript | ~500 KB | Gedeeltelijk | Nee | Basis | Nee | Nee |

> Chat-UI's (CodePilot, CUI, claude-code-webui) zijn interfaces *voor* Claude Code. claude-view is een dashboard dat je bestaande terminalsessies in de gaten houdt. Ze vullen elkaar aan.

---

## Installatie

| Methode | Commando |
|--------|---------|
| **Shell** (aanbevolen) | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |
| **Plugin** (auto-start) | `claude plugin add @claude-view/plugin` |

Het shell-installatiescript downloadt een voorgebouwde binary (~10 MB), installeert naar `~/.claude-view/bin` en voegt het toe aan je PATH. Draai daarna gewoon `claude-view`.

**Enige vereiste:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) geinstalleerd.

<details>
<summary><strong>Configuratie</strong></summary>
<br>

| Omgevingsvariabele | Standaard | Beschrijving |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` of `PORT` | `47892` | Overschrijf de standaardpoort |

</details>

<details>
<summary><strong>Zelf hosten & lokale ontwikkeling</strong></summary>
<br>

De voorgebouwde binary wordt geleverd met authenticatie, delen en mobiele relay ingebouwd. Bouw je vanuit broncode? Deze functies zijn **opt-in via omgevingsvariabelen** -- laat er een weg en die functie wordt gewoon uitgeschakeld.

| Omgevingsvariabele | Functie | Zonder |
|-------------|---------|------------|
| `SUPABASE_URL` | Inloggen / authenticatie | Authenticatie uitgeschakeld -- volledig lokaal, zonder-accounts-modus |
| `RELAY_URL` | Mobiel koppelen | QR-koppeling niet beschikbaar |
| `SHARE_WORKER_URL` + `SHARE_VIEWER_URL` | Versleuteld delen | Deelknop verborgen |

```bash
bun dev    # volledig lokaal, geen cloudafhankelijkheden
```

</details>

<details>
<summary><strong>Enterprise / Sandbox-omgevingen</strong></summary>
<br>

Als je machine schrijfbewerkingen beperkt (DataCloak, CrowdStrike, zakelijke DLP):

```bash
cp crates/server/.env.example .env
# Uncomment CLAUDE_VIEW_DATA_DIR
```

Dit houdt database, zoekindex en lockbestanden binnen de repo. Stel `CLAUDE_VIEW_SKIP_HOOKS=1` in om hookregistratie over te slaan in alleen-lezen-omgevingen.

</details>

---

## Veelgestelde vragen

<details>
<summary><strong>"Not signed in"-banner verschijnt terwijl ik ingelogd ben</strong></summary>
<br>

claude-view controleert je Claude-inloggegevens door `~/.claude/.credentials.json` te lezen (met macOS Keychain-fallback). Probeer deze stappen:

1. **Controleer Claude CLI-authenticatie:** `claude auth status`
2. **Controleer inloggegevensbestand:** `cat ~/.claude/.credentials.json` -- zou een `claudeAiOauth`-sectie moeten bevatten met een `accessToken`
3. **Controleer macOS Keychain:** `security find-generic-password -s "Claude Code-credentials" -w`
4. **Controleer tokenvervaldatum:** Kijk naar `expiresAt` in de credentials JSON -- als verlopen, voer `claude auth login` uit
5. **Controleer HOME:** `echo $HOME` -- de server leest vanuit `$HOME/.claude/.credentials.json`

Als alle controles slagen en de banner blijft, meld het op [Discord](https://discord.gg/G7wdZTpRfu).

</details>

<details>
<summary><strong>Welke gegevens benadert claude-view?</strong></summary>
<br>

claude-view leest de JSONL-sessiebestanden die Claude Code schrijft naar `~/.claude/projects/`. Het indexeert ze lokaal met SQLite en Tantivy. **Er verlaten geen gegevens je machine** tenzij je expliciet de versleutelde deelfunctie gebruikt. Telemetrie is opt-in en standaard uitgeschakeld.

</details>

<details>
<summary><strong>Werkt het met Claude Code in VS Code / Cursor / IDE-extensies?</strong></summary>
<br>

Ja. claude-view monitort alle Claude Code-sessies ongeacht hoe ze zijn gestart -- terminal CLI, VS Code-extensie, Cursor of Agent SDK. Elke sessie toont een bronbadge (Terminal, VS Code, SDK) zodat je kunt filteren op startmethode.

</details>

---

## Community

- **Website:** [claudeview.ai](https://claudeview.ai) -- documentatie, changelog, blog
- **Discord:** [Word lid van de server](https://discord.gg/G7wdZTpRfu) -- ondersteuning, functieverzoeken, discussie
- **Plugin:** [`@claude-view/plugin`](https://www.npmjs.com/package/@claude-view/plugin) -- 86 MCP-tools, 9 skills, auto-start

---

<details>
<summary><strong>Ontwikkeling</strong></summary>
<br>

Vereisten: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Installeer alle werkruimte-afhankelijkheden
bun dev            # Start full-stack dev (Rust + Web + Sidecar met hot reload)
```

### Werkruimte-indeling

| Pad | Pakket | Doel |
|------|---------|---------|
| `apps/web/` | `@claude-view/web` | React SPA (Vite) -- hoofd-webfrontend |
| `apps/share/` | `@claude-view/share` | Deelviewer SPA -- Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | Expo native app |
| `apps/landing/` | `@claude-view/landing` | Astro 5 landingspagina (geen client-side JS) |
| `packages/shared/` | `@claude-view/shared` | Gedeelde types & thematokens |
| `packages/design-tokens/` | `@claude-view/design-tokens` | Kleuren, spacing, typografie |
| `packages/plugin/` | `@claude-view/plugin` | Claude Code plugin (MCP-server + tools + skills) |
| `crates/` | -- | Rust-backend (Axum) |
| `sidecar/` | -- | Node.js sidecar (Agent SDK-bridge) |
| `infra/share-worker/` | -- | Cloudflare Worker -- deel-API (R2 + D1) |
| `infra/install-worker/` | -- | Cloudflare Worker -- installatiescript met downloadtracking |

### Dev-commando's

| Commando | Beschrijving |
|---------|-------------|
| `bun dev` | Full-stack dev -- Rust + Web + Sidecar met hot reload |
| `bun run dev:web` | Alleen webfrontend |
| `bun run dev:server` | Alleen Rust-backend |
| `bun run build` | Bouw alle werkruimten |
| `bun run preview` | Bouw web + serveer via release-binary |
| `bun run lint:all` | Lint JS/TS + Rust (Clippy) |
| `bun run typecheck` | TypeScript typecontrole |
| `bun run test` | Voer alle tests uit (Turbo) |
| `bun run test:rust` | Voer Rust-tests uit |
| `bun run storybook` | Start Storybook voor componentontwikkeling |
| `bun run dist:test` | Bouw + verpak + installeer + draai (volledige distributietest) |

### Releasen

```bash
bun run release          # patch bump
bun run release:minor    # minor bump
git push origin main --tags    # triggert CI → bouwt → publiceert automatisch naar npm
```

</details>

---

## Platformondersteuning

| Platform | Status |
|----------|--------|
| macOS (Apple Silicon) | Beschikbaar |
| macOS (Intel) | Beschikbaar |
| Linux (x64) | Gepland |
| Windows (x64) | Gepland |

---

## Gerelateerd

- **[claudeview.ai](https://claudeview.ai)** -- Officiele website, documentatie en changelog
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** -- Claude Code plugin met 86 MCP-tools en 9 skills. `claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** -- Claude Code verwijdert je sessies na 30 dagen. Dit bewaart ze. `npx claude-backup`

---

<div align="center">

Als **claude-view** je helpt te zien wat je AI-agents doen, overweeg dan een ster te geven.

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT &copy; 2026

</div>
