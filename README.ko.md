# claude-view

<p align="center">
  <strong>Claude Code 파워 유저를 위한 실시간 모니터 & 코파일럿.</strong>
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

## 문제

프로젝트 3개가 열려 있습니다. 각 프로젝트에는 여러 git 워크트리가 있습니다. 각 워크트리에서는 여러 Claude Code 세션이 실행 중입니다. 어떤 것은 생각 중이고, 어떤 것은 여러분의 입력을 기다리고 있고, 어떤 것은 컨텍스트 한계에 도달하려 하고, 하나는 10분 전에 끝났는데 잊어버렸습니다.

Cmd-Tab으로 15개의 터미널 창을 오가며 어떤 세션이 무슨 작업을 하고 있었는지 기억하려 합니다. 캐시가 만료된 걸 모르고 토큰을 낭비합니다. 모든 것을 볼 수 있는 단일 장소가 없어 작업 흐름을 잃습니다. 그리고 "생각 중..." 스피너 뒤에서 Claude는 서브 에이전트를 생성하고, MCP 서버를 호출하고, 스킬을 실행하고, 훅을 발동시키고 있는데 — 그 어떤 것도 보이지 않습니다.

**Claude Code는 매우 강력합니다. 하지만 대시보드 없이 10개 이상의 동시 세션을 운영하는 것은 속도계 없이 운전하는 것과 같습니다.**

## 솔루션

**claude-view**는 Claude Code 세션과 함께 작동하는 실시간 대시보드입니다. 브라우저 탭 하나로 모든 세션이 보이고, 전체 컨텍스트를 한눈에 파악할 수 있습니다.

```bash
npx claude-view
```

그게 전부입니다. 브라우저에서 열립니다. 모든 세션 — 라이브와 과거 — 하나의 워크스페이스에서.

---

## 제공 기능

### 실시간 모니터

| 기능 | 중요한 이유 |
|---------|---------------|
| **마지막 메시지가 포함된 세션 카드** | 장시간 실행 중인 각 세션이 무엇을 하고 있는지 즉시 파악 |
| **알림 사운드** | 세션이 완료되거나 입력이 필요할 때 알림 — 터미널 폴링 중단 |
| **컨텍스트 게이지** | 세션별 실시간 컨텍스트 윈도우 사용량 — 위험 영역에 있는 세션 확인 |
| **캐시 워밍 카운트다운** | 프롬프트 캐시 만료 시점을 정확히 파악하여 토큰 절약 타이밍 조절 |
| **비용 추적** | 세션별 및 전체 지출과 캐시 절약 내역 |
| **서브 에이전트 시각화** | 전체 에이전트 트리 — 서브 에이전트, 상태, 호출 중인 도구 확인 |
| **다중 뷰** | 그리드, 리스트 또는 모니터 모드(라이브 채팅 그리드) — 워크플로우에 맞게 선택 |

### 풍부한 채팅 히스토리

| 기능 | 중요한 이유 |
|---------|---------------|
| **전체 대화 브라우저** | 모든 세션, 모든 메시지, 마크다운과 코드 블록 완전 렌더링 |
| **도구 호출 시각화** | 파일 읽기, 편집, bash 명령, MCP 호출, 스킬 실행 — 텍스트만이 아님 |
| **간략/상세 전환** | 대화를 빠르게 훑거나 모든 도구 호출을 상세 확인 |
| **스레드 뷰** | 서브 에이전트 계층구조로 에이전트 대화 추적 |
| **내보내기** | 컨텍스트 재개 또는 공유를 위한 Markdown 내보내기 |

### 고급 검색

| 기능 | 중요한 이유 |
|---------|---------------|
| **전체 텍스트 검색** | 모든 세션에 걸쳐 검색 — 메시지, 도구 호출, 파일 경로 |
| **프로젝트 & 브랜치 필터** | 현재 작업 중인 프로젝트로 범위 지정 |
| **커맨드 팔레트** | Cmd+K로 세션 간 이동, 뷰 전환, 무엇이든 검색 |

### 에이전트 내부 — 숨겨진 것을 확인

Claude Code는 "생각 중..." 뒤에서 터미널에 표시되지 않는 많은 작업을 수행합니다. claude-view는 이 모든 것을 노출합니다.

