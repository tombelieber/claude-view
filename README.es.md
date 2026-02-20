# claude-view

<p align="center">
  <strong>Monitor en vivo y copiloto para usuarios avanzados de Claude Code.</strong>
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

## El Problema

Tienes 3 proyectos abiertos. Cada proyecto tiene múltiples worktrees de git. Cada worktree tiene múltiples sesiones de Claude Code ejecutándose. Algunas están pensando, otras esperan tu input, algunas están a punto de alcanzar los límites de contexto, y una terminó hace 10 minutos pero te olvidaste.

Haces Cmd-Tab entre 15 ventanas de terminal tratando de recordar qué sesión estaba haciendo qué. Quemas tokens porque un caché expiró mientras no mirabas. Pierdes el flujo porque no hay un solo lugar para ver todo. Y detrás de ese spinner "pensando...", Claude está generando sub-agentes, llamando servidores MCP, ejecutando skills, disparando hooks — y no puedes ver nada de eso.

**Claude Code es increíblemente potente. Pero manejar 10+ sesiones concurrentes sin un panel es como conducir sin velocímetro.**

## La Solución

**claude-view** es un panel en tiempo real que funciona junto a tus sesiones de Claude Code. Una pestaña de navegador, cada sesión visible, contexto completo de un vistazo.

```bash
npx claude-view
```

Eso es todo. Se abre en tu navegador. Todas tus sesiones — en vivo y pasadas — en un solo espacio de trabajo.

---

## Lo Que Obtienes

### Monitor en Vivo

| Característica | Por qué importa |
|---------|---------------|
| **Tarjetas de sesión con último mensaje** | Recuerda al instante en qué está trabajando cada sesión de larga duración |
| **Sonidos de notificación** | Recibe un aviso cuando una sesión termina o necesita tu input — deja de sondear terminales |
| **Medidor de contexto** | Uso de ventana de contexto en tiempo real por sesión — ve cuáles están en zona de peligro |
| **Cuenta regresiva de caché** | Sabe exactamente cuándo expira el caché de prompts para programar tu siguiente mensaje y ahorrar tokens |
| **Seguimiento de costos** | Gasto por sesión y agregado con desglose de ahorro por caché |
| **Visualización de sub-agentes** | Ve el árbol completo de agentes — sub-agentes, su estado y qué herramientas están llamando |
| **Múltiples vistas** | Grid, Lista o modo Monitor (grid de chat en vivo) — elige lo que se adapte a tu flujo |

### Historial de Chat Enriquecido

| Característica | Por qué importa |
|---------|---------------|
| **Navegador de conversación completo** | Cada sesión, cada mensaje, completamente renderizado con markdown y bloques de código |
| **Visualización de llamadas a herramientas** | Ve lecturas de archivos, ediciones, comandos bash, llamadas MCP, invocaciones de skills — no solo texto |
| **Toggle compacto / detallado** | Revisa la conversación rápidamente o profundiza en cada llamada a herramienta |
| **Vista de hilos** | Sigue conversaciones de agentes con jerarquías de sub-agentes |
| **Exportar** | Exportación Markdown para retomar contexto o compartir |

### Búsqueda Avanzada

| Característica | Por qué importa |
|---------|---------------|
| **Búsqueda de texto completo** | Busca a través de todas las sesiones — mensajes, llamadas a herramientas, rutas de archivos |
| **Filtros de proyecto y rama** | Limita el alcance al proyecto en el que estás trabajando ahora |
| **Paleta de comandos** | Cmd+K para saltar entre sesiones, cambiar vistas, encontrar cualquier cosa |

### Internos del Agente — Ve Lo Oculto

Claude Code hace mucho detrás de "pensando..." que nunca se muestra en tu terminal. claude-view expone todo.

