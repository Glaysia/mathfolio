# Mathcraft 개발 계획

**2026-10-24 SDK:** [WASM SDK 0.1.0](WASM-SDK.md)의 실제 Canvas·Worker·문서 bytes·저장 승인·벡터 PDF를 브라우저에서 검증했다. [0214](analysis/0214-wasm-sdk.md)에 크기와 Windows 전후 성능, 양 OS 회귀를 기록한다. 플러그인 자체와 실제 호스트 통합 검증은 별도 레포에서 진행한다.

**2026-10-23 진행:** 보고된 Windows 스크롤 지연은 [0211](analysis/0211-windows-scroll-renderer-fix.md)에서 수정·사용자 확인·양 OS 회귀 검증을 마쳤다. [UI 1차 개편](analysis/0212-shared-native-chrome.md)에 공통 egui 도구·속성·테마·검색 도움말·입력 격리와 글꼴 경계, 양 OS 검사와 실제 창 결과를 기록했다. 새 UI의 사용자 확인과 WASM 실행은 후속이다. 같은 회차 대비 추가 그리기 비용과 도움말 간격 outlier도 공개하며 전체 성능 동등을 완료로 선언하지 않는다.

**2026-10-23 구조 합의:** [네이티브·WASM 공통 구현 계획](NATIVE-WASM.md)을 따른다. 독립판은 JS/TS 실행 엔진·브라우저·WebView 없는 Rust 네이티브 앱으로 유지한다. 계산·문서·편집·조판·egui UI는 공통 Rust로 두고, 같은 구현의 WASM SDK를 별도 Obsidian·VS Code TS 플러그인에 연결한다. library 추출의 native 회귀는 위0212에서 통과했다. 전체 WASM·Worker·두 호스트 실행은 미검증이다.

**현재 우선순위 — 2026-10-22:** 사용자가 현재 기능을 다음 단계의 기준선으로 수용했다. 다음 작업은 아래 두 가지로 고정한다. **기존 단축키는 변경하지 않는다.** [CLI·MCP·skill 연동](AGENT-INTEGRATION.md)은 보관된 후속 계획이다.

| 우선순위 | 작업 | 완료 기준 |
| --- | --- | --- |
| 1 | [스크롤 버벅임 개선](PERFORMANCE.md) | release의 입력→조판→그리기→GPU/표시 지연을 분리하고, 실제 병목을 수정한 같은 조건의 전후 자료를 남긴다. |
| 2 | [현대적인 UI/UX](UI-MODERNIZATION.md)와 [네이티브·WASM 구조](NATIVE-WASM.md) | 문서 중심 도구·속성·단축키 안내를 정리하고, 같은 Rust 구현을 Windows/Kubuntu 독립판과 Obsidian/VS Code Desktop에서 검증한다. 독립판에는 웹 실행 엔진을 넣지 않는다. |

진행 순서는 **수정된 native 기준선·library 추출 검증 → 호스트/계산 작업 경계 분리 → 실제 WASM·Worker와 두 호스트 최소 검증 → 공통 egui UI 개선 → 두 OS·두 호스트 회귀 확인**이다. 현재 기능 기준선과 기존 검증 기록은 유지한다. 스크롤 수정 결과는 위0211에 기록하며 나머지는 후속 계획이다.

**UI 개편 전 기능 체크포인트:** [현재 보존 범위](CHECKPOINT-2026-10-20.md). 기존 r222 고정 검증은 [입력 행렬 편집·조판·인쇄](analysis/0207-typed-source-matrix-window.md)에 보존한다. 아래 이름 붙은 이전 검증은 당시 이력이다.

**XY 첫 수식 후속:** [0181 계약](analysis/0181-xy-typed-expression-contract.md)·[0182 통합](analysis/0182-xy-typed-expression-integration.md)에 typed source와 일반 expression-only 편집, label/contextual/문자 아래첨자 보존·물리량 pointwise 계산·Undo/Redo·Pending 저장·cold reopen·벡터 출력을 연결한다. typed-xy-r193은 Windows/peets 각각1486검사, 양 OS98MCDX/10별도XML·58PDF 검증을 통과했다. 원본 전체 navigator·IME·sampling/domain·legend와 marker CPU 공백은 남아 있으며 완성률로 환산하지 않는다.


기존 기능의 채택 검증은 [입력 행렬 편집·조판·인쇄](analysis/0207-typed-source-matrix-window.md) r222까지 기록되어 있다. 이전 cohort·metadata/fmt 실패 이력은 원문 receipt로 보존한다.

