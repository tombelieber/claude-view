<div align="center">

# claude-view

**Centre de controle pour Claude Code**

Vous avez 10 agents IA en cours d'execution. L'un a termine il y a 12 minutes. Un autre a atteint sa limite de contexte. Un troisieme attend une approbation d'outil. Vous faites des <kbd>Cmd</kbd>+<kbd>Tab</kbd> entre les terminaux, depensant 200$/mois a l'aveugle.

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

**Une commande. Toutes les sessions visibles. En temps reel.**

</div>

---

## Qu'est-ce que claude-view ?

claude-view est un tableau de bord open source qui surveille chaque session Claude Code sur votre machine — agents en direct, conversations passees, couts, sous-agents, hooks, appels d'outils — au meme endroit. Backend Rust, frontend React, binaire d'environ 10 Mo. Zero configuration, zero compte, 100 % local.

**30 versions. 85 outils MCP. 9 skills. Un seul `npx claude-view`.**

---

## Moniteur en direct

Visualisez chaque session en cours d'un seul coup d'oeil. Plus besoin de jongler entre les onglets de terminal.

| Fonctionnalite | Ce qu'elle fait |
|---------|-------------|
| **Cartes de session** | Chaque carte affiche le dernier message, le modele, le cout et le statut — sachez instantanement sur quoi travaille chaque agent |
| **Chat multi-session** | Ouvrez les sessions cote a cote dans des onglets style VS Code (dockview). Glissez pour diviser horizontalement ou verticalement |
| **Jauge de contexte** | Remplissage de la fenetre de contexte en temps reel par session — identifiez quels agents sont en zone de danger avant qu'ils n'atteignent la limite |
| **Compte a rebours du cache** | Sachez exactement quand le cache de prompt expire pour synchroniser vos messages et economiser des tokens |
| **Suivi des couts** | Depense par session et agregee avec ventilation des tokens — survolez pour voir la repartition entree/sortie/cache par modele |
| **Arbre des sous-agents** | Visualisez l'arborescence complete des agents generes, leur statut, leurs couts et les outils qu'ils appellent |
| **Notifications sonores** | Recevez une alerte quand une session se termine, rencontre une erreur ou necessite votre intervention — arretez de scruter les terminaux |
| **Vues multiples** | Grille, Liste, Kanban ou mode Moniteur — choisissez ce qui correspond a votre flux de travail |
| **Couloirs Kanban** | Regroupez les sessions par projet ou branche — disposition visuelle en couloirs pour les flux de travail multi-projets |
| **Sessions recemment fermees** | Les sessions terminees apparaissent dans « Recemment fermees » au lieu de disparaitre — persiste entre les redemarrages du serveur |
| **Messages en file d'attente** | Les messages en attente dans la file s'affichent comme des bulles avec un badge « En attente » |
| **Pilote par SSE** | Toutes les donnees en direct sont poussees via Server-Sent Events — elimine totalement les risques de cache obsolete |

---

## Chat et conversation

Lisez, recherchez et interagissez avec n'importe quelle session — en direct ou historique.

| Fonctionnalite | Ce qu'elle fait |
|---------|-------------|
| **Chat en direct unifie** | Historique et messages en temps reel dans une seule conversation defilable — sans changement d'onglet |
| **Mode developpeur** | Basculez entre les vues Chat et Developpeur par session. Le mode developpeur affiche les cartes d'outils, les cartes d'evenements, les metadonnees de hooks et la trace d'execution complete avec des filtres |
| **Navigateur de conversations complet** | Chaque session, chaque message, entierement rendu avec markdown et blocs de code |
| **Visualisation des appels d'outils** | Consultez les lectures de fichiers, les modifications, les commandes bash, les appels MCP, les invocations de skills — pas seulement du texte |
| **Bascule compact / detaille** | Survolez la conversation ou examinez chaque appel d'outil en detail |
| **Vue en fil** | Suivez les conversations d'agents avec les hierarchies de sous-agents et l'indentation des fils |
| **Evenements de hooks integres** | Les hooks pre/post-outil sont affiches comme des blocs de conversation — visualisez les hooks qui se declenchent en parallele de la conversation |
| **Export** | Export Markdown pour reprendre le contexte ou partager |
| **Selection groupee et archivage** | Selectionnez plusieurs sessions pour un archivage groupe avec etat de filtre persistant |
| **Partage chiffre** | Partagez n'importe quelle session via un lien chiffre de bout en bout — AES-256-GCM, zero confiance serveur, la cle reside uniquement dans le fragment d'URL |

