<div align="center">

# claude-view

**Claude Code를 위한 미션 컨트롤**

AI 에이전트 10개가 실행 중입니다. 하나는 12분 전에 완료됐고, 다른 하나는 컨텍스트 제한에 도달했으며, 세 번째는 도구 승인을 기다리고 있습니다. <kbd>Cmd</kbd>+<kbd>Tab</kbd>으로 터미널을 전환하며 매달 $200를 눈먼 채 쓰고 있습니다.

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

**명령어 하나. 모든 세션 확인. 실시간.**

</div>

---

## claude-view란?

claude-view는 머신에서 실행되는 모든 Claude Code 세션을 모니터링하는 오픈소스 대시보드입니다 — 라이브 에이전트, 과거 대화, 비용, 서브에이전트, 훅, 도구 호출을 한곳에서 확인하세요. Rust 백엔드, React 프론트엔드, ~10 MB 바이너리. 설정 불필요, 계정 불필요, 100% 로컬.

**30번의 릴리스. 86개의 MCP 도구. 9개의 스킬. `npx claude-view` 하나면 끝.**

---

## 라이브 모니터

실행 중인 모든 세션을 한눈에 확인하세요. 더 이상 터미널 탭을 전환할 필요가 없습니다.

| 기능 | 설명 |
|---------|-------------|
| **세션 카드** | 각 카드에 마지막 메시지, 모델, 비용, 상태가 표시됩니다 — 모든 에이전트가 무슨 작업을 하는지 즉시 파악 |
| **멀티 세션 채팅** | VS Code 스타일 탭(dockview)으로 세션을 나란히 열 수 있습니다. 드래그로 가로 또는 세로 분할 |
| **컨텍스트 게이지** | 세션별 실시간 컨텍스트 윈도우 사용량 — 제한에 도달하기 전에 위험 구간의 에이전트를 확인 |
| **캐시 카운트다운** | 프롬프트 캐시 만료 시점을 정확히 확인하여 토큰을 절약할 수 있는 타이밍에 메시지 전송 |
| **비용 추적** | 세션별 및 전체 지출을 토큰 분석과 함께 확인 — 마우스를 올리면 모델별 입력/출력/캐시 분류 표시 |
| **서브에이전트 트리** | 생성된 에이전트의 전체 트리, 상태, 비용, 호출 중인 도구 확인 |
| **알림 사운드** | 세션 완료, 오류, 입력 필요 시 알림음 수신 — 더 이상 터미널을 확인할 필요 없음 |
| **다중 뷰** | 그리드, 리스트, 칸반, 모니터 모드 — 워크플로에 맞는 뷰 선택 |
| **칸반 스윔레인** | 프로젝트 또는 브랜치별 세션 그룹화 — 멀티 프로젝트 워크플로를 위한 시각적 스윔레인 레이아웃 |
| **최근 종료** | 종료된 세션은 사라지지 않고 "최근 종료"에 표시 — 서버 재시작 후에도 유지 |
| **대기 메시지** | 큐에서 대기 중인 메시지가 "Queued" 배지와 함께 보류 버블로 표시 |
| **SSE 기반** | 모든 라이브 데이터가 Server-Sent Events를 통해 푸시 — 오래된 캐시 문제 완전 제거 |

---

## 채팅 및 대화

라이브 또는 과거 세션을 읽고, 검색하고, 상호작용하세요.