갱신: 2026-10-22. 현재 구현 판정은 [원본 기능별 통합 목록](FEATURES.md)을 단일 기준으로 사용한다. 아래 호환 공백은 기존 구현 범위의 기록이며 현재 두 작업에 추가 기능 개발을 포함시키지 않는다.

## 목표와 범위

Mathcad Prime의 워크시트·구조 수식 편집·계산·문서 동작을 Rust로 독립 구현한다. UI는 Excalidraw처럼 간결하고 현대적인 작업 공간으로 개선하고, 기존 수식 편집 의미와 단축키를 유지한다. 제품의 가치는 수식과 결과를 같은 종이에서 작성하고 수정하는 사용성에 있다. REA로 이 흐름의 명령·상태·저장 표현을 확인하며, 계산 함수 수만으로 완료를 판정하지 않는다.

개발은 현재 Windows PC에서 진행한다. 필수 실행 환경은 Windows와 Kubuntu 26.04이며 Linux 검증 대상은 `ssh peets`다. macOS는 우선 검증 대상에서 제외하되 코어의 플랫폼 종속성은 최소화한다.

Obsidian 플러그인과 VS Code 확장은 별도 프로젝트로 유지한다. 이번 UI 작업에서는 양쪽에 내장할 공통 Rust/egui 컴포넌트·WASM SDK와 최소 어댑터 검증을 진행한다. 호스트 IO·작업 실행·수명만 분리하고 TS에 계산·편집을 재구현하지 않는다. API·산출물·단계별 통과 조건은 [구조 계획](NATIVE-WASM.md)을 따른다.

Mathcad 호환 기능은 REA 기록·설치 도움말·XSD·기존 원본 실행 표본으로 존재와 의미를 확인한다. 불명확한 내부 경로를 추정해 호환 기능으로 부르지 않는다. 새 UI와 단축키 안내, 후속 AI 연동은 사용자 지시에 따른 Mathcraft 제품 설계로 구분한다. 원본 코드·리소스·글꼴은 제품에 복제하지 않는다.

## 현재 상태

[FEATURES.md](FEATURES.md)에 원본 기능 단위의 구현됨·부분 구현·미구현, 지원 경로, 원본 근거와 남은 차이를 모았다. 기존 87개는 분야 요약으로 내리고 세부 항목 555행을 설치 근거 216파일에 연결했다. 내부 바인딩 874행·단위/상수 227개와 중복 합산하지 않는다. 다음은 작업 묶음이며 완료율이 아니다.

| 작업 묶음 | 현재 범위 | 완료를 막는 항목 |
| --- | --- | --- |
| 기본 공학 계산서 | 자유 배치·기본 수식/단위·행렬·서식·XY 연결 | 전체 원본 편집 전이·단위/배열 범위·도표의 전체 native source 규약 |
| 기술 계산 문서 | 프로그램 Enter·행렬 Tab, 다중 trace, Find/Minerr, 날짜, 용지/여백·기본 인쇄/PDF | 전체 제어문 저작·풀이 블록 인라인 UX·전체 CAS/solver·출력 fidelity |
| 문서 열기·결과 표시 | 저장 표현 복원, F5/Ctrl+F5 분리, Pending 유지, 평면 영역 비활성화, 백그라운드 계산·협력 중지/재개 | 전체 native cache·정밀 의존성·Area 전파·원본 queue 게시 시점·외부 IO 중단·중첩/Area 결과의 전체 표시 범위 조절 |
| 종이 오른쪽 초과 영역 | Prime Page/Draft 전환의 기본 경로 | 전체 원본 조작/배치 검증 |
| 성능·실행 | release 실행, backend 의존성 축소, idle 복제/불필요 조판 감소 | 실제 GPU·대규모 재계산·두 OS 사용 성능 측정 |

2026-10-20 사용자의 최신 평가는 **80%**다. 이전15%/40% 개발 목표와11%/20% 목표는 아래 기능 개발 이력으로 보존한다. 이 수치는 사용자 평가이며 계측된 Prime 호환율이 아니다. 제한된 기능이나 테스트 개수를 완료율로 환산하지 않는다.

## 이전 40% 목표의 작업 범위