---

## Mecanismes internes des agents

Claude Code fait beaucoup de choses derriere `"thinking..."` qui n'apparaissent jamais dans votre terminal. claude-view expose tout cela.

| Fonctionnalite | Ce qu'elle fait |
|---------|-------------|
| **Conversations de sous-agents** | Arborescence complete des agents generes, leurs prompts, leurs sorties et la ventilation cout/tokens par agent |
| **Appels de serveurs MCP** | Quels outils MCP sont invoques et leurs resultats |
| **Suivi des skills / hooks / plugins** | Quels skills se sont declenches, quels hooks ont ete executes, quels plugins sont actifs |
| **Enregistrement des evenements de hooks** | Capture de hooks en double canal (WebSocket en direct + retroaction JSONL) — chaque evenement est enregistre et consultable, meme pour les sessions passees |
| **Badges de source de session** | Chaque session indique comment elle a ete lancee : Terminal, VS Code, Agent SDK ou d'autres points d'entree |
| **Derive de branche worktree** | Detecte quand les branches git worktree divergent — affiche dans le moniteur en direct et l'historique |
| **Puces de mention @File** | Les references `@filename` sont extraites et affichees sous forme de puces — survolez pour le chemin complet |
| **Chronologie d'utilisation des outils** | Journal d'actions de chaque paire tool_use/tool_result avec le chronometrage |
| **Remontee des erreurs** | Les erreurs remontent jusqu'a la carte de session — plus d'echecs enfouis |
| **Inspecteur de messages bruts** | Explorez le JSON brut de n'importe quel message quand vous avez besoin de la vue complete |

---

## Recherche

| Fonctionnalite | Ce qu'elle fait |
|---------|-------------|
| **Recherche plein texte** | Recherchez dans toutes les sessions — messages, appels d'outils, chemins de fichiers. Propulse par Tantivy (natif Rust, classe Lucene) |
| **Moteur de recherche unifie** | Tantivy plein texte + pre-filtre SQLite executent en parallele — un seul endpoint, resultats en moins de 50 ms |
| **Filtres par projet et branche** | Delimitez la portee au projet ou a la branche sur lequel vous travaillez en ce moment |
| **Palette de commandes** | <kbd>Cmd</kbd>+<kbd>K</kbd> pour naviguer entre les sessions, changer de vue, trouver n'importe quoi |

---

## Analytiques

Une suite analytique complete pour votre utilisation de Claude Code. Comme le tableau de bord de Cursor, mais plus approfondi.

<details>
<summary><strong>Tableau de bord</strong></summary>
<br>

| Fonctionnalite | Description |
|---------|-------------|
| **Metriques semaine par semaine** | Nombre de sessions, utilisation de tokens, cout — compares a votre periode precedente |
| **Carte thermique d'activite** | Grille de 90 jours style GitHub montrant l'intensite d'utilisation quotidienne |
| **Top skills / commandes / outils MCP / agents** | Classements de vos invocables les plus utilises — cliquez sur l'un d'eux pour rechercher les sessions correspondantes |
| **Projets les plus actifs** | Graphique en barres des projets classes par nombre de sessions |
| **Ventilation de l'utilisation des outils** | Total des modifications, lectures et commandes bash sur toutes les sessions |
| **Sessions les plus longues** | Acces rapide a vos sessions marathon avec leur duree |

</details>

<details>
<summary><strong>Contributions IA</strong></summary>
<br>

| Fonctionnalite | Description |
|---------|-------------|
| **Suivi de la production de code** | Lignes ajoutees/supprimees, fichiers touches, nombre de commits — sur toutes les sessions |
| **Metriques de ROI des couts** | Cout par commit, cout par session, cout par ligne de sortie IA — avec graphiques de tendance |
| **Comparaison de modeles** | Ventilation cote a cote de la production et de l'efficacite par modele (Opus, Sonnet, Haiku) |
| **Courbe d'apprentissage** | Taux de re-edition au fil du temps — constatez vos progres en prompting |
| **Ventilation par branche** | Vue par branche depliable avec exploration des sessions |
| **Efficacite des skills** | Quels skills ameliorent reellement votre production vs ceux qui n'ont aucun effet |