| 기능 | 설명 |
|---------|-------------|
| **통합 라이브 채팅** | 히스토리와 실시간 메시지가 하나의 스크롤 가능한 대화에 통합 — 탭 전환 불필요 |
| **개발자 모드** | 세션별로 채팅과 개발자 뷰를 전환합니다. 개발자 모드에서는 도구 카드, 이벤트 카드, 훅 메타데이터, 필터 칩이 포함된 전체 실행 추적을 확인 |
| **전체 대화 브라우저** | 모든 세션, 모든 메시지를 마크다운과 코드 블록으로 완전히 렌더링하여 표시 |
| **도구 호출 시각화** | 파일 읽기, 편집, bash 명령, MCP 호출, 스킬 실행을 확인 — 텍스트만이 아닌 모든 것 |
| **간결/상세 토글** | 대화를 간략히 훑거나 모든 도구 호출을 상세히 확인 |
| **스레드 뷰** | 서브에이전트 계층 구조와 들여쓰기된 스레딩으로 에이전트 대화를 추적 |
| **인라인 훅 이벤트** | 대화 전/후 도구 훅이 대화 블록으로 렌더링 — 대화와 함께 훅 실행을 확인 |
| **내보내기** | 컨텍스트 재개 또는 공유를 위한 마크다운 내보내기 |
| **일괄 선택 및 아카이브** | 여러 세션을 선택하여 일괄 아카이브, 필터 상태 유지 |
| **암호화 공유** | E2E 암호화 링크로 세션 공유 — AES-256-GCM, 서버 신뢰 불필요, 키는 URL 프래그먼트에만 존재 |

---

## 에이전트 내부

Claude Code는 터미널에서 보이지 않는 `"thinking..."` 뒤에서 많은 작업을 수행합니다. claude-view는 그 모든 것을 보여줍니다.

| 기능 | 설명 |
|---------|-------------|
| **서브에이전트 대화** | 생성된 에이전트의 전체 트리, 프롬프트, 출력, 에이전트별 비용/토큰 분석 |
| **MCP 서버 호출** | 어떤 MCP 도구가 호출되고 있는지와 그 결과 확인 |
| **스킬 / 훅 / 플러그인 추적** | 어떤 스킬이 실행되었는지, 어떤 훅이 작동했는지, 어떤 플러그인이 활성 상태인지 확인 |
| **훅 이벤트 기록** | 듀얼 채널 훅 캡처 (라이브 WebSocket + JSONL 백필) — 과거 세션에서도 모든 이벤트를 기록하고 탐색 가능 |
| **세션 소스 배지** | 각 세션의 시작 방법 표시: Terminal, VS Code, Agent SDK 또는 기타 진입점 |
| **워크트리 브랜치 분기** | git 워크트리 브랜치 분기를 감지 — 라이브 모니터와 히스토리에 표시 |
| **@File 멘션 칩** | `@filename` 참조를 추출하여 칩으로 표시 — 마우스를 올리면 전체 경로 확인 |
| **도구 사용 타임라인** | 모든 tool_use/tool_result 쌍의 타이밍을 포함한 액션 로그 |
| **오류 노출** | 오류가 세션 카드에 표시 — 숨겨진 실패 없음 |
| **원시 메시지 인스펙터** | 전체 그림이 필요할 때 메시지의 원시 JSON을 상세히 확인 |

---

## 검색

| 기능 | 설명 |
|---------|-------------|
| **전문 검색** | 모든 세션에서 검색 — 메시지, 도구 호출, 파일 경로. Tantivy (Rust 네이티브, Lucene급) 기반 |
| **통합 검색 엔진** | Tantivy 전문 검색 + SQLite 사전 필터를 병렬 실행 — 단일 엔드포인트, 50ms 이하 응답 |
| **프로젝트 및 브랜치 필터** | 현재 작업 중인 프로젝트나 브랜치로 범위 지정 |
| **커맨드 팔레트** | <kbd>Cmd</kbd>+<kbd>K</kbd>로 세션 간 이동, 뷰 전환, 무엇이든 검색 |

---

## 분석

Claude Code 사용에 대한 종합 분석 도구입니다. Cursor의 대시보드와 비슷하지만 더 깊이 있습니다.

<details>
<summary><strong>대시보드</strong></summary>
<br>

