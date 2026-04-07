<div align="center">

# claude-view

**Centro de Control para Claude Code**

Tienes 10 agentes de IA ejecutándose. Uno terminó hace 12 minutos. Otro alcanzó su límite de contexto. Un tercero necesita aprobación de herramientas. Estás haciendo <kbd>Cmd</kbd>+<kbd>Tab</kbd> entre terminales, gastando $200/mes a ciegas.

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

**Un comando. Todas las sesiones visibles. En tiempo real.**

</div>

---

## ¿Qué es claude-view?

claude-view es un panel de control de código abierto que monitorea cada sesión de Claude Code en tu máquina — agentes en vivo, conversaciones pasadas, costos, sub-agentes, hooks, llamadas a herramientas — en un solo lugar. Backend en Rust, frontend en React, binario de ~10 MB. Sin configuración, sin cuentas, 100% local.

**30 versiones. 85 herramientas MCP. 9 skills. Un solo `npx claude-view`.**

---

## Monitor en Vivo

Ve cada sesión en ejecución de un vistazo. Se acabó el cambiar entre pestañas de terminal.

| Característica | Qué hace |
|---------|-------------|
| **Tarjetas de sesión** | Cada tarjeta muestra el último mensaje, modelo, costo y estado — sabe al instante en qué está trabajando cada agente |
| **Chat multi-sesión** | Abre sesiones lado a lado en pestañas estilo VS Code (dockview). Arrastra para dividir horizontal o verticalmente |
| **Indicador de contexto** | Llenado de ventana de contexto en tiempo real por sesión — ve qué agentes están en zona de peligro antes de que alcancen el límite |
| **Cuenta regresiva de caché** | Sabe exactamente cuándo expira la caché de prompts para que puedas programar mensajes y ahorrar tokens |
| **Seguimiento de costos** | Gasto por sesión y agregado con desglose de tokens — pasa el cursor para ver división de entrada/salida/caché por modelo |
| **Árbol de sub-agentes** | Ve el árbol completo de agentes generados, su estado, costos y qué herramientas están llamando |
| **Sonidos de notificación** | Recibe alertas cuando una sesión termina, tiene errores o necesita tu intervención — deja de revisar terminales |
| **Múltiples vistas** | Cuadrícula, Lista, Kanban o modo Monitor — elige lo que se adapte a tu flujo de trabajo |
| **Carriles Kanban** | Agrupa sesiones por proyecto o rama — disposición visual de carriles para flujos de trabajo multi-proyecto |
| **Cerradas recientemente** | Las sesiones que terminan aparecen en "Cerradas Recientemente" en vez de desaparecer — persiste entre reinicios del servidor |
| **Mensajes en cola** | Los mensajes esperando en la cola se muestran como burbujas pendientes con una insignia "En Cola" |
| **Impulsado por SSE** | Todos los datos en vivo se envían vía Server-Sent Events — elimina por completo los riesgos de caché obsoleta |

---

## Chat y Conversación

Lee, busca e interactúa con cualquier sesión — en vivo o histórica.

| Característica | Qué hace |
|---------|-------------|
| **Chat en vivo unificado** | Historial y mensajes en tiempo real en una sola conversación desplazable — sin cambiar de pestaña |
| **Modo desarrollador** | Alterna entre vistas de Chat y Desarrollador por sesión. El modo desarrollador muestra tarjetas de herramientas, tarjetas de eventos, metadatos de hooks y la traza de ejecución completa con chips de filtro |
| **Explorador completo de conversaciones** | Cada sesión, cada mensaje, completamente renderizado con markdown y bloques de código |
| **Visualización de llamadas a herramientas** | Ve lecturas de archivos, ediciones, comandos bash, llamadas MCP, invocaciones de skills — no solo texto |
| **Alternador compacto / detallado** | Revisa la conversación rápidamente o profundiza en cada llamada a herramienta |
| **Vista de hilos** | Sigue conversaciones de agentes con jerarquías de sub-agentes e hilos indentados |
| **Eventos de hooks en línea** | Hooks pre/post herramienta renderizados como bloques de conversación — ve los hooks disparándose junto a la conversación |
| **Exportar** | Exportación en Markdown para reanudar contexto o compartir |
| **Selección masiva y archivo** | Selecciona múltiples sesiones para archivar en lote con estado de filtro persistente |
| **Compartir cifrado** | Comparte cualquier sesión mediante un enlace cifrado de extremo a extremo — AES-256-GCM, cero confianza en el servidor, la clave solo existe en el fragmento de la URL |