</details>

<details>
<summary><strong>Perspectives</strong> <em>(experimental)</em></summary>
<br>

| Fonctionnalite | Description |
|---------|-------------|
| **Detection de patterns** | Patterns comportementaux decouverts a partir de votre historique de sessions |
| **Comparaisons avant vs maintenant** | Comparez votre premier mois a votre utilisation recente |
| **Ventilation par categorie** | Treemap de vos usages de Claude — refactoring, fonctionnalites, debogage, etc. |
| **Score de maitrise IA** | Un seul chiffre de 0 a 100 suivant votre efficacite globale |

> Les perspectives et le score de maitrise sont experimentaux. A considerer comme indicatifs, pas definitifs.

</details>

---

## Plans, prompts et equipes

| Fonctionnalite | Ce qu'elle fait |
|---------|-------------|
| **Navigateur de plans** | Consultez vos `.claude/plans/` directement dans le detail de la session — plus besoin de chercher dans les fichiers |
| **Historique des prompts** | Recherche plein texte dans tous les prompts que vous avez envoyes avec regroupement par template et classification d'intention |
| **Tableau de bord d'equipe** | Consultez les responsables d'equipe, les messages de la boite de reception, les taches d'equipe et les modifications de fichiers de tous les membres |
| **Analytiques des prompts** | Classements des templates de prompts, distribution des intentions et statistiques d'utilisation |

---

## Moniteur systeme

| Fonctionnalite | Ce qu'elle fait |
|---------|-------------|
| **Jauges CPU / RAM / Disque en direct** | Metriques systeme en temps reel diffusees via SSE avec transitions animees fluides |
| **Tableau de bord des composants** | Consultez les metriques du sidecar et de l'IA embarquee : utilisation VRAM, CPU, RAM et nombre de sessions par composant |
| **Liste des processus** | Processus regroupes par nom, tries par CPU — voyez ce que fait reellement votre machine pendant que les agents s'executent |

---

## IA embarquee

Executez un LLM local pour la classification de phase des sessions — aucun appel API, aucun cout supplementaire.

| Fonctionnalite | Ce qu'elle fait |
|---------|-------------|
| **Agnostique du fournisseur** | Connectez-vous a n'importe quel endpoint compatible OpenAI — oMLX, Ollama, LM Studio ou votre propre serveur |
| **Selecteur de modele** | Choisissez parmi un registre de modeles selectionnes avec les besoins en RAM affiches |
| **Classification de phase** | Les sessions sont etiquetees avec leur phase actuelle (codage, debogage, planification, etc.) avec affichage controle par seuil de confiance |
| **Gestion intelligente des ressources** | Classification stabilisee par EMA avec backoff exponentiel — 93 % de reduction du gaspillage GPU par rapport a une interrogation naive |

---

## Plugin

`@claude-view/plugin` donne a Claude un acces natif aux donnees de votre tableau de bord — 85 outils MCP, 9 skills et demarrage automatique.

```bash
claude plugin add @claude-view/plugin
```

### Demarrage automatique

Chaque session Claude Code demarre automatiquement le tableau de bord. Plus besoin de lancer `npx claude-view` manuellement.

### 85 outils MCP

8 outils faits main avec une sortie optimisee pour Claude :

| Outil | Description |
|------|-------------|
| `list_sessions` | Parcourir les sessions avec filtres |
| `get_session` | Detail complet de la session avec messages et metriques |
| `search_sessions` | Recherche plein texte dans toutes les conversations |
| `get_stats` | Vue d'ensemble du tableau de bord — total des sessions, couts, tendances |
| `get_fluency_score` | Score de maitrise IA (0-100) avec ventilation |
| `get_token_stats` | Utilisation des tokens avec taux de hit du cache |
| `list_live_sessions` | Agents en cours d'execution (temps reel) |
| `get_live_summary` | Cout agrege et statut pour aujourd'hui |