기본 구조 편집·Area·문서 흐름·native 결과·XY 확정/공유 축을 통합했고, 다음 원본 작성 흐름의 공백을 연속해서 채운다. 각 묶음은 입력·수정·계산 확정·Undo/Redo·저장·재열기까지 검증하며, 구현 범위를 기능 목록의 실제 원본 항목에 연결한다.

| 우선순위 | 원본 작성 흐름 | 확인할 결과 |
|---|---|---|
| 1 | 구조 수식 편집 | 연산자 선택/교체, Typeover 두 단계 삭제, 이름 아래첨자·라벨, 컨테이너 이동과 문자/범위 편집이 원본 상태 전이를 따른다. |
| 2 | 행렬·표·함수·프로그램 | 일반 행렬과 표의 실제 선택/편집/열 계산, 함수 정의·호출과 프로그램의 주요 편집 경계를 계산·native 저장에 연결한다. |
| 3 | 기술 문서 작성 | 텍스트 서식 저작, 페이지 나눔·Area 등 주요 문서 구조의 현재 공백을 원본 저장 구조와 함께 구현한다. |
| 4 | 도표·풀이 블록 | 기존 XY·solve 계산을 실제 원본 저작/수정/재열기 흐름에 연결하고, 도표 MCDX export 등 막힌 왕복 경로를 채운다. |
| 5 | 계산 결과·단위·출력 | 제공 원본 계산서의 결과·표·단위·색상과 벡터 PDF를 유지하며 저장 표시/계산 명령의 미지원 상태를 확대한다. |
| 6 | 반응성과 양 OS 통합 | 큰 문서의 열기·첫 페이지·이동·키 입력·Enter를 분리 측정하고, 증가 표본도 보존한다. Windows·peets에서 동일 소스 회귀와 원본 reference/native/PDF를 검증한다. |

이 묶음의 과거 검증은 GUI·스크린샷·원본 런타임 없이 REA와 CPU 입력/상태·파일 검사로 진행했다. 사용자는2026-10-23 GUI 사용 금지를 해제했다. 간접 검사만으로 전체 UI/UX나 사용자가 체감하는 완성도를 선언하지 않는다.

## 기존 기능의 남은 공백과 조사 이력

1. 저장 결과·계산 명령과 평면 영역 비활성화는 [0032](analysis/0032-calculation-commands-and-saved-results.md)/[0037](analysis/0037-disabled-regions.md)에 반영했다. [백그라운드 계산·중지·재개](analysis/0041-background-calculation-ui.md)도 연결했다. 전체 native 결과 형식, Area 비활성 전파, 정밀 의존성·원본 queue와 결과 게시 시점·외부 IO 중단 계약을 채운다.

   일반 입력과 Enter/영역 이탈 확정은 [0044](analysis/0044-edit-commit-implementation.md)에 분리했다. [제공 PDF 비교](analysis/0043-pdf-output-comparison.md)에서 확인한 백분율·극형식은 [결과 서식 연결](analysis/0047-result-formatting-integration.md)에 반영했다. 실제 원본의 [표·색상 표시](analysis/0048-native-table-and-colors.md), [벡터 PDF](analysis/0049-vector-pdf-output.md), [단위 표시](analysis/0050-unit-regressions.md), [입력 이력 비용](analysis/0051-typing-history-cost.md)을 교정했다. 가져온 표의 [셀 편집](analysis/0053-spec-table-editing.md)과 [열 계산](analysis/0054-spec-table-calculation.md)을 연결했다. [새 표·행/열·범위 명령](analysis/0059-spec-table-structural-commands.md), [원본 단축키·아래첨자·비교/논리 입력](analysis/0057-shortcut-and-subscript-contract.md), [계산 준비·드래그 비용](analysis/0058-input-and-calculation-latency.md)을 추가로 연결했다. Rich Text 저작·원본 전체 clipboard·편집 전이와 전체 수식 조판은 남은 호환 공백이다.
2. 프로그램 Enter 줄·행렬 마지막 Tab은 [0036](analysis/0036-program-lines-and-matrix-tab.md)에 반영했다. 전체 삭제/unwrap·다중 셀·큰 기호 결과 resize handle·인라인 풀이 블록·단위/배열/상징 누락을 이어서 채운다.
3. 실제 날짜와 기본 용지/여백·인쇄/PDF는 [0035](analysis/0035-printing-implementation.md)/[0038](analysis/0038-saved-date-fields.md)에 반영했다. 명시 페이지 나눔·지원 Area와 기본 도표 native export는0145/0147에 연결했다. 일반 공간 편집·전체 rich text/native 출력·도표 전체 source 규약과 실제 장치 검증이 남아 있다.
4. 미구현 엔진 기능은 바인딩 이름을 발견한 것과 실제 계산을 구분해 기능별로 분석·구현한다. FFT/DFT·회귀/보간·분포·ODE·고급 선형대수·전체 풀이/CAS의 타입·단위·오류 경계를 먼저 확정한다.
5. Windows와 peets에서 같은 소스의 fmt·Clippy·workspace 테스트·빌드·기존 참조 비교를 수행한다. 최신 소스의 검증과 과거 Linux 검증을 구분한다.