---

## Internos del Agente

Claude Code hace mucho detrás de `"thinking..."` que nunca se muestra en tu terminal. claude-view expone todo.

| Característica | Qué hace |
|---------|-------------|
| **Conversaciones de sub-agentes** | Árbol completo de agentes generados, sus prompts, salidas y desglose de costo/tokens por agente |
| **Llamadas a servidores MCP** | Qué herramientas MCP se están invocando y sus resultados |
| **Seguimiento de skills / hooks / plugins** | Qué skills se dispararon, qué hooks se ejecutaron, qué plugins están activos |
| **Registro de eventos de hooks** | Captura de hooks de doble canal (WebSocket en vivo + respaldo JSONL) — cada evento registrado y navegable, incluso para sesiones pasadas |
| **Insignias de origen de sesión** | Cada sesión muestra cómo fue iniciada: Terminal, VS Code, Agent SDK u otros puntos de entrada |
| **Divergencia de rama en worktree** | Detecta cuándo las ramas de git worktree divergen — se muestra en el monitor en vivo y en el historial |
| **Chips de mención @File** | Las referencias `@filename` se extraen y muestran como chips — pasa el cursor para ver la ruta completa |
| **Línea temporal de uso de herramientas** | Registro de acciones de cada par tool_use/tool_result con tiempos |
| **Surfacing de errores** | Los errores suben a la tarjeta de sesión — sin fallos enterrados |
| **Inspector de mensajes sin procesar** | Profundiza en el JSON sin procesar de cualquier mensaje cuando necesites el panorama completo |

---

## Búsqueda

| Característica | Qué hace |
|---------|-------------|
| **Búsqueda de texto completo** | Busca en todas las sesiones — mensajes, llamadas a herramientas, rutas de archivos. Impulsado por Tantivy (nativo de Rust, clase Lucene) |
| **Motor de búsqueda unificado** | Tantivy texto completo + pre-filtro SQLite se ejecutan en paralelo — un endpoint, resultados en menos de 50ms |
| **Filtros de proyecto y rama** | Limita al proyecto o rama en el que estás trabajando ahora mismo |
| **Paleta de comandos** | <kbd>Cmd</kbd>+<kbd>K</kbd> para saltar entre sesiones, cambiar vistas, encontrar cualquier cosa |

---

## Analíticas

Un conjunto completo de analíticas para tu uso de Claude Code. Piensa en el panel de Cursor, pero más profundo.

<details>
<summary><strong>Panel de Control</strong></summary>
<br>

| Característica | Descripción |
|---------|-------------|
| **Métricas semana a semana** | Cantidad de sesiones, uso de tokens, costo — comparado con tu período anterior |
| **Mapa de calor de actividad** | Cuadrícula estilo GitHub de 90 días mostrando la intensidad de uso diario |
| **Top skills / comandos / herramientas MCP / agentes** | Clasificaciones de tus invocables más usados — haz clic en cualquiera para buscar sesiones coincidentes |
| **Proyectos más activos** | Gráfico de barras de proyectos clasificados por cantidad de sesiones |
| **Desglose de uso de herramientas** | Total de ediciones, lecturas y comandos bash en todas las sesiones |
| **Sesiones más largas** | Acceso rápido a tus sesiones maratón con duración |

</details>

<details>
<summary><strong>Contribuciones de IA</strong></summary>
<br>

| Característica | Descripción |
|---------|-------------|
| **Seguimiento de código generado** | Líneas añadidas/eliminadas, archivos tocados, cantidad de commits — en todas las sesiones |
| **Métricas de ROI de costo** | Costo por commit, costo por sesión, costo por línea de código generado — con gráficos de tendencia |
| **Comparación de modelos** | Desglose lado a lado de producción y eficiencia por modelo (Opus, Sonnet, Haiku) |
| **Curva de aprendizaje** | Tasa de re-edición a lo largo del tiempo — observa cómo mejoras en el uso de prompts |
| **Desglose por rama** | Vista colapsable por rama con detalle de sesiones |
| **Efectividad de skills** | Qué skills realmente mejoran tu producción vs cuáles no |