Plus **78 outils auto-generes** a partir de la specification OpenAPI a travers 26 categories (contributions, perspectives, coaching, exports, workflows, et plus).

### 9 Skills

| Skill | Ce qu'il fait |
|-------|-------------|
| `/session-recap` | Resume d'une session specifique — commits, metriques, duree |
| `/daily-cost` | Depenses du jour, sessions en cours, utilisation des tokens |
| `/standup` | Journal de travail multi-session pour les mises a jour de standup |
| `/coaching` | Conseils de coaching IA et gestion de regles personnalisees |
| `/insights` | Analyse de patterns comportementaux |
| `/project-overview` | Resume de projet a travers les sessions |
| `/search` | Recherche en langage naturel |
| `/export-data` | Export des sessions en CSV/JSON |
| `/team-status` | Vue d'ensemble de l'activite de l'equipe |

---

## Workflows

| Fonctionnalite | Ce qu'elle fait |
|---------|-------------|
| **Constructeur de workflows** | Creez des workflows multi-etapes avec une disposition style VS Code, apercu de diagramme Mermaid et editeur YAML |
| **Rail de chat LLM en streaming** | Generez des definitions de workflows en temps reel via un chat integre |
| **Executeur d'etapes** | Visualisez les colonnes d'etapes, les cartes de tentatives et la barre de progression pendant l'execution de votre workflow |
| **Workflows integres** | Plan Polisher et Plan Executor sont inclus par defaut |

---

## Ouvrir dans l'IDE

| Fonctionnalite | Ce qu'elle fait |
|---------|-------------|
| **Ouverture de fichier en un clic** | Les fichiers references dans les sessions s'ouvrent directement dans votre editeur |
| **Detection automatique de votre editeur** | VS Code, Cursor, Zed et d'autres — aucune configuration necessaire |
| **Partout ou c'est utile** | Le bouton apparait dans l'onglet Changements, les en-tetes de fichiers et les en-tetes de projets Kanban |
| **Memoire de preference** | Votre editeur prefere est memorise entre les sessions |

---

## Comment c'est construit

| | |
|---|---|
| **Rapide** | Backend Rust avec analyse JSONL acceleree par SIMD, E/S mappees en memoire — indexe des milliers de sessions en quelques secondes |
| **Temps reel** | File watcher + SSE + WebSocket multiplexe avec heartbeat, relecture d'evenements et recuperation apres crash |
| **Compact** | Telechargement d'environ 10 Mo, environ 27 Mo sur disque. Aucune dependance d'execution, aucun daemon en arriere-plan |
| **100 % local** | Toutes les donnees restent sur votre machine. Zero telemetrie par defaut, zero compte requis |
| **Zero config** | `npx claude-view` et c'est parti. Pas de cles API, pas de configuration, pas de comptes |
| **Pilote par FSM** | Les sessions de chat fonctionnent sur une machine a etats finis avec des phases explicites et des evenements types — deterministe, sans conditions de course |

<details>
<summary><strong>Les chiffres</strong></summary>
<br>

Mesure sur un Mac serie M avec 1 493 sessions sur 26 projets :

| Metrique | claude-view | Tableau de bord Electron typique |
|--------|:-----------:|:--------------------------:|
| **Telechargement** | **~10 Mo** | 150-300 Mo |
| **Sur disque** | **~27 Mo** | 300-500 Mo |
| **Demarrage** | **< 500 ms** | 3-8 s |
| **RAM (index complet)** | **~50 Mo** | 300-800 Mo |
| **Indexer 1 500 sessions** | **< 1 s** | N/A |
| **Dependances d'execution** | **0** | Node.js + Chromium |

Techniques cles : pre-filtre SIMD (`memchr`), analyse JSONL par memoire mappee, recherche plein texte Tantivy, tranches zero-copie du mmap a travers l'analyse jusqu'a la reponse.

</details>

---

## Comparaison