| Característica | Por qué importa |
|---------|---------------|
| **Conversaciones de sub-agentes** | Ve el árbol completo de agentes generados, sus prompts y sus resultados |
| **Llamadas a servidores MCP** | Ve qué herramientas MCP se están invocando y sus resultados |
| **Seguimiento de skills / hooks / plugins** | Sabe qué skills se dispararon, qué hooks se ejecutaron, qué plugins están activos |
| **Registro de eventos de hooks** | Cada evento de hook es capturado y navegable — revisa qué se disparó y cuándo. *(Requiere que claude-view esté ejecutándose mientras las sesiones están activas; no puede rastrear eventos históricos retroactivamente)* |
| **Línea de tiempo de uso de herramientas** | Log de acciones de cada par tool_use/tool_result con temporización |
| **Surfacing de errores** | Los errores aparecen en la tarjeta de sesión — no más fallos enterrados |
| **Inspector de mensajes raw** | Profundiza en el JSON raw de cualquier mensaje cuando necesites la imagen completa |

### Analíticas

Una suite completa de analíticas para tu uso de Claude Code. Piensa en el panel de Cursor, pero más profundo.

**Resumen del Panel**

| Característica | Descripción |
|---------|-------------|
| **Métricas semana a semana** | Conteo de sesiones, uso de tokens, costo — comparado con tu período anterior |
| **Mapa de calor de actividad** | Grid estilo GitHub de 90 días mostrando tu intensidad diaria de uso de Claude Code |
| **Top skills / comandos / herramientas MCP / agentes** | Rankings de tus invocables más usados — haz clic en cualquiera para buscar sesiones coincidentes |
| **Proyectos más activos** | Gráfico de barras de proyectos ordenados por conteo de sesiones |
| **Desglose de uso de herramientas** | Total de ediciones, lecturas y comandos bash a través de todas las sesiones |
| **Sesiones más largas** | Acceso rápido a tus sesiones maratónicas con duración |

**Contribuciones de IA**

| Característica | Descripción |
|---------|-------------|
| **Seguimiento de output de código** | Líneas añadidas/eliminadas, archivos tocados, conteo de commits — a través de todas las sesiones |
| **Métricas de ROI de costo** | Costo por commit, costo por sesión, costo por línea de output de IA — con gráficos de tendencia |
| **Comparación de modelos** | Desglose lado a lado de output y eficiencia por modelo (Opus, Sonnet, Haiku) |
| **Curva de aprendizaje** | Tasa de re-edición a lo largo del tiempo — ve cómo mejoras en prompting |
| **Desglose por rama** | Vista colapsable por rama con drill-down de sesiones |
| **Efectividad de skills** | Qué skills realmente mejoran tu output vs cuáles no |

**Insights** *(experimental)*

| Característica | Descripción |
|---------|-------------|
| **Detección de patrones** | Patrones de comportamiento descubiertos de tu historial de sesiones |
| **Benchmarks entonces vs ahora** | Compara tu primer mes con tu uso reciente |
| **Desglose por categoría** | Treemap de para qué usas Claude — refactorización, features, debugging, etc. |
| **Puntuación de Fluidez IA** | Un solo número 0-100 que rastrea tu efectividad general |

> **Nota:** Insights y Puntuación de Fluidez están en etapa experimental temprana. Tómalos como direccionales, no definitivos.

---

## Diseñado Para el Flujo

claude-view está diseñado para el desarrollador que:

- Ejecuta **3+ proyectos simultáneamente**, cada uno con múltiples worktrees
- Tiene **10-20 sesiones de Claude Code** abiertas en cualquier momento
- Necesita cambiar de contexto rápido sin perder el rastro de lo que está corriendo
- Quiere **optimizar el gasto de tokens** programando mensajes alrededor de las ventanas de caché
- Se frustra con Cmd-Tab entre terminales para revisar agentes

Una pestaña de navegador. Todas las sesiones. Mantente en el flujo.

---

## Cómo Está Construido

| | |
|---|---|
| **Ultra rápido** | Backend Rust con parsing JSONL acelerado por SIMD, I/O mapeado en memoria — indexa miles de sesiones en segundos |
| **Tiempo real** | File watcher + SSE + WebSocket para actualizaciones en vivo sub-segundo en todas las sesiones |
| **Huella mínima** | Un solo binario de ~15 MB. Sin dependencias de runtime, sin daemons en segundo plano |
| **100% local** | Todos los datos permanecen en tu máquina. Cero telemetría, cero nube, cero peticiones de red |
| **Cero configuración** | `npx claude-view` y listo. Sin API keys, sin setup, sin cuentas |

