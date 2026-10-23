# Mathcraft

라이선스: **[GNU AGPLv3 only](LICENSE)** (`AGPL-3.0-only`). Mathcraft 자체 작성 소스와 문서에 적용하며, 포함된 제3자 구성요소의 원래 라이선스는 [NOTICE.md](NOTICE.md)를 따른다.

**2026-10-23:** [Windows 스크롤 지연 수정](docs/analysis/0211-windows-scroll-renderer-fix.md)을 반영했다. 같은 실행체의 native 간격을 약219ms에서6.5ms로 줄였고 사용자가 개선을 확인했다.

**UI 1차 개편:** [간결한 도구 모음·선택 속성·테마·검색 도움말](docs/analysis/0212-shared-native-chrome.md)을 공통 Rust egui로 적용했다. 기존 키와 Enter 확정을 유지하며 검색/속성 입력을 문서 명령과 격리한다. Windows/peets 회귀와 실제 Windows 화면을 확인했다. 이후 [WASM SDK](docs/WASM-SDK.md)의 실제 브라우저 실행을 검증했다. UI 개편 당시 스크롤 비교의 추가 그리기 비용도 기록했다.

**2026-10-24:** [WASM SDK 사용법](docs/WASM-SDK.md) · [용량·호환·native 성능 검증](docs/analysis/0214-wasm-sdk.md). SDK 빌드는 `python scripts/build-wasm.py`이며 산출물은 `dist/wasm-sdk`와 배포용 tar.gz에 생성된다.

[호스트 색상 연동·Nord](docs/HOST-THEMES.md): UI 테마 메뉴와 Rust 색 토큰 계약을 제공한다. CSS 수집·설정 저장과 실제 내장 연결은 별도 플러그인 레포의 후속 범위다.

**현재 두 작업:** [스크롤 성능 개선](docs/PERFORMANCE.md)과 [현대적인 UI/UX](docs/UI-MODERNIZATION.md). [네이티브·WASM 구조 계획](docs/NATIVE-WASM.md)에 따라 독립판은 JS/TS 실행 엔진·브라우저·WebView 없는 Rust 앱으로 유지하고, 공통 Rust/egui 구현의 WASM을 별도 Obsidian·VS Code TS 플러그인에 연결한다. [WASM SDK 0.1.0](docs/WASM-SDK.md)의 Canvas·Worker·열기/저장·벡터 PDF를 실제 브라우저에서 검증했다. 두 플러그인의 실제 연결은 별도 레포의 범위다. 기존 단축키를 유지한다. [기능 체크포인트](docs/CHECKPOINT-2026-10-20.md)와 [r222 고정 검증](docs/analysis/0207-typed-source-matrix-window.md)은 당시 이력으로 보존한다.

**XY 첫 수식 후속:** [0181 계약](docs/analysis/0181-xy-typed-expression-contract.md)·[0182 통합](docs/analysis/0182-xy-typed-expression-integration.md)에 typed source와 일반 expression-only 편집, label/contextual/문자 아래첨자 보존·물리량 pointwise 계산·Undo/Redo·Pending 저장·cold reopen·벡터 출력을 연결한다. typed-xy-r193은 Windows/peets 각각1486검사, 양 OS98MCDX/10별도XML·58PDF 검증을 통과했다. 원본 전체 navigator·IME·sampling/domain·legend와 marker CPU 공백은 남아 있으며 완성률로 환산하지 않는다.


Mathcad Prime의 워크시트·수식 편집·계산·문서 동작을 맞추는 Rust 오픈소스 독립 구현 프로젝트.

- 개발 위치: 이 Windows PC의 `C:\Users\harry\Projects\mathcraft`
- 필수 실행 환경: Windows, Kubuntu 26.04 (`ssh peets`)
- macOS: 우선 검증 대상에서 제외하되 플랫폼 종속 설계는 최소화
- 현재 단계: 사용자가 현재 기능을 기준선으로 수용했고 Windows 스크롤 지연을 수정했다. 공통 egui UI/UX의 1차 개편을 반영했으며, WASM SDK의 실제 브라우저 실행을 검증했고, 두 호스트 플러그인은 별도 레포에서 연결한다. 원본 전체 호환 범위는 기능 목록에 별도로 기록한다.