| Outil | Categorie | Stack | Taille | Moniteur en direct | Chat multi-session | Recherche | Analytiques | Outils MCP |
|------|----------|-------|:----:|:------------:|:------------------:|:------:|:---------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | Moniteur + espace de travail | Rust | **~10 Mo** | **Oui** | **Oui** | **Oui** | **Oui** | **85** |
| [opcode](https://github.com/winfunc/opcode) | GUI + gestionnaire de sessions | Tauri 2 | ~13 Mo | Partiel | Non | Non | Oui | Non |
| [ccusage](https://github.com/ryoppippi/ccusage) | Suivi d'utilisation CLI | TypeScript | ~600 Ko | Non | Non | Non | CLI | Non |
| [CodePilot](https://github.com/op7418/CodePilot) | Interface de chat bureau | Electron | ~140 Mo | Non | Non | Non | Non | Non |
| [claude-run](https://github.com/kamranahmedse/claude-run) | Visionneuse d'historique | TypeScript | ~500 Ko | Partiel | Non | Basique | Non | Non |

> Les interfaces de chat (CodePilot, CUI, claude-code-webui) sont des interfaces *pour* Claude Code. claude-view est un tableau de bord qui surveille vos sessions de terminal existantes. Ils sont complementaires.

---

## Installation

| Methode | Commande |
|--------|---------|
| **Shell** (recommande) | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |
| **Plugin** (demarrage automatique) | `claude plugin add @claude-view/plugin` |

L'installateur shell telecharge un binaire pre-compile (environ 10 Mo), l'installe dans `~/.claude-view/bin` et l'ajoute a votre PATH. Ensuite, lancez simplement `claude-view`.

**Seule exigence :** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) installe.

<details>
<summary><strong>Configuration</strong></summary>
<br>

| Variable d'environnement | Defaut | Description |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` ou `PORT` | `47892` | Remplacer le port par defaut |

</details>

<details>
<summary><strong>Auto-hebergement et developpement local</strong></summary>
<br>

Le binaire pre-compile inclut l'authentification, le partage et le relai mobile integres. Vous compilez depuis les sources ? Ces fonctionnalites sont **optionnelles via des variables d'environnement** — omettez-en une et cette fonctionnalite est simplement desactivee.

| Variable d'environnement | Fonctionnalite | Sans elle |
|-------------|---------|------------|
| `SUPABASE_URL` | Connexion / authentification | Authentification desactivee — mode entierement local, sans compte |
| `RELAY_URL` | Appairage mobile | Appairage QR indisponible |
| `SHARE_WORKER_URL` + `SHARE_VIEWER_URL` | Partage chiffre | Bouton de partage masque |

```bash
bun dev    # entierement local, aucune dependance cloud
```

</details>

<details>
<summary><strong>Entreprise / Environnements sandbox</strong></summary>
<br>

Si votre machine restreint les ecritures (DataCloak, CrowdStrike, DLP d'entreprise) :

```bash
cp crates/server/.env.example .env
# Decommentez CLAUDE_VIEW_DATA_DIR
```

Cela conserve la base de donnees, l'index de recherche et les fichiers de verrouillage a l'interieur du depot. Definissez `CLAUDE_VIEW_SKIP_HOOKS=1` pour ignorer l'enregistrement des hooks dans les environnements en lecture seule.

</details>

---

## FAQ

<details>
<summary><strong>La banniere « Non connecte » s'affiche alors que je suis connecte</strong></summary>
<br>

claude-view verifie vos identifiants Claude en lisant `~/.claude/.credentials.json` (avec repli sur le trousseau macOS). Essayez ces etapes :

1. **Verifiez l'authentification Claude CLI :** `claude auth status`
2. **Verifiez le fichier d'identifiants :** `cat ~/.claude/.credentials.json` — devrait contenir une section `claudeAiOauth` avec un `accessToken`
3. **Verifiez le trousseau macOS :** `security find-generic-password -s "Claude Code-credentials" -w`
4. **Verifiez l'expiration du token :** Regardez `expiresAt` dans le JSON des identifiants — s'il est depasse, executez `claude auth login`
5. **Verifiez HOME :** `echo $HOME` — le serveur lit depuis `$HOME/.claude/.credentials.json`

Si toutes les verifications sont concluantes et que la banniere persiste, signalez-le sur [Discord](https://discord.gg/G7wdZTpRfu).

</details>

<details>
<summary><strong>A quelles donnees claude-view accede-t-il ?</strong></summary>
<br>

claude-view lit les fichiers de session JSONL que Claude Code ecrit dans `~/.claude/projects/`. Il les indexe localement a l'aide de SQLite et Tantivy. **Aucune donnee ne quitte votre machine** sauf si vous utilisez explicitement la fonctionnalite de partage chiffre. La telemetrie est optionnelle et desactivee par defaut.

</details>

<details>
<summary><strong>Est-ce que ca fonctionne avec Claude Code dans VS Code / Cursor / les extensions IDE ?</strong></summary>
<br>

Oui. claude-view surveille toutes les sessions Claude Code quel que soit leur mode de lancement — CLI terminal, extension VS Code, Cursor ou Agent SDK. Chaque session affiche un badge de source (Terminal, VS Code, SDK) pour que vous puissiez filtrer par methode de lancement.

</details>

---

## Communaute

- **Site web :** [claudeview.ai](https://claudeview.ai) — documentation, changelog, blog
- **Discord :** [Rejoindre le serveur](https://discord.gg/G7wdZTpRfu) — support, demandes de fonctionnalites, discussions
- **Plugin :** [`@claude-view/plugin`](https://www.npmjs.com/package/@claude-view/plugin) — 85 outils MCP, 9 skills, demarrage automatique

---

<details>
<summary><strong>Developpement</strong></summary>
<br>

Prerequis : [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Installer toutes les dependances du workspace
bun dev            # Demarrer le dev full-stack (Rust + Web + Sidecar avec rechargement a chaud)
```

### Organisation du workspace

| Chemin | Package | Fonction |
|------|---------|---------|
| `apps/web/` | `@claude-view/web` | SPA React (Vite) — frontend web principal |
| `apps/share/` | `@claude-view/share` | SPA de visualisation de partage — Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | Application native Expo |
| `apps/landing/` | `@claude-view/landing` | Page d'accueil Astro 5 (zero JS cote client) |
| `packages/shared/` | `@claude-view/shared` | Types partages et tokens de theme |
| `packages/design-tokens/` | `@claude-view/design-tokens` | Couleurs, espacement, typographie |
| `packages/plugin/` | `@claude-view/plugin` | Plugin Claude Code (serveur MCP + outils + skills) |
| `crates/` | — | Backend Rust (Axum) |
| `sidecar/` | — | Sidecar Node.js (pont Agent SDK) |
| `infra/share-worker/` | — | Cloudflare Worker — API de partage (R2 + D1) |
| `infra/install-worker/` | — | Cloudflare Worker — script d'installation avec suivi des telechargements |

### Commandes de developpement

| Commande | Description |
|---------|-------------|
| `bun dev` | Dev full-stack — Rust + Web + Sidecar avec rechargement a chaud |
| `bun run dev:web` | Frontend web uniquement |
| `bun run dev:server` | Backend Rust uniquement |
| `bun run build` | Compiler tous les workspaces |
| `bun run preview` | Compiler le web + servir via le binaire de release |
| `bun run lint:all` | Lint JS/TS + Rust (Clippy) |
| `bun run typecheck` | Verification de types TypeScript |
| `bun run test` | Executer tous les tests (Turbo) |
| `bun run test:rust` | Executer les tests Rust |
| `bun run storybook` | Lancer Storybook pour le developpement de composants |
| `bun run dist:test` | Compiler + packager + installer + executer (test de distribution complet) |

### Publication

```bash
bun run release          # increment patch
bun run release:minor    # increment mineur
git push origin main --tags    # declenche la CI → compile → publie automatiquement sur npm
```

</details>

---

## Support des plateformes

| Plateforme | Statut |
|----------|--------|
| macOS (Apple Silicon) | Disponible |
| macOS (Intel) | Disponible |
| Linux (x64) | Prevu |
| Windows (x64) | Prevu |

---

## Liens utiles

- **[claudeview.ai](https://claudeview.ai)** — Site officiel, documentation et changelog
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** — Plugin Claude Code avec 85 outils MCP et 9 skills. `claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** — Claude Code supprime vos sessions apres 30 jours. Cet outil les sauvegarde. `npx claude-backup`

---

<div align="center">

Si **claude-view** vous aide a voir ce que font vos agents IA, pensez a lui donner une etoile.

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT &copy; 2026

</div>