---

## Inicio Rápido

```bash
npx claude-view
```

Se abre en `http://localhost:47892`.

### Configuración

| Variable de Entorno | Predeterminado | Descripción |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` o `PORT` | `47892` | Sobrescribir el puerto predeterminado |

---

## Instalación

| Método | Comando |
|--------|---------|
| **npx** (recomendado) | `npx claude-view` |
| **Script shell** (sin Node requerido) | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

### Requisitos

- **Claude Code** instalado ([consíguelo aquí](https://docs.anthropic.com/en/docs/claude-code)) — esto crea los archivos de sesión que monitoreamos

---

## Comparativa

Otras herramientas son o visores (navegar historial) o monitores simples. Ninguna combina monitoreo en tiempo real, historial de chat enriquecido, herramientas de debugging y búsqueda avanzada en un solo espacio de trabajo.

```
                    Pasivo ←————————————→ Activo
                         |                  |
            Solo ver     |  ccusage         |
                         |  History Viewer  |
                         |  clog            |
                         |                  |
            Solo         |  claude-code-ui  |
            monitor      |  Agent Sessions  |
                         |                  |
            Espacio de   |  ★ claude-view   |
            trabajo      |                  |
            completo     |                  |
```

---

## Comunidad

Únete al [servidor de Discord](https://discord.gg/G7wdZTpRfu) para soporte, solicitudes de funciones y discusión.

---

## ¿Te gusta este proyecto?

Si **claude-view** te ayuda a manejar Claude Code, considera darle una estrella. Ayuda a otros a descubrir esta herramienta.

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## Desarrollo

Prerrequisitos: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # Instalar dependencias del frontend
bun dev            # Iniciar desarrollo full-stack (Rust + Vite con hot reload)
```

| Comando | Descripción |
|---------|-------------|
| `bun dev` | Desarrollo full-stack — Rust se reinicia automáticamente al cambiar, Vite HMR |
| `bun dev:server` | Solo backend Rust (con cargo-watch) |
| `bun dev:client` | Solo frontend Vite (asume backend ejecutándose) |
| `bun run build` | Construir frontend para producción |
| `bun run preview` | Construir + servir via binario de release |
| `bun run lint` | Lint de frontend (ESLint) y backend (Clippy) |
| `bun run fmt` | Formatear código Rust |
| `bun run check` | Typecheck + lint + test (gate de pre-commit) |
| `bun test` | Ejecutar suite de tests Rust (`cargo test --workspace`) |
| `bun test:client` | Ejecutar tests de frontend (vitest) |
| `bun run test:e2e` | Ejecutar tests end-to-end de Playwright |

### Testing de Distribución en Producción

```bash
bun run dist:test    # Un comando: build → pack → install → run
```

O paso a paso:

| Comando | Descripción |
|---------|-------------|
| `bun run dist:pack` | Empaquetar binario + frontend en tarball en `/tmp/` |
| `bun run dist:install` | Extraer tarball a `~/.cache/claude-view/` (simula descarga de primera ejecución) |
| `bun run dist:run` | Ejecutar el wrapper npx usando el binario en caché |
| `bun run dist:test` | Todo lo anterior en un solo paso |
| `bun run dist:clean` | Eliminar todos los archivos de caché dist y temporales |

### Lanzamiento

```bash
bun run release          # bump de patch: 0.1.0 → 0.1.1
bun run release:minor    # bump minor: 0.1.0 → 0.2.0
bun run release:major    # bump major: 0.1.0 → 1.0.0
```

Esto incrementa la versión en `npx-cli/package.json`, hace commit y crea un tag de git. Luego:

```bash
git push origin main --tags    # dispara CI → construye todas las plataformas → auto-publica en npm
```

---

## Soporte de Plataformas

| Plataforma | Estado |
|----------|--------|
| macOS (Apple Silicon) | Disponible |
| macOS (Intel) | Disponible |
| Linux (x64) | Planificado |
| Windows (x64) | Planificado |

---

## Licencia

MIT © 2026