정적 REA와 headless 검사에 더해 GUI 검증을 허용한다(2026-10-23 사용자 지시). 실제 화면·IME·클립보드·원본 재열기 확인은 직접 검증한 항목만 완료로 바꾸며, GUI 허용 자체를 검증 결과로 세지 않는다. Windows native 스크롤 병목은 [0209](analysis/0209-windows-native-software-rendering.md)에 기록한다.

## 구조와 검증 경계

- `mathcraft-editor`: 안정 ID의 구조 수식, 캐럿·선택·명령 이력.
- `mathcraft-document`: 원본 MCDX ZIP parts·관계 보존, 문서 모델과 편집 export.
- `mathcraft-compat`: 라벨·scope·MathML 직접 실행과 참조 비교.
- `mathcraft-app`: 자유 배치 워크시트·수식 조판·리본·문서 입출력.
- `mathcraft-cli`: GUI 없는 MathML/native 실행과 검증.
- `mathcraft-core`: 값·차원 기반과 초기 문자열 엔진. 새 호환 기능은 문자열 평탄화 경로로 판정하지 않는다.

원본의 기존 38개 실행 표본은 제한된 차등 기준이다. 새 수치 기능의 독립 수학 기대값, 편집 상태/저장 불변식 검사와 구분한다. MCDX는 미편집 parts를 보존하고 편집 시 지원 구조만 생성하며 결과 캐시를 무효화한다. XSD 통과와 원본 앱 재열기는 별도 검증이다.

검증 기록은 [CONFORMANCE](CONFORMANCE.md), 실행은 [PROTOTYPE](PROTOTYPE.md), 환경/명령은 [DEVELOPMENT](DEVELOPMENT.md), 작성 철학과 근거는 [DESIGN](DESIGN.md)에 둔다. 분석 기록은 당시 근거로 보존하고 현재 상태를 대신하지 않는다.

설치 프로그램·제품 바이너리·전체 디컴파일 산출물은 Git에서 제외한다. REA는 개발 분석 도구이며 제품 런타임 의존성이 아니다. 원격 저장소·CI 실행 여부는 실제 원격/실행 기록으로 확인하며 로컬 빌드 성공을 배포 완료로 세지 않는다.

이어진 구현과 원본 근거: [배열 원소 정의](analysis/0063-indexed-definition-contract.md), [표 clipboard 교정](analysis/0064-table-clipboard-application.md), [계산 준비 추가 감소](analysis/0062-input-preparation-second-pass.md)를 반영했다. 지원 범위와 미확정 동작은 각 기록 및 FEATURES/CONFORMANCE를 따른다.

후속 [일반 행렬 편집·저장 통합](analysis/0068-matrix-workflow-integration.md)은 원본 범위 선택 모드의 Copy/Cut/Delete와 Clear의 차이를 연결한다. [배열 참조 비용](analysis/0067-array-reference-cost.md)은 계산기 내부 전체 복제를 줄였으며 전체 앱 지연이 해결됐다는 판정과 구분한다. 남은 원본 기능을 계속 같은 목록에서 관리한다.

[구조 연산자 통합](analysis/0072-structural-operators-integration.md)은 native opcode·키·조판·저장 캐시까지 검사한다. Ctrl+Shift+C를 잃던 입력 backend를 최소 수정하고 실제 Copy 이벤트를 키로 추측하는 처리를 제거했다. 전용 연산자의 전체 기호 변환과 물리 단위 type-lowering, 다른 편집/문서 기능은 계속 목록에 남긴다.

[전역·벡터 연산 통합](analysis/0076-global-vector-integration.md)은 전역 선계산과 실패 전파, 원소별 연산자/함수 계산, 3×1 외적을 연결한다. nested 배열·상징 및 전체 원본 편집 전이는 미완료이며 테스트 수를 제품 완료율로 환산하지 않는다.