</details>

<details>
<summary><strong>Insights</strong> <em>(experimental)</em></summary>
<br>

| Característica | Descripción |
|---------|-------------|
| **Detección de patrones** | Patrones de comportamiento descubiertos a partir de tu historial de sesiones |
| **Benchmarks Antes vs Ahora** | Compara tu primer mes con el uso reciente |
| **Desglose por categoría** | Treemap de para qué usas Claude — refactorización, funcionalidades, depuración, etc. |
| **Puntuación de Fluidez IA** | Un solo número de 0-100 que rastrea tu efectividad general |

> Los Insights y la Puntuación de Fluidez son experimentales. Tratar como orientativos, no definitivos.

</details>

---

## Planes, Prompts y Equipos

| Característica | Qué hace |
|---------|-------------|
| **Explorador de planes** | Ve tus `.claude/plans/` directamente en el detalle de sesión — sin buscar más entre archivos |
| **Historial de prompts** | Búsqueda de texto completo en todos los prompts que has enviado con agrupación por plantilla y clasificación de intención |
| **Panel de equipos** | Ve líderes de equipo, mensajes de bandeja de entrada, tareas del equipo y cambios de archivos de todos los miembros |
| **Analíticas de prompts** | Clasificaciones de plantillas de prompts, distribución de intención y estadísticas de uso |

---

## Monitor del Sistema

| Característica | Qué hace |
|---------|-------------|
| **Indicadores de CPU / RAM / Disco en vivo** | Métricas del sistema en tiempo real transmitidas vía SSE con transiciones animadas suaves |
| **Panel de componentes** | Ve métricas del sidecar y la IA local: uso de VRAM, CPU, RAM y cantidad de sesiones por componente |
| **Lista de procesos** | Procesos agrupados por nombre, ordenados por CPU — ve qué está haciendo realmente tu máquina mientras los agentes se ejecutan |

---

## IA en el Dispositivo

Ejecuta un LLM local para clasificación de fases de sesión — sin llamadas a API, sin costo adicional.

| Característica | Qué hace |
|---------|-------------|
| **Agnóstico de proveedor** | Conéctate a cualquier endpoint compatible con OpenAI — oMLX, Ollama, LM Studio o tu propio servidor |
| **Selector de modelo** | Elige de un registro curado de modelos con requisitos de RAM mostrados |
| **Clasificación de fases** | Las sesiones se etiquetan con su fase actual (codificación, depuración, planificación, etc.) usando visualización con umbral de confianza |
| **Gestión inteligente de recursos** | Clasificación estabilizada por EMA con retroceso exponencial — 93% de reducción de desperdicio de GPU vs sondeo ingenuo |

---

## Plugin

`@claude-view/plugin` le da a Claude acceso nativo a los datos de tu panel de control — 85 herramientas MCP, 9 skills e inicio automático.

```bash
claude plugin add @claude-view/plugin
```

### Inicio automático

Cada sesión de Claude Code inicia automáticamente el panel de control. No necesitas ejecutar `npx claude-view` manualmente.

### 85 herramientas MCP

8 herramientas diseñadas a mano con salida optimizada para Claude:

| Herramienta | Descripción |
|------|-------------|
| `list_sessions` | Explora sesiones con filtros |
| `get_session` | Detalle completo de sesión con mensajes y métricas |
| `search_sessions` | Búsqueda de texto completo en todas las conversaciones |
| `get_stats` | Resumen del panel — total de sesiones, costos, tendencias |
| `get_fluency_score` | Puntuación de Fluidez IA (0-100) con desglose |
| `get_token_stats` | Uso de tokens con tasa de acierto de caché |
| `list_live_sessions` | Agentes actualmente en ejecución (tiempo real) |
| `get_live_summary` | Costo agregado y estado del día |