| 기능 | 중요한 이유 |
|---------|---------------|
| **서브 에이전트 대화** | 생성된 에이전트의 전체 트리, 프롬프트 및 출력 확인 |
| **MCP 서버 호출** | 어떤 MCP 도구가 호출되었고 그 결과 확인 |
| **스킬/훅/플러그인 추적** | 어떤 스킬이 발동되고, 어떤 훅이 실행되었으며, 어떤 플러그인이 활성 상태인지 파악 |
| **훅 이벤트 기록** | 모든 훅 이벤트가 캡처되고 탐색 가능 — 무엇이 언제 발동되었는지 확인. *(claude-view가 세션 활성 중에 실행되어야 함; 과거 이벤트를 소급 추적할 수 없음)* |
| **도구 사용 타임라인** | 모든 tool_use/tool_result 쌍과 타이밍의 액션 로그 |
| **에러 표출** | 에러가 세션 카드에 표시 — 더 이상 숨겨진 실패 없음 |
| **원시 메시지 인스펙터** | 전체 그림이 필요할 때 메시지의 원시 JSON 확인 |

### 분석

Claude Code 사용을 위한 풍부한 분석 스위트. Cursor의 대시보드를 떠올리되, 더 깊이 있게.

**대시보드 개요**

| 기능 | 설명 |
|---------|-------------|
| **주간 비교 지표** | 세션 수, 토큰 사용량, 비용 — 이전 기간과 비교 |
| **활동 히트맵** | 90일 GitHub 스타일 그리드로 일일 Claude Code 사용 강도 표시 |
| **상위 스킬/명령/MCP 도구/에이전트** | 가장 많이 사용한 호출 항목의 리더보드 — 클릭하여 매칭 세션 검색 |
| **가장 활발한 프로젝트** | 세션 수로 순위가 매겨진 프로젝트 막대 차트 |
| **도구 사용 분석** | 모든 세션에 걸친 총 편집, 읽기, bash 명령 수 |
| **최장 세션** | 지속 시간이 포함된 마라톤 세션 빠른 접근 |

**AI 기여**

| 기능 | 설명 |
|---------|-------------|
| **코드 출력 추적** | 추가/삭제된 라인, 수정된 파일, 전체 세션의 커밋 수 |
| **비용 ROI 지표** | 커밋당 비용, 세션당 비용, AI 출력 라인당 비용 — 트렌드 차트 포함 |
| **모델 비교** | 모델별(Opus, Sonnet, Haiku) 출력과 효율성의 나란히 비교 |
| **학습 곡선** | 시간에 따른 재편집 비율 — 프롬프팅 능력 향상 확인 |
| **브랜치 분석** | 세션 드릴다운이 포함된 접을 수 있는 브랜치별 뷰 |
| **스킬 효과** | 어떤 스킬이 실제로 출력을 개선하는지, 어떤 것이 그렇지 않은지 |

**인사이트** *(실험적)*

| 기능 | 설명 |
|---------|-------------|
| **패턴 감지** | 세션 히스토리에서 발견된 행동 패턴 |
| **과거 vs 현재 벤치마크** | 첫 달과 최근 사용 비교 |
| **카테고리 분석** | Claude 사용 용도의 트리맵 — 리팩토링, 기능, 디버깅 등 |
| **AI 유창성 점수** | 전반적인 효과성을 추적하는 0-100 단일 점수 |

> **참고:** 인사이트와 유창성 점수는 초기 실험 단계입니다. 방향성 지표로 참고하세요.

---

## 작업 흐름을 위해 설계

claude-view는 다음과 같은 개발자를 위해 설계되었습니다:

- **3개 이상의 프로젝트**를 동시에 실행하며, 각각 여러 워크트리 보유
- 항상 **10-20개의 Claude Code 세션**을 열어둠
- 무엇이 실행 중인지 놓치지 않으면서 빠른 컨텍스트 전환 필요
- 캐시 윈도우에 맞춰 메시지 타이밍을 조절하여 **토큰 지출 최적화** 원함
- 에이전트 확인을 위해 터미널을 Cmd-Tab하는 것에 좌절

브라우저 탭 하나. 모든 세션. 작업 흐름 유지.

---

## 기술 구성

| | |
|---|---|
| **초고속** | SIMD 가속 JSONL 파싱, 메모리 매핑 I/O의 Rust 백엔드 — 수천 세션을 수초 내에 인덱싱 |
| **실시간** | 파일 워처 + SSE + WebSocket으로 모든 세션의 서브초 실시간 업데이트 |
| **작은 풋프린트** | 단일 ~15 MB 바이너리. 런타임 의존성 없음, 백그라운드 데몬 없음 |
| **100% 로컬** | 모든 데이터는 여러분의 머신에. 텔레메트리 제로, 클라우드 제로, 네트워크 요청 제로 |
| **제로 설정** | `npx claude-view`로 끝. API 키 불필요, 설정 불필요, 계정 불필요 |

---