| 기능 | 설명 |
|---------|-------------|
| **주간 비교 지표** | 세션 수, 토큰 사용량, 비용 — 이전 기간과 비교 |
| **활동 히트맵** | 일일 사용 강도를 보여주는 90일 GitHub 스타일 그리드 |
| **상위 스킬 / 명령 / MCP 도구 / 에이전트** | 가장 많이 사용한 호출의 리더보드 — 클릭하면 해당 세션 검색 |
| **가장 활발한 프로젝트** | 세션 수 기준 프로젝트 순위 막대 차트 |
| **도구 사용 분석** | 모든 세션의 총 편집, 읽기, bash 명령 |
| **가장 긴 세션** | 소요 시간과 함께 마라톤 세션에 빠르게 접근 |

</details>

<details>
<summary><strong>AI 기여</strong></summary>
<br>

| 기능 | 설명 |
|---------|-------------|
| **코드 출력 추적** | 추가/삭제된 줄, 접근한 파일, 커밋 수 — 모든 세션에 걸쳐 |
| **비용 ROI 지표** | 커밋당 비용, 세션당 비용, AI 출력 줄당 비용 — 추세 차트 포함 |
| **모델 비교** | 모델별(Opus, Sonnet, Haiku) 출력 및 효율성을 나란히 분석 |
| **학습 곡선** | 시간에 따른 재편집 비율 — 프롬프팅 실력이 향상되는 것을 확인 |
| **브랜치 분석** | 세션 드릴다운이 가능한 접기/펼치기 브랜치별 뷰 |
| **스킬 효과** | 어떤 스킬이 실제로 출력을 개선하는지, 어떤 것이 그렇지 않은지 |

</details>

<details>
<summary><strong>인사이트</strong> <em>(실험적)</em></summary>
<br>

| 기능 | 설명 |
|---------|-------------|
| **패턴 감지** | 세션 히스토리에서 발견된 행동 패턴 |
| **과거 vs 현재 벤치마크** | 첫 달과 최근 사용량 비교 |
| **카테고리 분석** | Claude 사용 용도의 트리맵 — 리팩토링, 기능 개발, 디버깅 등 |
| **AI 유창성 점수** | 전반적인 효율성을 추적하는 0-100 단일 점수 |

> 인사이트와 유창성 점수는 실험적 기능입니다. 방향성 참고용으로만 활용하세요.

</details>

---

## 플랜, 프롬프트 및 팀

| 기능 | 설명 |
|---------|-------------|
| **플랜 브라우저** | `.claude/plans/`을 세션 상세에서 직접 확인 — 더 이상 파일을 찾아 헤맬 필요 없음 |
| **프롬프트 히스토리** | 전송한 모든 프롬프트에 대한 전문 검색, 템플릿 클러스터링, 의도 분류 포함 |
| **팀 대시보드** | 팀 리더, 수신함 메시지, 팀 작업, 모든 팀원의 파일 변경 사항 확인 |
| **프롬프트 분석** | 프롬프트 템플릿 리더보드, 의도 분포, 사용 통계 |

---

## 시스템 모니터

| 기능 | 설명 |
|---------|-------------|
| **실시간 CPU / RAM / 디스크 게이지** | SSE를 통한 실시간 시스템 메트릭 스트리밍, 부드러운 애니메이션 전환 |
| **컴포넌트 대시보드** | 사이드카 및 온디바이스 AI 메트릭 확인: VRAM 사용량, CPU, RAM, 컴포넌트별 세션 수 |
| **프로세스 목록** | 이름별로 그룹화되고 CPU 순으로 정렬된 프로세스 — 에이전트 실행 중 머신이 실제로 무엇을 하고 있는지 확인 |

---

## 온디바이스 AI

세션 페이즈 분류를 위한 로컬 LLM 실행 — API 호출 없음, 추가 비용 없음.

