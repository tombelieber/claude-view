# claude-view

<p align="center">
  <strong>Live monitor & copiloot voor Claude Code power users.</strong>
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
  <a href="https://www.npmjs.com/package/claude-view"><img src="https://img.shields.io/npm/v/claude-view.svg" alt="npm version"></a>
  <a href="https://claudeview.ai"><img src="https://img.shields.io/badge/Website-claudeview.ai-orange" alt="Website"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://discord.gg/G7wdZTpRfu"><img src="https://img.shields.io/discord/1325420051266592859?color=5865F2&logo=discord&logoColor=white&label=Discord" alt="Discord"></a>
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

---

## Het Probleem

Je hebt 3 projecten open. Elk project heeft meerdere git worktrees. Elke worktree heeft meerdere Claude Code sessies draaien. Sommige denken na, sommige wachten op jou, sommige raken bijna de contextlimieten, en eentje was 10 minuten geleden klaar maar die ben je vergeten.

Je Cmd-Tabt door 15 terminalvensters terwijl je probeert te herinneren welke sessie wat deed. Je verspilt tokens omdat een cache verlopen is terwijl je niet keek. Je verliest je flow omdat er geen enkele plek is om alles te zien. En achter die "denkt na..." spinner spawnt Claude sub-agents, roept MCP-servers aan, voert skills uit, activeert hooks — en je kunt er niets van zien.

**Claude Code is ongelooflijk krachtig. Maar 10+ gelijktijdige sessies vliegen zonder dashboard is als rijden zonder snelheidsmeter.**

## De Oplossing

**claude-view** is een realtime dashboard dat naast je Claude Code sessies draait. Eén browsertab, elke sessie zichtbaar, volledige context in één oogopslag.

```bash
curl -fsSL https://get.claudeview.ai/install.sh | sh
```

Dat is het. Opent in je browser. Al je sessies — live en afgelopen — in één workspace.

---

## Wat Je Krijgt

### Live Monitor

| Functie | Waarom het belangrijk is |
|---------|---------------|
| **Sessiekaarten met laatste bericht** | Herinner je direct waar elke langlopende sessie mee bezig is |
| **Notificatiegeluiden** | Krijg een melding wanneer een sessie klaar is of je input nodig heeft — stop met terminals pollen |
| **Contextmeter** | Realtime contextvenstergebruik per sessie — zie welke in de gevarenzone zitten |
| **Cache-warm-aftelling** | Weet precies wanneer de prompt-cache verloopt zodat je je volgende bericht kunt timen om tokens te besparen |
| **Kostenvolgorde** | Uitgaven per sessie en totaal — hover voor token/kosten-uitsplitsing met cache-besparingen per categorie |
| **Sub-agent visualisatie** | Zie de volledige agent-boom — sub-agents, hun status en welke tools ze aanroepen |
| **Meerdere weergaven** | Grid, Lijst, Kanban of Monitor-modus — kies wat past bij je workflow |
| **Kanban-swimlanes** | Groepeer sessies op project of branch — visuele swimlane-layout voor multi-project workflows |

### Rijke Chatgeschiedenis

| Functie | Waarom het belangrijk is |
|---------|---------------|
| **Volledige gespreksbrowser** | Elke sessie, elk bericht, volledig weergegeven met markdown en codeblokken |
| **Tool-aanroep visualisatie** | Zie bestandslezingen, bewerkingen, bash-commando's, MCP-aanroepen, skill-invocaties — niet alleen tekst |
| **Compact / uitgebreide toggle** | Blader snel door het gesprek of duik in elke tool-aanroep |
| **Thread-weergave** | Volg agentgesprekken met sub-agent hiërarchieën |
| **Exporteren** | Markdown-export voor contexthervatting of delen |
| **Bulkselectie & archivering** | Selecteer meerdere sessies voor batch-archivering met persistente filterstatus |
| **Versleuteld delen** | Deel elke sessie via E2E-versleutelde link — nul serververtrouwen |

### Geavanceerd Zoeken

| Functie | Waarom het belangrijk is |
|---------|---------------|
| **Volledige-tekst zoeken** | Zoek door alle sessies — berichten, tool-aanroepen, bestandspaden |
| **Project- & branchfilters** | Beperk tot het project waar je nu aan werkt |
| **Commandopalet** | Cmd+K om tussen sessies te springen, weergaven te wisselen, alles te vinden |

### Agent Internals — Zie Wat Verborgen Is

Claude Code doet veel achter "denkt na..." dat nooit in je terminal verschijnt. claude-view legt alles bloot.

