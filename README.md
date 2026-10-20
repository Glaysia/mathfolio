# Mathcraft

**UI 개편 전 기능 체크포인트:** [현재 보존 범위](docs/CHECKPOINT-2026-10-20.md). 다음 작업은 [현대적인 UI와 기존 단축키 안내 개선](docs/UI-MODERNIZATION.md)이다. 기존 r222 고정 검증은 [입력 행렬 편집·조판·인쇄](docs/analysis/0207-typed-source-matrix-window.md)에 보존한다. 아래 이름 붙은 이전 검증은 당시 이력이다.

**XY 첫 수식 후속:** [0181 계약](docs/analysis/0181-xy-typed-expression-contract.md)·[0182 통합](docs/analysis/0182-xy-typed-expression-integration.md)에 typed source와 일반 expression-only 편집, label/contextual/문자 아래첨자 보존·물리량 pointwise 계산·Undo/Redo·Pending 저장·cold reopen·벡터 출력을 연결한다. typed-xy-r193은 Windows/peets 각각1486검사, 양 OS98MCDX/10별도XML·58PDF 검증을 통과했다. 원본 전체 navigator·IME·sampling/domain·legend와 marker CPU 공백은 남아 있으며 완성률로 환산하지 않는다.


Mathcad Prime의 워크시트·수식 편집·계산·문서 동작을 맞추는 Rust 오픈소스 독립 구현 프로젝트.

- 개발 위치: 이 Windows PC의 `C:\Users\harry\Projects\mathcraft`
- 필수 실행 환경: Windows, Kubuntu 26.04 (`ssh peets`)
- macOS: 우선 검증 대상에서 제외하되 플랫폼 종속 설계는 최소화
- 현재 단계: REA로 원본의 수식 작성·편집·계산 상태 전이를 추적하고 Rust에 연결 중. 원본 전체 호환은 미완료다.

UI는 Excalidraw처럼 간결하고 현대적인 작업 공간으로 개선하며, Mathcad 방식의 수식 편집·계산 의미와 기존 단축키는 유지한다.

사용자의 최신 완성도 평가는80%다. 다음 작업은 [현대적인 UI와 단축키 안내 개선](docs/UI-MODERNIZATION.md)이며 기존 단축키를 유지한다. [CLI·MCP·skill](docs/AGENT-INTEGRATION.md)은 후속 계획으로 기록했다. 작업 범위와 검증 기준은 [개발 계획](docs/PLAN.md)을 따른다.

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

우선 [UI 개편 계획](docs/UI-MODERNIZATION.md)을 진행한다. 아래 항목은 기존 기능의 남은 공백이며, [AI 연동](docs/AGENT-INTEGRATION.md)은 후속 계획이다.

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
