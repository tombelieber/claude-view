# claude-view

<p align="center">
  <strong>Moniteur en direct et copilote pour les utilisateurs avancés de Claude Code.</strong>
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

## Le Problème

Vous avez 3 projets ouverts. Chaque projet a plusieurs worktrees git. Chaque worktree a plusieurs sessions Claude Code en cours. Certaines réfléchissent, d'autres attendent votre input, certaines sont sur le point d'atteindre les limites de contexte, et une s'est terminée il y a 10 minutes mais vous l'avez oublié.

Vous faites Cmd-Tab entre 15 fenêtres de terminal en essayant de vous rappeler quelle session faisait quoi. Vous brûlez des tokens parce qu'un cache a expiré pendant que vous ne regardiez pas. Vous perdez votre flow parce qu'il n'y a pas d'endroit unique pour tout voir. Et derrière ce spinner "réflexion en cours...", Claude génère des sous-agents, appelle des serveurs MCP, exécute des skills, déclenche des hooks — et vous ne pouvez rien voir.

**Claude Code est incroyablement puissant. Mais piloter 10+ sessions concurrentes sans tableau de bord, c'est comme conduire sans compteur de vitesse.**

## La Solution

**claude-view** est un tableau de bord en temps réel qui fonctionne aux côtés de vos sessions Claude Code. Un onglet de navigateur, chaque session visible, contexte complet en un coup d'œil.

```bash
npx claude-view
```

C'est tout. S'ouvre dans votre navigateur. Toutes vos sessions — en direct et passées — dans un seul espace de travail.

---

## Ce Que Vous Obtenez

### Moniteur en Direct

| Fonctionnalité | Pourquoi c'est important |
|---------|---------------|
| **Cartes de session avec dernier message** | Rappelez-vous instantanément ce que fait chaque session de longue durée |
| **Sons de notification** | Soyez alerté quand une session se termine ou nécessite votre input — arrêtez de sonder les terminaux |
| **Jauge de contexte** | Utilisation de la fenêtre de contexte en temps réel par session — voyez lesquelles sont en zone de danger |
| **Compte à rebours du cache** | Sachez exactement quand le cache de prompts expire pour programmer votre prochain message et économiser des tokens |
| **Suivi des coûts** | Dépense par session et agrégée avec ventilation des économies de cache |
| **Visualisation des sous-agents** | Voyez l'arbre complet des agents — sous-agents, leur statut et les outils qu'ils appellent |
| **Vues multiples** | Grille, Liste ou mode Moniteur (grille de chat en direct) — choisissez ce qui convient à votre workflow |

### Historique de Chat Enrichi

| Fonctionnalité | Pourquoi c'est important |
|---------|---------------|
| **Navigateur de conversation complet** | Chaque session, chaque message, entièrement rendu avec markdown et blocs de code |
| **Visualisation des appels d'outils** | Voyez les lectures de fichiers, éditions, commandes bash, appels MCP, invocations de skills — pas seulement du texte |
| **Toggle compact / détaillé** | Survolez la conversation ou plongez dans chaque appel d'outil |
| **Vue par fils** | Suivez les conversations d'agents avec les hiérarchies de sous-agents |
| **Export** | Export Markdown pour la reprise de contexte ou le partage |

### Recherche Avancée

| Fonctionnalité | Pourquoi c'est important |
|---------|---------------|
| **Recherche plein texte** | Cherchez à travers toutes les sessions — messages, appels d'outils, chemins de fichiers |
| **Filtres projet et branche** | Limitez la portée au projet sur lequel vous travaillez maintenant |
| **Palette de commandes** | Cmd+K pour naviguer entre les sessions, changer de vue, trouver n'importe quoi |

### Internes de l'Agent — Voyez Ce Qui Est Caché

Claude Code fait beaucoup de choses derrière "réflexion en cours..." qui n'apparaissent jamais dans votre terminal. claude-view expose tout.