| 기능 | 설명 |
|---------|-------------|
| **프로바이더 무관** | OpenAI 호환 엔드포인트에 연결 — oMLX, Ollama, LM Studio 또는 자체 서버 |
| **모델 선택기** | RAM 요구사항이 표시된 엄선된 모델 레지스트리에서 선택 |
| **페이즈 분류** | 신뢰도 기반 표시로 세션에 현재 페이즈 태그 지정 (코딩, 디버깅, 계획 등) |
| **스마트 리소스 관리** | EMA 안정화 분류와 지수 백오프 — 나이브 폴링 대비 93% GPU 낭비 감소 |

---

## 플러그인

`@claude-view/plugin`은 Claude에게 대시보드 데이터에 대한 네이티브 접근 권한을 제공합니다 — 86개의 MCP 도구, 9개의 스킬, 자동 시작.

```bash
claude plugin add @claude-view/plugin
```

### 자동 시작

모든 Claude Code 세션이 자동으로 대시보드를 시작합니다. 수동으로 `npx claude-view`를 실행할 필요가 없습니다.

### 86개 MCP 도구

Claude에 최적화된 출력을 가진 8개의 수제 도구:

| 도구 | 설명 |
|------|-------------|
| `list_sessions` | 필터를 사용한 세션 탐색 |
| `get_session` | 메시지와 메트릭을 포함한 전체 세션 상세 |
| `search_sessions` | 모든 대화에 대한 전문 검색 |
| `get_stats` | 대시보드 개요 — 총 세션, 비용, 추세 |
| `get_fluency_score` | AI 유창성 점수 (0-100) 분석 포함 |
| `get_token_stats` | 캐시 적중률을 포함한 토큰 사용량 |
| `list_live_sessions` | 현재 실행 중인 에이전트 (실시간) |
| `get_live_summary` | 오늘의 종합 비용 및 상태 |

추가로 27개 카테고리(기여, 인사이트, 코칭, 내보내기, 워크플로 등)에 걸친 OpenAPI 스펙에서 **자동 생성된 78개 도구**.

### 9개 스킬

| 스킬 | 설명 |
|-------|-------------|
| `/session-recap` | 특정 세션 요약 — 커밋, 메트릭, 소요 시간 |
| `/daily-cost` | 오늘의 지출, 실행 중인 세션, 토큰 사용량 |
| `/standup` | 스탠드업 업데이트를 위한 멀티 세션 작업 로그 |
| `/coaching` | AI 코칭 팁 및 커스텀 규칙 관리 |
| `/insights` | 행동 패턴 분석 |
| `/project-overview` | 세션 전반에 걸친 프로젝트 요약 |
| `/search` | 자연어 검색 |
| `/export-data` | CSV/JSON으로 세션 내보내기 |
| `/team-status` | 팀 활동 개요 |

---

## 워크플로

| 기능 | 설명 |
|---------|-------------|
| **워크플로 빌더** | VS Code 스타일 레이아웃, Mermaid 다이어그램 미리보기, YAML 에디터를 갖춘 다단계 워크플로 생성 |
| **스트리밍 LLM 채팅 레일** | 내장 채팅을 통해 워크플로 정의를 실시간으로 생성 |
| **스테이지 러너** | 워크플로 실행 시 스테이지 컬럼, 시도 카드, 진행 바를 시각화 |
| **내장 시드 워크플로** | Plan Polisher와 Plan Executor가 기본 제공 |

---

## IDE에서 열기

| 기능 | 설명 |
|---------|-------------|
| **원클릭 파일 열기** | 세션에서 참조된 파일이 에디터에서 직접 열림 |
| **에디터 자동 감지** | VS Code, Cursor, Zed 등 — 설정 불필요 |
| **필요한 모든 곳에** | 변경 사항 탭, 파일 헤더, 칸반 프로젝트 헤더에 버튼 표시 |
| **환경 설정 기억** | 선호하는 에디터가 세션 간에 기억됨 |

---

## 구현 방식