## 빠른 시작

```bash
npx claude-view
```

`http://localhost:47892`에서 열립니다.

### 구성

| 환경 변수 | 기본값 | 설명 |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` 또는 `PORT` | `47892` | 기본 포트 변경 |

---

## 설치

| 방법 | 명령 |
|--------|---------|
| **npx** (권장) | `npx claude-view` |
| **셸 스크립트** (Node 불필요) | `curl -sL https://raw.githubusercontent.com/tombelieber/claude-view/main/start.sh \| bash` |
| **Git clone** | `git clone https://github.com/tombelieber/claude-view.git && cd claude-view && ./start.sh` |

### 요구 사항

- **Claude Code** 설치 완료 ([여기에서 다운로드](https://docs.anthropic.com/en/docs/claude-code)) — 모니터링할 세션 파일이 생성됩니다

---

## 비교

다른 도구들은 뷰어(히스토리 탐색)이거나 단순한 모니터입니다. 실시간 모니터링, 풍부한 채팅 히스토리, 디버깅 도구, 고급 검색을 하나의 워크스페이스에 결합한 도구는 없습니다.

```
                    수동 ←————————————→ 능동
                         |                  |
            보기만      |  ccusage         |
                         |  History Viewer  |
                         |  clog            |
                         |                  |
            모니터만    |  claude-code-ui  |
                         |  Agent Sessions  |
                         |                  |
            완전한      |  ★ claude-view   |
            워크스페이스 |                  |
```

---

## 커뮤니티

지원, 기능 요청 및 토론을 위해 [Discord 서버](https://discord.gg/G7wdZTpRfu)에 참여하세요.

---

## 이 프로젝트가 마음에 드시나요?

**claude-view**가 Claude Code 활용에 도움이 되었다면, 스타를 눌러주세요. 다른 분들이 이 도구를 발견하는 데 도움이 됩니다.

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## 개발

사전 요구 사항: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # 프론트엔드 의존성 설치
bun dev            # 풀스택 개발 시작 (Rust + Vite 핫 리로드)
```

| 명령 | 설명 |
|---------|-------------|
| `bun dev` | 풀스택 개발 — 변경 시 Rust 자동 재시작, Vite HMR |
| `bun dev:server` | Rust 백엔드만 (cargo-watch 포함) |
| `bun dev:client` | Vite 프론트엔드만 (백엔드 실행 가정) |
| `bun run build` | 프로덕션용 프론트엔드 빌드 |
| `bun run preview` | 빌드 + 릴리스 바이너리로 서빙 |
| `bun run lint` | 프론트엔드(ESLint)와 백엔드(Clippy) 린트 |
| `bun run fmt` | Rust 코드 포맷 |
| `bun run check` | 타입 체크 + 린트 + 테스트 (커밋 전 게이트) |
| `bun test` | Rust 테스트 스위트 실행 (`cargo test --workspace`) |
| `bun test:client` | 프론트엔드 테스트 실행 (vitest) |
| `bun run test:e2e` | Playwright 엔드투엔드 테스트 실행 |

### 프로덕션 배포 테스트

```bash
bun run dist:test    # 한 명령: 빌드 → 패킹 → 설치 → 실행
```

또는 단계별로:

| 명령 | 설명 |
|---------|-------------|
| `bun run dist:pack` | 바이너리 + 프론트엔드를 `/tmp/`에 tarball로 패키지 |
| `bun run dist:install` | tarball을 `~/.cache/claude-view/`에 추출 (첫 실행 다운로드 시뮬레이션) |
| `bun run dist:run` | 캐시된 바이너리로 npx 래퍼 실행 |
| `bun run dist:test` | 위 전체를 한 명령으로 |
| `bun run dist:clean` | 모든 dist 캐시 및 임시 파일 제거 |

### 릴리스

```bash
bun run release          # 패치 범프: 0.1.0 → 0.1.1
bun run release:minor    # 마이너 범프: 0.1.0 → 0.2.0
bun run release:major    # 메이저 범프: 0.1.0 → 1.0.0
```

`npx-cli/package.json`의 버전을 범프하고, 커밋하고, git 태그를 생성합니다. 그 다음:

```bash
git push origin main --tags    # CI 트리거 → 전 플랫폼 빌드 → npm에 자동 퍼블리시
```

---

## 플랫폼 지원

| 플랫폼 | 상태 |
|----------|--------|
| macOS (Apple Silicon) | 사용 가능 |
| macOS (Intel) | 사용 가능 |
| Linux (x64) | 예정 |
| Windows (x64) | 예정 |

---

## 라이선스

MIT © 2026