| Functie | Waarom het belangrijk is |
|---------|---------------|
| **Sub-agent gesprekken** | Zie de volledige boom van gegenereerde agents, hun prompts en hun outputs |
| **MCP-server aanroepen** | Zie welke MCP-tools worden aangeroepen en hun resultaten |
| **Skill / hook / plugin tracking** | Weet welke skills zijn geactiveerd, welke hooks zijn gedraaid, welke plugins actief zijn |
| **Hook-event opname** | Dual-channel hook-vastlegging (live + JSONL-backfill) — elk hook-event opgenomen en doorzoekbaar, ook voor eerdere sessies |
| **Worktree-branch-drift** | Detecteert wanneer git worktree-branches divergeren — getoond in live monitor en geschiedenis |
| **Tool-gebruik tijdlijn** | Actielog van elk tool_use/tool_result-paar met timing |
| **Fout-surfacing** | Fouten verschijnen op de sessiekaart — geen verborgen mislukkingen meer |
| **Raw-bericht inspecteur** | Duik in de raw JSON van elk bericht wanneer je het complete beeld nodig hebt |

### Analytics

Een uitgebreide analysesuite voor je Claude Code gebruik. Denk aan het dashboard van Cursor, maar dieper.

**Dashboard Overzicht**

| Functie | Beschrijving |
|---------|-------------|
| **Week-over-week metrics** | Sessietelling, tokengebruik, kosten — vergeleken met je vorige periode |
| **Activiteiten-heatmap** | 90-daagse GitHub-stijl grid die je dagelijkse Claude Code gebruiksintensiteit toont |
| **Top skills / commando's / MCP-tools / agents** | Ranglijsten van je meestgebruikte aanroepbare items — klik op een om bijpassende sessies te zoeken |
| **Meest actieve projecten** | Staafdiagram van projecten gerangschikt op sessietelling |
| **Tool-gebruik uitsplitsing** | Totale bewerkingen, lezingen en bash-commando's over alle sessies |
| **Langste sessies** | Snelle toegang tot je marathonsessies met duur |

**AI-Bijdragen**

| Functie | Beschrijving |
|---------|-------------|
| **Code-output tracking** | Regels toegevoegd/verwijderd, bestanden geraakt, commit-telling — over alle sessies |
| **Kosten-ROI metrics** | Kosten per commit, kosten per sessie, kosten per regel AI-output — met trendgrafieken |
| **Modelvergelijking** | Zij-aan-zij uitsplitsing van output en efficiëntie per model (Opus, Sonnet, Haiku) |
| **Leercurve** | Herbewerkingspercentage over tijd — zie jezelf beter worden in prompting |
| **Branch-uitsplitsing** | Inklapbare per-branch weergave met sessie drill-down |
| **Skill-effectiviteit** | Welke skills je output daadwerkelijk verbeteren vs welke niet |

**Inzichten** *(experimenteel)*

| Functie | Beschrijving |
|---------|-------------|
| **Patroondetectie** | Gedragspatronen ontdekt uit je sessiegeschiedenis |
| **Toen vs nu benchmarks** | Vergelijk je eerste maand met recent gebruik |
| **Categorie-uitsplitsing** | Treemap van waarvoor je Claude gebruikt — refactoring, features, debugging, enz. |
| **AI-Vaardigheidsscore** | Eén enkel 0-100 getal dat je algehele effectiviteit volgt |

> **Opmerking:** Inzichten en Vaardigheidsscore zijn in een vroeg experimenteel stadium. Beschouw ze als richtinggevend, niet definitief.

---

## Gebouwd voor Flow

claude-view is ontworpen voor de ontwikkelaar die:

- **3+ projecten tegelijkertijd** draait, elk met meerdere worktrees
- Op elk moment **10-20 Claude Code sessies** open heeft
- Snel van context moet wisselen zonder het overzicht te verliezen
- **Tokenuitgaven wil optimaliseren** door berichten te timen rond cache-vensters
- Gefrustreerd is door Cmd-Tab door terminals om agents te controleren
- **Worktree-bewust** — detecteert branch-drift over git worktrees heen

---

## Hoe Het Is Gebouwd