| | |
|---|---|
| **빠름** | SIMD 가속 JSONL 파싱, 메모리 매핑 I/O를 갖춘 Rust 백엔드 — 수천 개의 세션을 수 초 만에 인덱싱 |
| **실시간** | 파일 워처 + SSE + 하트비트, 이벤트 리플레이, 크래시 복구를 갖춘 멀티플렉스 WebSocket |
| **초소형** | ~10 MB 다운로드, ~27 MB 디스크 사용. 런타임 의존성 없음, 백그라운드 데몬 없음 |
| **100% 로컬** | 모든 데이터가 내 머신에 보관. 기본적으로 텔레메트리 없음, 필수 계정 없음 |
| **설정 불필요** | `npx claude-view` 하면 끝. API 키 없음, 설정 없음, 계정 없음 |
| **FSM 기반** | 채팅 세션이 명시적 페이즈와 타입 이벤트를 가진 유한 상태 머신으로 실행 — 결정적, 레이스 프리 |

<details>
<summary><strong>수치로 보기</strong></summary>
<br>

M 시리즈 Mac에서 26개 프로젝트의 1,493개 세션으로 측정:

| 지표 | claude-view | 일반적인 Electron 대시보드 |
|--------|:-----------:|:--------------------------:|
| **다운로드** | **~10 MB** | 150-300 MB |
| **디스크 사용** | **~27 MB** | 300-500 MB |
| **시작 시간** | **< 500 ms** | 3-8 s |
| **RAM (전체 인덱스)** | **~50 MB** | 300-800 MB |
| **1,500 세션 인덱싱** | **< 1 s** | N/A |
| **런타임 의존성** | **0** | Node.js + Chromium |

핵심 기술: SIMD 사전 필터 (`memchr`), 메모리 매핑 JSONL 파싱, Tantivy 전문 검색, mmap에서 파싱을 거쳐 응답까지 제로 카피 슬라이스.

</details>

---

## 비교