Más **78 herramientas auto-generadas** a partir de la especificación OpenAPI en 26 categorías (contribuciones, insights, coaching, exportaciones, workflows y más).

### 9 Skills

| Skill | Qué hace |
|-------|-------------|
| `/session-recap` | Resume una sesión específica — commits, métricas, duración |
| `/daily-cost` | Gasto del día, sesiones en ejecución, uso de tokens |
| `/standup` | Registro de trabajo multi-sesión para actualizaciones de standup |
| `/coaching` | Consejos de coaching de IA y gestión de reglas personalizadas |
| `/insights` | Análisis de patrones de comportamiento |
| `/project-overview` | Resumen de proyecto entre sesiones |
| `/search` | Búsqueda en lenguaje natural |
| `/export-data` | Exporta sesiones a CSV/JSON |
| `/team-status` | Resumen de actividad del equipo |

---

## Workflows

| Característica | Qué hace |
|---------|-------------|
| **Constructor de workflows** | Crea workflows de múltiples etapas con disposición estilo VS Code, vista previa de diagrama Mermaid y editor YAML |
| **Rail de chat LLM con streaming** | Genera definiciones de workflow en tiempo real mediante chat integrado |
| **Ejecutor de etapas** | Visualiza columnas de etapas, tarjetas de intentos y barra de progreso mientras tu workflow se ejecuta |
| **Workflows semilla incluidos** | Plan Polisher y Plan Executor vienen listos de fábrica |

---

## Abrir en IDE

| Característica | Qué hace |
|---------|-------------|
| **Apertura de archivos con un clic** | Los archivos referenciados en sesiones se abren directamente en tu editor |
| **Detecta tu editor automáticamente** | VS Code, Cursor, Zed y otros — sin necesidad de configuración |
| **Donde importa** | El botón aparece en la pestaña de Cambios, encabezados de archivos y encabezados de proyecto en Kanban |
| **Memoria de preferencia** | Tu editor preferido se recuerda entre sesiones |

---

## Cómo Está Construido

| | |
|---|---|
| **Rápido** | Backend en Rust con análisis JSONL acelerado por SIMD, I/O mapeado en memoria — indexa miles de sesiones en segundos |
| **Tiempo real** | File watcher + SSE + WebSocket multiplexado con heartbeat, repetición de eventos y recuperación ante fallos |
| **Compacto** | ~10 MB de descarga, ~27 MB en disco. Sin dependencias en tiempo de ejecución, sin daemons en segundo plano |
| **100% local** | Todos los datos permanecen en tu máquina. Cero telemetría por defecto, cero cuentas requeridas |
| **Sin configuración** | `npx claude-view` y listo. Sin claves API, sin configuración, sin cuentas |
| **Impulsado por FSM** | Las sesiones de chat se ejecutan sobre una máquina de estados finitos con fases explícitas y eventos tipados — determinista, sin condiciones de carrera |

<details>
<summary><strong>Los Números</strong></summary>
<br>

Medido en un Mac serie M con 1,493 sesiones en 26 proyectos:

| Métrica | claude-view | Panel típico con Electron |
|--------|:-----------:|:--------------------------:|
| **Descarga** | **~10 MB** | 150-300 MB |
| **En disco** | **~27 MB** | 300-500 MB |
| **Inicio** | **< 500 ms** | 3-8 s |
| **RAM (índice completo)** | **~50 MB** | 300-800 MB |
| **Indexar 1,500 sesiones** | **< 1 s** | N/A |
| **Dependencias en ejecución** | **0** | Node.js + Chromium |

Técnicas clave: pre-filtro SIMD (`memchr`), análisis JSONL mapeado en memoria, búsqueda de texto completo Tantivy, slices zero-copy desde mmap pasando por parse hasta response.

</details>

---

## Cómo Se Compara