| | |
|---|---|
| **Razend snel** | Rust-backend met SIMD-versnelde JSONL-parsing, memory-mapped I/O — indexeert duizenden sessies in seconden |
| **Realtime** | File watcher + SSE + uniforme WebSocket met heartbeat, event-replay en crashherstel |
| **Kleine voetafdruk** | ~10 MB download, ~27 MB op schijf. Geen runtime-afhankelijkheden, geen achtergrond-daemons |
| **100% lokaal** | Alle gegevens blijven op jouw machine. Nul telemetrie, geen verplichte accounts. Optioneel versleuteld delen beschikbaar. |
| **Nul configuratie** | `npx claude-view` en klaar. Geen API-keys, geen setup, geen accounts |

---

## Installatie

| Methode | Commando |
|--------|---------|
| **Shell** (aanbevolen) | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |

Het shell-installatiescript downloadt een voorgebouwd binair bestand (~10 MB), installeert het in `~/.claude-view/bin` en voegt het toe aan je PATH. Daarna hoef je alleen `claude-view` uit te voeren.

### Configuratie

| Omgevingsvariabele | Standaard | Beschrijving |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` of `PORT` | `47892` | Standaard poort overschrijven |
| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | Data-directory overschrijven |

---

**Enige vereiste:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) geïnstalleerd — dit maakt de sessiebestanden aan die we monitoren.

---

## Vergelijking

Andere tools zijn ofwel viewers (geschiedenis doorzoeken) of simpele monitors. Geen enkele combineert realtime monitoring, rijke chatgeschiedenis, debugging tools en geavanceerd zoeken in één workspace.

```
                    Passief ←————————————→ Actief
                         |                  |
            Alleen       |  ccusage         |
            bekijken     |  History Viewer  |
                         |  clog            |
                         |                  |
            Alleen       |  claude-code-ui  |
            monitor      |  Agent Sessions  |
                         |                  |
            Complete     |  ★ claude-view   |
            workspace    |                  |
```

---

## Community

Word lid van de [Discord server](https://discord.gg/G7wdZTpRfu) voor ondersteuning, functie-aanvragen en discussie.

---

## Vind je dit project leuk?

Als **claude-view** je helpt Claude Code te beheersen, overweeg dan een ster te geven. Het helpt anderen deze tool te ontdekken.

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## Ontwikkeling

Vereisten: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Frontend-afhankelijkheden installeren
bun dev            # Full-stack ontwikkeling starten (Rust + Vite met hot reload)
```

| Commando | Beschrijving |
|---------|-------------|
| `bun dev` | Full-stack ontwikkeling — Rust herstart automatisch bij wijzigingen, Vite HMR |
| `bun dev:server` | Alleen Rust-backend (met cargo-watch) |
| `bun dev:client` | Alleen Vite-frontend (veronderstelt draaiende backend) |
| `bun run build` | Frontend voor productie bouwen |
| `bun run preview` | Bouwen + serveren via release binary |
| `bun run lint` | Lint frontend (ESLint) en backend (Clippy) |
| `bun run fmt` | Rust-code formatteren |
| `bun run check` | Typecheck + lint + test (pre-commit gate) |
| `bun test` | Rust testsuite uitvoeren (`cargo test --workspace`) |
| `bun test:client` | Frontend tests uitvoeren (vitest) |
| `bun run test:e2e` | Playwright end-to-end tests uitvoeren |

### Productie Distributie Testen

```bash
bun run dist:test    # Eén commando: build → pack → install → run
```

Of stap voor stap:

| Commando | Beschrijving |
|---------|-------------|
| `bun run dist:pack` | Binary + frontend verpakken als tarball in `/tmp/` |
| `bun run dist:install` | Tarball uitpakken naar `~/.cache/claude-view/` (simuleert eerste download) |
| `bun run dist:run` | npx-wrapper uitvoeren met gecachte binary |
| `bun run dist:test` | Alles hierboven in één commando |
| `bun run dist:clean` | Alle dist cache en tijdelijke bestanden verwijderen |

### Release

```bash
bun run release          # patch bump: 0.1.0 → 0.1.1
bun run release:minor    # minor bump: 0.1.0 → 0.2.0
bun run release:major    # major bump: 0.1.0 → 1.0.0
```

Dit verhoogt de versie in `npx-cli/package.json`, commit en maakt een git tag. Vervolgens:

```bash
git push origin main --tags    # triggert CI → bouwt alle platformen → publiceert automatisch naar npm
```

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

- **[claudeview.ai](https://claudeview.ai)** — Officiële website, documentatie en changelog
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** — Claude Code plugin met 8 MCP-tools en 3 skills. `claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** — Claude Code verwijdert je sessies na 30 dagen. Deze tool bewaart ze. `npx claude-backup`

---

## Licentie

MIT © 2026