| 도구 | 카테고리 | 스택 | 크기 | 라이브 모니터 | 멀티 세션 채팅 | 검색 | 분석 | MCP 도구 |
|------|----------|-------|:----:|:------------:|:------------------:|:------:|:---------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | 모니터 + 워크스페이스 | Rust | **~10 MB** | **지원** | **지원** | **지원** | **지원** | **86** |
| [opcode](https://github.com/winfunc/opcode) | GUI + 세션 관리자 | Tauri 2 | ~13 MB | 부분 지원 | 미지원 | 미지원 | 지원 | 미지원 |
| [ccusage](https://github.com/ryoppippi/ccusage) | CLI 사용량 추적기 | TypeScript | ~600 KB | 미지원 | 미지원 | 미지원 | CLI | 미지원 |
| [CodePilot](https://github.com/op7418/CodePilot) | 데스크톱 채팅 UI | Electron | ~140 MB | 미지원 | 미지원 | 미지원 | 미지원 | 미지원 |
| [claude-run](https://github.com/kamranahmedse/claude-run) | 히스토리 뷰어 | TypeScript | ~500 KB | 부분 지원 | 미지원 | 기본 | 미지원 | 미지원 |

> 채팅 UI(CodePilot, CUI, claude-code-webui)는 Claude Code를 *위한* 인터페이스입니다. claude-view는 기존 터미널 세션을 감시하는 대시보드입니다. 서로 보완적인 관계입니다.

---

## 설치

| 방법 | 명령어 |
|--------|---------|
| **Shell** (권장) | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |
| **Plugin** (자동 시작) | `claude plugin add @claude-view/plugin` |

Shell 설치 프로그램은 사전 빌드된 바이너리(~10 MB)를 다운로드하고 `~/.claude-view/bin`에 설치한 뒤 PATH에 추가합니다. 그런 다음 `claude-view`를 실행하면 됩니다.

**유일한 요구사항:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) 설치.

<details>
<summary><strong>설정</strong></summary>
<br>

| 환경 변수 | 기본값 | 설명 |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` or `PORT` | `47892` | 기본 포트 변경 |

</details>

<details>
<summary><strong>셀프 호스팅 및 로컬 개발</strong></summary>
<br>

사전 빌드된 바이너리에는 인증, 공유, 모바일 릴레이가 내장되어 있습니다. 소스에서 빌드하시나요? 이 기능들은 **환경 변수를 통한 옵트인** 방식입니다 — 생략하면 해당 기능이 비활성화됩니다.

| 환경 변수 | 기능 | 없을 경우 |
|-------------|---------|------------|
| `SUPABASE_URL` | 로그인 / 인증 | 인증 비활성화 — 완전 로컬, 무계정 모드 |
| `RELAY_URL` | 모바일 페어링 | QR 페어링 사용 불가 |
| `SHARE_WORKER_URL` + `SHARE_VIEWER_URL` | 암호화 공유 | 공유 버튼 숨김 |

```bash
bun dev    # 완전 로컬, 클라우드 의존성 없음
```

</details>

<details>
<summary><strong>엔터프라이즈 / 샌드박스 환경</strong></summary>
<br>

머신에서 쓰기가 제한된 경우 (DataCloak, CrowdStrike, 기업 DLP):

```bash
cp crates/server/.env.example .env
# CLAUDE_VIEW_DATA_DIR 주석 해제
```

이렇게 하면 데이터베이스, 검색 인덱스, 락 파일이 저장소 내에 유지됩니다. 읽기 전용 환경에서 훅 등록을 건너뛰려면 `CLAUDE_VIEW_SKIP_HOOKS=1`을 설정하세요.

</details>

---

## FAQ

<details>
<summary><strong>로그인했는데도 "Not signed in" 배너가 표시됩니다</strong></summary>
<br>

claude-view는 `~/.claude/.credentials.json`을 읽어 Claude 자격 증명을 확인합니다 (macOS Keychain 폴백 포함). 다음 단계를 시도해 보세요:

1. **Claude CLI 인증 확인:** `claude auth status`
2. **자격 증명 파일 확인:** `cat ~/.claude/.credentials.json` — `accessToken`이 포함된 `claudeAiOauth` 섹션이 있어야 합니다
3. **macOS Keychain 확인:** `security find-generic-password -s "Claude Code-credentials" -w`
4. **토큰 만료 확인:** 자격 증명 JSON에서 `expiresAt`을 확인 — 만료되었으면 `claude auth login` 실행
5. **HOME 확인:** `echo $HOME` — 서버가 `$HOME/.claude/.credentials.json`에서 읽습니다

모든 확인을 통과했는데도 배너가 계속 표시되면 [Discord](https://discord.gg/G7wdZTpRfu)에서 보고해 주세요.

</details>

<details>
<summary><strong>claude-view는 어떤 데이터에 접근하나요?</strong></summary>
<br>

claude-view는 Claude Code가 `~/.claude/projects/`에 기록하는 JSONL 세션 파일을 읽습니다. SQLite와 Tantivy를 사용하여 로컬에서 인덱싱합니다. 암호화 공유 기능을 명시적으로 사용하지 않는 한 **데이터가 머신을 벗어나지 않습니다**. 텔레메트리는 옵트인이며 기본적으로 꺼져 있습니다.

</details>

<details>
<summary><strong>VS Code / Cursor / IDE 확장 프로그램의 Claude Code에서도 작동하나요?</strong></summary>
<br>

네. claude-view는 시작 방법에 관계없이 모든 Claude Code 세션을 모니터링합니다 — 터미널 CLI, VS Code 확장, Cursor 또는 Agent SDK. 각 세션에 소스 배지(Terminal, VS Code, SDK)가 표시되어 시작 방법으로 필터링할 수 있습니다.

</details>

---

## 커뮤니티

- **웹사이트:** [claudeview.ai](https://claudeview.ai) — 문서, 변경 로그, 블로그
- **Discord:** [서버 참여](https://discord.gg/G7wdZTpRfu) — 지원, 기능 요청, 토론
- **플러그인:** [`@claude-view/plugin`](https://www.npmjs.com/package/@claude-view/plugin) — 86개 MCP 도구, 9개 스킬, 자동 시작

---

<details>
<summary><strong>개발</strong></summary>
<br>

사전 요구사항: [Rust](https://rustup.rs/), [Bun](https://bun.sh/), `cargo install cargo-watch`

```bash
bun install        # 모든 워크스페이스 의존성 설치
bun dev            # 풀스택 개발 시작 (Rust + Web + Sidecar 핫 리로드)
```

### 워크스페이스 레이아웃

| 경로 | 패키지 | 용도 |
|------|---------|---------|
| `apps/web/` | `@claude-view/web` | React SPA (Vite) — 메인 웹 프론트엔드 |
| `apps/share/` | `@claude-view/share` | 공유 뷰어 SPA — Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | Expo 네이티브 앱 |
| `apps/landing/` | `@claude-view/landing` | Astro 5 랜딩 페이지 (클라이언트 사이드 JS 없음) |
| `packages/shared/` | `@claude-view/shared` | 공유 타입 및 테마 토큰 |
| `packages/design-tokens/` | `@claude-view/design-tokens` | 색상, 간격, 타이포그래피 |
| `packages/plugin/` | `@claude-view/plugin` | Claude Code 플러그인 (MCP 서버 + 도구 + 스킬) |
| `crates/` | — | Rust 백엔드 (Axum) |
| `sidecar/` | — | Node.js 사이드카 (Agent SDK 브릿지) |
| `infra/share-worker/` | — | Cloudflare Worker — 공유 API (R2 + D1) |
| `infra/install-worker/` | — | Cloudflare Worker — 다운로드 추적이 포함된 설치 스크립트 |

### 개발 명령어

| 명령어 | 설명 |
|---------|-------------|
| `bun dev` | 풀스택 개발 — Rust + Web + Sidecar 핫 리로드 |
| `bun run dev:web` | 웹 프론트엔드만 |
| `bun run dev:server` | Rust 백엔드만 |
| `bun run build` | 모든 워크스페이스 빌드 |
| `bun run preview` | 웹 빌드 + 릴리스 바이너리로 서빙 |
| `bun run lint:all` | JS/TS + Rust (Clippy) 린트 |
| `bun run typecheck` | TypeScript 타입 체크 |
| `bun run test` | 모든 테스트 실행 (Turbo) |
| `bun run test:rust` | Rust 테스트 실행 |
| `bun run storybook` | 컴포넌트 개발용 Storybook 실행 |
| `bun run dist:test` | 빌드 + 패키징 + 설치 + 실행 (전체 배포 테스트) |

### 릴리스

```bash
bun run release          # 패치 버전 업
bun run release:minor    # 마이너 버전 업
git push origin main --tags    # CI 트리거 → 빌드 → npm에 자동 게시
```

</details>

---

## 플랫폼 지원

| 플랫폼 | 상태 |
|----------|--------|
| macOS (Apple Silicon) | 지원 |
| macOS (Intel) | 지원 |
| Linux (x64) | 계획 중 |
| Windows (x64) | 계획 중 |

---

## 관련 프로젝트

- **[claudeview.ai](https://claudeview.ai)** — 공식 웹사이트, 문서, 변경 로그
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** — 86개 MCP 도구와 9개 스킬을 갖춘 Claude Code 플러그인. `claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** — Claude Code는 30일 후 세션을 삭제합니다. 이 도구가 세션을 저장합니다. `npx claude-backup`

---

<div align="center">

**claude-view**가 AI 에이전트의 작업을 파악하는 데 도움이 되었다면, 스타를 남겨주세요.

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT &copy; 2026

</div>