[기본 숫자 표기](analysis/0080-scalar-notation-integration.md)는 Percent·Degree·Polar의 원본 descriptor/키/XML과 지원 계산을 연결한다. 전체 편집·상징/타입 공백은 기능 목록에 유지한다.

[행·열 대입](analysis/0083-array-slice-integration.md)은 원본에서 확인한 named-base·scalar-index부터 일반·전역·프로그램 계산에 연결한다. 범위 slice와 상위 typing 전체 일치는 남은 공백이다.

[프로그램 지역 함수](analysis/0087-local-function-integration.md)는 원본 증거가 있는 named parameter·앞선 local capture부터 작성·계산에 연결한다. 캡처 변경·함수값·고차 함수는 지원 경계로 남긴다.

[첫 입력 통합 검증](analysis/0092-committed-input-integration.md)에서 실제 열기 뒤 불필요한 준비와 첫 행렬 Tab을 교정했다. 다음 비용 조사는 큰 문서 Enter의 전체 문맥 라벨 처리·표시 트리 복원이며 Manual/F5·generation·취소·오류 시점을 유지한다. 이 비용 교정을 새 기능 완료나 전체 20%로 집계하지 않는다.

[라벨 순환 통합](analysis/0094-label-cycle-integration.md)은 Ctrl+Q와 자동(None) 리본을 원본 상태·native 저장·계산에 연결한다. 단위/변수 이름 충돌과 함수 인자·local·표의 기존 추론은 검사하지만 전체 원본 wildcard와 Typeover/override 호스트를 완료 처리하지 않는다.

[프로그램 제어문 통합](analysis/0098-program-controls-integration.md)은 Ctrl+J 5개 단어와 break/continue의 빈 자리 삽입·안쪽 삭제를 연결한다. 후속은 원본 if/else/elseif/alsoif/try의 구조·pair repair, populated 위치의 삽입·분할 및 삭제 전이다. [cold 준비](analysis/0097-cold-worksheet-preparation.md)는 많은 작은 조각의 XML parse/assembly를 줄였으며 label walk·표시 준비·확정의 다른 단계는 별도 개선 대상이다.

[다중 조건 분기](analysis/0101-conditional-branch-integration.md)는 native 편집·계산과 확인된 경계의 구조 입력을 연결한다. 후속은 원본 populated split/merge·ancestor repair·프로그램 glyph/DPI 배치다. Ctrl+J if의 descriptor 순서는 [0099](analysis/0099-conditional-branch-contract.md)의 설치 framework 정적 근거를 따른다.

[Try/on error](analysis/0105-try-control-integration.md)의 확인된 입력·삭제·계산·native 경로와 [표시 DOM 재사용](analysis/0102-display-preparation-cost.md)을 연결했다. 후속은 일반 populated split/merge·ancestor repair·전체 오류 분류·프로그램 glyph/DPI·남은 XML/label/signature 준비 비용이다.

[선택 수식의 프로그램 삽입](analysis/0109-program-insertion-integration.md)과 [프레임 중복 제거](analysis/0107-frame-interaction-cost.md)를 연결했다. 후속은 원본 일반 partial/implicit parser·전체 ancestor repair/커서 상태·미확정 root 계산 경계·native cold alignment와 실제 GPU 지연이다.

[암시적 배율·확정 준비](analysis/0113-implicit-scale-integration.md)의 원본 단축키·이름 함수·implicit Ctrl+J를 연결했다. 후속은 일반 partial/adjacency parser·함숫값·원본 전체 navigation/repair, 확정 준비의 남은 직렬화/signature와 native cold 정렬이다. 현재40% 목표 달성과 이 구현 범위는 구분한다.

[괄호·컨테이너/직렬화](analysis/0117-parenthesis-integration.md)를 연결했다. 다음 공백은 일반 lexer/rich-run·모든 navigator/repair·미지원 조합과 남은 signature/clone·native cold 정렬이다. 현재40% 목표 달성과 이 구현 범위는 구분한다.

[일반 연산자 입력](analysis/0121-infix-entry-integration.md)은 숫자/이름 문자 범위·Normal 교체/Scope·alternate·커서를 연결한다. 문서 열기 중복을 별도로 측정한다. 다음은 formatted 이름의 모든 run·callee/parent repair·구조 삭제/변환과 큰 문서 signature 비용이며, 새 테스트 수를 전체20%로 환산하지 않는다.