UI는 Excalidraw처럼 간결하고 현대적인 작업 공간으로 개선하며, Mathcad 방식의 수식 편집·계산 의미와 기존 단축키는 유지한다.

2026-10-22 사용자 지시에 따라 스크롤 버벅임과 현대적인 UI/UX, 두 작업에 집중한다. 2026-10-23 합의한 구조에 따라 UI는 Rust egui로 유지하고 Obsidian·VS Code에서도 같은 WASM 편집 화면을 사용할 수 있게 분리하며, 기존 단축키의 설명·툴팁·도움말을 강화한다. [CLI·MCP·skill](docs/AGENT-INTEGRATION.md)은 보관된 후속 계획이다. 작업 범위와 검증 기준은 [개발 계획](docs/PLAN.md)을 따른다.

제품의 중심은 종이에 쓰듯 수식을 작성하고 고치는 경험이다. 계산 엔진은 그 흐름을 지탱한다. [작성 경험의 기준](docs/DESIGN.md)에 원본 근거와 구현·검증 범위를 연결했다.

[원본 기능별 통합 구현 목록](docs/FEATURES.md) — 세부 동작·명령 555행, 계산 바인딩 874행, 사용자 표기·단위 대조를 구분한 현재 상태의 단일 기준.

현재 커밋의 [UI 개편 전 검증 기록](docs/CHECKPOINT-2026-10-20.md)을 참고한다. [r222 입력 행렬](docs/analysis/0207-typed-source-matrix-window.md)과 이전 cohort·실패 이력은 당시 기록으로 보존한다.

[계산 엔진 상태와 실행법](docs/ENGINE.md) · [원본 계산 바인딩 목록](docs/spec/mathcad12-engine-catalog.json)

[개발 계획](docs/PLAN.md) · [환경 조사](docs/ENVIRONMENT.md) · [분석 기록 양식](docs/OBSERVATION_TEMPLATE.md)

[REA 분석 환경](docs/REA.md) · [Mathcad Prime 12 최초 분석 결과](docs/analysis/0001-initial-inspection.md)

[프로토타입 사용법과 검증 범위](docs/PROTOTYPE.md)

[호환성 검증과 현재 한계](docs/CONFORMANCE.md) · [수식 편집 분석](docs/analysis/0002-editor-semantics.md) · [계산 분석](docs/analysis/0003-calculation-semantics.md) · [문서 분석](docs/analysis/0004-document-semantics.md)

## 개발 실행

프로젝트의 `rust-toolchain.toml`에서 Rust 1.99.0과 rustfmt, Clippy, rust-analyzer를 고정한다.

```sh
cargo run -p mathcraft-cli --locked
cargo run --release -p mathcraft-app --locked
cargo run -p mathcraft-cli --locked -- conformance tests/reference/mathcad12.json
cargo run -p mathcraft-cli --locked -- native-inspect INPUT.mcdx
cargo run -p mathcraft-cli --locked -- native-copy INPUT.mcdx NEW.mcdx
cargo run -p mathcraft-cli --locked -- native-run INPUT.mcdx
cargo run -p mathcraft-cli --locked -- features
cargo run -p mathcraft-cli --locked -- mathml-run examples/engineering.xml
```

전체 검사는 Windows에서 `./scripts/check.ps1`, Linux에서 `bash scripts/check.sh`로 실행한다. 신규 환경 준비 절차와 검증 상태는 [개발 환경](docs/DEVELOPMENT.md)을 참고한다.

## 다음 작업

2026-10-23: [서식 있는 Text 선택 후 휠 입력이 프레임마다 밀리는 문제](docs/analysis/0208-scroll-input-backlog.md)를 수정했다. 현재 스크롤 성능 작업의 첫 수정이며, 기존 키·계산·저장 의미를 유지한다.