| Herramienta | Categoría | Stack | Tamaño | Monitor en vivo | Chat multi-sesión | Búsqueda | Analíticas | Herramientas MCP |
|------|----------|-------|:----:|:------------:|:------------------:|:------:|:---------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | Monitor + workspace | Rust | **~10 MB** | **Sí** | **Sí** | **Sí** | **Sí** | **85** |
| [opcode](https://github.com/winfunc/opcode) | GUI + gestor de sesiones | Tauri 2 | ~13 MB | Parcial | No | No | Sí | No |
| [ccusage](https://github.com/ryoppippi/ccusage) | Rastreador de uso CLI | TypeScript | ~600 KB | No | No | No | CLI | No |
| [CodePilot](https://github.com/op7418/CodePilot) | UI de chat de escritorio | Electron | ~140 MB | No | No | No | No | No |
| [claude-run](https://github.com/kamranahmedse/claude-run) | Visor de historial | TypeScript | ~500 KB | Parcial | No | Básica | No | No |

> Las UIs de chat (CodePilot, CUI, claude-code-webui) son interfaces *para* Claude Code. claude-view es un panel de control que observa tus sesiones de terminal existentes. Son complementarios.

---

## Instalación

| Método | Comando |
|--------|---------|
| **Shell** (recomendado) | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |
| **Plugin** (inicio automático) | `claude plugin add @claude-view/plugin` |

El instalador de shell descarga un binario pre-compilado (~10 MB), lo instala en `~/.claude-view/bin` y lo agrega a tu PATH. Luego simplemente ejecuta `claude-view`.

**Único requisito:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) instalado.

<details>
<summary><strong>Configuración</strong></summary>
<br>

| Variable de Entorno | Por Defecto | Descripción |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` o `PORT` | `47892` | Sobrescribe el puerto por defecto |

</details>

<details>
<summary><strong>Self-Hosting y Desarrollo Local</strong></summary>
<br>

El binario pre-compilado incluye autenticación, compartir y relay móvil integrados. ¿Compilando desde el código fuente? Estas características son **opcionales mediante variables de entorno** — omite cualquiera y esa característica simplemente se desactiva.

| Variable de Entorno | Característica | Sin ella |
|-------------|---------|------------|
| `SUPABASE_URL` | Login / autenticación | Autenticación desactivada — modo completamente local, sin cuentas |
| `RELAY_URL` | Emparejamiento móvil | Emparejamiento QR no disponible |
| `SHARE_WORKER_URL` + `SHARE_VIEWER_URL` | Compartir cifrado | Botón de compartir oculto |

```bash
bun dev    # completamente local, sin dependencias en la nube
```

</details>

<details>
<summary><strong>Enterprise / Entornos Sandbox</strong></summary>
<br>

Si tu máquina restringe escrituras (DataCloak, CrowdStrike, DLP corporativo):

```bash
cp crates/server/.env.example .env
# Descomenta CLAUDE_VIEW_DATA_DIR
```

Esto mantiene la base de datos, el índice de búsqueda y los archivos de bloqueo dentro del repositorio. Establece `CLAUDE_VIEW_SKIP_HOOKS=1` para omitir el registro de hooks en entornos de solo lectura.

</details>

---

## Preguntas Frecuentes

<details>
<summary><strong>Aparece el banner "Not signed in" aunque estoy conectado</strong></summary>
<br>

claude-view verifica tus credenciales de Claude leyendo `~/.claude/.credentials.json` (con respaldo de macOS Keychain). Prueba estos pasos:

1. **Verifica la autenticación de Claude CLI:** `claude auth status`
2. **Revisa el archivo de credenciales:** `cat ~/.claude/.credentials.json` — debería tener una sección `claudeAiOauth` con un `accessToken`
3. **Revisa macOS Keychain:** `security find-generic-password -s "Claude Code-credentials" -w`
4. **Revisa la expiración del token:** Mira `expiresAt` en el JSON de credenciales — si ya pasó, ejecuta `claude auth login`
5. **Revisa HOME:** `echo $HOME` — el servidor lee de `$HOME/.claude/.credentials.json`

Si todas las verificaciones pasan y el banner persiste, repórtalo en [Discord](https://discord.gg/G7wdZTpRfu).

</details>

<details>
<summary><strong>¿A qué datos accede claude-view?</strong></summary>
<br>

claude-view lee los archivos de sesión JSONL que Claude Code escribe en `~/.claude/projects/`. Los indexa localmente usando SQLite y Tantivy. **Ningún dato sale de tu máquina** a menos que uses explícitamente la función de compartir cifrado. La telemetría es opcional y está desactivada por defecto.

</details>

<details>
<summary><strong>¿Funciona con Claude Code en VS Code / Cursor / extensiones de IDE?</strong></summary>
<br>

Sí. claude-view monitorea todas las sesiones de Claude Code sin importar cómo fueron iniciadas — terminal CLI, extensión de VS Code, Cursor o Agent SDK. Cada sesión muestra una insignia de origen (Terminal, VS Code, SDK) para que puedas filtrar por método de inicio.

</details>

---

## Comunidad

- **Sitio web:** [claudeview.ai](https://claudeview.ai) — documentación, changelog, blog
- **Discord:** [Únete al servidor](https://discord.gg/G7wdZTpRfu) — soporte, solicitudes de funcionalidades, discusión
- **Plugin:** [`@claude-view/plugin`](https://www.npmjs.com/package/@claude-view/plugin) — 85 herramientas MCP, 9 skills, inicio automático

---

<details>
<summary><strong>Desarrollo</strong></summary>
<br>

Requisitos previos: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Instala todas las dependencias del workspace
bun dev            # Inicia desarrollo full-stack (Rust + Web + Sidecar con hot reload)
```

### Estructura del Workspace

| Ruta | Paquete | Propósito |
|------|---------|---------|
| `apps/web/` | `@claude-view/web` | SPA React (Vite) — frontend web principal |
| `apps/share/` | `@claude-view/share` | SPA del visor de compartir — Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | App nativa Expo |
| `apps/landing/` | `@claude-view/landing` | Página de aterrizaje Astro 5 (cero JavaScript del lado del cliente) |
| `packages/shared/` | `@claude-view/shared` | Tipos compartidos y tokens de tema |
| `packages/design-tokens/` | `@claude-view/design-tokens` | Colores, espaciado, tipografía |
| `packages/plugin/` | `@claude-view/plugin` | Plugin de Claude Code (servidor MCP + herramientas + skills) |
| `crates/` | — | Backend en Rust (Axum) |
| `sidecar/` | — | Sidecar Node.js (puente Agent SDK) |
| `infra/share-worker/` | — | Cloudflare Worker — API de compartir (R2 + D1) |
| `infra/install-worker/` | — | Cloudflare Worker — script de instalación con seguimiento de descargas |

### Comandos de Desarrollo

| Comando | Descripción |
|---------|-------------|
| `bun dev` | Desarrollo full-stack — Rust + Web + Sidecar con hot reload |
| `bun run dev:web` | Solo frontend web |
| `bun run dev:server` | Solo backend Rust |
| `bun run build` | Compila todos los workspaces |
| `bun run preview` | Compila web + sirve mediante binario de release |
| `bun run lint:all` | Lint JS/TS + Rust (Clippy) |
| `bun run typecheck` | Verificación de tipos TypeScript |
| `bun run test` | Ejecuta todas las pruebas (Turbo) |
| `bun run test:rust` | Ejecuta pruebas de Rust |
| `bun run storybook` | Lanza Storybook para desarrollo de componentes |
| `bun run dist:test` | Compila + empaqueta + instala + ejecuta (prueba de distribución completa) |

### Publicación de Versiones

```bash
bun run release          # incremento de patch
bun run release:minor    # incremento de minor
git push origin main --tags    # activa CI → compila → auto-publica en npm
```

</details>

---

## Soporte de Plataformas

| Plataforma | Estado |
|----------|--------|
| macOS (Apple Silicon) | Disponible |
| macOS (Intel) | Disponible |
| Linux (x64) | Planeado |
| Windows (x64) | Planeado |

---

## Relacionado

- **[claudeview.ai](https://claudeview.ai)** — Sitio web oficial, documentación y changelog
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** — Plugin de Claude Code con 85 herramientas MCP y 9 skills. `claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** — Claude Code elimina tus sesiones después de 30 días. Esto las guarda. `npx claude-backup`

---

<div align="center">

Si **claude-view** te ayuda a ver lo que tus agentes de IA están haciendo, considera darle una estrella.

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT &copy; 2026

</div>