| Fonctionnalité | Pourquoi c'est important |
|---------|---------------|
| **Conversations de sous-agents** | Voyez l'arbre complet des agents générés, leurs prompts et leurs résultats |
| **Appels aux serveurs MCP** | Voyez quels outils MCP sont invoqués et leurs résultats |
| **Suivi des skills / hooks / plugins** | Sachez quels skills se sont déclenchés, quels hooks ont tourné, quels plugins sont actifs |
| **Enregistrement d'événements de hooks** | Chaque événement de hook est capturé et navigable — retournez vérifier ce qui s'est déclenché et quand. *(Nécessite que claude-view soit en cours d'exécution pendant que les sessions sont actives ; ne peut pas tracer les événements historiques rétroactivement)* |
| **Chronologie d'utilisation des outils** | Log d'actions de chaque paire tool_use/tool_result avec timing |
| **Remontée d'erreurs** | Les erreurs remontent à la carte de session — plus de défaillances enterrées |
| **Inspecteur de messages bruts** | Plongez dans le JSON brut de n'importe quel message quand vous avez besoin de l'image complète |

### Analytiques

Une suite d'analytiques riche pour votre utilisation de Claude Code. Pensez au tableau de bord de Cursor, mais en plus profond.

**Aperçu du Tableau de Bord**

| Fonctionnalité | Description |
|---------|-------------|
| **Métriques semaine par semaine** | Nombre de sessions, utilisation de tokens, coût — comparé à votre période précédente |
| **Carte de chaleur d'activité** | Grille style GitHub de 90 jours montrant l'intensité quotidienne de votre utilisation de Claude Code |
| **Top skills / commandes / outils MCP / agents** | Classements de vos invocables les plus utilisés — cliquez sur n'importe lequel pour chercher les sessions correspondantes |
| **Projets les plus actifs** | Graphique en barres des projets classés par nombre de sessions |
| **Ventilation d'utilisation des outils** | Total des éditions, lectures et commandes bash à travers toutes les sessions |
| **Sessions les plus longues** | Accès rapide à vos sessions marathon avec durée |

**Contributions IA**

| Fonctionnalité | Description |
|---------|-------------|
| **Suivi du code produit** | Lignes ajoutées/supprimées, fichiers touchés, nombre de commits — à travers toutes les sessions |
| **Métriques de ROI coût** | Coût par commit, coût par session, coût par ligne de sortie IA — avec graphiques de tendance |
| **Comparaison de modèles** | Ventilation côte à côte du rendement et de l'efficacité par modèle (Opus, Sonnet, Haiku) |
| **Courbe d'apprentissage** | Taux de re-édition au fil du temps — voyez-vous progresser en prompting |
| **Ventilation par branche** | Vue pliable par branche avec drill-down de sessions |
| **Efficacité des skills** | Quels skills améliorent réellement votre production vs ceux qui ne le font pas |

**Insights** *(expérimental)*

| Fonctionnalité | Description |
|---------|-------------|
| **Détection de patterns** | Patterns comportementaux découverts dans votre historique de sessions |
| **Benchmarks avant vs maintenant** | Comparez votre premier mois à votre utilisation récente |
| **Ventilation par catégorie** | Treemap de ce pour quoi vous utilisez Claude — refactorisation, features, debugging, etc. |
| **Score de Fluidité IA** | Un seul chiffre 0-100 qui suit votre efficacité globale |

> **Note :** Insights et Score de Fluidité sont en phase expérimentale précoce. Considérez-les comme directionnels, pas définitifs.

---

## Conçu Pour le Flow

claude-view est conçu pour le développeur qui :

- Exécute **3+ projets simultanément**, chacun avec plusieurs worktrees
- A **10-20 sessions Claude Code** ouvertes à tout moment
- A besoin de changer de contexte rapidement sans perdre le fil
- Veut **optimiser les dépenses de tokens** en programmant les messages autour des fenêtres de cache
- Est frustré de faire Cmd-Tab entre les terminaux pour vérifier les agents

Un onglet de navigateur. Toutes les sessions. Restez dans le flow.

---

## Comment C'est Construit

| | |
|---|---|
| **Ultra rapide** | Backend Rust avec parsing JSONL accéléré par SIMD, I/O mappé en mémoire — indexe des milliers de sessions en secondes |
| **Temps réel** | File watcher + SSE + WebSocket pour des mises à jour en direct sub-seconde sur toutes les sessions |
| **Empreinte minimale** | Un seul binaire de ~15 Mo. Pas de dépendances runtime, pas de démons en arrière-plan |
| **100% local** | Toutes les données restent sur votre machine. Zéro télémétrie, zéro cloud, zéro requêtes réseau |
| **Zéro configuration** | `npx claude-view` et c'est fait. Pas de clés API, pas de setup, pas de compte |

---

## Démarrage Rapide

```bash
npx claude-view
```

S'ouvre à `http://localhost:47892`.

### Configuration

| Variable d'Environnement | Défaut | Description |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` ou `PORT` | `47892` | Remplacer le port par défaut |

---

## Installation

| Méthode | Commande |
|--------|---------|
| **npx** (recommandé) | `npx claude-view` |
| **Script shell** (Node non requis) | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

### Prérequis

- **Claude Code** installé ([obtenez-le ici](https://docs.anthropic.com/en/docs/claude-code)) — cela crée les fichiers de session que nous surveillons

---

## Comparatif

Les autres outils sont soit des visualiseurs (parcourir l'historique) soit de simples moniteurs. Aucun ne combine surveillance en temps réel, historique de chat enrichi, outils de debugging et recherche avancée dans un seul espace de travail.

```
                    Passif ←————————————→ Actif
                         |                  |
            Vue seule    |  ccusage         |
                         |  History Viewer  |
                         |  clog            |
                         |                  |
            Moniteur     |  claude-code-ui  |
            seul         |  Agent Sessions  |
                         |                  |
            Espace de    |  ★ claude-view   |
            travail      |                  |
            complet      |                  |
```

---

## Communauté

Rejoignez le [serveur Discord](https://discord.gg/G7wdZTpRfu) pour le support, les demandes de fonctionnalités et les discussions.

---

## Vous aimez ce projet ?

Si **claude-view** vous aide à maîtriser Claude Code, pensez à lui donner une étoile. Cela aide les autres à découvrir cet outil.

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## Développement

Prérequis : [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Installer les dépendances frontend
bun dev            # Démarrer le développement full-stack (Rust + Vite avec hot reload)
```

| Commande | Description |
|---------|-------------|
| `bun dev` | Développement full-stack — Rust redémarre automatiquement sur les changements, Vite HMR |
| `bun dev:server` | Backend Rust uniquement (avec cargo-watch) |
| `bun dev:client` | Frontend Vite uniquement (suppose le backend en cours) |
| `bun run build` | Compiler le frontend pour la production |
| `bun run preview` | Compiler + servir via le binaire release |
| `bun run lint` | Lint du frontend (ESLint) et du backend (Clippy) |
| `bun run fmt` | Formater le code Rust |
| `bun run check` | Typecheck + lint + test (porte de pré-commit) |
| `bun test` | Exécuter la suite de tests Rust (`cargo test --workspace`) |
| `bun test:client` | Exécuter les tests frontend (vitest) |
| `bun run test:e2e` | Exécuter les tests end-to-end Playwright |

### Test de la Distribution en Production

```bash
bun run dist:test    # Une commande : build → pack → install → run
```

Ou étape par étape :

| Commande | Description |
|---------|-------------|
| `bun run dist:pack` | Empaqueter binaire + frontend en tarball dans `/tmp/` |
| `bun run dist:install` | Extraire le tarball dans `~/.cache/claude-view/` (simule le premier téléchargement) |
| `bun run dist:run` | Exécuter le wrapper npx avec le binaire en cache |
| `bun run dist:test` | Tout ce qui précède en une seule commande |
| `bun run dist:clean` | Supprimer tous les fichiers de cache dist et temporaires |

### Publication

```bash
bun run release          # bump patch : 0.1.0 → 0.1.1
bun run release:minor    # bump mineur : 0.1.0 → 0.2.0
bun run release:major    # bump majeur : 0.1.0 → 1.0.0
```

Cela incrémente la version dans `npx-cli/package.json`, fait un commit et crée un tag git. Ensuite :

```bash
git push origin main --tags    # déclenche CI → compile toutes les plateformes → auto-publie sur npm
```

---

## Support des Plateformes

| Plateforme | Statut |
|----------|--------|
| macOS (Apple Silicon) | Disponible |
| macOS (Intel) | Disponible |
| Linux (x64) | Planifié |
| Windows (x64) | Planifié |

---

## Licence

MIT © 2026