1. [스크롤 버벅임 개선](docs/PERFORMANCE.md): release의 실제 병목을 분리 측정하고 같은 조건에서 전후를 확인한다.
2. [현대적인 UI/UX](docs/UI-MODERNIZATION.md)와 [네이티브·WASM 투트랙](docs/NATIVE-WASM.md): [공통 SDK](docs/WASM-SDK.md)의 브라우저 검증과 native 회귀를 유지하고, 별도 플러그인 레포에서 호스트 저장·테마·입력·수명을 연결한다.

## 기존 기능의 남은 공백과 구현 이력

아래는 현재 지원 범위와 후속 호환 공백의 기록이다. 이번 두 작업과 구분한다.

1. 통합 목록에 드러난 원본 기능 공백과 동작 차이를 고친다. 저장 결과·계산 명령·영역 비활성화·프로그램 줄·행렬 Tab·날짜·기본 인쇄/PDF를 연결했다. 긴 계산은 [백그라운드에서 실행하고 중지·재개](docs/analysis/0041-background-calculation-ui.md)한다. [인쇄](docs/analysis/0035-printing-implementation.md)와 [성능 측정](docs/analysis/0034-rendering-and-calculation-cost.md)을 참고한다. 전체 native 영역·정밀 의존성·원본 queue 게시 시점·외부 IO 중단·source 직접 표시 범위 조절과 중첩 결과의 전체 형식은 남아 있다.
2. 편집→계산→수정→Undo→저장 흐름의 재현 테스트를 확대.
   일반 입력과 Enter/영역 이탈의 [확정 경계](docs/analysis/0044-edit-commit-implementation.md), Percent·복소수·저장 결과의 [서식 연결](docs/analysis/0047-result-formatting-integration.md)을 반영했다. 실제 1015 원본의 [표·색상 복원](docs/analysis/0048-native-table-and-colors.md), [벡터 PDF](docs/analysis/0049-vector-pdf-output.md), [단위 표시 수정](docs/analysis/0050-unit-regressions.md)과 [입력 이력 비용 감소](docs/analysis/0051-typing-history-cost.md)를 추가했다. 가져온 표의 [셀 편집](docs/analysis/0053-spec-table-editing.md)·[열 계산](docs/analysis/0054-spec-table-calculation.md)과 [확대/DPI 조판·캐시 비용 수정](docs/analysis/0055-screen-rendering-cost.md)을 연결했다. [원본 단축키·아래첨자](docs/analysis/0057-shortcut-and-subscript-contract.md), [계산 준비·이동 비용](docs/analysis/0058-input-and-calculation-latency.md), [새 표와 행/열·범위 명령](docs/analysis/0059-spec-table-structural-commands.md)을 이어서 연결했다. 전체 원본 조판·편집 일치는 미완료다.
3. Windows와 peets에서 GUI 없이 상태 전이·MathML·MCDX 및 기존 참조 결과를 검증.

설치 프로그램과 원본 제품 바이너리는 Git 추적에서 제외한다.

이어진 구현과 원본 근거: [배열 원소 정의](docs/analysis/0063-indexed-definition-contract.md), [표 clipboard 교정](docs/analysis/0064-table-clipboard-application.md), [계산 준비 추가 감소](docs/analysis/0062-input-preparation-second-pass.md)를 반영했다. 지원 범위와 미확정 동작은 각 기록 및 FEATURES/CONFORMANCE를 따른다.

일반 행렬의 [직사각형 선택·삭제·Clear·저장](docs/analysis/0068-matrix-workflow-integration.md)을 연결했고, [반복 배열 조회의 전체 복제](docs/analysis/0067-array-reference-cost.md)를 제거했다. 원본에서 거절하는 범위 Paste·일반 입력과 미완료 선택 전이는 구분한다.

[전용 구조 연산자](docs/analysis/0072-structural-operators-integration.md)는 factorial·transpose·conjugate·절댓값/행렬식·Norm·행/열 추출을 리본과 원본 키, native 저장, 계산까지 연결한다. 함수 이름만 표시하는 경로와 구분하며 원본의 모든 타입·편집 전이를 지원한다는 뜻은 아니다.

[전역 정의·벡터화·외적](docs/analysis/0076-global-vector-integration.md)은 전용 연산자 작성·원본 키·계산·저장 복원을 연결한다. 표 셀의 표현식 전용 제한도 검사한다.

[백분율·도·극형식 입력](docs/analysis/0080-scalar-notation-integration.md)은 %·° 후위 연산자와 Ctrl+Shift+P 극형식을 지원 범위 안에서 계산·저장까지 연결한다. Degree와 DMS 함수를 구분한다.

[행·열 좌변 정의](docs/analysis/0083-array-slice-integration.md)는 일반·전역·프로그램 대입과 Enter·Undo·native 왕복을 연결한다. [확정 준비 비용](docs/analysis/0084-prepared-source-cost.md)은 수정 시 전체 XML 재해석을 줄이고 cold 한계도 함께 기록한다.

[프로그램 지역 함수](docs/analysis/0087-local-function-integration.md)는 `f(x)` 뒤 원본 `{` 입력으로 선언을 만들고 계산·native 저장까지 연결한다. [계산 서명 비용](docs/analysis/0088-calculation-signature-cost.md)은 기존 source 바이트를 보존하는 cache와 cold 한계를 기록한다.

[첫 입력과 포커스 교정](docs/analysis/0092-committed-input-integration.md)은 커서·키 해제의 불필요한 계산 준비와 첫 행렬 Tab 누락을 고친다. 실제 열기 뒤 입력 비용은 줄었으며 큰 문서의 Enter 확정 비용은 남아 있다.

[라벨 순환·자동 복원](docs/analysis/0094-label-cycle-integration.md)은 원본 Ctrl+Q 순서와 리본의 자동 라벨을 연결한다. None(`*`)의 native 저장을 보존하고 단위·변수 이름이 겹치는 계산 문맥을 구분한다.

[프로그램 제어문과 준비 비용](docs/analysis/0098-program-controls-integration.md)은 Ctrl+J의 while/for/return/break/continue 변환·삭제·Undo·native 저장과 큰 워크시트의 XML 준비 최적화를 연결한다. 전체 프로그램 저작과 Enter 지연은 아직 개선 중이다.

[조건 분기 작성·원본 행 배치](docs/analysis/0101-conditional-branch-integration.md)는 if/else if/also if/else의 편집·Ctrl+J·저장·계산을 연결하고 직렬화 비용을 줄인다. 원본의 header/test와 그 아래 본문 배치를 따르며 일반 split/merge는 남아 있다.

[Try/on error와 표시 준비](docs/analysis/0105-try-control-integration.md)는 원본 Ctrl+J·두 본문·헤더/최초 입력칸 삭제·계산 오류 처리·저장·PDF를 연결한다. [DOM 재사용](docs/analysis/0102-display-preparation-cost.md)으로 Enter의 중복 파싱을 줄였으며 원본 전체 오류·조판과 일반 구조 repair는 남아 있다.

[선택 수식의 프로그램 삽입과 프레임 비용](docs/analysis/0109-program-insertion-integration.md)은 Scope의 기존 본문 보존·Return 방향·전체 값 선택 교체를 원본대로 구분하고 입력 스냅샷·높이 재조판·pending 글자 paint 복제를 줄인다. 일반 문자 분할·implicit parser와 전체 원본 동등성은 남아 있다.

[암시적 배율과 확정 준비](docs/analysis/0113-implicit-scale-integration.md)는 자동 입력/Ctrl+Shift+U·함수 RHS·Ctrl+J 본문 보존을 연결하고 XML 왕복을 줄인다. 원본에 맞춘 기능 범위·양 OS 검증·전체 Enter 성능과 남은 차이를 함께 기록한다.

[괄호와 입력 컨테이너](docs/analysis/0117-parenthesis-integration.md)는 `2(3)`·이름 호출·부분 선택·닫기와 지수/분모에서의 ArrowRight 경계를 원본대로 구분한다. 직렬화 비용과 양 OS 전체 검증·남은 차이를 함께 기록한다.

[일반 연산자 입력](docs/analysis/0121-infix-entry-integration.md)은 `1234` 중간의 `+`를 `12+34`로 나누며, 문자 전체 선택과 Scope·음수/뺄셈·커서·Ctrl+- 종료를 구분한다. 문서 열기 중복 처리와 검증 범위도 함께 기록한다.
