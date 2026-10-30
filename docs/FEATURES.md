# Mathcad Prime 기능별 통합 구현 목록

**UI 개편 전 기능 체크포인트:** [현재 보존 범위](CHECKPOINT-2026-10-20.md). 다음 작업은 [현대적인 UI와 기존 단축키 안내 개선](UI-MODERNIZATION.md)이다. 기존 r222 고정 검증은 [입력 행렬 편집·조판·인쇄](analysis/0207-typed-source-matrix-window.md)에 보존한다. 아래 이름 붙은 이전 검증은 당시 이력이다.

**XY 첫 수식 후속:** [0181 계약](analysis/0181-xy-typed-expression-contract.md)·[0182 통합](analysis/0182-xy-typed-expression-integration.md)에 typed source와 일반 expression-only 편집, label/contextual/문자 아래첨자 보존·물리량 pointwise 계산·Undo/Redo·Pending 저장·cold reopen·벡터 출력을 연결한다. typed-xy-r193은 Windows/peets 각각1486검사, 양 OS98MCDX/10별도XML·58PDF 검증을 통과했다. 원본 전체 navigator·IME·sampling/domain·legend와 marker CPU 공백은 남아 있으며 완성률로 환산하지 않는다.


기준: **2026-10-30** (기능 조사 2026-10-20, CAD 다중 선택 갱신 2026-10-30), 설치본 **Prime 12.0.0.1**, 현재 작업 트리의 Rust 소스와 회귀 검사. 이 문서를 현재 상태의 단일 기준으로 사용한다. [계획](PLAN.md), [엔진](ENGINE.md), [사용법](PROTOTYPE.md)은 이 목록을 참조한다.


**슬롯 키 후속:** [0179 계약](analysis/0179-xy-slot-key-authoring.md)·[0180 통합](analysis/0180-xy-slot-key-integration.md)에 실제 Shift+Enter·빈 Backspace/Delete·방향별 포커스·혼합 Undo·IME·배치 F5/저장과 곡선 identity 보존을 연결했다. r184 양 OS 검증을 통과했다. HD154/155는 부분 구현을 유지하며, 이전 단위 변환 구현 뒤 미구현으로 남은 HD156을 부분 구현으로 정정했다. 현재553행은 부분148/미구현405다. 위 단계별 남은 범위는 당시 이력이며 첫 math typed 편집·전체 navigator·legend·marker CPU 공백은 계속 남아 있다.

## 판정 기준과 조사 범위

| 상태 | 의미 |
| --- | --- |
| 구현됨 | 행에 명시한 좁은 동작 범위에 실행 경로와 회귀 근거가 있다. 제품 전체·모든 플랫폼·원본 외형의 동일성을 뜻하지 않는다. |
| 부분 구현 | 실행되는 경로가 있으나 원본의 입력, 타입, 알고리즘, 저장 또는 조작 범위가 빠져 있다. 엔진에만 있는 경로도 여기에 포함한다. |
| 미구현 | 해당 원본 동작의 독립 실행/작성 경로가 없다. 리본 버튼·타입 선언·REA 조사·원본 바이트 보존만으로 구현으로 판정하지 않는다. |

현재 목록은 다음 층으로 구성한다. **서로 겹치므로 합산하거나 제품 진행률로 환산하지 않는다.**

| 층 | 조사한 범위 | 현재 판정 |
| --- | --- | --- |
| [분야 요약](#family-summary) | 기존 87개 묶음 | 구현됨 4 / 부분 구현 69 / 미구현 14. 아래 세부 행을 압축한 개요 |
| [세부 사용자 동작·명령·설정](#detailed-features) | 555개 행 | 구현됨 1 / 부분 구현 159 / 미구현 395. 작성·문서·도표·상징 키워드/수정자·공개 연동과 추가 사용자 함수/overload |
| [원본 계산 바인딩](#engine-bindings) | 21개 모듈, 874행 | 내부·호환 이름 포함. 함수 개수·UI 기능 개수와 다름 |
| [사용자 표기 대조](#public-spellings) | 34개 이름 | 기존 내부 바인딩과 같은 기능. 별도 신규 기능 수에 합산하지 않음 |
| [SI 단위·물리 상수](#si-vocabulary) | 단위 212 / 상수 15 | 값/차원 lookup의 구현 범위. 기능 227개가 추가 완성됐다는 뜻이 아님 |

기존 87개를 최종 기능 수로 제시한 것은 조사 범위와 세분도가 부족했다. 이번에는 설치 도움말의 편집 명령·페이지/도표 옵션·접이식 영역·Include·템플릿·검색/출력·스크립팅·공개 자동화 API까지 넓혔다. [조사 기록](analysis/0031-feature-inventory-audit.md)과 [행별 근거·구현 경로·SHA-256](spec/mathcad12-feature-audit.json)을 함께 유지한다.

**표·입력 갱신:** 가져온 표의 [셀 편집·저장](analysis/0053-spec-table-editing.md)과 [열 계산](analysis/0054-spec-table-calculation.md)에 더해 [새 표·행/열·범위 명령](analysis/0059-spec-table-structural-commands.md)을 연결했다. [원본 단축키·이름 아래첨자·비교/논리 편집](analysis/0057-shortcut-and-subscript-contract.md), [입력 준비와 드래그 비용](analysis/0058-input-and-calculation-latency.md), [확대/DPI 조판](analysis/0055-screen-rendering-cost.md), [좌표 JSON 정밀도](analysis/0056-native-cache-float-roundtrip.md)도 교정했다. 전체 원본 편집·native 표현과 실제 GPU 사용성은 남아 있으며, 최신 양 OS 검증은 [CONFORMANCE](CONFORMANCE.md)를 따른다. 이 갱신을 전체 완성률 20%로 환산하지 않는다.

**일반 행렬 후속:** [원본 선택 모드](analysis/0065-matrix-selection-contract.md)와 [실제 입력 연결](analysis/0066-matrix-selection-application.md)을 별도로 조사해 직사각형 드래그·Copy/Cut/Delete·Clear 및 저장 왕복을 연결했다. [배열 참조 비용](analysis/0067-array-reference-cost.md)은 셀 하나를 읽을 때 전체 배열을 복제하던 계산 경로를 줄인다. [통합 검사](analysis/0068-matrix-workflow-integration.md)는 초안/확정과 다른 편집기의 Undo 보존을 구분한다.

**구조 연산자 후속:** [원본 이름·저장·grouping](analysis/0069-structural-array-operators-contract.md), [키·조판](analysis/0070-structural-array-operators-input.md), [배열 계산](analysis/0071-structural-array-operators-calculation.md)을 [리본·저장·표시 캐시](analysis/0072-structural-operators-integration.md)까지 연결했다. Norm의 내부 명칭과 물리 타입 미확정 부분을 분리하며, Ctrl+Shift+C는 Copy 이벤트를 추측하지 않고 입력 백엔드에서 원래 Key로 보존한다.

**전역·벡터 연산 후속:** [전역 정의·벡터화·외적](analysis/0076-global-vector-integration.md)의 전용 저작·키·native 저장·계산·캐시를 연결했다. 전역은 화면 아래쪽에서도 위쪽 식에 적용하고, 벡터화는 일반 행렬 곱과 구분한다. 표 셀에서는 원본 표현식 전용 제한을 보존한다. 전체 원본 동작·제품 진행률과 구분한다.

**프로그램 지역 함수 후속:** [0087](analysis/0087-local-function-integration.md)은 원본 `{` 입력부터 named 함수 선언·계산·확정·native 왕복을 연결한다. 함수값·변경된 capture의 미확정 의미는 지원 경계로 남긴다. [0088](analysis/0088-calculation-signature-cost.md)은 exact JSON 재사용의 수정 비용과 cold·보존 payload 증가를 함께 기록한다.

**첫 입력·확정·포커스 후속:** [0092](analysis/0092-committed-input-integration.md)는 실제 열기 뒤 커서/키 해제의 전체 계산 준비 제거, 첫 draft의 이전 입력 지연 준비, 첫 Matrix Tab/Shift+Tab 교정과 양 OS 회귀를 기록한다. Ctrl+- 이름 아래첨자 계약을 유지한다. 첫 Text 비용과 Enter 비용을 함께 측정하며 기존 기능 판정·항목 수는 바꾸지 않는다.

**라벨 순환 후속:** [0093](analysis/0093-label-cycle-contract.md)의 원본 enum·writer·RL 근거를 따라 [0094](analysis/0094-label-cycle-integration.md)에 Ctrl+Q·자동 리본·None(*) 계산/저장을 연결했다. EA-016을 부분 구현으로 옮겼으며 전체 자동 라벨 추론·Typeover·override 호스트는 남아 있다.

**프로그램 제어문·확정 비용 후속:** [0096 원본 입력 계약](analysis/0096-program-control-authoring-contract.md)과 [0098 통합](analysis/0098-program-controls-integration.md)에 Ctrl+J의 지원 5개 키워드·break/continue 리본 빈 자리 삽입·안쪽 삭제·단일 Undo·native 저장을 연결한다. if/else 계열의 pair repair와 일반 위치 삽입은 남아 있다. [0095 비용 분해](analysis/0095-worksheet-preparation-cost.md)·[0097 제한된 batch 준비](analysis/0097-cold-worksheet-preparation.md)는 source/metadata oracle과 적용 경계 및 남은 Enter 비용을 기록한다.

**조건 분기 후속:** [0099](analysis/0099-conditional-branch-contract.md)·[0101](analysis/0101-conditional-branch-integration.md)에 원본 flat IF·BODY·단일 alternate mode·최소 if/별도 분기 삽입·Ctrl+J if와 인접 분기 단어·세로 조판·pointer operator 선택을 연결했다. EA-071/072/073/080의 지원 깊이를 갱신했다. [0100](analysis/0100-xml-escape-cost.md)은 바이트·오류를 유지한 XML 직렬화 개선과 warm 악화를 함께 기록한다.

**괄호·컨테이너 후속:** [0114](analysis/0114-parenthesis-adjacency-contract.md)·[0117](analysis/0117-parenthesis-integration.md)에 Normal/Scope·인접 Scale/APPLY·부분 UTF16·컨테이너/닫기·ArrowRight·native를 연결했다. [0115](analysis/0115-fused-serialization-cost.md)의 단일 buffer는 독립 byte/error oracle와 최종 전체 Enter로 비교한다. 전체 lexer/navigator/repair는 미완료다.

**일반 연산자 입력 후속:** [0118 원본 계약](analysis/0118-normal-infix-entry-contract.md)·[0121 통합](analysis/0121-infix-entry-integration.md)에 Normal 문자 분할·전체 값 교체·Scope·음수/뺄셈 alternate·경로별 커서·Ctrl+- 종료와 Enter 확정을 연결한다. 7개 행의 지원 깊이를 갱신하며 부분 구현 분류를 유지한다. 일반 rich-run·전체 parent repair와 navigator는 남아 있다.

**구조 삭제·서명 비용 후속:** [0122 원본 삭제 계약](analysis/0122-structural-deletion-contract.md)에 따라 일반 숫자·무라벨 이름의 lexical 결합과 실패 시 Scale, Group/Fraction/Power의 방향별 상태, glyph 중앙 클릭·단항 결합 복구·Undo/Enter/native를 연결했다. EA-010은 부분 구현을 유지하며 formatted 라벨 결합·일반 Typeover·특수 연산자·동일 우선순위 repair는 남아 있다. [0123 서명 비용](analysis/0123-signature-emission-cost.md)은 구 JSON/SHA bytes·저장 캐시·예산을 보존한 framing/64KiB buffer와 cold 개선·warm 악화를 함께 기록한다.

**편집 교체·formatted·Typeover 후속:** [0130](analysis/0130-editing-repair-integration.md)은 원본 [0126](analysis/0126-typeover-deletion-contract.md)·[0127](analysis/0127-formatted-deletion-contract.md)·[0129](analysis/0129-operator-replacement-contract.md)에 맞춘 교체·삭제와 [0128](analysis/0128-worksheet-preparation-cost.md)의 준비 비용을 연결한다. EA-010은 부분 구현을 유지한다. 사용자가 평가한15%를 기준으로40% 목표 작업을 계속하며 이번 검사 수를 완성률로 환산하지 않는다.

**암시적 배율 후속:** [0110](analysis/0110-implicit-adjacency-contract.md)·[0113](analysis/0113-implicit-scale-integration.md)에 자동/수동 Scale·focus-only ∘·함수 RHS 적용·Ctrl+J 본문 보존을 연결했다. [0111](analysis/0111-commit-preparation-design.md)의 typed 준비는 full-DOM·exact source·cache/error oracle와 전체 Enter 측정으로 검증한다. 일반 parser와 함숫값 식은 미완료다.

**프로그램 선택·프레임 비용 후속:** [0106](analysis/0106-populated-program-insertion-contract.md)·[0109](analysis/0109-program-insertion-integration.md)에 Scope primal BODY·Program ID 재사용·Return 방향/커서·Normal 전체 값 문자 선택 교체·Undo·native 계산을 연결했다. EA-071/074~079의 지원 깊이를 갱신했다. [0107](analysis/0107-frame-interaction-cost.md)은 입력 snapshot·정확한 현 frame 높이·동등한 opacity paint와 page+tessellation 비용을 구분한다.

**Try와 표시 비용 후속:** [0103 원본](analysis/0103-try-control-contract.md)·[0104 삭제](analysis/0104-try-deletion-contract.md)·[0105 구현/검증](analysis/0105-try-control-integration.md)에 Ctrl+J try·빈 두 BODY·header/fresh 입력 삭제·native·계산 fallback·PDF를 연결했다. EA-079를 부분 구현으로 옮겼으며 오류 taxonomy·일반 repair는 남아 있다. [0102](analysis/0102-display-preparation-cost.md)의 DOM 재사용은 전체 Enter와 worksheet+display를 함께 측정했다.

**행·열 좌변 후속:** [0083](analysis/0083-array-slice-integration.md)은 일반·전역·프로그램 대입과 Enter·Undo·native 왕복을 연결한다. [0084](analysis/0084-prepared-source-cost.md)는 warm 준비의 전체 XML DOM 재해석을 줄인 측정과 cold 한계를 기록한다.

**기본 숫자 표기 후속:** [백분율·도·극형식](analysis/0080-scalar-notation-integration.md)의 전용 입력·리본·계산·native 저장을 연결했다. Degree는 단항 °이며 기존 도·분·초 표기를 교정했다. [확정 준비 비용](analysis/0079-commit-preparation-cost.md)은 별도 성능 경로로 기록한다.

**계산 수명 주기 갱신:** 문서 열기는 계산 없이 지원되는 저장 표현을 복원한다. F5는 보이는 활성 영역을 먼저 강제 갱신한 뒤 마지막 보이는 계산 영역까지 Pending 구간을 처리하고, Ctrl+F5는 전체 강제 계산한다. Pending 결과는 유지하며 해당 영역만 흐리게 표시한다. [저장 결과·계산 명령](analysis/0032-calculation-commands-and-saved-results.md), [평면 비활성 영역](analysis/0037-disabled-regions.md), [백그라운드 계산·중지·재개](analysis/0041-background-calculation-ui.md)를 연결했다. 전체 native 결과 형식·정밀 의존성·Area 전파·원본 queue 게시 시점·외부 IO 중단은 미완료다. 큰 기호 결과의 원본 오른쪽 resize handle도 미구현이다(EA-087).

원본 근거는 설치 바이너리의 REA 기록·해시, 설치 도움말·XSD 및 기존 [38개 실행 표본](../tests/reference/mathcad12.json)이다. 새 엔진/편집 검사는 독립 구현의 회귀 검사이며 원본을 새로 실행한 차등 검사가 아니다. GUI·스크린샷·COM을 새로 사용하지 않았다. 실제 Windows/Linux IME·클립보드, 원본에서의 새 MCDX 재열기, 전체 화면/조작 일치는 미검증이다. 검증 이력은 [CONFORMANCE](CONFORMANCE.md)를 참고한다.

일반 타이핑은 계산 입력과 분리하고 Enter·영역 이탈 때 확정한다. [원본 계약](analysis/0042-edit-commit-contract.md)과 [구현](analysis/0044-edit-commit-implementation.md)을 따른다. 사용자 제공 [두 PDF 비교](analysis/0043-pdf-output-comparison.md)에서 확인한 차이에 따라 빈 근호 입력칸과 [결과 서식](analysis/0047-result-formatting-integration.md)을 교정했다. 이후 제공된 실제 1015 MCDX에서 [6열 Spec Table·색상/강조](analysis/0048-native-table-and-colors.md)를 복원하고 [벡터 PDF](analysis/0049-vector-pdf-output.md), [SI 단위 표시](analysis/0050-unit-regressions.md), [입력 이력 비용](analysis/0051-typing-history-cost.md)을 수정했다. 표의 전체 편집 전이·계산 타입, Rich Text 작성과 전체 조판은 남아 있다. 제공된 두 PDF와 원본 MCDX는 변경하지 않았다.

이 목록은 **조사한 원본 기능의 현재 목록**이다. Prime 전체를 빠짐없이 조사했다는 주장은 하지 않는다. 미조사 기능은 원본 증거를 확보한 뒤 추가한다. 원본 근거 없는 편의 기능·내부 테스트 도구·자체 파일 형식은 제품 기능 항목에 넣지 않는다. 원본에 대응하지 않는 결과 확장 창은 제거된 상태다.

<a id="family-summary"></a>

## 분야 요약

이 87행은 세부 목록과 겹치는 상위 묶음이다. 상위 묶음의 일부가 동작하더라도 그 아래 개별 명령은 미구현일 수 있다.

### 워크시트와 계산 상태

[원본·구현 근거](analysis/0015-worksheet-feedback.md) · [페이지](analysis/0029-result-visibility.md) · [앱 코드·회귀 검사](../crates/mathfolio-app/src/worksheet_ui.rs)

| ID | 원본 기능 | 상태 | 구현 범위와 남은 차이 |
| --- | --- | --- | --- |
| W01 | 자유 위치 수식·텍스트 영역 | 부분 구현 | 배치·이동·복사·삭제·저장 연결. 겹침·정렬·모든 원본 영역 종류와 조작은 미완료. |
| W02 | 위치에 따른 계산 순서 | 부분 구현 | 저장 좌표와 격자 좌표의 기존 원본 38개 표본 일치. 동일 위치 tie·모든 중첩 scope는 미확정. |
| W03 | 자동/수동 계산·F5/Ctrl+F5 | 부분 구현 | 보이는 범위/전체 강제 계산 분리, binding 재사용·Pending 유지/dim·평면 비활성 제외·백그라운드 계산·협력 중지/재개. 정밀 의존성·Area 전파·영역별 즉시 결과 게시·전체 원본 queue는 미완료. |
| W04 | 문서 열기 후 계산 결과 표시 | 부분 구현 | 지원 native literal 및 자체 수식/기호/도표 저장 표현을 실행 없이 복원. 캐시 누락/불일치는 Pending. 전체 native 결과 형식·평가 환경 복원은 미완료. |
| W05 | 오류 탐색·재정의 경고 | 부분 구현 | 위치 순서 이전/다음 오류와 경고 전환. 전체 오류 분류·인자·정확한 부분식 위치 표시 미완료. |
| W06 | Page/Draft 보기와 오른쪽 초과 영역 전환 | 부분 구현 | 본문 오른쪽 전환·문서 리본·Draft의 여백/머리글 숨김·점선 경계 연결. 원본 외형·전체 입력 상호작용 실기 검증 미완료. |
| W07 | 문서 확대/축소·너비 맞춤·격자 | 부분 구현 | 배율 역변환과 페이지 좌표 회귀 검사. 전체 화면·인쇄 배율 및 두 OS 실기 검증 미완료. |
| W08 | 리본의 탭·문맥 명령 | 부분 구현 | 설치 BAML의 배치 근거 반영. 미지원 명령은 비활성화이며 리본 존재를 기능 구현으로 세지 않음. |

### 수식 작성과 편집

[원본 계약](spec/mathcad12-editor-contract.json) · [편집 분석](analysis/0002-editor-semantics.md) · [선택](analysis/0014-expression-scope.md) · [문맥 명령](analysis/0017-contextual-commands.md) · [구조 입력](analysis/0026-structured-program-calculus-editing.md) · [편집 코드·검사](../crates/mathfolio-editor/src/lib.rs) · [위젯·검사](../crates/mathfolio-app/src/formula.rs)

| ID | 원본 기능 | 상태 | 구현 범위와 남은 차이 |
| --- | --- | --- | --- |
| E01 | 분수·지수·근호·절댓값·괄호 입력 | 부분 구현 | 구조 입력·배치·클릭·저장 연결. 전체 문맥별 wrapping·삭제·원본 글꼴 기하는 미완료. |
| E02 | 정의·평가·정의 RHS 평가 | 부분 구현 | a:=2, a=, a:=2=와 반복 = 계산 요청 연결. 원본 전체 =/: 문맥 분기 미완료. |
| E03 | 함수 정의·호출·범위 변수·행렬 인덱스 입력 | 부분 구현 | 편집 트리→MathML→계산 연결. 모든 입력 제스처·선택·자동 라벨 정책은 미완료. |
| E04 | 캐럿 이동·Tab·Ctrl+방향키·Home/End | 부분 구현 | 구조·기하 기반 이동 및 회귀 검사. 원본 전체 navigation 상태기계 미복구. |
| E05 | Space 선택 확장·문자 범위 선택·드래그 | 부분 구현 | 방향·진입 커서·UTF-16 범위 및 일반 행렬 직사각형 드래그·Copy/Cut/Delete 연결. 부분 피연산자·전체 typeover 전이는 미완료. |
| E06 | Undo/Redo와 커서·선택 복원 | 부분 구현 | 명령 단위 이력·붙여넣기 그룹·문서/트리 복원 검사. 전체 원본 명령 병합 정책 미확정. |
| E07 | 복사·잘라내기·붙여넣기 | 부분 구현 | 문자·내부 구조·영역 경로 연결. OS 클립보드와 타 앱 간 원본 포맷 호환 미검증. |
| E08 | IME 조합과 확정 입력 | 부분 구현 | headless preedit/commit/cancel·단일 Undo 검사. 실제 Windows/Linux IME 후보창·입력기 미검증. |
| E09 | 변수·함수·단위·상수 라벨 서식/변경 | 부분 구현 | 명시 라벨 보존·변경·표시 연결. 전체 contextual 라벨 추론 미완료. |
| E10 | 문자 아래첨자의 개별 서식 깊이 | 미구현 | 행렬 index와 별개인 원본 FormattedString의 문자별 깊이를 구현하지 않음. |
| E11 | 행렬 행·열 삽입/삭제 | 부분 구현 | Shift+Enter/Shift+Space·Tab·범위 행/열 삭제·크기 유지 Clear·단일 Undo 연결. 범위 Paste/일반 입력은 원본 모드대로 거절하며 source displaySome의 제한된 삽입·삭제를 연결했으며, 숨은 목표·복합식 진입 등 전체 경계는 미완료. |
| E12 | 프로그램 블록·지역 대입·if/else·for/while·return 입력 | 부분 구현 | Enter 줄 추가·빈 줄 삭제·중첩 경계·계산 연결. 전체 삭제/unwrap·elseif/alsoif·break/continue 저작은 미완료. |
| E13 | 적분·미분·합·곱 입력 | 부분 구현 | 유한 적분·미분·합/곱 템플릿과 엔진 연결. 모든 연산자 서식·algorithm hint·키 전이 미완료. |
| E14 | 상징 평가와 명령 입력 | 부분 구현 | 구조 화살표·명령·결과 표시 연결. 원본 전체 명령·표현 속성·상징 결과 편집 미완료. |
| E15 | 풀이 블록의 문서 내 작성/편집 | 부분 구현 | 제한된 Find/Minerr 구조 생성과 실행 연결. 현재 별도 편집 창은 임시 경로이며 Prime의 인라인 작성 동작 미완료. |

### 기본 계산·이름·프로그램

[원본 표본](../tests/reference/mathcad12.json) · [계산 분석](analysis/0003-calculation-semantics.md) · [프로그램](analysis/0006-programming-semantics.md) · [계산 코드](../crates/mathfolio-compat/src/lib.rs) · [검사](../crates/mathfolio-compat/tests/structured.rs)

| ID | 원본 기능 | 상태 | 구현 범위와 남은 차이 |
| --- | --- | --- | --- |
| C01 | 라벨이 다른 동명 식별자의 분리 | 구현됨 | 명시 라벨의 이름 identity 분리와 저장 보존. 자동 라벨 추론은 E09의 별도 미완료 범위. |
| C02 | 사용자 함수가 정의 시점 환경을 참조 | 구현됨 | a:=2; f(x):=a*x; a:=3; f(4)=8의 원본 표본과 회귀 검사. |
| C03 | 실패한 재정의 뒤 이전 값의 재사용 방지 | 구현됨 | 지원하지 않는 RHS·중복 global·불투명 native 정의의 실패와 종속 오류 전파 검사. |
| C04 | 실수·복소수 사칙연산·거듭제곱 | 부분 구현 | 독립 수치 경로와 일부 원본 표본. 모든 특이점·분기·범위·원본 정밀도 동일성은 미확인. |
| C05 | 지역/전역 정의·사용자 함수 | 부분 구현 | 기본 scope와 함수 환경 구현. 모든 원본 영역 계층·재귀·메타데이터 조합 미완료. |
| C06 | 비교·논리·조건 계산 | 부분 구현 | MathML 비교·and/or/xor/not 실행. 전체 편집 명령·오류/타입 규약 미완료. |
| C07 | 프로그램 지역 대입·분기·루프·return/break/continue | 부분 구현 | 지역 대입·분기·루프·제어문·제한된 indexed assignment/행열 좌변과 native 작성 연결. 전체 컨테이너/repair·타입/오류 경계 미완료. |
| C08 | 프로그램 indexed assignment·try/onerror | 부분 구현 | [0063](analysis/0063-indexed-definition-contract.md)의 indexed assignment와 [0105](analysis/0105-try-control-integration.md)의 try/onerror 작성·계산·native 경로가 있음. 전체 오류 taxonomy/repair·문자열/혼합 배열 대입 미완료. 이전 미구현 표기를 교정한다. |
| C09 | 문자열 값과 문자열 연산 | 부분 구현 | [0184](analysis/0184-string-value-contract.md)·[0185](analysis/0185-string-value-integration.md)의 String literal·정의/함수·기본 문자열 연산·설정·저장 결과/벡터 출력. 전체 Unicode/clipboard·num2str·혼합 문자열 행렬·format 등 미완료. |
| C10 | 삼각·역삼각·쌍곡·역쌍곡·exp/ln/log | 부분 구현 | scalar/허용 배열 dispatch 있음. 타입·차원·복소 branch 및 전체 overload 동일성 미완료. |
| C11 | 복소수 Re/Im/conj/arg·angle/atan2·sign 계열 | 부분 구현 | 정적 계약에 따른 제한된 scalar/배열 실행. 전체 원본 수치 비교 미완료. |
| C12 | 반올림·절삭·mod·factorial·타입 판별 | 부분 구현 | 대/소문자 반올림의 단위 규칙 포함. 전체 입력형·범위·NaN/문자열 규약 미완료. |
| C13 | ORIGIN·TOL·CTOL 등 계산 설정 | 부분 구현 | SYSTEM 기본값 5개와 일부 실제 사용 경로. 문서별 전체 설정 전달 미완료. |

### 단위와 배열·선형대수

[단위 사실](analysis/0010-unit-semantics.md) · [배열](analysis/0005-array-semantics.md) · [선형계](analysis/0025-linear-systems.md) · [고유계](analysis/0026-symmetric-eigensystems.md) · [코드](../crates/mathfolio-compat/src/arrays.rs) · [검사](../crates/mathfolio-compat/tests/arrays.rs)

| ID | 원본 기능 | 상태 | 구현 범위와 남은 차이 |
| --- | --- | --- | --- |
| A01 | SI 단위·물리 상수 | 부분 구현 | 분석한 SI 단위 212개·물리 상수 15개의 수치/차원 연결. 전체 원본 단위 체계·표시 정책은 미완료. |
| A02 | 차원 검사와 결과 단위 변환 | 부분 구현 | 물리량·표시 단위 분리와 기본 차원 오류 검사. 모든 혼합 차원/유리수 차원 표현 미완료. |
| A03 | 절대온도 affine 변환·통화·다른 단위계 | 부분 구현 | 유한 실수 °C/°F forward·표시 inverse와 Δ 단위 분리. 전체 형상·통화·다른 단위계·affine XY는 미완료 ([0190](analysis/0190-affine-temperature-contract.md), [0192](analysis/0192-affine-temperature-integration.md)). |
| A04 | 벡터·행렬·ORIGIN 인덱스·범위 | 부분 구현 | 구조 Value와 제한된 range/index 실행. 모든 배열 크기·미정의 셀·표현 조합 미완료. |
| A05 | scalar mapping·벡터 내적·행렬곱/나눗셈·정수 power | 부분 구현 | 복소 내적의 켤레와 지원 형상 검사. 전체 vectorize 구문·형상·차원 규약 미완료. |
| A06 | 전치·켤레·determinant·역행렬 | 부분 구현 | 제한된 유한 배열 경로. named det와 연산자 det의 단위 경계 구분. 전체 overload 미완료. |
| A07 | rows/cols/length/last·identity/diag/tr·augment/stack/submatrix | 부분 구현 | 구체 함수별 허용 형상·ORIGIN 검사. 전체 타입·대규모/희소 배열 미완료. |
| A08 | norm/norm1/norm2/norme/normi | 부분 구현 | 구현된 타입·형상에 한정. 전체 spectral 계산/원본 알고리즘 동일성 미완료. |
| A09 | lsolve 선형계·최소제곱 | 부분 구현 | 실수/복소 square·다중 RHS, full-column-rank 초과결정 지원. 부족결정·rank-deficient·혼합차원 factorization 거부. |
| A10 | eigenvals/eigenvecs | 부분 구현 | 32×32 이하 유한 실수 대칭·균일차원 행렬, 내림차순 고유값·정규화 열벡터. 일반/복소·좌고유벡터 미구현. |
| A11 | SVD·일반/복소 고유계·기타 분해 | 미구현 | 일부 역행렬/선형계 내부 소거를 원본 분해 함수 제공으로 세지 않음. |

### 미적분·통계·풀이·상징

[수치](analysis/0007-numerics-semantics.md) · [통계](analysis/0009-statistics-semantics.md) · [풀이](analysis/0011-solve-semantics.md) · [상징](analysis/0012-symbolics-semantics.md) · [극한](analysis/0024-symbolic-limits.md) · [엔진 검사](../crates/mathfolio-compat/tests)

| ID | 원본 기능 | 상태 | 구현 범위와 남은 차이 |
| --- | --- | --- | --- |
| N01 | root | 부분 구현 | 실수/복소 초기값·실수 bracket 경로. 독립 알고리즘·TOL·작업량 제한; 원본 전체 수렴 동작 미완료. |
| N02 | 유한 정적분 | 부분 구현 | 적응 수치적분 연결. 무한구간·전체 알고리즘 선택·원본 수렴 규약 미완료. |
| N03 | 수치 미분 | 부분 구현 | 0–5계 제한. 벡터·혼합 편미분·전체 수치 안정성/원본 일치 미완료. |
| N04 | 정수 경계 합·곱 | 부분 구현 | 유한 정수 bound 반복 실행. 모든 원본 range/무한/상징 조합 미완료. |
| N05 | polyroots | 부분 구현 | 제한된 계수·독립 근 계산. 원본 전체 수렴·중근·조건수 정책 미완료. |
| N06 | mean/gmean/hmean·median·var/Var·stdev/Stdev | 부분 구현 | 허용 실수/복소·단위/표본 경계에 한정. 전체 원본 overload 미완료. |
| N07 | cvar/corr | 부분 구현 | 실수·동일 형상에 한정. 복소 covariance 규약 미확정; signal covar와 별개. |
| N08 | min/max·sort/reverse/Rank | 부분 구현 | 열벡터·허용 배열 정렬과 동률 rank 처리. 전체 타입/정렬 overload 미완료. |
| N09 | Find/Minerr 풀이 블록 | 부분 구현 | scalar 실수 guess·등식·제한된 scope/solver 정의 내보내기. 부등식·복소/행렬 unknown·tuple/매개변수화 미완료. |
| N10 | Minimize/Maximize·일반 제약 최적화 | 미구현 | 원본 solver 함수의 독립 실행 경로 없음. |
| N11 | 상징 expand/simplify·정확 유리수·다항식 미분 | 부분 구현 | 제한된 다항식 독립 CAS. 일반 초월식·유리함수·전체 명령 미완료. |
| N12 | 상징 solve | 부분 구현 | 2차 이하 다항식에 한정. 일반 연립·초월 방정식 미완료. |
| N13 | 상징 limit/좌극한/우극한 | 부분 구현 | 유한점 다항식에 한정. 무한대·변수 분모·초월식 미완료. |
| N14 | 일반 factor·상징 부정적분·전체 CAS 변환 | 미구현 | 기존 제한 CAS 경로를 일반 상징 기능 구현으로 세지 않음. |
| N15 | FFT/DFT·역변환 | 미구현 | 원본 바인딩/분석 사실만 존재. Rust 실행 경로 없음. |
| N16 | ODE·PDE 풀이 | 미구현 | odesolve 및 일반 미분방정식 함수 경로 없음. |
| N17 | 회귀·보간·곡선 적합 | 미구현 | 원본 fitting 함수의 실행 경로 없음. |
| N18 | 확률분포·난수·Six Sigma | 미구현 | 원본 함수별 실행 경로 없음. |
| N19 | Bessel·특수 함수·금융 | 미구현 | 모듈 목록만 확보. 원본 함수별 수치 경로 없음. |
| N20 | 신호처리·wavelet·이미지 계산 | 미구현 | 원본 모듈 존재와 실행 기능을 구분. |
| N21 | 계산 함수의 외부 파일 읽기/쓰기 | 미구현 | MCDX 문서 열기와 별개인 원본 READ/WRITE 계열 함수 경로 없음. |

### 도표

[원본 근거·한계](analysis/0027-multi-trace-xy-plots.md) · [초기 도표 분석](analysis/0023-xy-plots.md) · [코드·검사](../crates/mathfolio-app/src/plots.rs)

| ID | 원본 기능 | 상태 | 구현 범위와 남은 차이 |
| --- | --- | --- | --- |
| P01 | XY 함수/범위/벡터 도표 | 부분 구현 | 무차원 실수, trace당 2,000점 제한. 독립 변수 기본 구간/샘플 연결. 단위 축·전체 원본 데이터 규약 미완료. |
| P02 | 여러 trace·축 식 편집 | 부분 구현 | 최대16 trace. 공유 x/y 목록·불균등 pairing, [0179](analysis/0179-xy-slot-key-authoring.md)의 Shift+Enter·빈 Backspace/Delete·포커스·draft/Enter·혼합 Undo 연결. 일반 typed expression·전체 navigator·legend layout 미완료. 새 통합 검증은 [0180](analysis/0180-xy-slot-key-integration.md)을 따른다. |
| P03 | 축 제목·명시 범위·선/표식·범례 | 부분 구현 | 기본 스타일 경로 연결. DashDot/DashDotDot 근사, 제목 복원·전체 축 옵션 미완료. |
| P04 | 도표 크기·확대·이동 | 부분 구현 | 저장 크기·Undo 및 viewport 이동 구현. 원본 전체 제스처·축 동작 미완료. |
| P05 | 편집한 XY 도표의 MCDX 저장 | 부분 구현 | [0147](analysis/0147-native-xy-integration.md): 기본 축·1–16 trace의 식/선 서식과 finite unitless 곡선 결과 저장·editable cold open·native copy. 불균등 axis lists도 [0156](analysis/0156-shared-xy-axis-integration.md)에 연결. [0176](analysis/0176-xy-physical-units-implementation.md)의 typed scaling/물리량·residual units/v3 저장까지 연결. 전체 단위 체계·marker·label·비기본 축은 미완료. |
| P06 | 보조 Y축·polar·contour·3D 도표 | 미구현 | 원본 기능의 작성/계산/저장 경로 없음. |

### 페이지·문서·출력

[문서 분석](analysis/0004-document-semantics.md) · [페이지](analysis/0020-prime-page-appearance.md) · [머리글/바닥글](analysis/0025-page-sections.md) · [문서 코드](../crates/mathfolio-document/src) · [서식](analysis/0022-math-formatting.md)

| ID | 원본 기능 | 상태 | 구현 범위와 남은 차이 |
| --- | --- | --- | --- |
| D01 | 숫자·복소수 결과 형식·자릿수·글꼴 크기 | 부분 구현 | General/Decimal/Scientific/Engineering/Percent·i/j·극형식·후행 0·임계값·원시 결과 재서식·직렬화/인쇄 연결. radix·round-trip·전체 문서/다중 선택·모든 조판 경계 미완료. |
| D02 | 수학 결과 조판 | 부분 구현 | 단위·분수·지수·행렬·상징 결과 표시, 원본 SI 대표 유도단위 14개 선택. 대체 글꼴 및 일부 간격 근사, 큰 결과 전체 표시·전체 단위 분해 규약 미완료. |
| D03 | 용지·방향·여백 설정 | 부분 구현 | 8종 용지·방향·프리셋/사용자 여백 편집·저장·native export·출력 연결. 명시 페이지 나눔·전체 pagination은 미완료. |
| D04 | 머리글·바닥글의 텍스트·위치 편집 | 부분 구현 | 기본 텍스트와 필드의 저장·Undo·MCDX 출력. Prime 인라인 UX·rich text/수식/이미지 미완료. |
| D05 | 페이지/전체 페이지·파일명·경로 필드 | 부분 구현 | 기본 preview·번호 설정·MCDX 필드 연결. 전체 필드 규약·원본 재열기 미검증. |
| D06 | 저장 날짜 필드 | 부분 구현 | 실제 저장 날짜의 7형식·화면/인쇄 연결, 열기/저장 때만 캐시 갱신. 전체 문화권·달력·자정 경과는 미검증. |
| D07 | 첫 페이지 별도 머리글/바닥글 | 미구현 | 원본 schema 존재만 확인. 실제 package 관계·작성 경로 미구현. |
| D08 | 서식 있는 텍스트·그림·임베디드 개체 작성 | 부분 구현 | 기존 authored/native Text·B/I/U·IME·저장에 같은 문단의 Run/Paragraph 경계·전체 선택 삭제, 실제 Ctrl+A 뒤 ordinary Text source 보존, authored full selected Backspace 후속 입력을 연결. 일반 임의 선택 치환·여러 문단·Paste/IME 대체·newline/병합·LineBreak·일반 typing affinity·그림/임베디드·원본 serializer 병합 동일성은 미완료. 입력 언어는 선언된 source 보존 정책이며 OS culture 동등성은 미검증. |
| D09 | MCDX 열기·기본 수식/텍스트 편집 | 부분 구현 | 지원 AST와 namespace/라벨/서식/좌표 처리, native rich text·색상 표시/출력, 가져온 Spec Table 셀 편집·열 계산. 새 표·전체 rich text 저작·전체 문서 의미 미완료. |
| D10 | 미편집 MCDX parts·관계 보존 | 구현됨 | 지원 ZIP 입력에서 각 항목의 바이트·압축방식 보존 검사. 컨테이너 전체 바이트/모든 ZIP 변형 동일성을 의미하지 않음. |
| D11 | MCDX 편집 저장과 계산 캐시 무효화 | 부분 구현 | 현재 native Ctrl+S·명시 Save As·own 형식 분리, loaded baseline/결과 proof 유지, 지원 source COW 반복 저장·검증. 읽기 전용 Save As 재질문·전체 native 구조는 미완료이며 설치 Prime 재열기는 미검증. |
| D12 | 인쇄·PDF 출력 | 부분 구현 | Ctrl+P·용지·범위·선택 페이지·벡터/검색 텍스트 PDF·Windows/CUPS 전송 경로, 오른쪽 여백의 표 출력 교정. 실제 프린터/드라이버·모든 native 영역·페이지 나눔은 미검증. |
| D13 | 전체 템플릿·보호 영역·외부 컴포넌트 연동 | 미구현 | 부분 보존을 원본 기능의 생성/실행/편집으로 세지 않음. |

<a id="detailed-features"></a>

## 세부 사용자 동작·명령·설정

553개는 아래 명시한 문맥의 행 수다. 같은 이름의 숫자 함수, 상징 수정자, 자동화 API는 서로 다른 경로다. 46개 행은 계산 바인딩만으로 포괄되지 않는 사용자 함수/상징 overload이며, 나머지 507개 행은 작성·문서·도표·상징 명령·연동이다. 서로 다른 세부 수준의 행을 제품 전체 기능 수로 주장하지 않는다.

각 행의 H 번호는 하단 [원본 근거 파일](#evidence-files)과 연결된다. 구현 경로·추가 제한은 [기계 판독 근거](spec/mathcad12-feature-audit.json)의 같은 ID에 있다. 미구현 API는 내부 Rust 기능으로 비슷한 값을 계산할 수 있어도 해당 공개 API가 제공되지 않는다는 뜻이다.

### 수식 편집 — 19행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| EA-001 | 수식 영역을 클릭·입력해 생성 | 부분 구현 | 자유 배치는 있으나 원본의 전체 입력 상태 전이는 미검증 | [H172](#h172) |
| EA-002 | 빈 자리 표시자 간 이동 | 부분 구현 | Tab·커서 이동은 있으나 원본 전이 규칙 전체 미복원 | [H098](#h098) |
| EA-003 | Enter로 수식 영역 밖으로 이동 | 부분 구현 | Enter를 워크시트로 전달하며 원본 전체 위치 규칙은 미검증 | [H098](#h098) |
| EA-004 | Home·End로 수식 시작·끝 이동 | 부분 구현 | 구조 경계 이동과 회귀는 있으나 원본 비교 미완료 | [H098](#h098) |
| EA-005 | Ctrl+좌우로 항 단위 이동 | 부분 구현 | 현재 항 경계 정책은 독립 구현 | [H098](#h098) |
| EA-006 | Shift+좌우로 항 그룹 확장 | 부분 구현 | 부분 선택 구현 범위에 한정 | [H098](#h098) |
| EA-007 | Space로 항 그룹 확장 | 부분 구현 | 그룹 범위 확장 경로는 있으나 원본 상태 전이 미검증 | [H098](#h098) |
| EA-008 | 항 그룹 묶기 | 부분 구현 | Normal 괄호의 빈 자리/전체 값 선택/부분 UTF16/인접 Scale·Call과 Scope 감싸기·커서를 구분. 원본 컨테이너·확인한 우선순위/닫기·Undo·native 저장 연결. 전체 rich-run lexer·navigator/repair 조합은 미완료. | [H178](#h178) |
| EA-009 | 여러 항에 연산자 한 번에 적용 | 부분 구현 | 선택 하위트리 래핑 범위에 한정 | [H148](#h148) |
| EA-010 | 기존 연산자 교체·삭제 | 부분 구현 | 기호 중심의 일반 이항·Fraction·Power 교체는 인접 피연산자·기본 grouping·한 Undo를 보존. formatted 이름/suffix·명시/문맥 라벨 결합과 마지막 문자→PH, Fraction 전체/Power script의 확인된 두 단계 Typeover·빨간 선택·Enter/Undo/native 연결. 전체 LinearSegment/APPLY·특수 연산자·rich-run·동일 우선순위 container repair와 사용자 설정 분기는 미완료. | [H192](#h192) |
| EA-011 | 리터럴 아래첨자 입력·식별 | 부분 구현 | Ctrl+-·리본의 이름 suffix 입력/선택/삭제/커서·Undo와 MathML 보존 연결. 전역 zoom 충돌 제거. 이름 중간의 임의 rich-run 깊이는 미구현. | [H207](#h207) |
| EA-012 | 2진수·8진수·16진수 표현 입력 | 부분 구현 | mode·lowercase typed source·native/own/cold·정수 계산 연결. 전체 partial parser·fraction·정밀도·키별 원본 동일성 미완료 ([0188](analysis/0188-radix-input-contract.md), [0189](analysis/0189-radix-and-string-sharing-integration.md)) | [H075](#h075) |
| EA-013 | 수식 강제 줄바꿈·토글 | 미구현 | EquationBreak 편집 노드 없음 | [H184](#h184) |
| EA-014 | 라벨 자동 추론 | 부분 구현 | contextual과 명시 None(*)를 보존·구분하며 선언·인자·프로그램·표·단위의 기존 제한된 추론에 연결. 전체 wildcard/type 추론 미구현. | [H016](#h016) |
| EA-015 | 사용자 라벨 명시 적용 | 부분 구현 | 라벨 리본의 6종과 자동(*) 복원·단일 Undo·이름/suffix/caret 보존 연결. 전체 원본 분류/스타일 미검증. | [H147](#h147) |
| EA-016 | Ctrl+Q로 라벨 순환 | 부분 구현 | 원본 Variable→Unit→Constant→Function→System→Keyword→None(*) 순환·contextual 시작점·단일 Undo·native 저장을 연결. 단일 식별자 범위·literal suffix 보존. Typeover/override 호스트·전체 자동 라벨 추론은 미완료. | [H016](#h016) |
| EA-019 | 기호·상수 갤러리 삽입 | 부분 구현 | 일부 그리스문자·상수만 제공 | [H179](#h179) |
| EA-020 | 단위 갤러리 삽입 | 부분 구현 | 실제 단위 메뉴는 명시 UNIT+Scale로 삽입해 동명 변수 충돌을 막는다. °C/°F·온도차·전체 원자 선택/Typeover 연결. 전체 분류/검색·partial/callee·빈 Area 첫 단위 작성은 미완료. | [H185](#h185) · [H204](#h204) |
| EA-021 | 결과 단위 자리표시자로 단위 변환 | 부분 구현 | 분리된 표시 단위 자리와 unitOverride를 유지하고 유한 실수 °C/°F의 inverse를 연결한다. 전체 단위함수·affine XY·일반 inverse·원본 입력/cache 전이는 미완료. | [H203](#h203) · [H204](#h204) |

### 연산자 저작 — 26행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| EA-022 | 정의 연산자 := | 부분 구현 | Define 노드·리본·계산 연결, 원본 전체 변환 규칙 미검증 | [H065](#h065) |
| EA-023 | 전역 정의 연산자 | 부분 구현 | 전용 GlobalDefine AST·Ctrl+Shift+~·리본·native 저장·위쪽 수식의 전역 선계산·실패 재정의 오류 전파 연결. RHS 평가와 초안/확정 경계를 검사한다. 행렬 이름 좌변·모든 영역 계층/기호 전이는 미완료. | [H085](#h085) |
| EA-024 | 숫자 평가 연산자 = | 부분 구현 | Evaluate 노드·결과 표시 연결, 결과 편집/모든 오버로드 미완료 | [H076](#h076) |
| EA-025 | 덧셈 연산자 | 부분 구현 | Normal 문자 시작/중간/부분·전체 선택의 Add 입력과 Scope 보존을 구분. 컨테이너 재파싱·우선순위·커서·Enter 확정·native를 연결. 전체 rich-run·navigator·symbolic 전이는 미완료. | [H031](#h031) |
| EA-026 | 부호 반전·뺄셈 연산자 | 부분 구현 | Normal 문맥의 음수/뺄셈 alternate·부분/전체 문자 선택·Scope 방향·커서·Undo·Enter/native를 연결. 전체 formatted 값·navigator/오류 전이는 미완료. | [H110](#h110) |
| EA-027 | 곱셈·벡터 내적 연산자 | 부분 구현 | Multiply의 Normal 문자 분할/교체·Scope·컨테이너 재파싱·계산/native를 지원. Scale/APPLY와 명시적 곱을 구분. 전체 벡터 overload·rich-run·navigator는 미완료. | [H109](#h109) |
| EA-028 | 배율·암시적 곱 연산자 | 부분 구현 | 자동 입력/Ctrl+Shift+U Scale·활성 ∘·값/단위 곱·이름 함수 적용에 인접 괄호와 원본 컨테이너 경계를 연결. 명시적 *는 유지. 일반 rich-run/parser·함숫값·정확한 원본 glyph/간격은 미완료. | [H130](#h130) |
| EA-029 | 세로 분수·인라인 나눗셈 | 부분 구현 | Fraction의 Normal 문자 분할/교체·Scope·분자/분모 컨테이너·ArrowRight 탈출·native를 연결. 기존 Fraction의 opaque handle과 새 Infix를 구분. InlineDivide와 전체 formatted/navigator 정책은 부분 구현. | [H072](#h072) |
| EA-030 | 거듭제곱·역행렬 표기 | 부분 구현 | Power의 Normal 문자 분할/교체·Scope·base 재파싱·exponent 컨테이너·ArrowRight·native를 연결. 지수 안의 단위는 유지. inverse overload·전체 rich-run/repair·navigator는 미완료. | [H080](#h080) |
| EA-031 | 제곱근·n제곱근 | 부분 구현 | Root 전용 입력·브리지·계산을 유지하고 일반 sqrt 이름 호출과 구분. sqrt(를 무조건 Root로 바꾸던 규칙 제거. 원본 모든 선택·입력 전이는 미완료. | [H135](#h135) |
| EA-032 | 팩토리얼 후위 연산자 | 부분 구현 | 전용 후위 Factorial AST·! 입력·리본·원본 MathML·Undo·계산 연결. 정수/차원·열벡터 mapping 검사, 전체 원본 편집/오류 전이는 미완료. | [H082](#h082) |
| EA-033 | 백분율 연산자 | 부분 구현 | 전용 Percent AST·% 후위 입력·리본·native 저장·scalar/복소/단위/행렬 원소의 0.01배 계산 연결. Power/NthRoot grouping 및 결과 Percent 서식 구분. 전체 원본 편집/기호 전이는 미완료. | [H117](#h117) |
| EA-034 | 도 단항 연산자 ° | 부분 구현 | 전용 단항 Degree AST·° 후위 입력·리본·degree_op 저장·scalar/복소/단위/행렬 원소의 π/180배 계산 연결. 별도 DMS 함수와 구분하며 기존 목록의 도·분·초 명칭을 교정. 전체 편집/기호 전이는 미완료. | [H066](#h066) |
| EA-035 | 복소수 극형식 연산자 | 부분 구현 | 전용 Polar AST·Ctrl+Shift+P·∠·왼쪽 결합·native 저장·무차원 실수/비음수 magnitude 및 scalar/열벡터 angle 계산 연결. 일반 곱과 벡터화 구분. 모든 단위 overload·nested 값·원본 편집/상징 전이는 미완료. | [H120](#h120) |
| EA-036 | 복소수 켤레 윗줄 연산자 | 부분 구현 | 전용 Conjugate AST·Ctrl+Shift+(·윗줄 벡터 조판·native 저장·복소수/배열 계산 연결. 모든 선택/상징 전이는 미완료. | [H058](#h058) |
| EA-037 | 비교 연산자 = ≠ < ≤ > ≥ | 부분 구현 | 6종 구조 AST·MathML·직접 입력·Ctrl+=/9/0와 Normal 문자 분할/교체·Scope를 연결. 연속 <=/>=/<>와 일반 = 평가를 구분. 전체 formatted/오류/문맥 전이는 미완료. | [H057](#h057) |
| EA-038 | 논리 AND·OR·XOR·NOT | 부분 구현 | 논리 AST·MathML·Ctrl+Shift+1/2/5/7·Normal 문자 분할/교체·Scope 방향을 연결. NOT의 prefix/인접 Scale와 부호 alternate를 구분. 전체 formatted/타입/오류 전이는 미완료. | [H106](#h106) |
| EA-039 | 집합 소속 연산자 ∈ | 미구현 | 집합/소속 연산자 편집·평가 없음 | [H095](#h095) |
| EA-040 | 범위 및 간격 지정 범위 | 부분 구현 | Range/StepRange와 계산 연결, 원본 범위 규칙 전체 미검증 | [H125](#h125) |
| EA-041 | 행렬 아래첨자·다중 인덱스 | 부분 구현 | Index 조회/정의·1/2 index·범위 정의·행벡터 대입/확장·ORIGIN·단위/복소수·Undo/MathML 연결. 문자열 원소·자기참조 범위 recurrence·일반 행벡터 단일-index 조회·fill metadata 없는 혼합 단위 확장은 미완료. | [H091](#h091) |
| EA-042 | 행·열 추출 연산자 | 부분 구현 | 전용 Row/Column AST·Ctrl+Shift+R/C·원본 우상첨자 각괄호·matrow/matcol·ORIGIN/단위/복소수 조회와 일반/전역/프로그램 named-base 좌변 대입·확장·실패 전파·Undo/native 저장 연결. range-valued slice index·전체 선택/viewport 전이·상위 typing 전체 일치는 미완료. | [H129](#h129) |
| EA-043 | 전치 윗첨자 연산자 | 부분 구현 | 전용 Transpose AST·Ctrl+Shift+T·원본 MathML·우상첨자·금지 자식 grouping·비켤레 행열 교환 연결. 전체 원본 편집/상징 전이는 미완료. | [H201](#h201) |
| EA-044 | 행렬식 세로줄 연산자 | 부분 구현 | 원본 절댓값/행렬식 명령을 Absolute/absval 단선으로 연결. scalar magnitude·열벡터 norm·정방 determinant 분기와 저장/편집 검사, 전체 물리 단위와 원본 오류 전이는 미완료. | [H070](#h070) |
| EA-045 | 행렬 노름 이중 세로줄 연산자 | 부분 구현 | 원본 Norm 명령은 Determinant/determinant 이중선이며 새 norm 태그를 만들지 않음. 리본·Ctrl+Shift+&#124;·계산·저장 연결. op_det/op_det2 최상위 물리 타입 lowering은 미확정. | [H111](#h111) |
| EA-046 | 벡터 외적 연산자 | 부분 구현 | 전용 CrossProduct AST·Ctrl+8·⨯·오른쪽 결합·native 저장 및 3×1 복소/단위 벡터의 비켤레 외적 계산 연결. 전체 원본 기호 변환·편집 전이·nested 값은 미완료. | [H209](#h209) |
| EA-047 | 벡터화 화살표 연산자 | 부분 구현 | 전용 Vectorize AST·Ctrl+Shift+^·윗화살표·native 저장 및 수식 안 연산자/함수의 원소별 계산 연결. 같은 shape·scalar broadcast·열 우선 오류/취소를 검사한다. nested 배열·프로그램/미적분/상징 및 모든 원본 타입은 미완료. | [H210](#h210) |

### 미적분 저작 — 8행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| EA-048 | 정적분 템플릿 | 부분 구현 | Integral 템플릿과 유한구간 계산 연결, 무한구간/원본 알고리즘 전체 미지원 | [H093](#h093) |
| EA-049 | 미분·고계 미분 템플릿 | 부분 구현 | Derivative 템플릿과 제한된 수치·다항식 계산 연결 | [H067](#h067) |
| EA-050 | 프라임 미분 표기 | 미구현 | Derivative 노드와 별개인 프라임 전용 저작 노드 없음 | [H067](#h067) |
| EA-051 | 편미분 템플릿 | 부분 구현 | partial 플래그 템플릿 존재, 일반 다변수 CAS 미지원 | [H067](#h067) |
| EA-052 | 합계 Σ 템플릿 | 부분 구현 | Summation 편집·계산 연결, 원본 전체 범위/기호 의미 미지원 | [H139](#h139) |
| EA-053 | 곱계 Π 템플릿 | 부분 구현 | Product 편집·계산 연결, 원본 전체 범위/기호 의미 미지원 | [H121](#h121) |
| EA-054 | 극한 및 좌극한·우극한 표기 | 부분 구현 | 다항식 유한 극한 계산 일부, 직접 저작 템플릿 및 일반 극한 없음 | [H100](#h100) |
| EA-055 | 기울기 ∇ 템플릿 | 미구현 | 전용 Gradient 편집 노드 없음 | [H086](#h086) |

### 행렬 저작 — 11행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| EA-056 | 행·열 수를 지정해 행렬 삽입 | 부분 구현 | Ctrl+M·빈 [·리본 무인자 삽입은 원본 1×1, 선택값 wrapping·인접 Scale과 실제 2×2 작성/Enter/Undo/native 연결. 원본 크기 선택 UI·전체 formatted 부분 선택·navigator/Typeover는 미완료. | [H107](#h107) |
| EA-057 | 현재 행 앞·뒤에 행 삽입 | 부분 구현 | 방향 지정 삽입과 Shift+Enter 지원, 원본 키 경계 비교 미완료 source displaySome의 제한된 삽입·삭제와 전체 셀/표시 범위 보존을 연결. 숨은 최종 목표와 복합식 삭제 진입은 원자적 no-op으로 남는다. CHECKPOINT-2026-10-20 참조. | [H169](#h169) |
| EA-058 | 현재 열 앞·뒤에 열 삽입 | 부분 구현 | 방향 지정 삽입과 Shift+Space 지원, 원본 키 경계 비교 미완료 source displaySome의 제한된 삽입·삭제와 전체 셀/표시 범위 보존을 연결. 숨은 최종 목표와 복합식 삭제 진입은 원자적 no-op으로 남는다. CHECKPOINT-2026-10-20 참조. | [H169](#h169) |
| EA-059 | 선택 행·열 삭제 | 부분 구현 | 현재 셀·직사각형 범위가 걸친 행/열 리본 삭제와 Delete/Backspace의 축별 조건·1×1 초기화 연결. 원본 모든 선택/커서 전이는 미완료. source displaySome의 제한된 삽입·삭제와 전체 셀/표시 범위 보존을 연결. 숨은 최종 목표와 복합식 삭제 진입은 원자적 no-op으로 남는다. CHECKPOINT-2026-10-20 참조. | [H169](#h169) |
| EA-060 | 선택 셀 내용 지우기 | 부분 구현 | 일반 행렬과 Spec Table의 범위/활성 셀 Clear Cells 연결. 크기·서식·선택 해제·단일 Undo·저장 왕복 검사. 전체 원본 전이는 미완료. | [H169](#h169) |
| EA-061 | 행렬 원소 사이 Tab·Shift+Tab 이동 | 부분 구현 | 기본 행렬의 행 우선 Tab/Shift+Tab·마지막 셀 새 행, source displaySome의 전체 주소/경계 pan·제한된 마지막 행 growth와 full12→13 생성 경로를 연결. 숨은 최종 목표·전체 중첩/Typeover 경계는 미완료. CHECKPOINT-2026-10-20/0207 참조. | [H097](#h097) |
| EA-062 | 행렬 마지막 원소 Tab으로 새 행 추가 | 부분 구현 | 마지막 셀 Tab이 빈 행 하나를 추가하고 첫 새 셀로 이동, 단일 Undo 검사. 전체 원본 행렬 선택/경계는 미완료. source displaySome의 제한된 삽입·삭제와 전체 셀/표시 범위 보존을 연결. 숨은 최종 목표와 복합식 삭제 진입은 원자적 no-op으로 남는다. CHECKPOINT-2026-10-20 참조. | [H097](#h097) |
| EA-063 | 행렬 위·아래 같은 열 이동 | 부분 구현 | 기본 좌표 이동과 source displaySome의 전체 행 주소·숨은 행 reveal·셀 내부 fraction 이동 우선·동일 배치의 연속 방향키를 연결. 전체 원본 navigator/복합 노드 상태는 미검증. 0207 참조. | [H097](#h097) |
| EA-064 | 빈 셀 Ctrl+좌우로 행렬 경계 건너뛰기 | 미구현 | 일반 항 단위 이동만 존재, 원본 빈 행렬 원소 예외 미구현 | [H097](#h097) |
| EA-065 | 큰 행렬 표시 행·열 크기 조절 | 부분 구현 | flat root 결과의 괄호·navigator 이동/내부8방향 resize·개별 서식 clone·단일 Undo·own/native/vector PDF, 입력 행렬의 per-node display 보존·표시 셀 조판·방향키/Tab source pan·실제 셀 수정/Enter/Undo/저장/벡터 인쇄와 제한된 shape 편집·부분 복사를 연결. source 직접 resize/drag·숨은 목표/복합식 삭제 진입·nested/Area 전체 형식은 미완료. CHECKPOINT-2026-10-20/0207 참조. | [H193](#h193) |
| EA-066 | 큰 행렬 인덱스 표시·숨기기 | 부분 구현 | 실제로 계산된 root Evaluate의 완전한 flat 행렬 결과·결과별 SYSTEM ORIGIN·리본 표시/숨기기·focus 임시 인덱스·단일 Undo/Redo·첫 개별 서식의 확인된 요청 크기/위치 clone·own/native 저장·벡터 PDF. 입력 행렬 안 커서·Pending·unknown cold ORIGIN은 이 명령 대상에서 제외. source/nested/Area·native cold ORIGIN 복원·원본 전체 print 계층은 미완료. 0205 참조. | [H166](#h166) |

### 프로그램 저작 — 14행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| EA-067 | 프로그램 블록 삽입 | 부분 구현 | 1개 문장 프로그램 템플릿과 브리지 지원 | [H122](#h122) |
| EA-068 | Enter로 프로그램 새 줄 추가 | 부분 구현 | 프로그램 문장 앞/뒤 Enter 줄 삽입과 중첩 loop 경계·Portable→MathML→계산 연결. 전체 삭제/unwrap·다중 분기 정책은 미완료. | [H122](#h122) |
| EA-069 | 프로그램 안에 하위 프로그램 중첩 | 부분 구현 | 중첩 AST 가능, 원본 줄 추가/블록 편집 정책 미완료 | [H122](#h122) |
| EA-070 | 로컬 대입 ← | 부분 구현 | LocalDefine의 이름·named Index/Row/Column·Function 좌변과 원본 { 입력·단일 Undo·native 저장·프로그램 scope 계산 연결. 함수 parameter/앞선 local capture 지원. 실제 캡처 변경 후 호출·함수값 alias/배열·고차 함수·range slice·전체 원본 프로그램 편집은 미완료. | [H103](#h103) |
| EA-071 | if·else 분기 저작 | 부분 구현 | 원본 flat IF·다중 test/body·terminal else·Ctrl+J·빈 자리와 Scope 본문 보존·명시 전체 값 문자 선택의 빈 구조 교체·Undo·native/세로 조판 지원. 일반 partial/인접식 분할·병합·전체 navigator/read-only는 미완료. | [H060](#h060) |
| EA-072 | elseif 분기 저작 | 부분 구현 | flat IF의 elseif 분기·원본 첫 일치 계산·native 저장·확인된 IF 오른쪽/인접 빈 문장 삽입·인접 단어 Ctrl+J·한 Undo 지원. 일반 위치 split/merge와 전체 전이는 미완료. | [H060](#h060) |
| EA-073 | alsoif 독립 분기 저작 | 부분 구현 | IF 전체 alternate flag·alsoif의 모든 일치 계산·native 저장·분기 삽입·인접 단어 Ctrl+J·middle operator 선택 모드 전환 지원. 혼합 native 분기는 원본 reader처럼 정규화, raw 계산 XML은 기존 strict 경계. 일반 repair 미완료. | [H060](#h060) |
| EA-074 | for 반복 블록 저작 | 부분 구현 | For 템플릿·Ctrl+J·원본 Scope의 본문 child2 보존·기존 Program 재사용·전체 값 문자 선택 교체·단일 Undo와 반복 계산/native 왕복 지원. 기존 수식을 무조건 iterable로 감싸던 경로를 교정. 일반 text split/implicit parser와 전체 navigator 미완료. | [H083](#h083) |
| EA-075 | while 반복 블록 저작 | 부분 구현 | While 템플릿·Ctrl+J·원본 Scope의 본문 child1 보존·기존 Program 재사용·전체 값 문자 선택 교체·단일 Undo와 반복 계산/native 왕복 지원. 기존 수식을 무조건 condition으로 감싸던 경로를 교정. 일반 text split/implicit parser와 전체 navigator 미완료. | [H211](#h211) |
| EA-076 | break 반복 종료 저작 | 부분 구현 | 전용 zero-arity AST·native XML·빈 자리 및 명시 전체 값 문자 선택 리본 교체·Ctrl+J·Right caret·inward 삭제·Undo·루프 종료 계산 연결. 일반 Scope/인접 parser와 전체 navigator/error 전이는 미완료. | [H113](#h113) |
| EA-077 | continue 다음 반복 저작 | 부분 구현 | 전용 zero-arity AST·native XML·빈 자리 및 명시 전체 값 문자 선택 리본 교체·Ctrl+J·Right caret·inward 삭제·Undo·반복 건너뛰기 계산 연결. 일반 Scope/인접 parser와 전체 navigator/error 전이는 미완료. | [H113](#h113) |
| EA-078 | return 반환 저작 | 부분 구현 | 빈 Return·Ctrl+J·Scope Left의 선택 식/단위 보존과 확인한 첫 native caret·Scope Right 거부·전체 값 문자 선택 교체·Undo·프로그램 조기 반환/native 왕복 지원. 일반 text split/implicit parser와 전체 nav-state 조합 미완료. | [H113](#h113) |
| EA-079 | try/on error 예외 분기 저작 | 부분 구현 | 원본 TRY 두 BODY·native tryCatch·Ctrl+J·빈 자리/Scope attempt 보존/전체 값 문자 선택 리본 교체·방향별 placeholder 이동·헤더 삭제/본문 병합·오류 대체·단위/반복 제어 지원. 미지원/상위 실패/호스트 중지·한도는 숨기지 않음. 전체 fault taxonomy/rollback·일반 repair·직접 root 수치 동등성은 미확정. | [H202](#h202) |
| EA-080 | Ctrl+J로 입력 단어를 프로그램 연산자로 변환 | 부분 구현 | Normal 무라벨 단어 Ctrl+J와 인접 Program의 else/elseif/alsoif 변환에 더해 direct Scale(keyword,rhs)의 While/For/If/Try/Return 본문 보존·Program 재사용·caret·한 Undo 지원. 명시적 Multiply는 별도. 일반 partial parser/branch split·merge·전체 mode/repair는 미완료. | [H060](#h060) |

### 기호 저작 — 9행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| EA-081 | 기호 평가 화살표·명령 자리표시자 삽입 | 부분 구현 | SymbolicEvaluation/Command 1개 템플릿, 전체 CAS 미구현 | [H140](#h140) |
| EA-082 | 기호 키워드·인수 직접 입력 | 부분 구현 | 현재 Command 인수 최대1개 제한, 원본 일반 인수목록 미지원 | [H182](#h182) |
| EA-083 | 쉼표로 기호 키워드 여러 개 작성 | 미구현 | 현재 편집 명령목록 추가 경로 없음 | [H182](#h182) |
| EA-084 | Shift+Enter로 기호 키워드 세로 쌓기 | 미구현 | 키워드 줄 추가 전용 입력 경로 없음 | [H182](#h182) |
| EA-085 | 연속 기호 평가 화살표 체이닝 | 부분 구현 | 중첩 기호 AST 표현 가능, 일반 체이닝 저작·의미 미완료 | [H206](#h206) |
| EA-086 | 기호 키워드 표시·숨기기 | 미구현 | Hide/Show Keywords 컨텍스트 명령 없음 | [H182](#h182) |
| EA-087 | 큰 기호결과 오른쪽 핸들로 폭 조절 | 미구현 | 원본 resize handle 방식 미구현, 별도 전체결과 모달은 원본 기능 아님 | [H200](#h200) |
| EA-088 | 기호결과를 함수 정의에 사용 | 부분 구현 | 제한된 다항식 기호 계산만 지원, 일반 기호 함수 정의 미완료 | [H162](#h162) |
| EA-089 | 기호 결과와 수치 결과 연속 평가 | 부분 구현 | 일부 다항식 결과의 계산 경로, 일반 CAS와 저작 정책 미완료 | [H056](#h056) |

### 기호 키워드 — 27행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| EA-090 | assume | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-091 | cauchy | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-092 | coeffs | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-093 | collect | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-094 | combine | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-095 | confrac | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-096 | decomp | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-097 | expand | 부분 구현 | 유리수 계수 다항식 전개 범위만 지원, 일반 함수/수정자 미지원 | [H015](#h015) |
| EA-098 | explicit | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-099 | factor | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-100 | float | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-101 | fully | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-102 | parfrac | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-103 | rectangular | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-104 | rewrite | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-105 | series | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-106 | simplify | 부분 구현 | 유리수 계수 다항식 정리 범위만 지원, 일반 CAS 미지원 | [H015](#h015) |
| EA-107 | solve | 부분 구현 | 2차 이하 다항식 해 범위만 지원, 일반식·매개변수·생성변수 미지원 | [H015](#h015) |
| EA-108 | standard | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-109 | substitute | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-110 | using | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-111 | fourier | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-112 | laplace | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-113 | ztrans | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-114 | invfourier | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-115 | invlaplace | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |
| EA-116 | invztrans | 미구현 | symbolics.rs에 이 키워드의 독립 실행 의미 없음; 메뉴/텍스트 입력이나 원본 보존만으로 구현으로 보지 않음 | [H015](#h015) |

### 기호 수정자 — 48행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| EA-117 | ALL | 미구현 | 적용 키워드: assume, explicit; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-118 | acos | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-119 | acot | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-120 | arg | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-121 | asin | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-122 | atan | 미구현 | 적용 키워드: rewrite, combine; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-123 | atan2 | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-124 | cauchy | 미구현 | 적용 키워드: 기호 적분 연산자; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-125 | coeff | 미구현 | 적용 키워드: decomp; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-126 | complex | 미구현 | 적용 키워드: assume, factor, parfrac; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-127 | cos | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-128 | cosh | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-129 | cot | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-130 | coth | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-131 | degree | 미구현 | 적용 키워드: coeffs; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-132 | domain | 미구현 | 적용 키워드: factor, parfrac; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-133 | even | 미구현 | 적용 키워드: assume; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-134 | exp | 미구현 | 적용 키워드: combine, rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-135 | expr | 미구현 | 적용 키워드: decomp; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-136 | fact | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-137 | fraction | 미구현 | 적용 키워드: confrac; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-138 | fully | 미구현 | 적용 키워드: solve, substitute, series, simplify, fourier, rewrite, rectangular, parfrac; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-139 | func | 미구현 | 적용 키워드: decomp; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-140 | gamma | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-141 | generated | 미구현 | 적용 키워드: solve; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-142 | integer | 미구현 | 적용 키워드: assume; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-143 | ln | 미구현 | 적용 키워드: combine, rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-144 | log | 미구현 | 적용 키워드: combine, rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-145 | matrix | 미구현 | 적용 키워드: confrac; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-146 | max | 미구현 | 적용 키워드: fully, simplify, decomp; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-147 | nfact | 미구현 | 적용 키워드: fourier; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-148 | odd | 미구현 | 적용 키워드: assume; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-149 | ofact | 미구현 | 적용 키워드: fourier; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-150 | parts | 미구현 | 적용 키워드: decomp; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-151 | power | 미구현 | 적용 키워드: decomp; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-152 | raw | 미구현 | 적용 키워드: fourier, invfourier, laplace, invlaplace, rewrite, ztrans, invztrans, substitute; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-153 | real | 미구현 | 적용 키워드: assume, factor, solve; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-154 | RealRange | 미구현 | 적용 키워드: assume; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-155 | sign | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-156 | signum | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-157 | sin | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-158 | sincos | 미구현 | 적용 키워드: combine, rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-159 | sinh | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-160 | sinhcosh | 미구현 | 적용 키워드: combine, rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-161 | tan | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-162 | tanh | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-163 | using | 미구현 | 적용 키워드: solve; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |
| EA-164 | Φ(Heaviside) | 미구현 | 적용 키워드: rewrite; 현재 CAS에는 해당 수정자 해석·실행 경로 없음 | [H018](#h018) |

### 문자열 — 2행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| EA-165 | 문자열 수식 입력·내부 편집·빈 문자열 | 부분 구현 | quote gesture·내부 기호/공백/ASCII quote의 문자 입력·UTF16 선택·삭제/빈 String·외곽 이동·Undo·Enter·native/own/cold/vector 연결. BMP authoring·16KiB 독립 atom 예산. 전체 clipboard/1024 제한 guard/문자 category 버전·surrogate 반쪽·외곽 삭제 미완료. docs/analysis/0184,0185 참조. r201: 첫 plot math의 String source·숫자 축 타입 오류·큰 String inert 값/Sync 보존을 일반/Area 저장 왕복에 연결. 저장 presentation 예산과 live/JSON 표시 경로는 별도. docs/analysis/0186 참조. | [H216](#h216) · [H138](#h138) |
| HD220 | 문자열 인덱스의 ORIGIN 설정을 문서 계산에 반영 | 부분 구현 | native 고유 ContentType Override·boolean/default·UTF16/비표준 part 경로를 읽고 SHA-bound 준비 cache·direct/worker·own/native 저장에 연결. 설정불명은 origin-sensitive 함수만 unsupported. 해당 설정의 원본 UI 저작·전체 문자열 함수/오류·collation 미완료. docs/analysis/0184,0185 참조. | [H138](#h138) |

### 영역·계산 — 9행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD001 | 자동 계산 토글 | 부분 구현 | 자동/수동·Pending·worker 연결. 일반 타이핑은 초안이며 Enter/영역 이탈 때 확정. Auto ON은 확정 입력만 재개하고 Auto OFF는 강제 Stop과 구분. 원본 정밀 queue는 미완료. | [H007](#h007) |
| HD002 | 진행 중인 모든 계산 중지 | 부분 구현 | 계산 중 Stop이 MANUAL 전환·협력 취소하고 완료 결과/중단행 이전 값을 보존. 긴 프로그램·수치·행렬·solve 반복 검사. 단일 파싱/native 블로킹의 즉시 강제 중단은 미지원. | [H007](#h007) |
| HD003 | 중지 뒤 마지막 미계산 영역부터 재개 | 부분 구현 | Auto ON으로 완료 prefix/global을 재사용하며 미계산 구간 재개, 수동 F5/Ctrl+F5 범위 유지. 원본 위치별 queue·정밀 의존성/게시 시점은 미완료. | [H007](#h007) |
| HD004 | 워크시트 전체 강제 재계산 | 부분 구현 | Ctrl+F5가 활성 편집을 반영하고 비활성 영역을 제외한 전체를 worker에서 강제 계산. 텍스트 편집으로 명시 계산을 교체하지 않음. 외부 파일/난수/컨트롤·Area 전파·전체 원본 queue는 미완료. | [H007](#h007) |
| HD005 | 보이는 마지막 영역까지 Calculate Now | 부분 구현 | F5가 보이는 활성 영역을 확정하고 가시 Pending 구간만 처리, 깨끗한 binding 재사용. 화면 밖 초안과 경계 밖 결과 유지·평면 비활성 제외. 정밀 의존성·Area는 미완료. | [H007](#h007) |
| HD006 | 활성 영역의 Calculate Now 강제 갱신 | 부분 구현 | 보이는 활성 영역 강제 갱신과 가시 dirty 구간/후속 도표를 worker에서 계산. 화면 밖 초안·비활성 영역 제외. 일반 입력과 명시 = 분리. 정밀 전파·Area·전체 queue 미완료. | [H007](#h007) |
| HD007 | 개별 영역 비활성화 | 부분 구현 | 평면 수식/도표 비활성 토글·dim·Undo·저장/열기·native disable-calc 보존. 모든 계산 명령에서 제외. Area 상위 전파·전체 원본 영역은 미완료. | [H007](#h007) |
| HD008 | 비활성 영역 편집 시 후속 계산 배제 | 부분 구현 | 비활성 원본 편집은 후속 계산/도표를 실행하지 않으며 이전 표시를 Pending으로 유지. cold engine·재활성화 검사. 정밀 의존성·Area는 미완료. | [H007](#h007) |
| HD009 | 문서 열기 시 캐시 결과 유지와 명시 재계산 | 부분 구현 | MCDX literal·자체 저장 표현을 계산 없이 복원. source/context 동일한 전체 순서에서 원본 raw 결과/진단 유지, 원문 변경은 전체 Pending+이전 payload 보존. current scalar/complex/dimensionless matrix·명시 단위변환 display값을 native 저장/재열기. 새 단위 monomial/structured diagnostics/range/symbolic·전체 결과 형식은 미완료. 현재 native Ctrl+S·명시 Save As에서도 loaded baseline과 기존 source/context/cache proof를 유지하며 반복 저장·own/native 전환을 지원. 알려진 Current native target의 regular-file readonly 속성에는 준비 전 질문·명시 Save As와 취소 원자성을 연결. ACL/lock·임의 오류 재시도·실제 modal focus/IME·전체 native 구조와 설치 Prime 재열기는 미완료/미검증. | [H007](#h007) |

### 문서·영역 — 24행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD010 | 여러 워크시트 탭 동시 열기 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H029](#h029) |
| HD011 | 현재 워크시트 탭 닫기 Ctrl+W | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H029](#h029) |
| HD012 | Ctrl+클릭 영역 다중 선택 | 미구현 | 앱의 selected는 단일 영역 경로. 수식 내부 선택과 별개 | [H131](#h131) |
| HD013 | 드래그 사각형으로 영역 다중 선택 | 구현됨 | 사용자 지정 CAD 규칙: 왼쪽→오른쪽 완전 포함, 오른쪽→왼쪽 교차. 페이지 여백·페이지 사이·종이 밖에서도 시작 가능. 수식 내부 선택과 별개. 원본은 양방향 교차이며 이 차이는 명시적 요청에 따른다. [검증](analysis/0217-rc1-workspace-selection.md) | [H131](#h131) |
| HD014 | Shift+클릭 구간 영역 선택 | 미구현 | 드래그 다중 선택은 있지만 Shift+클릭 구간 선택은 연결하지 않음 | [H131](#h131) |
| HD015 | Ctrl+A 모든 영역 선택 | 미구현 | 드래그 다중 선택은 있지만 Ctrl+A 전체 영역 선택은 연결하지 않음 | [H131](#h131) |
| HD016 | 영역 자유 위치 이동 | 부분 구현 | 개별·선택 묶음 drag, 상대 위치 유지·drop 1회 Undo·확장 페이지 이동 연결. 원본 전체 보호/겹침 계약은 미완료. [검증](analysis/0216-cad-region-selection.md) | [H188](#h188) |
| HD017 | 영역 복사·잘라내기·붙여넣기 | 부분 구현 | 내부 단일·다중 영역, 새 ID·상대 위치·일괄 검증·Undo 연결. 원본 OS 포맷과 읽기 전용 opaque 영역 복사는 미완료 | [H153](#h153) |
| HD018 | 선택 영역 삭제 | 부분 구현 | 단일·선택 묶음 삭제와 1회 Undo/Redo 연결. 원본 전체 보호/접이식 영역 계약은 미완료 | [H163](#h163) |
| HD019 | 한 줄 공간 삽입 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H145](#h145) |
| HD020 | 한 페이지 공간 삽입 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H145](#h145) |
| HD021 | 한 줄 공간 제거 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H145](#h145) |
| HD022 | 남은 빈 공간 제거 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H145](#h145) |
| HD023 | 겹친 영역 세로 분리 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H145](#h145) |
| HD024 | 겹친 영역 가로 분리 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H145](#h145) |
| HD025 | 명시 페이지 나눔 삽입 | 부분 구현 | 빈 root입력점 Ctrl+Enter·Document Spacing Add Page Break, local Y/X suffix와 후속 나눔 이동·한 Undo·native payload/좌표 저장·print 연결. PageWide 교차/보호/unknown geometry는 원자적 거절. 전체 overlap/다중선택·Area 내부 저작은 미완료. | [H183](#h183) |
| HD026 | 명시 페이지 나눔 삭제 | 부분 구현 | 원본 RemoveSpaceAbove/Below 중 단일 인접 pageBreak 구간을 blank Backspace/Delete·리본으로 제거. live 이전 bottom 기준 당김·한 Undo·native 저장. 일반 공백 제거·복수 pageBreak 제거와 전체 overlap은 미완료. | [H183](#h183) |
| HD027 | 영역 테두리 표시·숨김 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H142](#h142) |
| HD028 | 중첩된 개별 영역 테두리 표시·숨김 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H142](#h142) |
| HD029 | 텍스트 웹 링크 삽입·편집·제거 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H014](#h014) |
| HD030 | 텍스트 파일 링크 삽입·편집·제거 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H014](#h014) |
| HD031 | 영역 태그 지정 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H014](#h014) |
| HD032 | 문서 내 #태그 링크 이동 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H014](#h014) |
| HD033 | 다른 MCDX의 #태그 링크 이동 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H014](#h014) |

### 텍스트·그림 — 29행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD034 | 페이지 폭 Text Block과 후속 영역 자동 이동 | 미구현 | 현재 plain text 자유 영역과 별개인 원본 full-width reflow 없음 | [H024](#h024) |
| HD035 | 자유 위치 Text Box 입력 | 부분 구현 | 기존 authored/imported Text source·IME·서식·own/native 저장을 유지하며 같은 문단의 Run/Paragraph 경계·전체 범위를 포함한 선택 삭제를 연결. 실제 Ctrl+A 뒤 ordinary Text는 한 비어 있지 않은 direct-Run Paragraph에서 첫 nonempty Run의 current source를 보존하는 별도 경로이며 authored/native에 적용한다. native Unknown 밑줄 값을 None으로 만들지 않는다. 실제 authored full selected Backspace 뒤에는 원문/서식 검증된 제한적 ordinary Text 후속 owner를 연결한다. 한 LTR 문단 정렬·Undo/cold source와 authored canonical Left 복원은 기존 범위를 유지한다. 선언된 source-language-preservation profile이며 실제 OS 입력 culture 동등성은 미검증. 임의 선택 치환·여러 문단·Paste/IME 대체·newline/병합·LineBreak·임의 typing affinity·전체 Text Block reflow·원본 serializer Run 병합 동일성은 미완료. native의 이전 서식/분할 뒤 같은 원본 평문 대체가 original raw-body 증명과 기존 format-only 증명을 모두 충족하지 못하면 게시 전에 원자적으로 거절한다. | [H181](#h181) |
| HD036 | 텍스트 내부 수식 삽입 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H181](#h181) |
| HD037 | 텍스트 글꼴 이름 선택 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H181](#h181) |
| HD038 | 텍스트 글꼴 크기 선택 | 부분 구현 | 지원 imported/authored의 비어 있지 않은 같은 문단 선택에 1–1638 point·소수 한 자리 크기 명령을 확정 시 적용하고 DIP source·Undo·own/native/PDF에 보존. authored 새 owner는 명시 현재 서식 seed를 사용하며 원본 새 빈 serializer와 동일하다는 뜻은 아님. collapsed word/spring-loaded·전체 inherited catalog/문맥은 미완료. | [H181](#h181) |
| HD039 | 텍스트 색상 설정 | 부분 구현 | 지원 imported/authored 선택의 foreground RGBA와 ARGB source·기존 배경/style metadata 보존·Undo·own/native/vector 연결. imported alpha 검사는 기존 범위이며 새 authored packet PDF는 불투명 RGB를 scalar별 source와 대조. 전체 원본 picker/배경 변경·상속 문맥은 미완료. | [H181](#h181) |
| HD040 | 텍스트 굵게·기울임·밑줄 | 부분 구현 | 비어 있지 않은 지원 선택의 B/I/U·크기·색상을 유지. imported missing decoration의 명시 U toolbar None/Underline assignment와 Unknown 조회·예약 Ctrl+U toggle 구분은 그대로다. 실제 Ctrl+A ordinary Text는 첫 nonempty Run wrapper와 current source format을 보존하며 Unknown을 None으로 추정하지 않는다. authored selected Backspace의 제한적 후속 ordinary Text는 캡처한 Run 서식을 보존한다. 일반 collapsed/spring-loaded affinity·다른 decoration collections·임의 선택 전이·원본 새 빈 serializer 동일성은 미완료. | [H181](#h181) |
| HD041 | 텍스트 위첨자·아래첨자 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H181](#h181) |
| HD042 | 문단 들여쓰기 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H181](#h181) |
| HD043 | 문단 글머리 기호 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H181](#h181) |
| HD044 | 텍스트 스타일 적용 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H181](#h181) |
| HD045 | 텍스트 스타일 수정·삭제 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H181](#h181) |
| HD046 | 붙여넣은 외부 Rich Text 스타일 유지 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H181](#h181) |
| HD047 | 새 텍스트 기본 서식 지정 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H181](#h181) |
| HD048 | 텍스트 서식 초기화 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H181](#h181) |
| HD049 | 사용자 텍스트 스타일 생성 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H064](#h064) |
| HD050 | 기존 스타일 기반으로 스타일 상속 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H064](#h064) |
| HD051 | 스타일 문단 정렬·들여쓰기·글머리 설정 | 부분 구현 | 한 admitted LTR Paragraph의 Left/Center/Right/Justify 명시 리본·정확 Ctrl+L/E/R/J를 source-bound context/FIFO·collapsed caret·Undo/Redo·own/native cold에 연결. cached TextEdit/LayoutJob 정렬과 실제 pointer 회귀를 확인. 들여쓰기·글머리·전체 style catalog/global formatting·여러 문단·mapped provenance·RTL·일반 margin/LineHeight geometry는 미완료. 새 독립 alignment PDF 상태나 원본 WPF pixel/runtime parity는 주장하지 않음. | [H064](#h064)·[H181](#h181) |
| HD052 | 텍스트 영역 검색·치환 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H128](#h128) |
| HD053 | 수식의 숫자·식별자 검색·치환 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H128](#h128) |
| HD054 | 배열 셀·인덱스·문자 아래첨자 부분문자열 검색·치환 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H128](#h128) |
| HD055 | 파일에서 그림 삽입 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H180](#h180) |
| HD056 | 클립보드 그림 붙여넣기 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H180](#h180) |
| HD057 | 그림 위치 이동·크기 변경 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H180](#h180) |
| HD058 | 맞춤법 검사 토글·밑줄 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H021](#h021) |
| HD059 | 맞춤법 교정 제안 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H021](#h021) |
| HD060 | 현재 텍스트 영역에서 단어 무시 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H021](#h021) |
| HD061 | 사용자 사전에 단어 추가 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H021](#h021) |
| HD062 | 텍스트 교정 언어 선택 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H026](#h026) |

### 페이지·보기 — 14행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD063 | 용지 크기 선택 Letter/Legal/Ledger/A1/A2/A3/A4/A5 | 부분 구현 | 8종 용지 선택과 자체 저장·native 읽기/export·인쇄 물리 크기 연결. 전체 원본 페이지 전이·장치 출력은 미검증. | [H161](#h161) |
| HD064 | 세로·가로 페이지 방향 설정 | 부분 구현 | 세로/가로 설정·자체 저장·native export·출력 방향 연결. 원본 전체 pagination/드라이버 실기는 미검증. | [H161](#h161) |
| HD065 | 표준·좁게·넓게·Legacy 여백 선택 | 부분 구현 | 표준/좁게/넓게/Legacy 프리셋 및 native 읽기. 저장/export는 명시 DIP 여백으로 보존. 전체 원본 세션/페이지 배치 정책 미완료. | [H161](#h161) |
| HD066 | 상하좌우 사용자 여백 개별 설정 | 부분 구현 | 상하좌우 mm 편집·검증·자체 저장·native Custom Thickness export 및 1/2/4 숫자 읽기. 단위 접미사 전체 규칙은 미완료. | [H161](#h161) |
| HD067 | 최근 사용자 여백 세션 설정 재사용 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H161](#h161) |
| HD068 | 워크시트 눈금선 표시 토글 | 부분 구현 | 본문 grid 토글과 인쇄 반영, 주 눈금선 연결. 별도 머리글 눈금·모든 프레임 설정은 미완료. | [H161](#h161) |
| HD069 | 표준·세밀 눈금 크기 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H161](#h161) |
| HD070 | 주 눈금선 표시 토글 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H161](#h161) |
| HD071 | 본문·머리글·바닥글 프레임 표시 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H161](#h161) |
| HD072 | 워크시트 배경 Paper Color/Gray/White 선택 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H161](#h161) |
| HD073 | Page 보기 | 부분 구현 | Page/Draft, 초과영역 계산·토글 구현. 원본 전체 조작/출력/실기 비교 미완료 | [H161](#h161) |
| HD074 | Draft 보기 | 부분 구현 | Page/Draft, 초과영역 계산·토글 구현. 원본 전체 조작/출력/실기 비교 미완료 | [H161](#h161) |
| HD075 | 오른쪽 초과 영역이 있을 때 Draft 전환 버튼 | 부분 구현 | Page/Draft, 초과영역 계산·토글 구현. 원본 전체 조작/출력/실기 비교 미완료 | [H161](#h161) |
| HD076 | 문서 확대·축소 | 부분 구현 | 배율/좌표 역변환과 자체 도표 입력·x/y 행 라벨·진단·축 제목·인쇄 영역 밖 안내의 최종 glyph 밀도를 연결. 실제 workspace OS PPP1/1.5/2×문서 배율1/1.25/2 회귀. 의존성 눈금·범례·hover, 원본 slider·픽셀 동일성과 실제 GPU/OS 실기 비교는 미완료. | [H161](#h161) |

### 머리글·바닥글 — 15행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD077 | 머리글·바닥글 텍스트 편집 | 부분 구현 | 문자열/좌표 저장·출력 있음. 원본 직접 편집 UX와 rich text 미완료 | [H159](#h159) |
| HD078 | 머리글·바닥글 수식·표·그림 삽입 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H205](#h205) |
| HD079 | 머리글·바닥글 Excel 컴포넌트 삽입 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H205](#h205) |
| HD080 | 본문 영역을 머리글·바닥글로 복사 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H205](#h205) |
| HD081 | 머리글과 바닥글 사이 영역 드래그 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H205](#h205) |
| HD082 | PageUp/PageDown 머리글·바닥글 이동 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H205](#h205) |
| HD083 | 머리글·바닥글 프레임·눈금 토글 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H205](#h205) |
| HD084 | 페이지 번호 필드 삽입·제거 | 부분 구현 | 일부 필드/번호/offset 저장과 화면 표시 있음. 모든 원본 포맷·경계·새 MCDX 재열기 미검증 | [H159](#h159) |
| HD085 | 파일명·파일경로 필드 토글 | 부분 구현 | 일부 필드/번호/offset 저장과 화면 표시 있음. 모든 원본 포맷·경계·새 MCDX 재열기 미검증 | [H159](#h159) |
| HD086 | 페이지 시작 번호 지정 | 부분 구현 | 일부 필드/번호/offset 저장과 화면 표시 있음. 모든 원본 포맷·경계·새 MCDX 재열기 미검증 | [H159](#h159) |
| HD087 | 전체 페이지수 사용자 지정 | 부분 구현 | 일부 필드/번호/offset 저장과 화면 표시 있음. 모든 원본 포맷·경계·새 MCDX 재열기 미검증 | [H159](#h159) |
| HD088 | 저장 날짜 필드·포맷 선택 | 부분 구현 | 열기/저장 시 캐시한 실제 로컬 저장 날짜를 화면·인쇄 7형식으로 표시. 새 문서는 원본 null→Now. 자정 경과·모든 문화권은 미검증. | [H159](#h159) |
| HD089 | 시스템 긴 날짜·짧은 날짜 표시 | 부분 구현 | Windows OS 지역 짧은/긴 날짜, Linux locale 날짜 연결. Prime UI 문화권과 OS 설정 차이·Linux 긴 날짜 순서·비Gregorian 달력은 미완료. | [H159](#h159) |
| HD090 | 첫 페이지 별도 머리글·바닥글 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H159](#h159) |
| HD091 | 페이지 번호 표시 시작·종료 페이지 지정 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H213](#h213) |

### 출력·외부 문서 — 10행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD092 | 프린터로 워크시트 인쇄 | 부분 구현 | Ctrl+P·프린터 선택·매수·페이지 조판 및 Windows PrintDocument/Linux CUPS 전송 경로. 실제 대기열/장치·전체 native 영역 fidelity 미검증. | [H189](#h189) |
| HD093 | PDF 프린터 출력 | 부분 구현 | 경로·검색/선택 텍스트·부분 글꼴 PDF와 물리 크기·선택 범위 연결. Windows/CUPS 전송 경로, OS PDF 프린터/실물 출력·전체 원본 조판은 미검증. | [H189](#h189) |
| HD094 | XPS 프린터 출력 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H189](#h189) |
| HD095 | 모든 페이지 인쇄 | 부분 구현 | 본문 페이지 전체를 원래 크기로 조판·출력, Draft 오른쪽 제외. 전체 원본 영역·명시 페이지 나눔은 미완료. | [H189](#h189) |
| HD096 | 현재 페이지 인쇄 | 부분 구현 | 현재 보이는 페이지 인덱스와 출력 범위 연결. 복잡한 페이지 나눔·드라이버 실기는 미검증. | [H189](#h189) |
| HD097 | 페이지 범위 지정 인쇄 | 부분 구현 | 단일 번호/연속 1-3 범위 검증·출력, 쉼표 목록 거부. 프린터별 실제 출력은 미검증. | [H189](#h189) |
| HD098 | 선택 영역이 있는 페이지 인쇄 | 부분 구현 | 선택 영역이 걸친 페이지 전체를 원본 순서/번호로 출력. 현재 단일 영역 선택 범위이며 다중 영역·Area는 미완료. | [H189](#h189) |
| HD099 | 단일 워크시트 영역을 Word에 복사 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H009](#h009) |
| HD100 | 단일 워크시트 페이지를 Word에 복사 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H010](#h010) |
| HD101 | 여러 워크시트 페이지를 Word에 복사 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H008](#h008) |

### 템플릿 — 10행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD102 | MCTX 템플릿으로 새 워크시트 생성 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H023](#h023) |
| HD103 | 워크시트를 MCTX 템플릿으로 저장 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H023](#h023) |
| HD104 | 개인 My Templates 목록 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H023](#h023) |
| HD105 | 공유 Shared Templates 목록 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H023](#h023) |
| HD106 | 기본 Default Templates 목록 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H023](#h023) |
| HD107 | 최근 사용한 템플릿 목록 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H199](#h199) |
| HD108 | My Templates 폴더 변경 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H194](#h194) |
| HD109 | Shared Templates 폴더 변경 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H194](#h194) |
| HD110 | 새 워크시트 기본 템플릿 변경 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H194](#h194) |
| HD111 | 최근 워크시트 목록에 파일 고정 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H186](#h186) |

### 접이식 영역 — 10행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD112 | 빈 Area 작성·내부 영역 추가 | 부분 구현 | Ctrl+Shift+A·Document/Regions/Area가 빈 Area를 삽입하고, 내부 첫 Text/IME 확정에서 실제 자식 PH+draft를 생성. Enter는 확정 후 내부 다음 입력점으로 이동하며 source-only commit의 빈 Undo를 추가하지 않음. 새 owner/자식의 native ID/resultRef 할당과 저장 결과·vector PDF 연결. 선택 수식을 감싸는 명령은 원본에 없음. 전체 공간 확장·충돌·다중 child cut/paste·보호는 미완료. | [H002](#h002) |
| HD113 | Area 접기·펼치기와 접힌 내용 인쇄 제외 | 부분 구현 | 지원 Area 접기·펼치기, 계산 유지, 상태 Undo/저장, 접힌 자식·아이콘 vector 인쇄 제외. 보호 영역 조작·전체 원본 기하는 미완료. | [H151](#h151) |
| HD114 | Area 테두리 숨김·인쇄 제외 | 부분 구현 | 가져온 Area 테두리 전환·저장·확장 영역 vector 인쇄 반영. 원본 정확한 선 기하·일반 collapsed border resource 조합은 미확정. | [H088](#h088) |
| HD115 | 접힌 Area 내부 오류의 빨간 테두리 표시 | 부분 구현 | 가져온 Area 자식 오류를 접힌 빨간 선으로 표시하고 hidden border에서도 vector 인쇄. 전체 원본 오류/보호 조합과 정확한 색·기하는 미완료. | [H088](#h088) |
| HD116 | Area 전체 계산 일괄 비활성화 | 부분 구현 | 가져온 지원 Area의 자식 disable-calc 일괄 전환, any-enabled 방향, 계산·Undo·native 저장 연결. parent disable-calc 의미·외부/보호/혼합 영역 전파는 미지원. | [H152](#h152) |
| HD117 | Area 내부 개별 영역 계산 비활성화 | 부분 구현 | 가져온 Area 내부 개별 disable-calc를 계산에서 제외하고 이전 표현을 Pending으로 유지. 개별 자식의 직접 토글 UI·전체 원본 하위 영역은 미완료. | [H152](#h152) |
| HD118 | 암호 없는 Area 보호·해제 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H190](#h190) |
| HD119 | 암호를 사용하는 Area 보호·해제 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H190](#h190) |
| HD120 | Area 보호 타임스탬프 표시 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H190](#h190) |
| HD121 | Area 접힘·펼침 상태 잠금 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H190](#h190) |

### 포함 워크시트 — 9행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD122 | Include Worksheet 삽입 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H191](#h191) |
| HD123 | Include 상대·절대 경로 편집 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H191](#h191) |
| HD124 | Include 위치 이후 정의 전달·재정의 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H191](#h191) |
| HD125 | 포함 워크시트에서 풀이 블록 지역 정의 제외 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H191](#h191) |
| HD126 | 포함 워크시트 계산 설정 충돌 경고 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H191](#h191) |
| HD127 | Include 원본 워크시트 열기 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H191](#h191) |
| HD128 | Include 개별 Refresh로 최신 데이터 갱신 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H191](#h191) |
| HD129 | 포함 워크시트 캐시 켜기·끄기 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H053](#h053) |
| HD130 | 원본 경로 미접속 시 Include 캐시로 계산·경고 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H053](#h053) |

### 표 — 2행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD131 | 열별 이름·단위를 가진 Spec Table 입력 | 부분 구현 | 새 표·셀 편집/확정·행/열·범위 Copy/Cut/Backspace/Delete/Clear 연결. 본문 Copy는 행렬, 헤더/단위 Copy는 새 표. 수식 Paste는 활성 셀 구조 편집이며 임의 격자 분배를 제거했다. 헤더·단위·본문의 표현식 전용 context에서 정의 삽입·Paste를 거절한다. 원본 OS clipboard 형식·모든 셀 전이/조판은 미완료. | [H022](#h022) |
| HD132 | 표의 각 데이터셋을 열벡터 정의로 사용 | 부분 구현 | 열별 단위 scale·여러 값의 열벡터/한 값의 스칼라 정의와 후속 수식·이동/비활성·재계산 연결. 뒤 빈칸 제외·내부 빈칸 오류 처리. 전체 원본 오류/취소 게시 시점과 입력 타입은 미확정. | [H022](#h022) |

### 숫자·수식 서식 — 20행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD133 | General 결과 표시 | 부분 구현 | General·문서 기본값/local override·소스 보존·저장 결과 재서식 연결. 지수/영/복소수 임계값 적용. 전체 radix·round-trip·문서 전체 변경 UI 미완료. | [H176](#h176) |
| HD134 | Decimal 결과 표시 | 부분 구현 | Decimal·후행 0·자릿수·기존 결과 재서식 연결. 계산값과 표시 분리, native/자체 저장. 전체 radix·round-trip·모든 경계 반올림 미완료. | [H176](#h176) |
| HD135 | Scientific 결과 표시 | 부분 구현 | Scientific 및 저장된 signed 3자리 E 표기 연결. native/live/행렬·직렬화 검사. E 표기 전용 UI 근거 미확인·전체 원본 경계 미완료. | [H176](#h176) |
| HD136 | Engineering 결과 표시 | 부분 구현 | Engineering의 3배수 지수·후행 0·저장 E 표기 연결. 반올림 carry 재정규화 없는 관찰 경로 반영. 원본 모든 formatter 소비자 경계 미확인. | [H176](#h176) |
| HD137 | 소수 자릿수 표시 정밀도 지정 | 부분 구현 | 0..15 자릿수·후행 0·원시 결과 보존으로 재계산 없이 갱신. CIL/도움말16 차이 구분. round-trip sentinel 및 전체 속성 상속 UI 미완료. | [H176](#h176) |
| HD138 | Percent 결과 표시 | 부분 구현 | Percent 리본·표시×100/%·native percent 읽기/쓰기·원시 결과 cold 재서식·인쇄 연결. 입력 % 연산자와 별개. 0%의 원본 formatter 경계 미확인. | [H176](#h176) |
| HD139 | 2진·8진·16진 radix 표시 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H176](#h176) |
| HD140 | 소수 뒤 0 채움 표시 | 부분 구현 | 후행 0 리본·native/자체 저장·live/native literal/행렬·인쇄 동일 표시 연결. 전체 원본 반올림·round-trip 범위 미완료. | [H176](#h176) |
| HD141 | 복소수 Cartesian i/j 선택 | 부분 구현 | i/j 리본과 imaginary-value 직렬화·문서 기본/local 상속·원시 결과 재서식 연결. 전체 문서/다중 선택 변경 UI 미완료. | [H176](#h176) |
| HD142 | 복소수 Polar 라디안·도 선택 | 부분 구현 | polarRadians/polarPiRadians/polarDegrees 리본·크기/각 표시·native 왕복·저장 결과/인쇄 연결. 입력 Polar 연산자와 별개. 모든 원본 조판/소비자 경계 미확인. | [H176](#h176) |
| HD143 | 복소수 작은 성분 숨김 threshold | 부분 구현 | 원본 strict 상대 비율 임계값0..307 리본·표시·직렬화 연결. General 외 명시 보존은 XSD 근거 독립 정책. 원본 모든 소비자 경계 미확인. | [H176](#h176) |
| HD144 | 영 근사 threshold 설정 | 부분 구현 | 절대 영 임계값0..307 리본·표시·직렬화 연결. General 외 명시 보존은 XSD 근거 독립 정책. 전체 원본 반올림/서식 소비자 미확인. | [H176](#h176) |
| HD145 | General 지수 전환 threshold 설정 | 부분 구현 | General 지수 임계값 리본0..15/파일0..17 구분·작은 수/큰 수 전환 적용·native 저장 연결. nullable/round-trip 경계 미완료. | [H176](#h176) |
| HD146 | 결과 표시 서식 초기화 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H176](#h176) |
| HD147 | 라벨별 글꼴 굵게·기울임·밑줄 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H174](#h174) |
| HD148 | 라벨별 색상과 문서 전체 일괄 적용 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H174](#h174) |
| HD149 | 라벨별 기본 스타일 복원 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H174](#h174) |
| HD150 | 수식 글꼴 크기 변경 | 부분 구현 | 기본 크기 변경 있음. 전체 다중 영역/문서 기본 규약 미완료 | [H175](#h175) |
| HD151 | 수식 글꼴 family 선택 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H175](#h175) |
| HD152 | 수식 영역 서식 제거·초기화 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H175](#h175) |

### 기본 도표 — 27행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD153 | XY 함수·데이터 도표 | 부분 구현 | 무차원 실수·trace당2000점. 기본 native source/curve 저장·계산 없는 editable 재열기·벡터 출력 연결([0147](analysis/0147-native-xy-integration.md)). 단위·전체 규약 미완료 | [H030](#h030) |
| HD154 | 여러 trace 추가·삭제 | 부분 구현 | typed 첫 math·일반 expression-only 편집·label/contextual/문자 아래첨자 보존·구조/필드 Undo·단일 Enter·Pending·label-aware physical 계산·own/native/cold/vector 연결([0181](analysis/0181-xy-typed-expression-contract.md), [0182](analysis/0182-xy-typed-expression-integration.md)). Legacy 문자열 문서 유지. 전체 navigator/IME/legend·원본 sampling/domain 미완료.16은 앱 제한. | [H027](#h027) |
| HD155 | 길이가 다른 X/Y 축 식 목록의 trace pairing | 부분 구현 | typed 첫 math·일반 expression-only 편집·label/contextual/문자 아래첨자 보존·구조/필드 Undo·단일 Enter·Pending·label-aware physical 계산·own/native/cold/vector 연결([0181](analysis/0181-xy-typed-expression-contract.md), [0182](analysis/0182-xy-typed-expression-integration.md)). Legacy 문자열 문서 유지. 전체 navigator/IME/legend·원본 sampling/domain 미완료.16은 앱 제한. | [H027](#h027) |
| HD156 | 도표 축 식의 단위 변환·scale | 부분 구현 | [0176](analysis/0176-xy-physical-units-implementation.md)·[0178](analysis/0178-xy-units-integration.md)의 typed scaling 편집/Enter, 물리량 pointwise 계산·선형 배율 변환·source/residual units 분리·v3/native/vector 출력. affine/complex·derived 자동 선택·전체 단위 체계/label repair 미완료. | [H027](#h027) |
| HD157 | 축 식 legend 위치 이동·크기 변경 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H027](#h027) |
| HD158 | trace 선 색상 | 부분 구현 | 스타일 저장/표시 있음. DashDot 계열 근사 및 원본 선택 조작 미완료 | [H160](#h160) |
| HD159 | trace 선 두께 | 부분 구현 | 스타일 저장/표시 있음. DashDot 계열 근사 및 원본 선택 조작 미완료 | [H160](#h160) |
| HD160 | trace 선 패턴 | 부분 구현 | 스타일 저장/표시 있음. DashDot 계열 근사 및 원본 선택 조작 미완료 | [H160](#h160) |
| HD161 | trace 점 기호 | 부분 구현 | 스타일 저장/표시 있음. DashDot 계열 근사 및 원본 선택 조작 미완료 | [H160](#h160) |
| HD162 | 로그 축 스케일 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H003](#h003) |
| HD163 | 축 교차 위치 0,0 토글 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H003](#h003) |
| HD164 | 축 교차 위치 tick에 맞춰 이동 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H003](#h003) |
| HD165 | 축 tick 간격 지정 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H003](#h003) |
| HD166 | tick 표시 토글 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H003](#h003) |
| HD167 | tick 숫자 표시 토글 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H003](#h003) |
| HD168 | tick 숫자 형식·정밀도·뒤 0 설정 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H003](#h003) |
| HD169 | 첫·끝 tick 편집으로 축 범위 지정 | 부분 구현 | 별도 축 range 설정은 있음. 원본 tick 직접 편집과 수식 평가 미완료 | [H171](#h171) |
| HD170 | 수직·수평 marker 추가·삭제 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H017](#h017) |
| HD171 | marker callout 좌표 표시 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H017](#h017) |
| HD172 | marker 드래그·callout 값 입력으로 이동 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H017](#h017) |
| HD173 | XY·Polar 도표 제목 추가·위치·Rich Text 서식 | 부분 구현 | 기본 축 label 경로만 있음. 원본 제목 text region·full formatting 미완료 | [H119](#h119) |
| HD174 | XY 축 제목 추가·Rich Text 서식 | 부분 구현 | 기본 축 label 경로만 있음. 원본 제목 text region·full formatting 미완료 | [H119](#h119) |
| HD175 | XY·Polar 도표 범례 추가·제거 | 부분 구현 | XY 범례 토글 있음. Polar 및 원본 위치·편집 계약 미완료 | [H146](#h146) |
| HD176 | 범례 텍스트 서식·위치·크기 변경 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H170](#h170) |
| HD177 | XY·Polar 독립 X/Y 눈금선 토글 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H118](#h118) |
| HD178 | 도표 배경색 설정 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H195](#h195) |
| HD179 | 도표 축 식 숨김·표시 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H196](#h196) |

### 도표 종류 — 14행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD180 | Polar 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H020](#h020) |
| HD181 | Contour 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H006](#h006) |
| HD182 | Density 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H019](#h019) |
| HD183 | Box 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H154](#h154) |
| HD184 | Effects 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H157](#h157) |
| HD185 | Error 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H158](#h158) |
| HD186 | Pareto 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H115](#h115) |
| HD187 | Waterfall 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H156](#h156) |
| HD188 | 3D 함수 표면 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H001](#h001) |
| HD189 | 3D 행렬 표면 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H001](#h001) |
| HD190 | 3D 파라메트릭 곡선 도표 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H001](#h001) |
| HD191 | 3D 원근 보기 토글 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H165](#h165) |
| HD192 | 3D 표면 채움 색·없음 설정 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H149](#h149) |
| HD193 | 도표 함수 샘플 점 개수 설정 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H150](#h150) |

### Chart 컴포넌트 — 10행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD194 | 삽입형 Chart 컴포넌트와 외부 Chart 편집기 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H004](#h004) |
| HD195 | Chart 두 번째 Y축 추가·삭제 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H143](#h143) |
| HD196 | Chart trace를 두 번째 Y축으로 이동 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H187](#h187) |
| HD197 | Chart 축 화살표·선 색·패턴·두께 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H173](#h173) |
| HD198 | Chart tick 값 글꼴·각도·offset | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H173](#h173) |
| HD199 | Chart Scatter/Line/Area/Columns trace 선택 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H177](#h177) |
| HD200 | Chart 기호 opacity·두께·weight | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H177](#h177) |
| HD201 | Chart 영역 채움·opacity | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H177](#h177) |
| HD202 | Chart 보조 눈금선·tick 서식 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H144](#h144) |
| HD203 | Chart 스타일 템플릿 적용 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H005](#h005) |

### Excel 컴포넌트 — 4행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD204 | Excel 워크시트 컴포넌트 삽입·편집 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H013](#h013) |
| HD205 | Mathcad 입력 식을 Excel 셀에 전달 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H197](#h197) |
| HD206 | Excel 셀·범위를 Mathcad 출력 식으로 가져오기 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H198](#h198) |
| HD207 | Excel 컴포넌트 표시 서식 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H077](#h077) |

### 제품 연동 — 5행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD208 | Creo 입력 영역 지정·잠금 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H068](#h068) |
| HD209 | Creo 출력 영역 지정 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H069](#h069) |
| HD210 | 입출력 designation 목록 표시·영역 강조 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H164](#h164) |
| HD211 | 입출력 alias 편집·지정 해제 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H164](#h164) |
| HD212 | 입출력 designation 창 dock·float·자동 숨김 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H167](#h167) |

### 고급 컨트롤 — 7행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HD213 | 스크립트 Text Box 컨트롤 입력 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H025](#h025) |
| HD214 | 스크립트 Button 컨트롤 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H052](#h052) |
| HD215 | 스크립트 Check Box 컨트롤 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H055](#h055) |
| HD216 | 스크립트 Radio Button 컨트롤 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H124](#h124) |
| HD217 | 스크립트 List Box 컨트롤 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H102](#h102) |
| HD218 | 스크립트 Slider 컨트롤 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H134](#h134) |
| HD219 | 고급 컨트롤 입력·출력 배열 전달 | 미구현 | 원본 동작의 작성·실행 경로 없음 | [H092](#h092) |

### 공개 자동화: 앱 — 13행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI001 | 자동화 클라이언트에서 Application 객체 생성 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |
| HI002 | 자동화 앱 활성화 Activate | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |
| HI003 | 자동화 앱 창 표시·숨김 Visible | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |
| HI004 | 자동화 활성 워크시트 취득 ActiveWorksheet | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |
| HI005 | 자동화 워크시트 열기·새 문서 Open | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |
| HI006 | 자동화 모든 워크시트 닫기 CloseAll와 저장 정책 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |
| HI007 | 자동화 앱 종료 Quit와 연결 수명 계약 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |
| HI008 | 자동화 앱 버전 조회 GetVersion | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |
| HI009 | 자동화 읽기 옵션을 지정하여 OpenEx | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |
| HI010 | 자동화 열린 워크시트 수 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H049](#h049) |
| HI011 | 자동화 인덱스로 열린 워크시트 객체 취득 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H049](#h049) |
| HI012 | 자동화 이벤트 sink 초기화 InitializeEvents2 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |
| HI013 | 자동화 개별 이벤트 구독·해제 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H038](#h038) |

### 공개 자동화: 문서 — 20행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI014 | 자동화 워크시트 탭 활성화 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI015 | 자동화 워크시트 닫기와 저장·폐기 정책 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI016 | 자동화 워크시트 열림 상태 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI017 | 자동화 워크시트 파일 이름·전체 경로 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI018 | 자동화 워크시트 Modified 읽기·쓰기 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI019 | 자동화 워크시트 다른 이름·MCTX 저장 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI020 | 자동화 문서 계산 Synchronize | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI021 | 자동화 문서 계산 일시정지·재개 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI022 | 자동화 계산 대기 timeout 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI023 | 자동화 워크시트 제목 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI024 | 자동화 워크시트 탭 이름·아이콘 지정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI025 | 자동화 문서 닫기 안내 문구 지정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI026 | 자동화 표시 파일 경로 지정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI027 | 자동화 워크시트 현재 작업 디렉터리 지정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI028 | 자동화 입력 목록·출력 목록 취득 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI075 | 자동화 워크시트 현재 파일 저장 Save | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H215](#h215) |
| HI076 | 자동화 워크시트 제목 조회 GetTitle | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H215](#h215) |
| HI077 | 자동화 워크시트 읽기 전용 상태 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H215](#h215) |
| HI078 | 자동화 alias 입력 문자열 조회 InputGetStringValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H215](#h215) |
| HI079 | 자동화 alias 출력 문자열 조회 OutputGetStringValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H215](#h215) |

### 공개 자동화: 입력 — 8행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI029 | 자동화 alias 입력 실수·단위 설정 SetRealValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI030 | 자동화 alias 입력 행렬·단위 설정 SetMatrixValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI031 | 자동화 alias 문자열 입력 SetStringValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI032 | 자동화 alias S-expression 입력 SetSExprValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI033 | 자동화 alias 입력 실수·단위 조회 InputGetRealValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI034 | 자동화 alias 입력 행렬·단위 조회 InputGetMatrixValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI035 | 자동화 alias S-expression 조회 InputGetSExprValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI036 | 자동화 입력의 실수·문자열·행렬 통합 결과 조회 InputGetValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |

### 공개 자동화: 출력 — 5행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI037 | 자동화 alias 출력 실수·단위 조회 OutputGetRealValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI038 | 자동화 출력 실수의 지정 단위 변환 조회 OutputGetRealValueAs | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI039 | 자동화 alias 출력 행렬·단위 조회 OutputGetMatrixValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI040 | 자동화 출력 행렬의 지정 단위 변환 조회 OutputGetMatrixValueAs | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI041 | 자동화 출력의 실수·문자열·행렬 통합 결과 조회 OutputGetValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |

### 공개 자동화: 목록 — 2행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI042 | 자동화 입력 수와 인덱스별 alias 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H041](#h041) |
| HI043 | 자동화 출력 수와 인덱스별 alias 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H044](#h044) |

### 공개 자동화: 결과 — 2행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI044 | 자동화 계산 결과 타입·값·단위 취득 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H039](#h039) |
| HI045 | 자동화 오류 상태 조회: alias 없음·단위 불일치·계산 오류·timeout | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H039](#h039) |

### 공개 자동화: 행렬 — 4행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI046 | 자동화 NaN 초기 행렬 생성 CreateMatrix | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI047 | 자동화 행렬 원소 읽기 GetMatrixElement | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H043](#h043) |
| HI048 | 자동화 행렬 원소 쓰기 SetMatrixElement | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H043](#h043) |
| HI049 | 자동화 행렬 행·열 크기 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H043](#h043) |

### 공개 자동화: 일괄 입력 — 7행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI050 | 자동화 일괄 입력 setter 생성 CreateValuesSetter | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H047](#h047) |
| HI051 | 자동화 일괄 실수·단위 입력 추가 AddScalarValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H046](#h046) |
| HI052 | 자동화 일괄 행렬·단위 입력 추가 AddMatrixValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H046](#h046) |
| HI053 | 자동화 일괄 문자열 입력 추가 AddStringValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H046](#h046) |
| HI054 | 자동화 일괄 S-expression 입력 추가 AddSExprValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H046](#h046) |
| HI055 | 자동화 일괄 입력 반영·대기 SetValues | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H046](#h046) |
| HI056 | 자동화 일괄 입력 반영 결과를 alias·index로 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H045](#h045) |

### 공개 자동화: 옵션 — 5행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI057 | 문서 생성 시 고정 자동화 옵션 설정 SetOptionValue | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H048](#h048) |
| HI058 | 자동화 alias 비교 대소문자 무시 옵션 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H036](#h036) |
| HI059 | 자동화 문서 열기·저장 MRU와 CWD 갱신 억제 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H036](#h036) |
| HI060 | 자동화 문서 작업별 상태 생성 옵션 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H036](#h036) |
| HI061 | 자동화 외부 입력 갱신 요청 옵션 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H036](#h036) |

### 공개 자동화: 상태·충돌 — 4행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI062 | Creo 연동용 입력 원래 상태·현재 상태 추출 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H042](#h042) |
| HI063 | Creo 연동용 출력 상태 추출·상태 수 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H042](#h042) |
| HI064 | 자동화 전체 오류·경고 등록 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H040](#h040) |
| HI065 | 자동화 alias별 오류·경고 등록 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H040](#h040) |

### 공개 자동화: 이벤트 — 9행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI066 | WorksheetSaved 자동화 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H037](#h037) |
| HI067 | WorksheetClosed 자동화 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H037](#h037) |
| HI068 | WorksheetModified 자동화 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H037](#h037) |
| HI069 | WorksheetRenamed 자동화 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H037](#h037) |
| HI070 | WorksheetInputsOutputsSelected 자동화 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H037](#h037) |
| HI071 | 앱 Exit 자동화 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H037](#h037) |
| HI072 | WorksheetStatesGenerating 자동화 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H037](#h037) |
| HI073 | WorksheetStatesGenerated 자동화 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H037](#h037) |
| HI074 | WorksheetRequestToUpdateInputs 자동화 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H037](#h037) |

### 고급 컨트롤: 스크립팅 — 10행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI080 | 컨트롤 스크립팅 언어 VBScript 선택 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H035](#h035) |
| HI081 | 컨트롤 스크립팅 언어 JScript ES3 선택 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H035](#h035) |
| HI082 | 컨트롤 스크립팅 언어 Python 3.7~3.13 선택 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H035](#h035) |
| HI083 | 컨트롤 Script Editor 코드 편집 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H168](#h168) |
| HI084 | 컨트롤 Properties 탭 속성 편집 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H168](#h168) |
| HI085 | 컨트롤 스크립트 Apply 즉시 실행 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H168](#h168) |
| HI086 | 컨트롤 스크립트 미적용 변경 취소 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H168](#h168) |
| HI087 | 컨트롤 스크립트 오류 위치 표시·닫기 제한 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H168](#h168) |
| HI088 | 컨트롤 정의식 왼쪽 숨기기 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H168](#h168) |
| HI089 | 컨트롤 정의식 왼쪽 접기·왼쪽 정렬 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H168](#h168) |

### 고급 컨트롤: 실행 설정 — 7행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI090 | 스크립트 포함 문서 열기 실행 허용·차단 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H074](#h074) |
| HI091 | 차단한 컨트롤을 나중에 선택 활성화 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H074](#h074) |
| HI092 | 고급 컨트롤 경고 표시 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H074](#h074) |
| HI093 | 문서 열기 시 컨트롤 스크립트 실행 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H074](#h074) |
| HI094 | 전체 재계산 시 컨트롤 스크립트 실행 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H074](#h074) |
| HI095 | 저장된 컨트롤 데이터 유지와 상호작용 시 실행 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H074](#h074) |
| HI096 | Python 설치 경로 선택·변경 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H074](#h074) |

### 고급 컨트롤: 이벤트 — 9행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI097 | 컨트롤 Start 초기화 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H136](#h136) |
| HI098 | 컨트롤 Exec 입력·출력 처리 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H078](#h078) |
| HI099 | 컨트롤 Stop 정리 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H137](#h137) |
| HI100 | Button Click 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H051](#h051) |
| HI101 | Check Box Click 이벤트·Check 상태 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H054](#h054) |
| HI102 | List Box 선택 변경 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H101](#h101) |
| HI103 | List Box 더블 클릭 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H101](#h101) |
| HI104 | Radio Button 선택 변경 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H123](#h123) |
| HI105 | Slider 값 변경 이벤트 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H133](#h133) |

### 고급 컨트롤: 속성 — 9행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI106 | List Box 항목 추가·삽입·삭제·전체 초기화 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H032](#h032) |
| HI107 | List Box 부분·완전 문자열 검색 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H032](#h032) |
| HI108 | List Box 현재 선택·개수·항목 텍스트 조회 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H032](#h032) |
| HI109 | List Box 정렬·오른쪽 정렬 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H032](#h032) |
| HI110 | Slider 최소·최대·현재 값 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H033](#h033) |
| HI111 | Slider 줄·페이지 이동 크기 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H033](#h033) |
| HI112 | Slider 방향·반전·눈금 빈도 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H033](#h033) |
| HI113 | Text Box 텍스트·읽기 전용 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H034](#h034) |
| HI114 | Text Box 여러 줄·글꼴 크기·전경·배경색 설정 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H034](#h034) |

### 사용자 DLL 확장 — 12행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HI115 | 사용자 C/C++ DLL 함수를 워크시트 계산에 등록·호출 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H012](#h012) |
| HI116 | 하나의 DLL에 여러 사용자 함수 제공 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H012](#h012) |
| HI117 | 사용자 DLL 함수 복소 scalar 인자·반환값 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H012](#h012) |
| HI118 | 사용자 DLL 함수 복소 행렬 인자·반환값 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H012](#h012) |
| HI119 | 사용자 DLL 함수 문자열 인자·반환값 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H012](#h012) |
| HI120 | Custom Functions 폴더의 DLL을 재시작 시 로드 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H155](#h155) |
| HI121 | DLL 함수에 Function 라벨 자동 적용 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H155](#h155) |
| HI122 | 내장·사용자 함수와 DLL 이름 충돌 검사 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H011](#h011) |
| HI123 | DLL 사용자 오류 메시지 등록·인자 위치 표시 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H214](#h214) |
| HI124 | DLL 계산 사용자 중단 전달 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H214](#h214) |
| HI125 | DLL overflow·0 나누기·invalid 예외 처리 | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H214](#h214) |
| HI126 | C wrapper를 통한 Fortran 사용자 계산 DLL | 미구현 | 해당 공개 계약의 실행 경로 없음. 내부 계산·원본 보존은 API/확장 구현이 아님. | [H028](#h028) |

### 도움말 사용자 함수 — 41행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HF001 | beta | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H050](#h050) |
| HF002 | hypergeom | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H050](#h050) |
| HF003 | Zeta | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H050](#h050) |
| HF004 | if | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H059](#h059) |
| HF015 | Δ | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H071](#h071) |
| HF016 | Ei | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H079](#h079) |
| HF017 | IsFunction | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H081](#h081) |
| HF018 | SIUnitsOf | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H081](#h081) |
| HF019 | hasVariables | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H081](#h081) |
| HF020 | getVariables | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H081](#h081) |
| HF021 | FresnelC | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H084](#h084) |
| HF022 | FresnelS | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H084](#h084) |
| HF040 | Si | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H094](#h094) |
| HF041 | Ci | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H094](#h094) |
| HF042 | Shi | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H094](#h094) |
| HF043 | Chi | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H094](#h094) |
| HF044 | Jacob | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H096](#h096) |
| HF045 | LambertW | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H099](#h099) |
| HF046 | dilog | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H104](#h104) |
| HF047 | polylog | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H104](#h104) |
| HF048 | li | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H105](#h105) |
| HF049 | Li | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H105](#h105) |
| HF051 | odesolve | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H112](#h112) |
| HF052 | denom | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H114](#h114) |
| HF053 | numer | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H114](#h114) |
| HF054 | fact2 | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H114](#h114) |
| HF055 | IsPrime | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H114](#h114) |
| HF056 | pdesolve | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H116](#h116) |
| HF060 | format | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H138](#h138) |
| HF061 | °C | 부분 구현 | 명시 UNIT의 유한 실수 scalar forward·unitOverride inverse·단위 메뉴/source/native/own cold 표시. 직접 Matrix/Range/complex·일반 inverse·원본 cached affine unit metadata 미확정. 0190/0192 참조. | [H204](#h204) |
| HF062 | °F | 부분 구현 | 명시 UNIT의 유한 실수 scalar forward·unitOverride inverse·단위 메뉴/source/native/own cold 표시. 직접 Matrix/Range/complex·일반 inverse·원본 cached affine unit metadata 미확정. 0190/0192 참조. | [H204](#h204) |
| HF066 | clear | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H208](#h208) |
| HF067 | clear.sym | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H208](#h208) |
| HF068 | isContinuous | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H212](#h212) |
| HF069 | discontPoints | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H212](#h212) |
| HF070 | localExtrema | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H212](#h212) |
| HF071 | localMinima | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H212](#h212) |
| HF072 | localMaxima | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H212](#h212) |
| HF073 | globalExtrema | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H212](#h212) |
| HF074 | globalMinima | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H212](#h212) |
| HF075 | globalMaxima | 미구현 | 이 사용자 함수/overload의 독립 실행 경로 없음. | [H212](#h212) |

### 상징 함수 — 5행

| ID | 원본 기능/명령 | 상태 | 구현 범위·남은 차이 | 근거 |
| --- | --- | --- | --- | --- |
| HF076 | ellipticK | 미구현 | 완전/불완전 타원적분을 포함하는 상징 함수 실행 경로 없음. | [H073](#h073) |
| HF077 | ellipticF | 미구현 | 완전/불완전 타원적분을 포함하는 상징 함수 실행 경로 없음. | [H073](#h073) |
| HF078 | ellipticE | 미구현 | 완전/불완전 타원적분을 포함하는 상징 함수 실행 경로 없음. | [H073](#h073) |
| HF079 | ellipticPi | 미구현 | 완전/불완전 타원적분을 포함하는 상징 함수 실행 경로 없음. | [H073](#h073) |
| HF080 | Psi(n,z) | 미구현 | 상징 함수/overload의 독립 실행 경로 없음. 단일 인자 Psi 숫자 바인딩과 구분. | [H141](#h141) |

<a id="engine-bindings"></a>

## 원본 계산 바인딩 874개 개별 목록

[원본 사실 JSON](spec/mathcad12-engine-catalog.json)의 **21개 모듈·874행 전체**를 같은 순서로 수록했다. 기존 20개 모듈·868행에 REA로 확인한 EfiAffine 6개 바인딩을 추가했다. 각 모듈의 SHA-256·MVID·REA Evidence ID와 각 행의 target/인자 계약은 사실 JSON에 있다. 아래 번호는 이 스냅샷의 행 번호이며 영구 API ID가 아니다.

바인딩은 사용자 기능과 1:1이 아니다. 내부·호환용 이름도 포함한다. ‘부분 구현’은 이름 dispatcher 또는 명시한 MathML/native 대응 경로가 있다는 뜻이며 원본 ABI를 호출/복제했다는 뜻이 아니다. ‘미구현’인 내부 이름과 비슷한 사용자 기능을 별도 경로로 일부 제공할 수 있다. Rust XY 도표와 원본 plot 내부 바인딩 실행은 별개다.

현재 CLI `features --all`의 단순 이름 분류는 120개를 bounded-subset으로 표시한다. 이 표는 소스에서 확인한 미적분·Find/Minerr의 대응 경로도 반영한다. CLI 수치를 완성도나 정확한 전체 지원 목록으로 사용하지 않는다. 이 표의 상태는 **부분 구현 132 / 미구현 742**이며 바인딩 전체에 ‘구현됨’ 판정을 내린 항목은 없다.

<details>
<summary>efibessel — 38개</summary>

원본 근거: `ev_f189f9c3353626019c6170c95e6a7af9b56c23046b2aec8b9ff4fd865682192b`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 1 | `J0` | function | `mc_J0` | 미구현 | 독립 실행 경로 없음 |
| 2 | `J1` | function | `mc_J1` | 미구현 | 독립 실행 경로 없음 |
| 3 | `Jn` | function | `mc_Jn` | 미구현 | 독립 실행 경로 없음 |
| 4 | `J0_2E_sc` | function | `mc_J0_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 5 | `J1_2E_sc` | function | `mc_J1_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 6 | `Jn_2E_sc` | function | `mc_Jn_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 7 | `Y0` | function | `mc_Y0` | 미구현 | 독립 실행 경로 없음 |
| 8 | `Y1` | function | `mc_Y1` | 미구현 | 독립 실행 경로 없음 |
| 9 | `Yn` | function | `mc_Yn` | 미구현 | 독립 실행 경로 없음 |
| 10 | `K0` | function | `mc_K0` | 미구현 | 독립 실행 경로 없음 |
| 11 | `K1` | function | `mc_K1` | 미구현 | 독립 실행 경로 없음 |
| 12 | `Kn` | function | `mc_Kn` | 미구현 | 독립 실행 경로 없음 |
| 13 | `K0_2E_sc` | function | `mc_K0_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 14 | `K1_2E_sc` | function | `mc_K1_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 15 | `In` | function | `mc_In` | 미구현 | 독립 실행 경로 없음 |
| 16 | `I0_2E_sc` | function | `mc_I0_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 17 | `I1_2E_sc` | function | `mc_I1_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 18 | `In_2E_sc` | function | `mc_In_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 19 | `Y0_2E_sc` | function | `mc_Y0_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 20 | `Y1_2E_sc` | function | `mc_Y1_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 21 | `Yn_2E_sc` | function | `mc_Yn_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 22 | `I0` | function | `mc_I0` | 미구현 | 독립 실행 경로 없음 |
| 23 | `I1` | function | `mc_I1` | 미구현 | 독립 실행 경로 없음 |
| 24 | `H2_2E_sc` | function | `mc_H2_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 25 | `Ai` | function | `mc_Ai` | 미구현 | 독립 실행 경로 없음 |
| 26 | `DAi` | function | `mc_DAi` | 미구현 | 독립 실행 경로 없음 |
| 27 | `Ai_2E_sc` | function | `mc_Ai_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 28 | `DAi_2E_sc` | function | `mc_DAi_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 29 | `Kn_2E_sc` | function | `mc_Kn_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 30 | `H1` | function | `mc_H1` | 미구현 | 독립 실행 경로 없음 |
| 31 | `H2` | function | `mc_H2` | 미구현 | 독립 실행 경로 없음 |
| 32 | `H1_2E_sc` | function | `mc_H1_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 33 | `Bi` | function | `mc_Bi` | 미구현 | 독립 실행 경로 없음 |
| 34 | `DBi` | function | `mc_DBi` | 미구현 | 독립 실행 경로 없음 |
| 35 | `Bi_2E_sc` | function | `mc_Bi_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 36 | `DBi_2E_sc` | function | `mc_DBi_2E_sc` | 미구현 | 독립 실행 경로 없음 |
| 37 | `js` | function | `mc_js` | 미구현 | 독립 실행 경로 없음 |
| 38 | `ys` | function | `mc_ys` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>eficore — 105개</summary>

원본 근거: `ev_016bd2c29cfbda9d1596e0b3ad3b43f2bbe6bed7310aee9be644418fd2794630`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 39 | `element_of_C` | operator | `op_element_of_C` | 미구현 | 독립 실행 경로 없음 |
| 40 | `element_of_Q` | operator | `op_element_of_Q` | 미구현 | 독립 실행 경로 없음 |
| 41 | `element_of_R` | operator | `op_element_of_R` | 미구현 | 독립 실행 경로 없음 |
| 42 | `element_of_Z` | operator | `op_element_of_Z` | 미구현 | 독립 실행 경로 없음 |
| 43 | `element_of` | operator | `op_element_of` | 미구현 | 독립 실행 경로 없음 |
| 44 | `IsArray` | function | `mc_IsArray` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 45 | `IsScalar` | function | `mc_IsScalar` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 46 | `IsString` | function | `mc_IsString` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 47 | `IsRange` | function | `mc_IsRange` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 48 | `error` | function | `mc_error` | 미구현 | 독립 실행 경로 없음 |
| 49 | `lt` | operator | `op_lt` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 50 | `leq` | operator | `op_leq` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 51 | `gt` | operator | `op_gt` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 52 | `geq` | operator | `op_geq` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 53 | `not` | operator | `op_not` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 54 | `xor` | operator | `op_xor` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 55 | `or` | operator | `op_or` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 56 | `and` | operator | `op_and` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 57 | `IsNaN` | function | `mc_IsNaN` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 58 | `log` | function | `mc_log` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 59 | `round` | function | `mc_round` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 60 | `strlen` | function | `mc_strlen` | 부분 구현 | UTF16 code-unit 길이; String1MiB 앱 예산 |
| 61 | `concat` | function | `mc_concat` | 부분 구현 | scalar String 연결; 합계1MiB 앱 예산·전체 overload 미완료 |
| 62 | `search` | function | `mc_search` | 부분 구현 | ASCII·문서 ORIGIN 옵션·미발견값; 전체 .NET collation 미완료 |
| 63 | `signum` | function | `mc_signum` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 64 | `substr` | function | `mc_substr` | 부분 구현 | UTF16 인덱스·ORIGIN 옵션·끝 truncate; 서로게이트 반쪽 미지원 |
| 65 | `num2str` | function | `mc_num2str` | 미구현 | 독립 실행 경로 없음 |
| 66 | `nthroot` | operator | `op_nthroot` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 67 | `add` | operator | `op_add` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 68 | `sub` | operator | `op_sub` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 69 | `negate` | operator | `op_negate` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 70 | `angle` | function | `mc_angle` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 71 | `arg` | function | `mc_arg` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 72 | `asec` | function | `mc_asec` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 73 | `asech` | function | `mc_asech` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 74 | `conjugate` | operator | `op_conjugate` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 75 | `factorial` | operator | `op_factorial` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 76 | `sqrt` | operator | `op_sqrt` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 77 | `rootn` | operator | `op_rootn` | 미구현 | 독립 실행 경로 없음 |
| 78 | `acos` | function | `mc_acos` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 79 | `acosh` | function | `mc_acosh` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 80 | `acot` | function | `mc_acot` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 81 | `acoth` | function | `mc_acoth` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 82 | `acsc` | function | `mc_acsc` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 83 | `acsch` | function | `mc_acsch` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 84 | `cnorm` | function | `mc_cnorm` | 미구현 | 독립 실행 경로 없음 |
| 85 | `cos` | function | `mc_cos` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 86 | `cosh` | function | `mc_cosh` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 87 | `cot` | function | `mc_cot` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 88 | `coth` | function | `mc_coth` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 89 | `ceil` | function | `mc_ceil` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 90 | `Ceil` | function | `mc_Ceil` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 91 | `ceilMc11` | function | `mc_ceilMc11` | 미구현 | 독립 실행 경로 없음 |
| 92 | `CeilMc11` | function | `mc_CeilMc11` | 미구현 | 독립 실행 경로 없음 |
| 93 | `Im` | function | `mc_Im` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 94 | `ln0` | function | `mc_ln0` | 미구현 | 독립 실행 경로 없음 |
| 95 | `ln` | function | `mc_ln` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 96 | `mod` | function | `mc_mod` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 97 | `asin` | function | `mc_asin` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 98 | `asinh` | function | `mc_asinh` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 99 | `atan` | function | `mc_atan` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 100 | `atan2` | function | `mc_atan2` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 101 | `atanh` | function | `mc_atanh` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 102 | `csc` | function | `mc_csc` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 103 | `csch` | function | `mc_csch` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 104 | `csgn` | function | `mc_csgn` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 105 | `erf` | function | `mc_erf` | 미구현 | 독립 실행 경로 없음 |
| 106 | `erfc` | function | `mc_erfc` | 미구현 | 독립 실행 경로 없음 |
| 107 | `exp` | function | `mc_exp` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 108 | `floor` | function | `mc_floor` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 109 | `Floor` | function | `mc_Floor` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 110 | `floorMc11` | function | `mc_floorMc11` | 미구현 | 독립 실행 경로 없음 |
| 111 | `FloorMc11` | function | `mc_FloorMc11` | 미구현 | 독립 실행 경로 없음 |
| 112 | `sin` | function | `mc_sin` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 113 | `sinc` | function | `mc_sinc` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 114 | `sinh` | function | `mc_sinh` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 115 | `tan` | function | `mc_tan` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 116 | `Re` | function | `mc_Re` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 117 | `Round` | function | `mc_Round` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 118 | `sec` | function | `mc_sec` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 119 | `sech` | function | `mc_sech` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 120 | `sign` | function | `mc_sign` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 121 | `TruncMc11` | function | `mc_TruncMc11` | 미구현 | 독립 실행 경로 없음 |
| 122 | `sleep` | function | `mc_sleep` | 미구현 | 독립 실행 경로 없음 |
| 123 | `δ` | function | `mc_δ` | 미구현 | 독립 실행 경로 없음 |
| 124 | `Φ` | function | `mc_Φ` | 미구현 | 독립 실행 경로 없음 |
| 125 | `vec2str` | function | `mc_vec2str` | 미구현 | 독립 실행 경로 없음 |
| 126 | `tanh` | function | `mc_tanh` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 127 | `trunc` | function | `mc_trunc` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 128 | `Trunc` | function | `mc_Trunc` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 129 | `truncMc11` | function | `mc_truncMc11` | 미구현 | 독립 실행 경로 없음 |
| 130 | `str2vec` | function | `mc_str2vec` | 미구현 | 독립 실행 경로 없음 |
| 131 | `str2num` | function | `mc_str2num` | 부분 구현 | 유한 invariant decimal subset; 전체 문화권/형식 미완료 |
| 132 | `ℂ` | value | `mc_ℂ` | 미구현 | 독립 실행 경로 없음 |
| 133 | `ℚ` | value | `mc_ℚ` | 미구현 | 독립 실행 경로 없음 |
| 134 | `ℝ` | value | `mc_ℝ` | 미구현 | 독립 실행 경로 없음 |
| 135 | `ℤ` | value | `mc_ℤ` | 미구현 | 독립 실행 경로 없음 |
| 136 | `element_table` | operator | `op_element_table` | 미구현 | 독립 실행 경로 없음 |
| 137 | `e` | value | `mc_e` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 138 | `π` | value | `mc_π` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 139 | `_25_` | value | `mc__25_` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 140 | `γ` | value | `mc_γ` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 141 | `NaN` | value | `mc_NaN` | 미구현 | 독립 실행 경로 없음 |
| 142 | `undefined` | value | `mc_undefined` | 미구현 | 독립 실행 경로 없음 |
| 143 | `IEEE754Infinity` | value | `mc_IEEE754Infinity` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>eficalculus — 25개</summary>

원본 근거: `ev_f1e0046c9c1b2c2d84bab0ae6b667f5523008870a63ff79be1d9acece830c487`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 144 | `integral_` | function | `mc_integral_` | 미구현 | 독립 실행 경로 없음 |
| 145 | `rangeproduct` | operator | `op_rangeproduct` | 부분 구현 | N04 · 유한 정수 bound 곱의 MathML 경로 |
| 146 | `rangesum` | operator | `op_rangesum` | 부분 구현 | N04 · 유한 정수 bound 합의 MathML 경로 |
| 147 | `ε` | function | `mc_ε` | 미구현 | 독립 실행 경로 없음 |
| 148 | `integral_cb` | operator | `op_integral_cb` | 미구현 | 독립 실행 경로 없음 |
| 149 | `integral` | operator | `op_integral` | 부분 구현 | N02 · 유한구간 적분의 MathML 경로 |
| 150 | `deriv_n` | operator | `op_deriv_n` | 부분 구현 | N03 · 0–5계 MathML 경로 |
| 151 | `integral_0` | operator | `op_integral_0` | 미구현 | 독립 실행 경로 없음 |
| 152 | `deriv_0` | operator | `op_deriv_0` | 부분 구현 | N03 · 0계 MathML 경로 |
| 153 | `deriv_1` | operator | `op_deriv_1` | 부분 구현 | N03 · 1계 MathML 경로 |
| 154 | `deriv_2` | operator | `op_deriv_2` | 부분 구현 | N03 · 2계 MathML 경로 |
| 155 | `deriv_3` | operator | `op_deriv_3` | 부분 구현 | N03 · 3계 MathML 경로 |
| 156 | `deriv_4` | operator | `op_deriv_4` | 부분 구현 | N03 · 4계 MathML 경로 |
| 157 | `deriv_5` | operator | `op_deriv_5` | 부분 구현 | N03 · 5계 MathML 경로 |
| 158 | `pderiv_0` | operator | `op_pderiv_0` | 미구현 | 독립 실행 경로 없음 |
| 159 | `pderiv_1` | operator | `op_pderiv_1` | 미구현 | 독립 실행 경로 없음 |
| 160 | `pderiv_2` | operator | `op_pderiv_2` | 미구현 | 독립 실행 경로 없음 |
| 161 | `pderiv_3` | operator | `op_pderiv_3` | 미구현 | 독립 실행 경로 없음 |
| 162 | `pderiv_4` | operator | `op_pderiv_4` | 미구현 | 독립 실행 경로 없음 |
| 163 | `pderiv_5` | operator | `op_pderiv_5` | 미구현 | 독립 실행 경로 없음 |
| 164 | `limit` | operator | `op_limit` | 미구현 | 독립 실행 경로 없음 |
| 165 | `limitleft` | operator | `op_limitleft` | 미구현 | 독립 실행 경로 없음 |
| 166 | `limitright` | operator | `op_limitright` | 미구현 | 독립 실행 경로 없음 |
| 167 | `indef_integral` | operator | `op_indef_integral` | 미구현 | 독립 실행 경로 없음 |
| 168 | `pderiv_n` | operator | `op_pderiv_n` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efidata — 40개</summary>

원본 근거: `ev_c066eb94e46cadfa71576eb0a1595fe8133e5842d42cbc62a2ea0d41282c5544`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 169 | `qqplot` | function | `mc_qqplot` | 미구현 | 독립 실행 경로 없음 |
| 170 | `Rank` | function | `mc_Rank` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 171 | `order` | function | `mc_order` | 미구현 | 독립 실행 경로 없음 |
| 172 | `Scale` | function | `mc_Scale` | 미구현 | 독립 실행 경로 없음 |
| 173 | `percentile` | function | `mc_percentile` | 미구현 | 독립 실행 경로 없음 |
| 174 | `polyiter` | function | `mc_polyiter` | 미구현 | 독립 실행 경로 없음 |
| 175 | `localmin` | function | `mc_localmin` | 미구현 | 독립 실행 경로 없음 |
| 176 | `localmax` | function | `mc_localmax` | 미구현 | 독립 실행 경로 없음 |
| 177 | `trim` | function | `mc_trim` | 미구현 | 독립 실행 경로 없음 |
| 178 | `markNaN` | function | `mc_markNaN` | 미구현 | 독립 실행 경로 없음 |
| 179 | `LeastSquaresFit` | function | `mc_LeastSquaresFit` | 미구현 | 독립 실행 경로 없음 |
| 180 | `Grubbs` | function | `mc_Grubbs` | 미구현 | 독립 실행 경로 없음 |
| 181 | `ThreeSigma` | function | `mc_ThreeSigma` | 미구현 | 독립 실행 경로 없음 |
| 182 | `GrubbsClassic` | function | `mc_GrubbsClassic` | 미구현 | 독립 실행 경로 없음 |
| 183 | `rationalfit` | function | `mc_rationalfit` | 미구현 | 독립 실행 경로 없음 |
| 184 | `rationalfitnp` | function | `mc_rationalfitnp` | 미구현 | 독립 실행 경로 없음 |
| 185 | `VSmooth` | function | `mc_VSmooth` | 미구현 | 독립 실행 경로 없음 |
| 186 | `Thiele` | function | `mc_Thiele` | 미구현 | 독립 실행 경로 없음 |
| 187 | `Thielecoeff` | function | `mc_Thielecoeff` | 미구현 | 독립 실행 경로 없음 |
| 188 | `matchNaN` | function | `mc_matchNaN` | 미구현 | 독립 실행 경로 없음 |
| 189 | `filterNaN` | function | `mc_filterNaN` | 미구현 | 독립 실행 경로 없음 |
| 190 | `confidence` | function | `mc_confidence` | 미구현 | 독립 실행 경로 없음 |
| 191 | `PCAvariance` | function | `mc_PCAvariance` | 미구현 | 독립 실행 경로 없음 |
| 192 | `contingtbl` | function | `mc_contingtbl` | 미구현 | 독립 실행 경로 없음 |
| 193 | `Ftest` | function | `mc_Ftest` | 미구현 | 독립 실행 경로 없음 |
| 194 | `kendltau` | function | `mc_kendltau` | 미구현 | 독립 실행 경로 없음 |
| 195 | `kendltau2` | function | `mc_kendltau2` | 미구현 | 독립 실행 경로 없음 |
| 196 | `polycoeff` | function | `mc_polycoeff` | 미구현 | 독립 실행 경로 없음 |
| 197 | `polyint` | function | `mc_polyint` | 미구현 | 독립 실행 경로 없음 |
| 198 | `Spline2` | function | `mc_Spline2` | 미구현 | 독립 실행 경로 없음 |
| 199 | `Binterp` | function | `mc_Binterp` | 미구현 | 독립 실행 경로 없음 |
| 200 | `DWS` | function | `mc_DWS` | 미구현 | 독립 실행 경로 없음 |
| 201 | `Nipals` | function | `mc_Nipals` | 미구현 | 독립 실행 경로 없음 |
| 202 | `Nipals2` | function | `mc_Nipals2` | 미구현 | 독립 실행 경로 없음 |
| 203 | `loadings` | function | `mc_loadings` | 미구현 | 독립 실행 경로 없음 |
| 204 | `scores` | function | `mc_scores` | 미구현 | 독립 실행 경로 없음 |
| 205 | `PCAeigenvals` | function | `mc_PCAeigenvals` | 미구현 | 독립 실행 경로 없음 |
| 206 | `rationalint` | function | `mc_rationalint` | 미구현 | 독립 실행 경로 없음 |
| 207 | `Spear` | function | `mc_Spear` | 미구현 | 독립 실행 경로 없음 |
| 208 | `Bicubic2D` | function | `mc_Bicubic2D` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efidiffeq — 28개</summary>

원본 근거: `ev_3d7b9f99fe138f0459cfcc92d21af9508e0b36e4eefa1de1cef5b57c666a4a01`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 209 | `Adams` | function | `mc_Adams` | 미구현 | 독립 실행 경로 없음 |
| 210 | `_AdamsBDF` | function | `mc__AdamsBDF` | 미구현 | 독립 실행 경로 없음 |
| 211 | `BDF` | function | `mc_BDF` | 미구현 | 독립 실행 경로 없음 |
| 212 | `AdamsBDF` | function | `mc_AdamsBDF` | 미구현 | 독립 실행 경로 없음 |
| 213 | `dlsodi` | function | `mc_dlsodi` | 미구현 | 독립 실행 경로 없음 |
| 214 | `statespace` | function | `mc_statespace` | 미구현 | 독립 실행 경로 없음 |
| 215 | `dlsodij` | function | `mc_dlsodij` | 미구현 | 독립 실행 경로 없음 |
| 216 | `_radau` | function | `mc__radau` | 미구현 | 독립 실행 경로 없음 |
| 217 | `Rkadapt2` | function | `mc_Rkadapt2` | 미구현 | 독립 실행 경로 없음 |
| 218 | `Radau` | function | `mc_Radau` | 미구현 | 독립 실행 경로 없음 |
| 219 | `rkfixed` | function | `mc_rkfixed` | 미구현 | 독립 실행 경로 없음 |
| 220 | `Radau2` | function | `mc_Radau2` | 미구현 | 독립 실행 경로 없음 |
| 221 | `Bulstoer` | function | `mc_Bulstoer` | 미구현 | 독립 실행 경로 없음 |
| 222 | `Rkadapt` | function | `mc_Rkadapt` | 미구현 | 독립 실행 경로 없음 |
| 223 | `rkadapt` | function | `mc_rkadapt` | 미구현 | 독립 실행 경로 없음 |
| 224 | `stiffb` | function | `mc_stiffb` | 미구현 | 독립 실행 경로 없음 |
| 225 | `Stiffr` | function | `mc_Stiffr` | 미구현 | 독립 실행 경로 없음 |
| 226 | `Stiffb` | function | `mc_Stiffb` | 미구현 | 독립 실행 경로 없음 |
| 227 | `bulstoer` | function | `mc_bulstoer` | 미구현 | 독립 실행 경로 없음 |
| 228 | `stiffr` | function | `mc_stiffr` | 미구현 | 독립 실행 경로 없음 |
| 229 | `radau` | function | `mc_radau` | 미구현 | 독립 실행 경로 없음 |
| 230 | `pdeinterp` | function | `mc_pdeinterp` | 미구현 | 독립 실행 경로 없음 |
| 231 | `bvalfit` | function | `mc_bvalfit` | 미구현 | 독립 실행 경로 없음 |
| 232 | `sbval` | function | `mc_sbval` | 미구현 | 독립 실행 경로 없음 |
| 233 | `relax` | function | `mc_relax` | 미구현 | 독립 실행 경로 없음 |
| 234 | `multigrid` | function | `mc_multigrid` | 미구현 | 독립 실행 경로 없음 |
| 235 | `numol` | function | `mc_numol` | 미구현 | 독립 실행 경로 없음 |
| 236 | `numol_alt` | function | `mc_numol_alt` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efifinance — 19개</summary>

원본 근거: `ev_b3d196f126b4544ffd862302dbc2a239b8417d67f674d1fefbb596f4479be22a`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 237 | `cumint` | function | `mc_cumint` | 미구현 | 독립 실행 경로 없음 |
| 238 | `cumprn` | function | `mc_cumprn` | 미구현 | 독립 실행 경로 없음 |
| 239 | `eff` | function | `mc_eff` | 미구현 | 독립 실행 경로 없음 |
| 240 | `fv` | function | `mc_fv` | 미구현 | 독립 실행 경로 없음 |
| 241 | `fvadj` | function | `mc_fvadj` | 미구현 | 독립 실행 경로 없음 |
| 242 | `cnper` | function | `mc_cnper` | 미구현 | 독립 실행 경로 없음 |
| 243 | `crate` | function | `mc_crate` | 미구현 | 독립 실행 경로 없음 |
| 244 | `irr` | function | `mc_irr` | 미구현 | 독립 실행 경로 없음 |
| 245 | `mirr` | function | `mc_mirr` | 미구현 | 독립 실행 경로 없음 |
| 246 | `nom` | function | `mc_nom` | 미구현 | 독립 실행 경로 없음 |
| 247 | `nper` | function | `mc_nper` | 미구현 | 독립 실행 경로 없음 |
| 248 | `npv` | function | `mc_npv` | 미구현 | 독립 실행 경로 없음 |
| 249 | `fvc` | function | `mc_fvc` | 미구현 | 독립 실행 경로 없음 |
| 250 | `ipmt` | function | `mc_ipmt` | 미구현 | 독립 실행 경로 없음 |
| 251 | `pmt` | function | `mc_pmt` | 미구현 | 독립 실행 경로 없음 |
| 252 | `ppmt` | function | `mc_ppmt` | 미구현 | 독립 실행 경로 없음 |
| 253 | `pv` | function | `mc_pv` | 미구현 | 독립 실행 경로 없음 |
| 254 | `rate` | function | `mc_rate` | 미구현 | 독립 실행 경로 없음 |
| 255 | `time` | function | `mc_time` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efifitting — 30개</summary>

원본 근거: `ev_96e1df861e33808a49b4d65a0cbada2129599c5da9e7ed07d4231e808c5ee987`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 256 | `bspline` | function | `mc_bspline` | 미구현 | 독립 실행 경로 없음 |
| 257 | `expfit` | function | `mc_expfit` | 미구현 | 독립 실행 경로 없음 |
| 258 | `lgsfit` | function | `mc_lgsfit` | 미구현 | 독립 실행 경로 없음 |
| 259 | `logfit` | function | `mc_logfit` | 미구현 | 독립 실행 경로 없음 |
| 260 | `pwrfit` | function | `mc_pwrfit` | 미구현 | 독립 실행 경로 없음 |
| 261 | `sinfit` | function | `mc_sinfit` | 미구현 | 독립 실행 경로 없음 |
| 262 | `linfit` | function | `mc_linfit` | 미구현 | 독립 실행 경로 없음 |
| 263 | `linterp` | function | `mc_linterp` | 미구현 | 독립 실행 경로 없음 |
| 264 | `lnfit` | function | `mc_lnfit` | 미구현 | 독립 실행 경로 없음 |
| 265 | `cspline` | function | `mc_cspline` | 미구현 | 독립 실행 경로 없음 |
| 266 | `intercept` | function | `mc_intercept` | 미구현 | 독립 실행 경로 없음 |
| 267 | `interp` | function | `mc_interp` | 미구현 | 독립 실행 경로 없음 |
| 268 | `ksmooth` | function | `mc_ksmooth` | 미구현 | 독립 실행 경로 없음 |
| 269 | `line` | function | `mc_line` | 미구현 | 독립 실행 경로 없음 |
| 270 | `root` | function | `mc_root` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 271 | `root_bracket_` | function | `mc_root_bracket_` | 미구현 | 독립 실행 경로 없음 |
| 272 | `loess` | function | `mc_loess` | 미구현 | 독립 실행 경로 없음 |
| 273 | `lspline` | function | `mc_lspline` | 미구현 | 독립 실행 경로 없음 |
| 274 | `medfit` | function | `mc_medfit` | 미구현 | 독립 실행 경로 없음 |
| 275 | `medsmooth` | function | `mc_medsmooth` | 미구현 | 독립 실행 경로 없음 |
| 276 | `pspline` | function | `mc_pspline` | 미구현 | 독립 실행 경로 없음 |
| 277 | `regress` | function | `mc_regress` | 미구현 | 독립 실행 경로 없음 |
| 278 | `regress2` | function | `mc_regress2` | 미구현 | 독립 실행 경로 없음 |
| 279 | `slope` | function | `mc_slope` | 미구현 | 독립 실행 경로 없음 |
| 280 | `supsmooth` | function | `mc_supsmooth` | 미구현 | 독립 실행 경로 없음 |
| 281 | `polyroots` | function | `mc_polyroots` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 282 | `polyroots0` | function | `mc_polyroots0` | 미구현 | 독립 실행 경로 없음 |
| 283 | `polyroots_1` | function | `mc_polyroots_1` | 미구현 | 독립 실행 경로 없음 |
| 284 | `polyroots_2` | function | `mc_polyroots_2` | 미구현 | 독립 실행 경로 없음 |
| 285 | `genfit` | function | `mc_genfit` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efifourier — 14개</summary>

원본 근거: `ev_6856ea5aacc8261c4b0cba02103702d98edac55f41b931d799b48abf57401cd4`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 286 | `fft` | function | `mc_fft` | 미구현 | 독립 실행 경로 없음 |
| 287 | `FFT` | function | `mc_FFT` | 미구현 | 독립 실행 경로 없음 |
| 288 | `cfft` | function | `mc_cfft` | 미구현 | 독립 실행 경로 없음 |
| 289 | `CFFT` | function | `mc_CFFT` | 미구현 | 독립 실행 경로 없음 |
| 290 | `wave` | function | `mc_wave` | 미구현 | 독립 실행 경로 없음 |
| 291 | `iwave` | function | `mc_iwave` | 미구현 | 독립 실행 경로 없음 |
| 292 | `dft` | function | `mc_dft` | 미구현 | 독립 실행 경로 없음 |
| 293 | `idft` | function | `mc_idft` | 미구현 | 독립 실행 경로 없음 |
| 294 | `dftr` | function | `mc_dftr` | 미구현 | 독립 실행 경로 없음 |
| 295 | `idftr` | function | `mc_idftr` | 미구현 | 독립 실행 경로 없음 |
| 296 | `ifft` | function | `mc_ifft` | 미구현 | 독립 실행 경로 없음 |
| 297 | `IFFT` | function | `mc_IFFT` | 미구현 | 독립 실행 경로 없음 |
| 298 | `icfft` | function | `mc_icfft` | 미구현 | 독립 실행 경로 없음 |
| 299 | `ICFFT` | function | `mc_ICFFT` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efimatrix — 87개</summary>

원본 근거: `ev_21f7f73e57f41b16c939ad649823f910075949cb91e105748efad3e3720a4a1f`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 300 | `eigenvals` | function | `mc_eigenvals` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 301 | `eigenvecs` | function | `mc_eigenvecs` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 302 | `genvals` | function | `mc_genvals` | 미구현 | 독립 실행 경로 없음 |
| 303 | `svd2` | function | `mc_svd2` | 미구현 | 독립 실행 경로 없음 |
| 304 | `svds` | function | `mc_svds` | 미구현 | 독립 실행 경로 없음 |
| 305 | `eigenvec` | function | `mc_eigenvec` | 미구현 | 독립 실행 경로 없음 |
| 306 | `svdold` | function | `mc_svdold` | 미구현 | 독립 실행 경로 없음 |
| 307 | `getmaxthreads` | function | `mc_getmaxthreads` | 미구현 | 독립 실행 경로 없음 |
| 308 | `genvecs` | function | `mc_genvecs` | 미구현 | 독립 실행 경로 없음 |
| 309 | `lsolve` | function | `mc_lsolve` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 310 | `svd` | function | `mc_svd` | 미구현 | 독립 실행 경로 없음 |
| 311 | `Cholesky` | function | `mc_Cholesky` | 미구현 | 독립 실행 경로 없음 |
| 312 | `power` | operator | `op_power` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 313 | `mult` | operator | `op_mult` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 314 | `div` | operator | `op_div` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 315 | `setnumthreads` | function | `mc_setnumthreads` | 미구현 | 독립 실행 경로 없음 |
| 316 | `LU` | function | `mc_LU` | 미구현 | 독립 실행 경로 없음 |
| 317 | `QR` | function | `mc_QR` | 미구현 | 독립 실행 경로 없음 |
| 318 | `sigmasum` | operator | `op_sigmasum` | 미구현 | 독립 실행 경로 없음 |
| 319 | `sigmaproduct` | operator | `op_sigmaproduct` | 미구현 | 독립 실행 경로 없음 |
| 320 | `transpose` | operator | `op_transpose` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 321 | `vbar` | operator | `op_vbar` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 322 | `sqr` | operator | `op_sqr` | 미구현 | 독립 실행 경로 없음 |
| 323 | `abs` | operator | `op_abs` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 324 | `det` | operator | `op_det` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 325 | `cross` | operator | `op_cross` | 미구현 | 독립 실행 경로 없음 |
| 326 | `vbar2` | operator | `op_vbar2` | 미구현 | 독립 실행 경로 없음 |
| 327 | `abs2` | operator | `op_abs2` | 미구현 | 독립 실행 경로 없음 |
| 328 | `norm` | operator | `op_norm` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 329 | `norm2` | operator | `op_norm2` | 미구현 | 독립 실행 경로 없음 |
| 330 | `det2` | operator | `op_det2` | 미구현 | 독립 실행 경로 없음 |
| 331 | `abs` | function | `mc_abs` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 332 | `det` | function | `mc_det` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 333 | `norm` | function | `mc_norm` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 334 | `augment` | function | `mc_augment` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 335 | `cholesky` | function | `mc_cholesky` | 미구현 | 독립 실행 경로 없음 |
| 336 | `cols` | function | `mc_cols` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 337 | `cond1` | function | `mc_cond1` | 미구현 | 독립 실행 경로 없음 |
| 338 | `cond2` | function | `mc_cond2` | 미구현 | 독립 실행 경로 없음 |
| 339 | `conde` | function | `mc_conde` | 미구현 | 독립 실행 경로 없음 |
| 340 | `condi` | function | `mc_condi` | 미구현 | 독립 실행 경로 없음 |
| 341 | `csort` | function | `mc_csort` | 미구현 | 독립 실행 경로 없음 |
| 342 | `diag` | function | `mc_diag` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 343 | `gcd` | function | `mc_gcd` | 미구현 | 독립 실행 경로 없음 |
| 344 | `geninv` | function | `mc_geninv` | 미구현 | 독립 실행 경로 없음 |
| 345 | `identity` | function | `mc_identity` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 346 | `last` | function | `mc_last` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 347 | `lcm` | function | `mc_lcm` | 미구현 | 독립 실행 경로 없음 |
| 348 | `min` | function | `mc_min` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 349 | `norm1` | function | `mc_norm1` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 350 | `norm2` | function | `mc_norm2` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 351 | `norme` | function | `mc_norme` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 352 | `submatrix` | function | `mc_submatrix` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 353 | `skew` | function | `mc_skew` | 미구현 | 독립 실행 경로 없음 |
| 354 | `sort` | function | `mc_sort` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 355 | `stack` | function | `mc_stack` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 356 | `rsort` | function | `mc_rsort` | 미구현 | 독립 실행 경로 없음 |
| 357 | `tr` | function | `mc_tr` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 358 | `rows` | function | `mc_rows` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 359 | `reverse` | function | `mc_reverse` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 360 | `length` | function | `mc_length` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 361 | `lu` | function | `mc_lu` | 미구현 | 독립 실행 경로 없음 |
| 362 | `matrix` | function | `mc_matrix` | 미구현 | 독립 실행 경로 없음 |
| 363 | `max` | function | `mc_max` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 364 | `normi` | function | `mc_normi` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 365 | `qr` | function | `mc_qr` | 미구현 | 독립 실행 경로 없음 |
| 366 | `rank` | function | `mc_rank` | 미구현 | 독립 실행 경로 없음 |
| 367 | `rref` | function | `mc_rref` | 미구현 | 독립 실행 경로 없음 |
| 368 | `kronecker` | function | `mc_kronecker` | 미구현 | 독립 실행 경로 없음 |
| 369 | `vec_1` | function | `mc_vec_1` | 미구현 | 독립 실행 경로 없음 |
| 370 | `vlookup` | function | `mc_vlookup` | 미구현 | 독립 실행 경로 없음 |
| 371 | `vhlookup` | function | `mc_vhlookup` | 미구현 | 독립 실행 경로 없음 |
| 372 | `match` | function | `mc_match` | 미구현 | 독립 실행 경로 없음 |
| 373 | `lookup` | function | `mc_lookup` | 미구현 | 독립 실행 경로 없음 |
| 374 | `hlookup` | function | `mc_hlookup` | 미구현 | 독립 실행 경로 없음 |
| 375 | `Match` | function | `mc_Match` | 미구현 | 독립 실행 경로 없음 |
| 376 | `Lookup` | function | `mc_Lookup` | 미구현 | 독립 실행 경로 없음 |
| 377 | `Hlookup` | function | `mc_Hlookup` | 미구현 | 독립 실행 경로 없음 |
| 378 | `Vlookup` | function | `mc_Vlookup` | 미구현 | 독립 실행 경로 없음 |
| 379 | `VHlookup` | function | `mc_VHlookup` | 미구현 | 독립 실행 경로 없음 |
| 380 | `vec_3` | function | `mc_vec_3` | 미구현 | 독립 실행 경로 없음 |
| 381 | `vec_2` | function | `mc_vec_2` | 미구현 | 독립 실행 경로 없음 |
| 382 | `percent` | operator | `op_percent` | 미구현 | 독립 실행 경로 없음 |
| 383 | `percent_inverse` | operator | `op_percent_inverse` | 미구현 | 독립 실행 경로 없음 |
| 384 | `scale` | operator | `op_scale` | 부분 구현 | 값/단위 곱과 오른쪽의 이름 함수 적용. 일반 함숫값·전체 타입/배열 규칙의 원본 동등성은 미확정 |
| 385 | `scale_inverse` | operator | `op_scale_inverse` | 미구현 | 독립 실행 경로 없음 |
| 386 | `polar` | operator | `op_polar` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efigraphics — 13개</summary>

원본 근거: `ev_7b8ef9386f0b143f90a4883a1579aaf909bfd051997f4d03f3ea7eaa05988f4c`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 387 | `map3d` | function | `mc_map3d` | 미구현 | 독립 실행 경로 없음 |
| 388 | `CreateMesh` | function | `mc_CreateMesh` | 미구현 | 독립 실행 경로 없음 |
| 389 | `CreateSpace` | function | `mc_CreateSpace` | 미구현 | 독립 실행 경로 없음 |
| 390 | `PolyLookup` | function | `mc_PolyLookup` | 미구현 | 독립 실행 경로 없음 |
| 391 | `pol2xy` | function | `mc_pol2xy` | 미구현 | 독립 실행 경로 없음 |
| 392 | `xy2pol` | function | `mc_xy2pol` | 미구현 | 독립 실행 경로 없음 |
| 393 | `cyl2xyz` | function | `mc_cyl2xyz` | 미구현 | 독립 실행 경로 없음 |
| 394 | `xyz2cyl` | function | `mc_xyz2cyl` | 미구현 | 독립 실행 경로 없음 |
| 395 | `sph2xyz` | function | `mc_sph2xyz` | 미구현 | 독립 실행 경로 없음 |
| 396 | `xyz2sph` | function | `mc_xyz2sph` | 미구현 | 독립 실행 경로 없음 |
| 397 | `CreateMesh3` | function | `mc_CreateMesh3` | 미구현 | 독립 실행 경로 없음 |
| 398 | `CreateSpace3` | function | `mc_CreateSpace3` | 미구현 | 독립 실행 경로 없음 |
| 399 | `Polyhedron` | function | `mc_Polyhedron` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efiimage — 119개</summary>

원본 근거: `ev_41f667bcb01c06e2fcea254c21c5749681282d50a7b4d1aa6b7fb4e0eb3281eb`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 400 | `addnoise` | function | `mc_addnoise` | 미구현 | 독립 실행 경로 없음 |
| 401 | `and` | function | `mc_and` | 미구현 | 독립 실행 경로 없음 |
| 402 | `augment3` | function | `mc_augment3` | 미구현 | 독립 실행 경로 없음 |
| 403 | `binarize` | function | `mc_binarize` | 미구현 | 독립 실행 경로 없음 |
| 404 | `binarize2` | function | `mc_binarize2` | 미구현 | 독립 실행 경로 없음 |
| 405 | `cnvxhull` | function | `mc_cnvxhull` | 미구현 | 독립 실행 경로 없음 |
| 406 | `colgrad` | function | `mc_colgrad` | 미구현 | 독립 실행 경로 없음 |
| 407 | `compgrad` | function | `mc_compgrad` | 미구현 | 독립 실행 경로 없음 |
| 408 | `concomp` | function | `mc_concomp` | 미구현 | 독립 실행 경로 없음 |
| 409 | `binarize_5F_auto` | function | `mc_binarize_5F_auto` | 미구현 | 독립 실행 경로 없음 |
| 410 | `blend` | function | `mc_blend` | 미구현 | 독립 실행 경로 없음 |
| 411 | `canny` | function | `mc_canny` | 미구현 | 독립 실행 경로 없음 |
| 412 | `center` | function | `mc_center` | 미구현 | 독립 실행 경로 없음 |
| 413 | `centsmooth` | function | `mc_centsmooth` | 미구현 | 독립 실행 경로 없음 |
| 414 | `clip` | function | `mc_clip` | 미구현 | 독립 실행 경로 없음 |
| 415 | `close` | function | `mc_close` | 미구현 | 독립 실행 경로 없음 |
| 416 | `convol2d` | function | `mc_convol2d` | 미구현 | 독립 실행 경로 없음 |
| 417 | `convolve3` | function | `mc_convolve3` | 미구현 | 독립 실행 경로 없음 |
| 418 | `convolve5` | function | `mc_convolve5` | 미구현 | 독립 실행 경로 없음 |
| 419 | `dct2d` | function | `mc_dct2d` | 미구현 | 독립 실행 경로 없음 |
| 420 | `diacrisp` | function | `mc_diacrisp` | 미구현 | 독립 실행 경로 없음 |
| 421 | `dilate8` | function | `mc_dilate8` | 미구현 | 독립 실행 경로 없음 |
| 422 | `distform` | function | `mc_distform` | 미구현 | 독립 실행 경로 없음 |
| 423 | `equalize` | function | `mc_equalize` | 미구현 | 독립 실행 경로 없음 |
| 424 | `freichen` | function | `mc_freichen` | 미구현 | 독립 실행 경로 없음 |
| 425 | `funconv` | function | `mc_funconv` | 미구현 | 독립 실행 경로 없음 |
| 426 | `funcdeconv` | function | `mc_funcdeconv` | 미구현 | 독립 실행 경로 없음 |
| 427 | `funmap` | function | `mc_funmap` | 미구현 | 독립 실행 경로 없음 |
| 428 | `gaussconv` | function | `mc_gaussconv` | 미구현 | 독립 실행 경로 없음 |
| 429 | `gaussdeconv` | function | `mc_gaussdeconv` | 미구현 | 독립 실행 경로 없음 |
| 430 | `getnoise` | function | `mc_getnoise` | 미구현 | 독립 실행 경로 없음 |
| 431 | `difedge` | function | `mc_difedge` | 미구현 | 독립 실행 경로 없음 |
| 432 | `dilate` | function | `mc_dilate` | 미구현 | 독립 실행 경로 없음 |
| 433 | `dilate4` | function | `mc_dilate4` | 미구현 | 독립 실행 경로 없음 |
| 434 | `erode` | function | `mc_erode` | 미구현 | 독립 실행 경로 없음 |
| 435 | `erode4` | function | `mc_erode4` | 미구현 | 독립 실행 경로 없음 |
| 436 | `erode8` | function | `mc_erode8` | 미구현 | 독립 실행 경로 없음 |
| 437 | `extract` | function | `mc_extract` | 미구현 | 독립 실행 경로 없음 |
| 438 | `gray_5F_erode` | function | `mc_gray_5F_erode` | 미구현 | 독립 실행 경로 없음 |
| 439 | `gray_5F_open` | function | `mc_gray_5F_open` | 미구현 | 독립 실행 경로 없음 |
| 440 | `gray_5F_to_5F_rgb` | function | `mc_gray_5F_to_5F_rgb` | 미구현 | 독립 실행 경로 없음 |
| 441 | `hist2d` | function | `mc_hist2d` | 미구현 | 독립 실행 경로 없음 |
| 442 | `imhist` | function | `mc_imhist` | 미구현 | 독립 실행 경로 없음 |
| 443 | `imhist2` | function | `mc_imhist2` | 미구현 | 독립 실행 경로 없음 |
| 444 | `immse` | function | `mc_immse` | 미구현 | 독립 실행 경로 없음 |
| 445 | `imquant2` | function | `mc_imquant2` | 미구현 | 독립 실행 경로 없음 |
| 446 | `iwave2d` | function | `mc_iwave2d` | 미구현 | 독립 실행 경로 없음 |
| 447 | `kirsch` | function | `mc_kirsch` | 미구현 | 독립 실행 경로 없음 |
| 448 | `laplace4` | function | `mc_laplace4` | 미구현 | 독립 실행 경로 없음 |
| 449 | `laplace8` | function | `mc_laplace8` | 미구현 | 독립 실행 경로 없음 |
| 450 | `gray_5F_close` | function | `mc_gray_5F_close` | 미구현 | 독립 실행 경로 없음 |
| 451 | `gray_5F_dilate` | function | `mc_gray_5F_dilate` | 미구현 | 독립 실행 경로 없음 |
| 452 | `hls_5F_to_5F_rgb` | function | `mc_hls_5F_to_5F_rgb` | 미구현 | 독립 실행 경로 없음 |
| 453 | `horzflip` | function | `mc_horzflip` | 미구현 | 독립 실행 경로 없음 |
| 454 | `hsv_5F_to_5F_rgb` | function | `mc_hsv_5F_to_5F_rgb` | 미구현 | 독립 실행 경로 없음 |
| 455 | `idct2d` | function | `mc_idct2d` | 미구현 | 독립 실행 경로 없음 |
| 456 | `imsnr` | function | `mc_imsnr` | 미구현 | 독립 실행 경로 없음 |
| 457 | `invert` | function | `mc_invert` | 미구현 | 독립 실행 경로 없음 |
| 458 | `invert2` | function | `mc_invert2` | 미구현 | 독립 실행 경로 없음 |
| 459 | `invertimage` | function | `mc_invertimage` | 미구현 | 독립 실행 경로 없음 |
| 460 | `imquant` | function | `mc_imquant` | 미구현 | 독립 실행 경로 없음 |
| 461 | `matdeconv` | function | `mc_matdeconv` | 미구현 | 독립 실행 경로 없음 |
| 462 | `medfilt` | function | `mc_medfilt` | 미구현 | 독립 실행 경로 없음 |
| 463 | `moment_5F_invariant` | function | `mc_moment_5F_invariant` | 미구현 | 독립 실행 경로 없음 |
| 464 | `open` | function | `mc_open` | 미구현 | 독립 실행 경로 없음 |
| 465 | `or` | function | `mc_or` | 미구현 | 독립 실행 경로 없음 |
| 466 | `orthocrisp` | function | `mc_orthocrisp` | 미구현 | 독립 실행 경로 없음 |
| 467 | `orthocrisp5` | function | `mc_orthocrisp5` | 미구현 | 독립 실행 경로 없음 |
| 468 | `orthosmooth` | function | `mc_orthosmooth` | 미구현 | 독립 실행 경로 없음 |
| 469 | `orthosmooth5` | function | `mc_orthosmooth5` | 미구현 | 독립 실행 경로 없음 |
| 470 | `rgb_5F_to_5F_gray` | function | `mc_rgb_5F_to_5F_gray` | 미구현 | 독립 실행 경로 없음 |
| 471 | `rgb_5F_to_5F_hls` | function | `mc_rgb_5F_to_5F_hls` | 미구현 | 독립 실행 경로 없음 |
| 472 | `rgb_5F_to_5F_hsv` | function | `mc_rgb_5F_to_5F_hsv` | 미구현 | 독립 실행 경로 없음 |
| 473 | `laplace24` | function | `mc_laplace24` | 미구현 | 독립 실행 경로 없음 |
| 474 | `levelmap` | function | `mc_levelmap` | 미구현 | 독립 실행 경로 없음 |
| 475 | `mask` | function | `mc_mask` | 미구현 | 독립 실행 경로 없음 |
| 476 | `matconv` | function | `mc_matconv` | 미구현 | 독립 실행 경로 없음 |
| 477 | `prewitt` | function | `mc_prewitt` | 미구현 | 독립 실행 경로 없음 |
| 478 | `putregion` | function | `mc_putregion` | 미구현 | 독립 실행 경로 없음 |
| 479 | `quantfilt` | function | `mc_quantfilt` | 미구현 | 독립 실행 경로 없음 |
| 480 | `relerror` | function | `mc_relerror` | 미구현 | 독립 실행 경로 없음 |
| 481 | `replace` | function | `mc_replace` | 미구현 | 독립 실행 경로 없음 |
| 482 | `reg_5F_grow` | function | `mc_reg_5F_grow` | 미구현 | 독립 실행 경로 없음 |
| 483 | `rgb_5F_to_5F_yiq` | function | `mc_rgb_5F_to_5F_yiq` | 미구현 | 독립 실행 경로 없음 |
| 484 | `rgb_5F_to_5F_ycbcr` | function | `mc_rgb_5F_to_5F_ycbcr` | 미구현 | 독립 실행 경로 없음 |
| 485 | `roberts` | function | `mc_roberts` | 미구현 | 독립 실행 경로 없음 |
| 486 | `robinson3` | function | `mc_robinson3` | 미구현 | 독립 실행 경로 없음 |
| 487 | `rowgrad` | function | `mc_rowgrad` | 미구현 | 독립 실행 경로 없음 |
| 488 | `scale` | function | `mc_scale` | 미구현 | 독립 실행 경로 없음 |
| 489 | `shape_5F_features` | function | `mc_shape_5F_features` | 미구현 | 독립 실행 경로 없음 |
| 490 | `skeleton` | function | `mc_skeleton` | 미구현 | 독립 실행 경로 없음 |
| 491 | `robinson5` | function | `mc_robinson5` | 미구현 | 독립 실행 경로 없음 |
| 492 | `rotate` | function | `mc_rotate` | 미구현 | 독립 실행 경로 없음 |
| 493 | `rotate90` | function | `mc_rotate90` | 미구현 | 독립 실행 경로 없음 |
| 494 | `rotate180` | function | `mc_rotate180` | 미구현 | 독립 실행 경로 없음 |
| 495 | `rotate270` | function | `mc_rotate270` | 미구현 | 독립 실행 경로 없음 |
| 496 | `wave2d` | function | `mc_wave2d` | 미구현 | 독립 실행 경로 없음 |
| 497 | `wavescale` | function | `mc_wavescale` | 미구현 | 독립 실행 경로 없음 |
| 498 | `wiener2d` | function | `mc_wiener2d` | 미구현 | 독립 실행 경로 없음 |
| 499 | `skeleton2` | function | `mc_skeleton2` | 미구현 | 독립 실행 경로 없음 |
| 500 | `sobel` | function | `mc_sobel` | 미구현 | 독립 실행 경로 없음 |
| 501 | `subcolor` | function | `mc_subcolor` | 미구현 | 독립 실행 경로 없음 |
| 502 | `thin` | function | `mc_thin` | 미구현 | 독립 실행 경로 없음 |
| 503 | `threshold` | function | `mc_threshold` | 미구현 | 독립 실행 경로 없음 |
| 504 | `translate` | function | `mc_translate` | 미구현 | 독립 실행 경로 없음 |
| 505 | `unicrisp` | function | `mc_unicrisp` | 미구현 | 독립 실행 경로 없음 |
| 506 | `unismooth` | function | `mc_unismooth` | 미구현 | 독립 실행 경로 없음 |
| 507 | `unismooth5` | function | `mc_unismooth5` | 미구현 | 독립 실행 경로 없음 |
| 508 | `vertflip` | function | `mc_vertflip` | 미구현 | 독립 실행 경로 없음 |
| 509 | `warp` | function | `mc_warp` | 미구현 | 독립 실행 경로 없음 |
| 510 | `ycbcr_5F_to_5F_rgb` | function | `mc_ycbcr_5F_to_5F_rgb` | 미구현 | 독립 실행 경로 없음 |
| 511 | `yiq_5F_to_5F_rgb` | function | `mc_yiq_5F_to_5F_rgb` | 미구현 | 독립 실행 경로 없음 |
| 512 | `zoom` | function | `mc_zoom` | 미구현 | 독립 실행 경로 없음 |
| 513 | `READRAW` | function | `mc_READRAW` | 미구현 | 독립 실행 경로 없음 |
| 514 | `WRITERAW` | function | `mc_WRITERAW` | 미구현 | 독립 실행 경로 없음 |
| 515 | `idct` | function | `mc_idct` | 미구현 | 독립 실행 경로 없음 |
| 516 | `crosscorr` | function | `mc_crosscorr` | 미구현 | 독립 실행 경로 없음 |
| 517 | `convolve` | function | `mc_convolve` | 미구현 | 독립 실행 경로 없음 |
| 518 | `dct` | function | `mc_dct` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efiio — 42개</summary>

원본 근거: `ev_f74133fb014393104a639309c03dc5e2e4a11fa9662bd2c9033ba7af20f7558e`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 519 | `READPRN` | function | `mc_READPRN` | 미구현 | 독립 실행 경로 없음 |
| 520 | `WRITEPRN` | function | `mc_WRITEPRN` | 미구현 | 독립 실행 경로 없음 |
| 521 | `READ_5F_RED` | function | `mc_READ_5F_RED` | 미구현 | 독립 실행 경로 없음 |
| 522 | `READ_5F_GREEN` | function | `mc_READ_5F_GREEN` | 미구현 | 독립 실행 경로 없음 |
| 523 | `READ_5F_BLUE` | function | `mc_READ_5F_BLUE` | 미구현 | 독립 실행 경로 없음 |
| 524 | `WRITERGB` | function | `mc_WRITERGB` | 미구현 | 독립 실행 경로 없음 |
| 525 | `APPENDPRN` | function | `mc_APPENDPRN` | 미구현 | 독립 실행 경로 없음 |
| 526 | `READBMP` | function | `mc_READBMP` | 미구현 | 독립 실행 경로 없음 |
| 527 | `WRITEBMP` | function | `mc_WRITEBMP` | 미구현 | 독립 실행 경로 없음 |
| 528 | `READRGB` | function | `mc_READRGB` | 미구현 | 독립 실행 경로 없음 |
| 529 | `READ_5F_HLS_5F_LIGHT` | function | `mc_READ_5F_HLS_5F_LIGHT` | 미구현 | 독립 실행 경로 없음 |
| 530 | `READ_5F_HSV_5F_HUE` | function | `mc_READ_5F_HSV_5F_HUE` | 미구현 | 독립 실행 경로 없음 |
| 531 | `READ_5F_HSV_5F_VALUE` | function | `mc_READ_5F_HSV_5F_VALUE` | 미구현 | 독립 실행 경로 없음 |
| 532 | `READ_5F_HLS` | function | `mc_READ_5F_HLS` | 미구현 | 독립 실행 경로 없음 |
| 533 | `READ_5F_HLS_5F_SAT` | function | `mc_READ_5F_HLS_5F_SAT` | 미구현 | 독립 실행 경로 없음 |
| 534 | `READ_5F_HSV` | function | `mc_READ_5F_HSV` | 미구현 | 독립 실행 경로 없음 |
| 535 | `READ_5F_HSV_5F_SAT` | function | `mc_READ_5F_HSV_5F_SAT` | 미구현 | 독립 실행 경로 없음 |
| 536 | `READ_5F_IMAGE` | function | `mc_READ_5F_IMAGE` | 미구현 | 독립 실행 경로 없음 |
| 537 | `READ_5F_HUE` | function | `mc_READ_5F_HUE` | 미구현 | 독립 실행 경로 없음 |
| 538 | `READ_5F_LIGHT` | function | `mc_READ_5F_LIGHT` | 미구현 | 독립 실행 경로 없음 |
| 539 | `READ_5F_VALUE` | function | `mc_READ_5F_VALUE` | 미구현 | 독립 실행 경로 없음 |
| 540 | `READ_5F_HLS_5F_HUE` | function | `mc_READ_5F_HLS_5F_HUE` | 미구현 | 독립 실행 경로 없음 |
| 541 | `WRITE_5F_HLS` | function | `mc_WRITE_5F_HLS` | 미구현 | 독립 실행 경로 없음 |
| 542 | `WRITE_5F_HSV` | function | `mc_WRITE_5F_HSV` | 미구현 | 독립 실행 경로 없음 |
| 543 | `READBIN` | function | `mc_READBIN` | 미구현 | 독립 실행 경로 없음 |
| 544 | `WRITEBIN` | function | `mc_WRITEBIN` | 미구현 | 독립 실행 경로 없음 |
| 545 | `GETWAVINFO` | function | `mc_GETWAVINFO` | 미구현 | 독립 실행 경로 없음 |
| 546 | `READWAV` | function | `mc_READWAV` | 미구현 | 독립 실행 경로 없음 |
| 547 | `WRITEWAV` | function | `mc_WRITEWAV` | 미구현 | 독립 실행 경로 없음 |
| 548 | `LoadColormap` | function | `mc_LoadColormap` | 미구현 | 독립 실행 경로 없음 |
| 549 | `SaveColormap` | function | `mc_SaveColormap` | 미구현 | 독립 실행 경로 없음 |
| 550 | `READDATA` | function | `mc_READDATA` | 미구현 | 독립 실행 경로 없음 |
| 551 | `WRITEDATA` | function | `mc_WRITEDATA` | 미구현 | 독립 실행 경로 없음 |
| 552 | `WRITEFILE` | function | `mc_WRITEFILE` | 미구현 | 독립 실행 경로 없음 |
| 553 | `APPENDDATA` | function | `mc_APPENDDATA` | 미구현 | 독립 실행 경로 없음 |
| 554 | `WRITECSV` | function | `mc_WRITECSV` | 미구현 | 독립 실행 경로 없음 |
| 555 | `READCSV` | function | `mc_READCSV` | 미구현 | 독립 실행 경로 없음 |
| 556 | `READEXCEL` | function | `mc_READEXCEL` | 미구현 | 독립 실행 경로 없음 |
| 557 | `WRITETEXT` | function | `mc_WRITETEXT` | 미구현 | 독립 실행 경로 없음 |
| 558 | `WRITEEXCEL` | function | `mc_WRITEEXCEL` | 미구현 | 독립 실행 경로 없음 |
| 559 | `READFILE` | function | `mc_READFILE` | 미구현 | 독립 실행 경로 없음 |
| 560 | `READTEXT` | function | `mc_READTEXT` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efiplot — 12개</summary>

원본 근거: `ev_8df3023e2bf18f7991392b613b02474a7030087a1888824a2a05ad23c24c15e6`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 561 | `eval_xy_plot` | operator | `op_eval_xy_plot` | 미구현 | 독립 실행 경로 없음 |
| 562 | `format_xy_plot` | operator | `op_format_xy_plot` | 미구현 | 독립 실행 경로 없음 |
| 563 | `parametric_plot` | operator | `op_parametric_plot` | 미구현 | 독립 실행 경로 없음 |
| 564 | `parametric_quickplot` | operator | `op_parametric_quickplot` | 미구현 | 독립 실행 경로 없음 |
| 565 | `contour` | operator | `op_contour` | 미구현 | 독립 실행 경로 없음 |
| 566 | `plot3d` | operator | `op_plot3d` | 미구현 | 독립 실행 경로 없음 |
| 567 | `range_eval` | operator | `op_range_eval` | 미구현 | 독립 실행 경로 없음 |
| 568 | `logspace` | function | `mc_logspace` | 미구현 | 독립 실행 경로 없음 |
| 569 | `logpts` | function | `mc_logpts` | 미구현 | 독립 실행 경로 없음 |
| 570 | `scale_xy_plot` | operator | `op_scale_xy_plot` | 미구현 | 독립 실행 경로 없음 |
| 571 | `xy_plot` | operator | `op_xy_plot` | 미구현 | 독립 실행 경로 없음 |
| 572 | `xy_plot2` | operator | `op_xy_plot2` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efisignal — 76개</summary>

원본 근거: `ev_afd22eb31e437d1d4f20ceb3a814380c6b7ebae2edc838d37360fa02dc0877b0`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 573 | `bandpass` | function | `mc_bandpass` | 미구현 | 독립 실행 경로 없음 |
| 574 | `bandstop` | function | `mc_bandstop` | 미구현 | 독립 실행 경로 없음 |
| 575 | `costr` | function | `mc_costr` | 미구현 | 독립 실행 경로 없음 |
| 576 | `covar` | function | `mc_covar` | 미구현 | 독립 실행 경로 없음 |
| 577 | `cspectrum` | function | `mc_cspectrum` | 미구현 | 독립 실행 경로 없음 |
| 578 | `burg` | function | `mc_burg` | 미구현 | 독립 실행 경로 없음 |
| 579 | `ccepstrum` | function | `mc_ccepstrum` | 미구현 | 독립 실행 경로 없음 |
| 580 | `cepstrum` | function | `mc_cepstrum` | 미구현 | 독립 실행 경로 없음 |
| 581 | `chirpz` | function | `mc_chirpz` | 미구현 | 독립 실행 경로 없음 |
| 582 | `coherence` | function | `mc_coherence` | 미구현 | 독립 실행 경로 없음 |
| 583 | `convol` | function | `mc_convol` | 미구현 | 독립 실행 경로 없음 |
| 584 | `deconvol` | function | `mc_deconvol` | 미구현 | 독립 실행 경로 없음 |
| 585 | `deconvolve` | function | `mc_deconvolve` | 미구현 | 독립 실행 경로 없음 |
| 586 | `detrend` | function | `mc_detrend` | 미구현 | 독립 실행 경로 없음 |
| 587 | `dwavelet` | function | `mc_dwavelet` | 미구현 | 독립 실행 경로 없음 |
| 588 | `highpass` | function | `mc_highpass` | 미구현 | 독립 실행 경로 없음 |
| 589 | `hilbert` | function | `mc_hilbert` | 미구현 | 독립 실행 경로 없음 |
| 590 | `icostr` | function | `mc_icostr` | 미구현 | 독립 실행 경로 없음 |
| 591 | `idwavelet` | function | `mc_idwavelet` | 미구현 | 독립 실행 경로 없음 |
| 592 | `expsmooth` | function | `mc_expsmooth` | 미구현 | 독립 실행 경로 없음 |
| 593 | `fftfilt` | function | `mc_fftfilt` | 미구현 | 독립 실행 경로 없음 |
| 594 | `gain` | function | `mc_gain` | 미구현 | 독립 실행 경로 없음 |
| 595 | `dht` | function | `mc_dht` | 미구현 | 독립 실행 경로 없음 |
| 596 | `lcorr` | function | `mc_lcorr` | 미구현 | 독립 실행 경로 없음 |
| 597 | `lowpass` | function | `mc_lowpass` | 미구현 | 독립 실행 경로 없음 |
| 598 | `mag` | function | `mc_mag` | 미구현 | 독립 실행 경로 없음 |
| 599 | `interpolate` | function | `mc_interpolate` | 미구현 | 독립 실행 경로 없음 |
| 600 | `upsample` | function | `mc_upsample` | 미구현 | 독립 실행 경로 없음 |
| 601 | `idht` | function | `mc_idht` | 미구현 | 독립 실행 경로 없음 |
| 602 | `isintr` | function | `mc_isintr` | 미구현 | 독립 실행 경로 없음 |
| 603 | `makeri` | function | `mc_makeri` | 미구현 | 독립 실행 경로 없음 |
| 604 | `medfilt1d` | function | `mc_medfilt1d` | 미구현 | 독립 실행 경로 없음 |
| 605 | `movavg` | function | `mc_movavg` | 미구현 | 독립 실행 경로 없음 |
| 606 | `multirate` | function | `mc_multirate` | 미구현 | 독립 실행 경로 없음 |
| 607 | `phase` | function | `mc_phase` | 미구현 | 독립 실행 경로 없음 |
| 608 | `phasecor` | function | `mc_phasecor` | 미구현 | 독립 실행 경로 없음 |
| 609 | `plcorr` | function | `mc_plcorr` | 미구현 | 독립 실행 경로 없음 |
| 610 | `pspectrum` | function | `mc_pspectrum` | 미구현 | 독립 실행 경로 없음 |
| 611 | `quantize` | function | `mc_quantize` | 미구현 | 독립 실행 경로 없음 |
| 612 | `recenter` | function | `mc_recenter` | 미구현 | 독립 실행 경로 없음 |
| 613 | `timefreq` | function | `mc_timefreq` | 미구현 | 독립 실행 경로 없음 |
| 614 | `walsh` | function | `mc_walsh` | 미구현 | 독립 실행 경로 없음 |
| 615 | `yulew` | function | `mc_yulew` | 미구현 | 독립 실행 경로 없음 |
| 616 | `gaussn` | function | `mc_gaussn` | 미구현 | 독립 실행 경로 없음 |
| 617 | `snr` | function | `mc_snr` | 미구현 | 독립 실행 경로 없음 |
| 618 | `stft` | function | `mc_stft` | 미구현 | 독립 실행 경로 없음 |
| 619 | `remez` | function | `mc_remez` | 미구현 | 독립 실행 경로 없음 |
| 620 | `resample` | function | `mc_resample` | 미구현 | 독립 실행 경로 없음 |
| 621 | `response` | function | `mc_response` | 미구현 | 독립 실행 경로 없음 |
| 622 | `sintr` | function | `mc_sintr` | 미구현 | 독립 실행 경로 없음 |
| 623 | `twodconvol` | function | `mc_twodconvol` | 미구현 | 독립 실행 경로 없음 |
| 624 | `timecorr` | function | `mc_timecorr` | 미구현 | 독립 실행 경로 없음 |
| 625 | `filtfilt` | function | `mc_filtfilt` | 미구현 | 독립 실행 경로 없음 |
| 626 | `bessel` | function | `mc_bessel` | 미구현 | 독립 실행 경로 없음 |
| 627 | `butter` | function | `mc_butter` | 미구현 | 독립 실행 경로 없음 |
| 628 | `blackman` | function | `mc_blackman` | 미구현 | 독립 실행 경로 없음 |
| 629 | `cheby` | function | `mc_cheby` | 미구현 | 독립 실행 경로 없음 |
| 630 | `iirpass` | function | `mc_iirpass` | 미구현 | 독립 실행 경로 없음 |
| 631 | `iirstop` | function | `mc_iirstop` | 미구현 | 독립 실행 경로 없음 |
| 632 | `kaiser` | function | `mc_kaiser` | 미구현 | 독립 실행 경로 없음 |
| 633 | `cheby1` | function | `mc_cheby1` | 미구현 | 독립 실행 경로 없음 |
| 634 | `cheby2` | function | `mc_cheby2` | 미구현 | 독립 실행 경로 없음 |
| 635 | `costaper` | function | `mc_costaper` | 미구현 | 독립 실행 경로 없음 |
| 636 | `onefn` | function | `mc_onefn` | 미구현 | 독립 실행 경로 없음 |
| 637 | `whiten` | function | `mc_whiten` | 미구현 | 독립 실행 경로 없음 |
| 638 | `music` | function | `mc_music` | 미구현 | 독립 실행 경로 없음 |
| 639 | `grpdelay` | function | `mc_grpdelay` | 미구현 | 독립 실행 경로 없음 |
| 640 | `gaussian` | function | `mc_gaussian` | 미구현 | 독립 실행 경로 없음 |
| 641 | `hamming` | function | `mc_hamming` | 미구현 | 독립 실행 경로 없음 |
| 642 | `hanning` | function | `mc_hanning` | 미구현 | 독립 실행 경로 없음 |
| 643 | `iirhigh` | function | `mc_iirhigh` | 미구현 | 독립 실행 경로 없음 |
| 644 | `iirlow` | function | `mc_iirlow` | 미구현 | 독립 실행 경로 없음 |
| 645 | `nuttall` | function | `mc_nuttall` | 미구현 | 독립 실행 경로 없음 |
| 646 | `setwindow` | function | `mc_setwindow` | 미구현 | 독립 실행 경로 없음 |
| 647 | `taprect` | function | `mc_taprect` | 미구현 | 독립 실행 경로 없음 |
| 648 | `triangular` | function | `mc_triangular` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efisixsigma — 34개</summary>

원본 근거: `ev_b0f56771bcece1ce046991abf73b3fbc5be8d07ad281eec8dd3254be6f49cda2`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 649 | `fullfact` | function | `mc_fullfact` | 미구현 | 독립 실행 경로 없음 |
| 650 | `fractfact` | function | `mc_fractfact` | 미구현 | 독립 실행 경로 없음 |
| 651 | `fractresol` | function | `mc_fractresol` | 미구현 | 독립 실행 경로 없음 |
| 652 | `fractalias` | function | `mc_fractalias` | 미구현 | 독립 실행 경로 없음 |
| 653 | `boxwilson` | function | `mc_boxwilson` | 미구현 | 독립 실행 경로 없음 |
| 654 | `plackettburman` | function | `mc_plackettburman` | 미구현 | 독립 실행 경로 없음 |
| 655 | `boxplot` | function | `mc_boxplot` | 미구현 | 독립 실행 경로 없음 |
| 656 | `boxplotgraph` | function | `mc_boxplotgraph` | 미구현 | 독립 실행 경로 없음 |
| 657 | `pareto` | function | `mc_pareto` | 미구현 | 독립 실행 경로 없음 |
| 658 | `block` | function | `mc_block` | 미구현 | 독립 실행 경로 없음 |
| 659 | `anova` | function | `mc_anova` | 미구현 | 독립 실행 경로 없음 |
| 660 | `boxbehnken` | function | `mc_boxbehnken` | 미구현 | 독립 실행 경로 없음 |
| 661 | `fractruns` | function | `mc_fractruns` | 미구현 | 독립 실행 경로 없음 |
| 662 | `mtaguchi` | function | `mc_mtaguchi` | 미구현 | 독립 실행 경로 없음 |
| 663 | `randomize` | function | `mc_randomize` | 미구현 | 독립 실행 경로 없음 |
| 664 | `taguchi` | function | `mc_taguchi` | 미구현 | 독립 실행 경로 없음 |
| 665 | `quickscreen` | function | `mc_quickscreen` | 미구현 | 독립 실행 경로 없음 |
| 666 | `effects` | function | `mc_effects` | 미구현 | 독립 실행 경로 없음 |
| 667 | `effectsgraph` | function | `mc_effectsgraph` | 미구현 | 독립 실행 경로 없음 |
| 668 | `polylmfitc` | function | `mc_polylmfitc` | 미구현 | 독립 실행 경로 없음 |
| 669 | `multidfit` | function | `mc_multidfit` | 미구현 | 독립 실행 경로 없음 |
| 670 | `montecarlo` | function | `mc_montecarlo` | 미구현 | 독립 실행 경로 없음 |
| 671 | `doelabel` | function | `mc_doelabel` | 미구현 | 독립 실행 경로 없음 |
| 672 | `foldover` | function | `mc_foldover` | 미구현 | 독립 실행 경로 없음 |
| 673 | `fullfact2` | function | `mc_fullfact2` | 미구현 | 독립 실행 경로 없음 |
| 674 | `polyfitstat` | function | `mc_polyfitstat` | 미구현 | 독립 실행 경로 없음 |
| 675 | `polylmfit` | function | `mc_polylmfit` | 미구현 | 독립 실행 경로 없음 |
| 676 | `polyfit` | function | `mc_polyfit` | 미구현 | 독립 실행 경로 없음 |
| 677 | `polyfitc` | function | `mc_polyfitc` | 미구현 | 독립 실행 경로 없음 |
| 678 | `plotfilter` | function | `mc_plotfilter` | 미구현 | 독립 실행 경로 없음 |
| 679 | `plot3d` | function | `mc_plot3d` | 미구현 | 독립 실행 경로 없음 |
| 680 | `fractfact2` | function | `mc_fractfact2` | 미구현 | 독립 실행 경로 없음 |
| 681 | `boxwilson2` | function | `mc_boxwilson2` | 미구현 | 독립 실행 경로 없음 |
| 682 | `contour` | function | `mc_contour` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efisolver — 20개</summary>

원본 근거: `ev_a7357d8df6fd2002b102fb8a255c3656fbeb730e29291bcc2a5bf7ea76ada45f`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 683 | `solve_inner` | operator | `op_solve_inner` | 미구현 | 독립 실행 경로 없음 |
| 684 | `solve` | operator | `op_solve` | 미구현 | 독립 실행 경로 없음 |
| 685 | `solveblock` | operator | `op_solveblock` | 미구현 | 독립 실행 경로 없음 |
| 686 | `solveblock_cb` | operator | `op_solveblock_cb` | 미구현 | 독립 실행 경로 없음 |
| 687 | `find_tup` | operator | `op_find_tup` | 미구현 | 독립 실행 경로 없음 |
| 688 | `minerr_tup` | operator | `op_minerr_tup` | 미구현 | 독립 실행 경로 없음 |
| 689 | `minimize_tup` | operator | `op_minimize_tup` | 미구현 | 독립 실행 경로 없음 |
| 690 | `maximize_tup` | operator | `op_maximize_tup` | 미구현 | 독립 실행 경로 없음 |
| 691 | `find` | operator | `op_find` | 부분 구현 | N09 · scalar 실수 등식의 native solve 경로 |
| 692 | `minerr` | operator | `op_minerr` | 부분 구현 | N09 · scalar 실수 등식의 native solve 경로 |
| 693 | `minimize` | operator | `op_minimize` | 미구현 | 독립 실행 경로 없음 |
| 694 | `maximize` | operator | `op_maximize` | 미구현 | 독립 실행 경로 없음 |
| 695 | `find_tup_cb` | operator | `op_find_tup_cb` | 미구현 | 독립 실행 경로 없음 |
| 696 | `minerr_tup_cb` | operator | `op_minerr_tup_cb` | 미구현 | 독립 실행 경로 없음 |
| 697 | `minimize_tup_cb` | operator | `op_minimize_tup_cb` | 미구현 | 독립 실행 경로 없음 |
| 698 | `maximize_tup_cb` | operator | `op_maximize_tup_cb` | 미구현 | 독립 실행 경로 없음 |
| 699 | `find_cb` | operator | `op_find_cb` | 미구현 | 독립 실행 경로 없음 |
| 700 | `minerr_cb` | operator | `op_minerr_cb` | 미구현 | 독립 실행 경로 없음 |
| 701 | `minimize_cb` | operator | `op_minimize_cb` | 미구현 | 독립 실행 경로 없음 |
| 702 | `maximize_cb` | operator | `op_maximize_cb` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efispecial — 14개</summary>

원본 근거: `ev_5652916b5dd0558fb5dd3493051fbd32cb30ab010301992c529a85e89761fdf8`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 703 | `Lag` | function | `mc_Lag` | 미구현 | 독립 실행 경로 없음 |
| 704 | `Leg` | function | `mc_Leg` | 미구현 | 독립 실행 경로 없음 |
| 705 | `Tcheb` | function | `mc_Tcheb` | 미구현 | 독립 실행 경로 없음 |
| 706 | `Ucheb` | function | `mc_Ucheb` | 미구현 | 독립 실행 경로 없음 |
| 707 | `bei` | function | `mc_bei` | 미구현 | 독립 실행 경로 없음 |
| 708 | `ber` | function | `mc_ber` | 미구현 | 독립 실행 경로 없음 |
| 709 | `mhyper` | function | `mc_mhyper` | 미구현 | 독립 실행 경로 없음 |
| 710 | `ibeta` | function | `mc_ibeta` | 미구현 | 독립 실행 경로 없음 |
| 711 | `Jac` | function | `mc_Jac` | 미구현 | 독립 실행 경로 없음 |
| 712 | `Her` | function | `mc_Her` | 미구현 | 독립 실행 경로 없음 |
| 713 | `Γ` | function | `mc_Γ` | 미구현 | 독립 실행 경로 없음 |
| 714 | `lnΓ` | function | `mc_lnΓ` | 미구현 | 독립 실행 경로 없음 |
| 715 | `Psi` | function | `mc_Psi` | 미구현 | 독립 실행 경로 없음 |
| 716 | `fhyper` | function | `mc_fhyper` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efistatistics — 94개</summary>

원본 근거: `ev_ac11427460444eab5961abaef5db229c5e6cf3de53be6a39bdf01724ed5dc6ea`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 717 | `dbeta` | function | `mc_dbeta` | 미구현 | 독립 실행 경로 없음 |
| 718 | `dbinom` | function | `mc_dbinom` | 미구현 | 독립 실행 경로 없음 |
| 719 | `dcauchy` | function | `mc_dcauchy` | 미구현 | 독립 실행 경로 없음 |
| 720 | `dchisq` | function | `mc_dchisq` | 미구현 | 독립 실행 경로 없음 |
| 721 | `dexp` | function | `mc_dexp` | 미구현 | 독립 실행 경로 없음 |
| 722 | `dF` | function | `mc_dF` | 미구현 | 독립 실행 경로 없음 |
| 723 | `dnorm` | function | `mc_dnorm` | 미구현 | 독립 실행 경로 없음 |
| 724 | `dpois` | function | `mc_dpois` | 미구현 | 독립 실행 경로 없음 |
| 725 | `dt` | function | `mc_dt` | 미구현 | 독립 실행 경로 없음 |
| 726 | `dgamma` | function | `mc_dgamma` | 미구현 | 독립 실행 경로 없음 |
| 727 | `dgeom` | function | `mc_dgeom` | 미구현 | 독립 실행 경로 없음 |
| 728 | `dhypergeom` | function | `mc_dhypergeom` | 미구현 | 독립 실행 경로 없음 |
| 729 | `dlnorm` | function | `mc_dlnorm` | 미구현 | 독립 실행 경로 없음 |
| 730 | `dlogis` | function | `mc_dlogis` | 미구현 | 독립 실행 경로 없음 |
| 731 | `dnbinom` | function | `mc_dnbinom` | 미구현 | 독립 실행 경로 없음 |
| 732 | `phypergeom` | function | `mc_phypergeom` | 미구현 | 독립 실행 경로 없음 |
| 733 | `plnorm` | function | `mc_plnorm` | 미구현 | 독립 실행 경로 없음 |
| 734 | `plogis` | function | `mc_plogis` | 미구현 | 독립 실행 경로 없음 |
| 735 | `dunif` | function | `mc_dunif` | 미구현 | 독립 실행 경로 없음 |
| 736 | `dweibull` | function | `mc_dweibull` | 미구현 | 독립 실행 경로 없음 |
| 737 | `pbeta` | function | `mc_pbeta` | 미구현 | 독립 실행 경로 없음 |
| 738 | `pbinom` | function | `mc_pbinom` | 미구현 | 독립 실행 경로 없음 |
| 739 | `pcauchy` | function | `mc_pcauchy` | 미구현 | 독립 실행 경로 없음 |
| 740 | `pchisq` | function | `mc_pchisq` | 미구현 | 독립 실행 경로 없음 |
| 741 | `pexp` | function | `mc_pexp` | 미구현 | 독립 실행 경로 없음 |
| 742 | `pF` | function | `mc_pF` | 미구현 | 독립 실행 경로 없음 |
| 743 | `pgamma` | function | `mc_pgamma` | 미구현 | 독립 실행 경로 없음 |
| 744 | `pgeom` | function | `mc_pgeom` | 미구현 | 독립 실행 경로 없음 |
| 745 | `pnbinom` | function | `mc_pnbinom` | 미구현 | 독립 실행 경로 없음 |
| 746 | `pnorm` | function | `mc_pnorm` | 미구현 | 독립 실행 경로 없음 |
| 747 | `ppois` | function | `mc_ppois` | 미구현 | 독립 실행 경로 없음 |
| 748 | `pt` | function | `mc_pt` | 미구현 | 독립 실행 경로 없음 |
| 749 | `punif` | function | `mc_punif` | 미구현 | 독립 실행 경로 없음 |
| 750 | `pweibull` | function | `mc_pweibull` | 미구현 | 독립 실행 경로 없음 |
| 751 | `qbeta` | function | `mc_qbeta` | 미구현 | 독립 실행 경로 없음 |
| 752 | `qhypergeom` | function | `mc_qhypergeom` | 미구현 | 독립 실행 경로 없음 |
| 753 | `qlnorm` | function | `mc_qlnorm` | 미구현 | 독립 실행 경로 없음 |
| 754 | `qlogis` | function | `mc_qlogis` | 미구현 | 독립 실행 경로 없음 |
| 755 | `qnbinom` | function | `mc_qnbinom` | 미구현 | 독립 실행 경로 없음 |
| 756 | `qnorm` | function | `mc_qnorm` | 미구현 | 독립 실행 경로 없음 |
| 757 | `qpois` | function | `mc_qpois` | 미구현 | 독립 실행 경로 없음 |
| 758 | `qbinom` | function | `mc_qbinom` | 미구현 | 독립 실행 경로 없음 |
| 759 | `qcauchy` | function | `mc_qcauchy` | 미구현 | 독립 실행 경로 없음 |
| 760 | `qchisq` | function | `mc_qchisq` | 미구현 | 독립 실행 경로 없음 |
| 761 | `qexp` | function | `mc_qexp` | 미구현 | 독립 실행 경로 없음 |
| 762 | `qF` | function | `mc_qF` | 미구현 | 독립 실행 경로 없음 |
| 763 | `qgamma` | function | `mc_qgamma` | 미구현 | 독립 실행 경로 없음 |
| 764 | `qgeom` | function | `mc_qgeom` | 미구현 | 독립 실행 경로 없음 |
| 765 | `rgeom` | function | `mc_rgeom` | 미구현 | 독립 실행 경로 없음 |
| 766 | `rhypergeom` | function | `mc_rhypergeom` | 미구현 | 독립 실행 경로 없음 |
| 767 | `rlnorm` | function | `mc_rlnorm` | 미구현 | 독립 실행 경로 없음 |
| 768 | `qt` | function | `mc_qt` | 미구현 | 독립 실행 경로 없음 |
| 769 | `qunif` | function | `mc_qunif` | 미구현 | 독립 실행 경로 없음 |
| 770 | `qweibull` | function | `mc_qweibull` | 미구현 | 독립 실행 경로 없음 |
| 771 | `rbeta` | function | `mc_rbeta` | 미구현 | 독립 실행 경로 없음 |
| 772 | `rbinom` | function | `mc_rbinom` | 미구현 | 독립 실행 경로 없음 |
| 773 | `rcauchy` | function | `mc_rcauchy` | 미구현 | 독립 실행 경로 없음 |
| 774 | `rchisq` | function | `mc_rchisq` | 미구현 | 독립 실행 경로 없음 |
| 775 | `rexp` | function | `mc_rexp` | 미구현 | 독립 실행 경로 없음 |
| 776 | `rF` | function | `mc_rF` | 미구현 | 독립 실행 경로 없음 |
| 777 | `rgamma` | function | `mc_rgamma` | 미구현 | 독립 실행 경로 없음 |
| 778 | `rpois` | function | `mc_rpois` | 미구현 | 독립 실행 경로 없음 |
| 779 | `rt` | function | `mc_rt` | 미구현 | 독립 실행 경로 없음 |
| 780 | `runif` | function | `mc_runif` | 미구현 | 독립 실행 경로 없음 |
| 781 | `cvar` | function | `mc_cvar` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 782 | `gmean` | function | `mc_gmean` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 783 | `hist` | function | `mc_hist` | 미구현 | 독립 실행 경로 없음 |
| 784 | `rlogis` | function | `mc_rlogis` | 미구현 | 독립 실행 경로 없음 |
| 785 | `rnbinom` | function | `mc_rnbinom` | 미구현 | 독립 실행 경로 없음 |
| 786 | `rnorm` | function | `mc_rnorm` | 미구현 | 독립 실행 경로 없음 |
| 787 | `rweibull` | function | `mc_rweibull` | 미구현 | 독립 실행 경로 없음 |
| 788 | `Uniform` | function | `mc_Uniform` | 미구현 | 독립 실행 경로 없음 |
| 789 | `Normal` | function | `mc_Normal` | 미구현 | 독립 실행 경로 없음 |
| 790 | `LogNormal` | function | `mc_LogNormal` | 미구현 | 독립 실행 경로 없음 |
| 791 | `Weibull` | function | `mc_Weibull` | 미구현 | 독립 실행 경로 없음 |
| 792 | `combin` | function | `mc_combin` | 미구현 | 독립 실행 경로 없음 |
| 793 | `corr` | function | `mc_corr` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 794 | `correl` | function | `mc_correl` | 미구현 | 독립 실행 경로 없음 |
| 795 | `correl2d` | function | `mc_correl2d` | 미구현 | 독립 실행 경로 없음 |
| 796 | `histogram` | function | `mc_histogram` | 미구현 | 독립 실행 경로 없음 |
| 797 | `kurt` | function | `mc_kurt` | 미구현 | 독립 실행 경로 없음 |
| 798 | `hmean` | function | `mc_hmean` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 799 | `rnd` | function | `mc_rnd` | 미구현 | 독립 실행 경로 없음 |
| 800 | `Seed` | function | `mc_Seed` | 미구현 | 독립 실행 경로 없음 |
| 801 | `Stdev` | function | `mc_Stdev` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 802 | `var` | function | `mc_var` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 803 | `Var` | function | `mc_Var` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 804 | `permut` | function | `mc_permut` | 미구현 | 독립 실행 경로 없음 |
| 805 | `predict` | function | `mc_predict` | 미구현 | 독립 실행 경로 없음 |
| 806 | `stderr` | function | `mc_stderr` | 미구현 | 독립 실행 경로 없음 |
| 807 | `stdev` | function | `mc_stdev` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 808 | `mean` | function | `mc_mean` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 809 | `median` | function | `mc_median` | 부분 구현 | 이름 dispatch 존재; 본문 분야별 타입·크기·알고리즘 제한 적용 |
| 810 | `mode` | function | `mc_mode` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efisymbolics — 0개</summary>

원본 근거: `ev_4ec3934858bc4638131f3493b6269329bb4019a7edefbf161618518f3e8e01c4`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |

</details>

<details>
<summary>efiwavelets — 58개</summary>

원본 근거: `ev_70e6e9fc8cc029fe391f1420c5259fa2355970b0da9d7c5194a0eaa1813d4c67`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 811 | `applybs` | function | `mc_applybs` | 미구현 | 독립 실행 경로 없음 |
| 812 | `applytbl` | function | `mc_applytbl` | 미구현 | 독립 실행 경로 없음 |
| 813 | `basis_5F_display2d` | function | `mc_basis_5F_display2d` | 미구현 | 독립 실행 경로 없음 |
| 814 | `best_5F_basis` | function | `mc_best_5F_basis` | 미구현 | 독립 실행 경로 없음 |
| 815 | `bl` | function | `mc_bl` | 미구현 | 독립 실행 경로 없음 |
| 816 | `Bspline` | function | `mc_Bspline` | 미구현 | 독립 실행 경로 없음 |
| 817 | `coiflet` | function | `mc_coiflet` | 미구현 | 독립 실행 경로 없음 |
| 818 | `create_5F_level` | function | `mc_create_5F_level` | 미구현 | 독립 실행 경로 없음 |
| 819 | `evaltbl` | function | `mc_evaltbl` | 미구현 | 독립 실행 경로 없음 |
| 820 | `extract_5F_basis` | function | `mc_extract_5F_basis` | 미구현 | 독립 실행 경로 없음 |
| 821 | `dlmother` | function | `mc_dlmother` | 미구현 | 독립 실행 경로 없음 |
| 822 | `dlmother2d` | function | `mc_dlmother2d` | 미구현 | 독립 실행 경로 없음 |
| 823 | `lct` | function | `mc_lct` | 미구현 | 독립 실행 경로 없음 |
| 824 | `ilct` | function | `mc_ilct` | 미구현 | 독립 실행 경로 없음 |
| 825 | `icpt` | function | `mc_icpt` | 미구현 | 독립 실행 경로 없음 |
| 826 | `symmlet` | function | `mc_symmlet` | 미구현 | 독립 실행 경로 없음 |
| 827 | `daublet` | function | `mc_daublet` | 미구현 | 독립 실행 경로 없음 |
| 828 | `cpt` | function | `mc_cpt` | 미구현 | 독립 실행 경로 없음 |
| 829 | `dlfather` | function | `mc_dlfather` | 미구현 | 독립 실행 경로 없음 |
| 830 | `dlfather2d` | function | `mc_dlfather2d` | 미구현 | 독립 실행 경로 없음 |
| 831 | `dlwavelet2d` | function | `mc_dlwavelet2d` | 미구현 | 독립 실행 경로 없음 |
| 832 | `dwt` | function | `mc_dwt` | 미구현 | 독립 실행 경로 없음 |
| 833 | `dwti` | function | `mc_dwti` | 미구현 | 독립 실행 경로 없음 |
| 834 | `dwts` | function | `mc_dwts` | 미구현 | 독립 실행 경로 없음 |
| 835 | `evalbs` | function | `mc_evalbs` | 미구현 | 독립 실행 경로 없음 |
| 836 | `dlwavelet` | function | `mc_dlwavelet` | 미구현 | 독립 실행 경로 없음 |
| 837 | `wavelet` | function | `mc_wavelet` | 미구현 | 독립 실행 경로 없음 |
| 838 | `father` | function | `mc_father` | 미구현 | 독립 실행 경로 없음 |
| 839 | `wavelet2d` | function | `mc_wavelet2d` | 미구현 | 독립 실행 경로 없음 |
| 840 | `father2d` | function | `mc_father2d` | 미구현 | 독립 실행 경로 없음 |
| 841 | `get_5F_detail` | function | `mc_get_5F_detail` | 미구현 | 독립 실행 경로 없음 |
| 842 | `get_5F_detail2d` | function | `mc_get_5F_detail2d` | 미구현 | 독립 실행 경로 없음 |
| 843 | `get_5F_smooth` | function | `mc_get_5F_smooth` | 미구현 | 독립 실행 경로 없음 |
| 844 | `get_5F_smooth2d` | function | `mc_get_5F_smooth2d` | 미구현 | 독립 실행 경로 없음 |
| 845 | `get_5F_subband` | function | `mc_get_5F_subband` | 미구현 | 독립 실행 경로 없음 |
| 846 | `get_5F_subband2d` | function | `mc_get_5F_subband2d` | 미구현 | 독립 실행 경로 없음 |
| 847 | `mother2d` | function | `mc_mother2d` | 미구현 | 독립 실행 경로 없음 |
| 848 | `put_5F_detail` | function | `mc_put_5F_detail` | 미구현 | 독립 실행 경로 없음 |
| 849 | `put_5F_detail2d` | function | `mc_put_5F_detail2d` | 미구현 | 독립 실행 경로 없음 |
| 850 | `idwt` | function | `mc_idwt` | 미구현 | 독립 실행 경로 없음 |
| 851 | `idwti` | function | `mc_idwti` | 미구현 | 독립 실행 경로 없음 |
| 852 | `iwpts` | function | `mc_iwpts` | 미구현 | 독립 실행 경로 없음 |
| 853 | `mother` | function | `mc_mother` | 미구현 | 독립 실행 경로 없음 |
| 854 | `idwts` | function | `mc_idwts` | 미구현 | 독립 실행 경로 없음 |
| 855 | `iwpt` | function | `mc_iwpt` | 미구현 | 독립 실행 경로 없음 |
| 856 | `iwpti` | function | `mc_iwpti` | 미구현 | 독립 실행 경로 없음 |
| 857 | `put_5F_smooth` | function | `mc_put_5F_smooth` | 미구현 | 독립 실행 경로 없음 |
| 858 | `put_5F_smooth2d` | function | `mc_put_5F_smooth2d` | 미구현 | 독립 실행 경로 없음 |
| 859 | `put_5F_subband` | function | `mc_put_5F_subband` | 미구현 | 독립 실행 경로 없음 |
| 860 | `put_5F_subband2d` | function | `mc_put_5F_subband2d` | 미구현 | 독립 실행 경로 없음 |
| 861 | `swaveterp` | function | `mc_swaveterp` | 미구현 | 독립 실행 경로 없음 |
| 862 | `wpts` | function | `mc_wpts` | 미구현 | 독립 실행 경로 없음 |
| 863 | `swaveterp2d` | function | `mc_swaveterp2d` | 미구현 | 독립 실행 경로 없음 |
| 864 | `wavebs` | function | `mc_wavebs` | 미구현 | 독립 실행 경로 없음 |
| 865 | `waveterp` | function | `mc_waveterp` | 미구현 | 독립 실행 경로 없음 |
| 866 | `waveterp2d` | function | `mc_waveterp2d` | 미구현 | 독립 실행 경로 없음 |
| 867 | `wpt` | function | `mc_wpt` | 미구현 | 독립 실행 경로 없음 |
| 868 | `wpti` | function | `mc_wpti` | 미구현 | 독립 실행 경로 없음 |

</details>

<details>
<summary>efiaffine — 6개</summary>

원본 근거: `ev_3f13b1f7832b6c2672e420855bd0fdb79c6b9b0a6fc2b45fc664b97cad97fe75`.

| 번호 | 이름 | 종류 | 원본 바인딩 | 상태 | 경로/제한 |
| --- | --- | --- | --- | --- | --- |
| 869 | `DMS` | function | `mc_DMS` | 미구현 | 독립 실행 경로 없음 |
| 870 | `_2F_DMS` | function | `mc__2F_DMS` | 미구현 | 독립 실행 경로 없음 |
| 871 | `FIF` | function | `mc_FIF` | 미구현 | 독립 실행 경로 없음 |
| 872 | `_2F_FIF` | function | `mc__2F_FIF` | 미구현 | 독립 실행 경로 없음 |
| 873 | `hhmmss` | function | `mc_hhmmss` | 미구현 | 독립 실행 경로 없음 |
| 874 | `_2F_hhmmss` | function | `mc__2F_hhmmss` | 미구현 | 독립 실행 경로 없음 |

</details>

<a id="public-spellings"></a>

## 도움말의 사용자 표기와 내부 바인딩 대조

34개 이름은 도움말에서 확인했지만 기존 표는 내부 encoding/overload 이름으로만 보였던 항목이다. 별도 함수가 늘었다고 합산하지 않는다. 예를 들어 `READ_IMAGE`와 `mc_READ_5F_IMAGE`는 여기서 같은 원본 기능으로 연결한다. DMS/FIF/hhmmss는 이번에 추가한 EfiAffine 바인딩에 연결한다.

| 사용자 이름 | 원본 바인딩 | 상태 | 도움말 근거 |
| --- | --- | --- | --- |
| rgb_to_yiq | `mc_rgb_5F_to_5F_yiq` | 미구현 | [H061](#h061) |
| rgb_to_ycbcr | `mc_rgb_5F_to_5F_ycbcr` | 미구현 | [H061](#h061) |
| yiq_to_rgb | `mc_yiq_5F_to_5F_rgb` | 미구현 | [H061](#h061) |
| ycbcr_to_rgb | `mc_ycbcr_5F_to_5F_rgb` | 미구현 | [H061](#h061) |
| gray_to_rgb | `mc_gray_5F_to_5F_rgb` | 미구현 | [H062](#h062) |
| rgb_to_gray | `mc_rgb_5F_to_5F_gray` | 미구현 | [H062](#h062) |
| rgb_to_hls | `mc_rgb_5F_to_5F_hls` | 미구현 | [H063](#h063) |
| rgb_to_hsv | `mc_rgb_5F_to_5F_hsv` | 미구현 | [H063](#h063) |
| hls_to_rgb | `mc_hls_5F_to_5F_rgb` | 미구현 | [H063](#h063) |
| hsv_to_rgb | `mc_hsv_5F_to_5F_rgb` | 미구현 | [H063](#h063) |
| gray_close | `mc_gray_5F_close` | 미구현 | [H087](#h087) |
| gray_dilate | `mc_gray_5F_dilate` | 미구현 | [H087](#h087) |
| gray_erode | `mc_gray_5F_erode` | 미구현 | [H087](#h087) |
| gray_open | `mc_gray_5F_open` | 미구현 | [H087](#h087) |
| READ_IMAGE | `mc_READ_5F_IMAGE` | 미구현 | [H089](#h089) |
| READ_RED | `mc_READ_5F_RED` | 미구현 | [H090](#h090) |
| READ_GREEN | `mc_READ_5F_GREEN` | 미구현 | [H090](#h090) |
| READ_BLUE | `mc_READ_5F_BLUE` | 미구현 | [H090](#h090) |
| READ_HLS | `mc_READ_5F_HLS` | 미구현 | [H090](#h090) |
| WRITE_HLS | `mc_WRITE_5F_HLS` | 미구현 | [H090](#h090) |
| READ_HSV | `mc_READ_5F_HSV` | 미구현 | [H090](#h090) |
| WRITE_HSV | `mc_WRITE_5F_HSV` | 미구현 | [H090](#h090) |
| READ_HLS_SAT | `mc_READ_5F_HLS_5F_SAT` | 미구현 | [H090](#h090) |
| READ_HSV_SAT | `mc_READ_5F_HSV_5F_SAT` | 미구현 | [H090](#h090) |
| READ_HUE | `mc_READ_5F_HUE` | 미구현 | [H090](#h090) |
| READ_LIGHT | `mc_READ_5F_LIGHT` | 미구현 | [H090](#h090) |
| READ_VALUE | `mc_READ_5F_VALUE` | 미구현 | [H090](#h090) |
| moment_invariant | `mc_moment_5F_invariant` | 미구현 | [H108](#h108) |
| vec | `mc_vec_1`, `mc_vec_2`, `mc_vec_3` | 미구현 | [H126](#h126) |
| reg_grow | `mc_reg_5F_grow` | 미구현 | [H127](#h127) |
| shape_features | `mc_shape_5F_features` | 미구현 | [H132](#h132) |
| DMS | `mc_DMS` | 미구현 | [H204](#h204) |
| FIF | `mc_FIF` | 미구현 | [H204](#h204) |
| hhmmss | `mc_hhmmss` | 미구현 | [H204](#h204) |

<a id="si-vocabulary"></a>

## SI 단위·물리 상수 개별 목록

[원본 수치·차원 사실](spec/mathcad12-si-units.json)과 [단위 분석/검사](analysis/0010-unit-semantics.md)의 227개 이름을 빠짐없이 펼친다. 현재 판정은 모두 **부분 구현**이다. SI 값/차원 lookup은 있으나 전체 단위계·prefix 정책·표시 별칭·자동 라벨·비선형/affine 변환을 완성한 상태가 아니다. 이 표는 지원 vocabulary이며 제품 기능 227개를 별도로 완료했다는 의미가 아니다. 차원은 사실 JSON의 Length/Mass/Time/Current/Temperature/Substance/Luminosity 순서다.

<details>
<summary>단위 212개·물리 상수 15개</summary>

| 이름 | 원본 분류 | 상태 | SI 배율/값 | 원본 field token |
| --- | --- | --- | --- | --- |
| `A` | 단위 | 부분 구현 | 1 | `0x04000015` |
| `Angstrom` | 단위 | 부분 구현 | 1.0000000000000002e-10 | `0x040000e3` |
| `BTU` | 단위 | 부분 구현 | 1055.05585262 | `0x040000e2` |
| `BTU15` | 단위 | 부분 구현 | 1054.8 | `0x040000e0` |
| `Bq` | 단위 | 부분 구현 | 1 | `0x040000df` |
| `Btu` | 단위 | 부분 구현 | 1055.05585262 | `0x040000e1` |
| `C` | 단위 | 부분 구현 | 1 | `0x04000014` |
| `CBTU` | 단위 | 부분 구현 | 1054.615 | `0x040000de` |
| `F` | 단위 | 부분 구현 | 1 | `0x04000013` |
| `G` | 단위 | 부분 구현 | 0.0001 | `0x040000d9` |
| `GHz` | 단위 | 부분 구현 | 1000000000 | `0x040000d8` |
| `GN` | 단위 | 부분 구현 | 1000000000 | `0x0400007b` |
| `GPa` | 단위 | 부분 구현 | 1000000000 | `0x040000d7` |
| `Gy` | 단위 | 부분 구현 | 1 | `0x040000d6` |
| `H` | 단위 | 부분 구현 | 1 | `0x04000012` |
| `Hz` | 단위 | 부분 구현 | 1 | `0x040000d5` |
| `Hza` | 단위 | 부분 구현 | 6.283185307179586 | `0x040000d4` |
| `IBTU` | 단위 | 부분 구현 | 1054.5 | `0x040000d3` |
| `J` | 단위 | 부분 구현 | 1 | `0x04000011` |
| `K` | 단위 | 부분 구현 | 1 | `0x04000010` |
| `KA` | 단위 | 부분 구현 | 1000 | `0x040000d2` |
| `KHz` | 단위 | 부분 구현 | 1000 | `0x040000d1` |
| `KV` | 단위 | 부분 구현 | 1000 | `0x040000d0` |
| `KΩ` | 단위 | 부분 구현 | 1000 | `0x040000cf` |
| `L` | 단위 | 부분 구현 | 0.001 | `0x0400000f` |
| `MHz` | 단위 | 부분 구현 | 1000000 | `0x040000ce` |
| `MN` | 단위 | 부분 구현 | 1000000 | `0x0400007c` |
| `MPa` | 단위 | 부분 구현 | 1000000 | `0x040000c7` |
| `MW` | 단위 | 부분 구현 | 1000000 | `0x040000cc` |
| `MΩ` | 단위 | 부분 구현 | 1000000 | `0x040000cd` |
| `N` | 단위 | 부분 구현 | 1 | `0x0400000e` |
| `N.A` | 물리 상수 | 부분 구현 | 6.02214076e+23 | `0x0400001d` |
| `N·cm` | 단위 | 부분 구현 | 0.01 | `0x040000ca` |
| `N·m` | 단위 | 부분 구현 | 1 | `0x040000cb` |
| `N·mm` | 단위 | 부분 구현 | 0.001 | `0x040000c9` |
| `Oe` | 단위 | 부분 구현 | 79.57747154594767 | `0x040000c8` |
| `Pa` | 단위 | 부분 구현 | 1 | `0x0400000d` |
| `R` | 단위 | 부분 구현 | 0.5555555555555556 | `0x040000c6` |
| `R.gas` | 물리 상수 | 부분 구현 | 8.314462618 | `0x0400001c` |
| `R.∞` | 물리 상수 | 부분 구현 | 10973731.568157 | `0x0400001b` |
| `S` | 단위 | 부분 구현 | 1 | `0x0400000c` |
| `Smoot` | 단위 | 부분 구현 | 1.7018 | `0x04000030` |
| `Sv` | 단위 | 부분 구현 | 1 | `0x040000c4` |
| `T` | 단위 | 부분 구현 | 1 | `0x0400000b` |
| `TN` | 단위 | 부분 구현 | 1000000000000 | `0x0400007a` |
| `V` | 단위 | 부분 구현 | 1 | `0x0400000a` |
| `W` | 단위 | 부분 구현 | 1 | `0x04000009` |
| `Wb` | 단위 | 부분 구현 | 1 | `0x04000008` |
| `W·hr` | 단위 | 부분 구현 | 3600 | `0x04000093` |
| `W·s` | 단위 | 부분 구현 | 1 | `0x04000094` |
| `a.0` | 물리 상수 | 부분 구현 | 5.291772108e-11 | `0x040000b0` |
| `acre` | 단위 | 부분 구현 | 4046.856422400001 | `0x040000ba` |
| `acre·ft` | 단위 | 부분 구현 | 1233.4818375475204 | `0x040000b9` |
| `amp` | 단위 | 부분 구현 | 1 | `0x040000b8` |
| `arcmin` | 단위 | 부분 구현 | 0.0002908882086657216 | `0x040000b6` |
| `arcsec` | 단위 | 부분 구현 | 0.00000484813681109536 | `0x040000b7` |
| `atm` | 단위 | 부분 구현 | 101325 | `0x040000b5` |
| `bar` | 단위 | 부분 구현 | 100000 | `0x040000b4` |
| `barn` | 단위 | 부분 구현 | 1.0000000000000001e-28 | `0x040000b3` |
| `bhp` | 단위 | 부분 구현 | 9809.5 | `0x040000b2` |
| `bohr` | 단위 | 부분 구현 | 5.291772108e-11 | `0x040000b1` |
| `c` | 물리 상수 | 부분 구현 | 299792458 | `0x040000af` |
| `cal` | 단위 | 부분 구현 | 4.1868 | `0x040000ae` |
| `cal15` | 단위 | 부분 구현 | 4.1858 | `0x040000ad` |
| `cal20` | 단위 | 부분 구현 | 4.1819 | `0x040000ac` |
| `cd` | 단위 | 부분 구현 | 1 | `0x04000006` |
| `cfm` | 단위 | 부분 구현 | 0.0004719474432000001 | `0x0400008c` |
| `cm` | 단위 | 부분 구현 | 0.01 | `0x040000ab` |
| `coul` | 단위 | 부분 구현 | 1 | `0x040000aa` |
| `cubit` | 단위 | 부분 구현 | 0.4572 | `0x040000a5` |
| `day` | 단위 | 부분 구현 | 86400 | `0x040000a4` |
| `dcal` | 단위 | 부분 구현 | 4185.5 | `0x040000a3` |
| `deg` | 단위 | 부분 구현 | 0.017453292519943295 | `0x040000a2` |
| `dpi` | 단위 | 부분 구현 | 39.37007874015748 | `0x04000023` |
| `dyne` | 단위 | 부분 구현 | 0.00001 | `0x040000a0` |
| `e.c` | 물리 상수 | 부분 구현 | 1.602176634e-19 | `0x04000022` |
| `ehp` | 단위 | 부분 구현 | 746 | `0x0400009f` |
| `erg` | 단위 | 부분 구현 | 1.0000000000000001e-7 | `0x0400009e` |
| `farad` | 단위 | 부분 구현 | 1 | `0x0400009d` |
| `fl_oz` | 단위 | 부분 구현 | 0.000029573529562499998 | `0x0400009c` |
| `fortnight` | 단위 | 부분 구현 | 1209600 | `0x0400009b` |
| `ft` | 단위 | 부분 구현 | 0.3048 | `0x0400009a` |
| `ft·kip` | 단위 | 부분 구현 | 1355.8179483314004 | `0x04000098` |
| `ft·lbf` | 단위 | 부분 구현 | 1.3558179483314003 | `0x04000099` |
| `furlong` | 단위 | 부분 구현 | 201.16800000000003 | `0x04000091` |
| `g` | 단위 | 부분 구현 | 9.80665 | `0x04000090` |
| `gal` | 단위 | 부분 구현 | 0.0037854117839999997 | `0x0400008f` |
| `galUK` | 단위 | 부분 구현 | 0.004546090000000001 | `0x0400008e` |
| `gauss` | 단위 | 부분 구현 | 0.0001 | `0x0400008a` |
| `gm` | 단위 | 부분 구현 | 0.001 | `0x04000089` |
| `gpm` | 단위 | 부분 구현 | 0.00006309019639999999 | `0x0400008d` |
| `h` | 물리 상수 | 부분 구현 | 6.62607015e-34 | `0x04000021` |
| `hectare` | 단위 | 부분 구현 | 10000 | `0x04000088` |
| `henry` | 단위 | 부분 구현 | 1 | `0x04000087` |
| `hhp` | 단위 | 부분 구현 | 746.043 | `0x04000086` |
| `hp` | 단위 | 부분 구현 | 745.6998715822702 | `0x04000085` |
| `hpUK` | 단위 | 부분 구현 | 745.7 | `0x04000084` |
| `hr` | 단위 | 부분 구현 | 3600 | `0x04000083` |
| `in` | 단위 | 부분 구현 | 0.0254 | `0x04000082` |
| `in_Hg` | 단위 | 부분 구현 | 3386.38 | `0x04000081` |
| `in·kip` | 단위 | 부분 구현 | 112.98482902761668 | `0x04000096` |
| `in·lbf` | 단위 | 부분 구현 | 0.11298482902761668 | `0x04000097` |
| `in·ozf` | 단위 | 부분 구현 | 0.007061551814226043 | `0x04000095` |
| `joule` | 단위 | 부분 구현 | 1 | `0x04000080` |
| `k` | 물리 상수 | 부분 구현 | 1.380649e-23 | `0x0400001f` |
| `kA` | 단위 | 부분 구현 | 1000 | `0x0400007f` |
| `kHz` | 단위 | 부분 구현 | 1000 | `0x0400007e` |
| `kN` | 단위 | 부분 구현 | 1000 | `0x0400007d` |
| `kN·cm` | 단위 | 부분 구현 | 10 | `0x04000077` |
| `kN·m` | 단위 | 부분 구현 | 1000 | `0x04000078` |
| `kN·mm` | 단위 | 부분 구현 | 1 | `0x04000076` |
| `kPa` | 단위 | 부분 구현 | 1000 | `0x04000069` |
| `kV` | 단위 | 부분 구현 | 1000 | `0x04000075` |
| `kW` | 단위 | 부분 구현 | 1000 | `0x04000074` |
| `kW·hr` | 단위 | 부분 구현 | 3600000 | `0x04000092` |
| `katal` | 단위 | 부분 구현 | 1 | `0x04000072` |
| `kcal` | 단위 | 부분 구현 | 4186.8 | `0x04000071` |
| `kg` | 단위 | 부분 구현 | 1 | `0x04000005` |
| `kgf` | 단위 | 부분 구현 | 9.80665 | `0x04000070` |
| `kip` | 단위 | 부분 구현 | 4448.2216152605 | `0x0400006f` |
| `klf` | 단위 | 부분 구현 | 14593.902937206363 | `0x0400006e` |
| `km` | 단위 | 부분 구현 | 1000 | `0x0400006b` |
| `knot` | 단위 | 부분 구현 | 0.5144444444444445 | `0x0400006a` |
| `kph` | 단위 | 부분 구현 | 0.2777777777777778 | `0x04000068` |
| `kpi` | 단위 | 부분 구현 | 175126.83524647637 | `0x0400006d` |
| `ksf` | 단위 | 부분 구현 | 47880.258980335835 | `0x04000067` |
| `ksi` | 단위 | 부분 구현 | 6894757.293168361 | `0x04000066` |
| `kΩ` | 단위 | 부분 구현 | 1000 | `0x04000073` |
| `l` | 단위 | 부분 구현 | 0.001 | `0x04000061` |
| `lb` | 단위 | 부분 구현 | 0.45359237 | `0x04000065` |
| `lbf` | 단위 | 부분 구현 | 4.4482216152605 | `0x04000063` |
| `lbm` | 단위 | 부분 구현 | 0.45359237 | `0x04000064` |
| `liter` | 단위 | 부분 구현 | 0.001 | `0x04000062` |
| `lm` | 단위 | 부분 구현 | 1 | `0x04000060` |
| `lx` | 단위 | 부분 구현 | 1 | `0x04000004` |
| `m` | 단위 | 부분 구현 | 1 | `0x04000003` |
| `m.u` | 물리 상수 | 부분 구현 | 1.66053906892e-27 | `0x0400001e` |
| `mA` | 단위 | 부분 구현 | 0.001 | `0x0400005f` |
| `mBTU` | 단위 | 부분 구현 | 1055.8700000000001 | `0x0400005e` |
| `mC` | 단위 | 부분 구현 | 0.001 | `0x040000a9` |
| `mH` | 단위 | 부분 구현 | 0.001 | `0x0400005d` |
| `mL` | 단위 | 부분 구현 | 0.000001 | `0x0400005c` |
| `mV` | 단위 | 부분 구현 | 0.001 | `0x0400005b` |
| `mW` | 단위 | 부분 구현 | 0.001 | `0x04000057` |
| `mcal` | 단위 | 부분 구현 | 4.19002 | `0x04000053` |
| `mg` | 단위 | 부분 구현 | 0.000001 | `0x04000052` |
| `mgd` | 단위 | 부분 구현 | 0.04381263638888889 | `0x0400008b` |
| `mho` | 단위 | 부분 구현 | 1 | `0x04000051` |
| `mhp` | 단위 | 부분 구현 | 735.49875 | `0x04000050` |
| `mi` | 단위 | 부분 구현 | 1609.3440000000003 | `0x0400004e` |
| `micron` | 단위 | 부분 구현 | 0.000001 | `0x0400004f` |
| `mil` | 단위 | 부분 구현 | 0.0000254 | `0x0400004d` |
| `mile` | 단위 | 부분 구현 | 1609.3440000000003 | `0x0400004c` |
| `min` | 단위 | 부분 구현 | 60 | `0x0400004b` |
| `mm` | 단위 | 부분 구현 | 0.001 | `0x0400004a` |
| `mol` | 단위 | 부분 구현 | 1 | `0x04000049` |
| `mole` | 단위 | 부분 구현 | 1 | `0x04000002` |
| `mph` | 단위 | 부분 구현 | 0.4470400000000001 | `0x04000048` |
| `ms` | 단위 | 부분 구현 | 0.001 | `0x04000047` |
| `nA` | 단위 | 부분 구현 | 1e-9 | `0x040000c2` |
| `nC` | 단위 | 부분 구현 | 1e-9 | `0x040000a7` |
| `nF` | 단위 | 부분 구현 | 1e-9 | `0x04000045` |
| `nV` | 단위 | 부분 구현 | 1e-9 | `0x04000059` |
| `nW` | 단위 | 부분 구현 | 1e-9 | `0x04000055` |
| `newton` | 단위 | 부분 구현 | 1 | `0x04000046` |
| `nm` | 단위 | 부분 구현 | 1e-9 | `0x04000044` |
| `nmi` | 단위 | 부분 구현 | 1852 | `0x04000043` |
| `ns` | 단위 | 부분 구현 | 1e-9 | `0x040000bc` |
| `oersted` | 단위 | 부분 구현 | 79.57747154594767 | `0x04000042` |
| `ohm` | 단위 | 부분 구현 | 1 | `0x04000041` |
| `oz` | 단위 | 부분 구현 | 0.028349523125 | `0x04000040` |
| `ozf` | 단위 | 부분 구현 | 0.2780138509537812 | `0x0400003f` |
| `pA` | 단위 | 부분 구현 | 1e-12 | `0x040000c1` |
| `pC` | 단위 | 부분 구현 | 1e-12 | `0x040000a6` |
| `pF` | 단위 | 부분 구현 | 1e-12 | `0x0400003e` |
| `pV` | 단위 | 부분 구현 | 1e-12 | `0x04000058` |
| `pW` | 단위 | 부분 구현 | 1e-12 | `0x04000054` |
| `pcf` | 단위 | 부분 구현 | 157.08746384624618 | `0x0400003d` |
| `pci` | 단위 | 부분 구현 | 271447.1375263134 | `0x0400003c` |
| `plf` | 단위 | 부분 구현 | 14.593902937206362 | `0x0400006c` |
| `poise` | 단위 | 부분 구현 | 0.1 | `0x0400003b` |
| `ps` | 단위 | 부분 구현 | 1e-12 | `0x040000bb` |
| `psf` | 단위 | 부분 구현 | 47.88025898033584 | `0x04000039` |
| `psi` | 단위 | 부분 구현 | 6894.757293168361 | `0x0400003a` |
| `rad` | 단위 | 부분 구현 | 1 | `0x04000038` |
| `rev` | 단위 | 부분 구현 | 6.283185307179586 | `0x04000037` |
| `rpm` | 단위 | 부분 구현 | 0.10471975511965977 | `0x04000036` |
| `s` | 단위 | 부분 구현 | 1 | `0x04000001` |
| `sec` | 단위 | 부분 구현 | 1 | `0x04000035` |
| `siemens` | 단위 | 부분 구현 | 1 | `0x04000034` |
| `slug` | 단위 | 부분 구현 | 14.5939029372064 | `0x04000033` |
| `sr` | 단위 | 부분 구현 | 1 | `0x04000032` |
| `stokes` | 단위 | 부분 구현 | 0.0001 | `0x04000031` |
| `tBTU` | 단위 | 부분 구현 | 1054.35 | `0x0400002f` |
| `tcal` | 단위 | 부분 구현 | 4.184 | `0x0400002e` |
| `tesla` | 단위 | 부분 구현 | 1 | `0x0400002d` |
| `ton` | 단위 | 부분 구현 | 907.18474 | `0x0400002c` |
| `tonf` | 단위 | 부분 구현 | 8896.443230521 | `0x0400002b` |
| `tonne` | 단위 | 부분 구현 | 1000 | `0x0400002a` |
| `tonnef` | 단위 | 부분 구현 | 9806.65 | `0x04000079` |
| `torr` | 단위 | 부분 구현 | 133.32236842105263 | `0x04000029` |
| `volt` | 단위 | 부분 구현 | 1 | `0x04000028` |
| `watt` | 단위 | 부분 구현 | 1 | `0x04000027` |
| `weber` | 단위 | 부분 구현 | 1 | `0x04000026` |
| `yd` | 단위 | 부분 구현 | 0.9144000000000001 | `0x04000025` |
| `yr` | 단위 | 부분 구현 | 31556925.9747 | `0x04000024` |
| `°` | 단위 | 부분 구현 | 0.017453292519943295 | `0x040000a1` |
| `°R` | 단위 | 부분 구현 | 0.5555555555555556 | `0x040000c5` |
| `Δ°C` | 단위 | 부분 구현 | 1 | `0x040000dd` |
| `Δ°F` | 단위 | 부분 구현 | 0.5555555555555556 | `0x040000dc` |
| `Δ℃` | 단위 | 부분 구현 | 1 | `0x040000db` |
| `Δ℉` | 단위 | 부분 구현 | 0.5555555555555556 | `0x040000da` |
| `Φ.0` | 물리 상수 | 부분 구현 | 2.067833848e-15 | `0x04000019` |
| `Ω` | 단위 | 부분 구현 | 1 | `0x04000007` |
| `α` | 물리 상수 | 부분 구현 | 0.0072973525643 | `0x0400001a` |
| `ε.0` | 물리 상수 | 부분 구현 | 8.8541878188e-12 | `0x040000e4` |
| `μ.0` | 물리 상수 | 부분 구현 | 0.00000125663706127 | `0x040000e5` |
| `μA` | 단위 | 부분 구현 | 0.000001 | `0x040000c3` |
| `μC` | 단위 | 부분 구현 | 0.000001 | `0x040000a8` |
| `μF` | 단위 | 부분 구현 | 0.000001 | `0x040000c0` |
| `μH` | 단위 | 부분 구현 | 0.000001 | `0x040000bf` |
| `μV` | 단위 | 부분 구현 | 0.000001 | `0x0400005a` |
| `μW` | 단위 | 부분 구현 | 0.000001 | `0x04000056` |
| `μm` | 단위 | 부분 구현 | 0.000001 | `0x040000be` |
| `μs` | 단위 | 부분 구현 | 0.000001 | `0x040000bd` |
| `σ` | 물리 상수 | 부분 구현 | 5.670374419e-8 | `0x04000018` |
| `ℏ` | 물리 상수 | 부분 구현 | 1.054571817e-34 | `0x04000020` |

</details>

<a id="evidence-files"></a>

## 세부 기능의 원본 근거 파일

모든 경로는 설치 폴더 `C:/Program Files/PTC/Mathcad Prime 12.0.0.1/` 기준이다. H 번호는 이 조사 스냅샷의 식별자다. 제목/본문·표·공개 XML 계약을 확인한 파일의 SHA-256을 기록하며, 파일 이름만으로 기능을 추정하지 않는다. 본문 원문·원본 구현·이미지 자산은 이 문서에 복제하지 않는다. 전체 상대 경로·제목·SHA와 각 기능의 구현 경로는 [근거 JSON](spec/mathcad12-feature-audit.json)에 있다.

| 근거 ID | 설치본 상대 경로 | SHA-256 |
| --- | --- | --- |
| <a id="h001"></a>H001 | `Help/en/PTC_Mathcad_Help/about_3D_plots.html` | `8967cfafd712f563b2bb70c4fc9660d9d2b2171700c3b87116835334f5531096` |
| <a id="h002"></a>H002 | `Help/en/PTC_Mathcad_Help/about_areas.html` | `02658b5fb4fa843fea78ec5adea331e64f8917945dc9f1d3abfb7bc2a32eb32d` |
| <a id="h003"></a>H003 | `Help/en/PTC_Mathcad_Help/about_axes_and_tick_marks.html` | `c2f305a7eadfa3b7030ca0e6763fb9f1136e3ad5cefcd9402cebcf260f6b144c` |
| <a id="h004"></a>H004 | `Help/en/PTC_Mathcad_Help/about_chart_components.html` | `000ceff0d1a4aabb8967a9906871b9432176f5db001a596aac60dc1b50c6d64e` |
| <a id="h005"></a>H005 | `Help/en/PTC_Mathcad_Help/about_chart_templates.html` | `107ed03cc6dfd36fdc87efafd99ae7a4f6de80d4e6c2cf5deb299b8f012ec562` |
| <a id="h006"></a>H006 | `Help/en/PTC_Mathcad_Help/about_contour_plots.html` | `7fd4114a8e0d821b401832dcff2ce310ac64affdf0eb7c6da7eb7bb212ffbba0` |
| <a id="h007"></a>H007 | `Help/en/PTC_Mathcad_Help/about_controlling_region_calculation.html` | `e9c3025f61e9a9bde4092c91613789eb3cf457814d92ce43e86f176d6773a005` |
| <a id="h008"></a>H008 | `Help/en/PTC_Mathcad_Help/about_copying_multi_worksheet_pages_to_word.html` | `c95490f559d87a0f27ff0b8238820e40c76928128a2086db3bc86aa2fff27c08` |
| <a id="h009"></a>H009 | `Help/en/PTC_Mathcad_Help/about_copying_single_region_to_word.html` | `af6e276a1253d8130b9e449ac158ae1590c7e580197115c2cc1002b0a0461ef8` |
| <a id="h010"></a>H010 | `Help/en/PTC_Mathcad_Help/about_copying_single_worksheet_page_to_word.html` | `c851ee1209d83cd5106a53f764b0ac463aba4266b79ff38ec4ec4c8ec6e81069` |
| <a id="h011"></a>H011 | `Help/en/PTC_Mathcad_Help/about_custom_function_name_conflicts.html` | `d7fdde459d11e15ae95b397cd3261841fe48b7188839b3b1f03d1a8161a9da07` |
| <a id="h012"></a>H012 | `Help/en/PTC_Mathcad_Help/about_custom_functions.html` | `e41500983ec289854a01171bed2605edbf65273071220c8c4678b9d904cc19ce` |
| <a id="h013"></a>H013 | `Help/en/PTC_Mathcad_Help/about_excel_component.html` | `cfbc4e3b2dc5e227c210e24711604237671a47aa2498b2f8b56b693984c75f7f` |
| <a id="h014"></a>H014 | `Help/en/PTC_Mathcad_Help/about_hyperlinks.html` | `72f5a65267f1db9613608a24101478f3d04d495fcb460d93b429a633e517ef50` |
| <a id="h015"></a>H015 | `Help/en/PTC_Mathcad_Help/about_keywords.html` | `05973fe54793d72e5d4435219252b00fa49e4673c35f6ed2eccef28dcf79c9ee` |
| <a id="h016"></a>H016 | `Help/en/PTC_Mathcad_Help/about_labels.html` | `355b735fe41d46eb702a385666ea002467230b87fb95a52164d81f10d2be9183` |
| <a id="h017"></a>H017 | `Help/en/PTC_Mathcad_Help/about_markers_and_callouts.html` | `f68ca6e44ab4293f60ae53ac8ccf1c6f7b4b83885a2ea3448e0be517412619ca` |
| <a id="h018"></a>H018 | `Help/en/PTC_Mathcad_Help/about_modifiers.html` | `a1b26f40b578a5e8319e3c1d82c7b7c4a9077a598d9b727716561d326927af0b` |
| <a id="h019"></a>H019 | `Help/en/PTC_Mathcad_Help/about_plot_density.html` | `1dd0c22f0d7f34760d7002a1358f07020b518cff53b72e2d74a279ffdb8dfad8` |
| <a id="h020"></a>H020 | `Help/en/PTC_Mathcad_Help/about_polar_plots.html` | `d8ec99e95a33753d44d43f4da943b7459dd12e874b24fe731bfbcb8392856c44` |
| <a id="h021"></a>H021 | `Help/en/PTC_Mathcad_Help/about_spell_check.html` | `525cd3c3c1224fbe5a211767a34234deb6ab8f4b7adf1388ca0057bea35f638e` |
| <a id="h022"></a>H022 | `Help/en/PTC_Mathcad_Help/about_tables.html` | `8bebe8eaa0c17b8dbd8ba63b37af07be679b2c76685062f86c2355a982eb9304` |
| <a id="h023"></a>H023 | `Help/en/PTC_Mathcad_Help/about_templates.html` | `1f53f1e92eb31f18c829deb28d1e1b89e31c4aa31e660b64aeb475f023eb3271` |
| <a id="h024"></a>H024 | `Help/en/PTC_Mathcad_Help/about_text_and_images.html` | `1c11bfe0c7495ad7f1e59c870f29b63df3efda32a624a80d3e03194aab91fba9` |
| <a id="h025"></a>H025 | `Help/en/PTC_Mathcad_Help/about_text_box.html` | `53c9ad393eeeb928749113734333c76ab288940e6318ee1002231a5e0d3d3e55` |
| <a id="h026"></a>H026 | `Help/en/PTC_Mathcad_Help/about_the_spell_check_proofing_language.html` | `fb04e8ebca5eff54cd376ab405ecf041655afa7ec46d0ce6b8dd6f0fda1c5b24` |
| <a id="h027"></a>H027 | `Help/en/PTC_Mathcad_Help/about_traces_and_axis_expressions.html` | `baf556ff6816bf5c774f90891fb8f98e830d2bf121684ee0ee2ba56630795ce3` |
| <a id="h028"></a>H028 | `Help/en/PTC_Mathcad_Help/about_using_fortran_in_your_custom_functions.html` | `84dfd3fa723f28cdd6675720d1200ac4afd315c010d675777093645ff731e4a8` |
| <a id="h029"></a>H029 | `Help/en/PTC_Mathcad_Help/about_worksheets_and_regions.html` | `2a1d4e695c96df276cdf53723e59cbd59cf457a98b5b55b1e4f902e185360ba5` |
| <a id="h030"></a>H030 | `Help/en/PTC_Mathcad_Help/about_XY_plots.html` | `224c27dfa28fd38ccf07fa0baa6e87b27dc58c0ca21aaa6aafa85f79b87fd165` |
| <a id="h031"></a>H031 | `Help/en/PTC_Mathcad_Help/addition_operator.html` | `a2a4784b68227d071cb67b5802b4abbad84887e86851ae2c5ec60d948ab03f2f` |
| <a id="h032"></a>H032 | `Help/en/PTC_Mathcad_Help/advanced_control_listbox_class.html` | `1fafec918b769f84a0356978659b6a9b16eba06e6fc8d48de3292408b1864c16` |
| <a id="h033"></a>H033 | `Help/en/PTC_Mathcad_Help/advanced_control_slider_class.html` | `f694727a9e43329db95022ed57907eb1dd8ac1b7d2311a4f27b1331fd8ad0718` |
| <a id="h034"></a>H034 | `Help/en/PTC_Mathcad_Help/advanced_control_textbox_class.html` | `d60503879eb23c7f3ed99218e4b4d22e4fd2fc741158e2577843bb11d0eb17de` |
| <a id="h035"></a>H035 | `Help/en/PTC_Mathcad_Help/advanced_controls.html` | `6a5e03d5bd11982bc6dc88dea3f2d1edb471bf500e9bf1cf77dc9e2a55276eca` |
| <a id="h036"></a>H036 | `Help/en/PTC_Mathcad_Help/API/Enum_WorksheetReadonlyOption.html` | `ddf5646fe05e13da1acc424bdab952cac8418c9f8758eb31d8905c732e4a457e` |
| <a id="h037"></a>H037 | `Help/en/PTC_Mathcad_Help/API/IMathcadPrimeEvents2.html` | `e29b65559ea1a1dd688c19042a9ec2209060ab0e8e13d727a0999afc5752d5d2` |
| <a id="h038"></a>H038 | `Help/en/PTC_Mathcad_Help/API/Object_Application.html` | `b89965bb6338af9cbb0323d9fa1a4f2614415b67014521ba7880d7fe7fcf928a` |
| <a id="h039"></a>H039 | `Help/en/PTC_Mathcad_Help/API/Object_GetValueResult.html` | `93488ab1cd174ad3b16a5eb357c297ccb32aaee63603ca222fc292e35315c0f9` |
| <a id="h040"></a>H040 | `Help/en/PTC_Mathcad_Help/API/Object_InputOutputConflicts.html` | `37d3ed7ae20a4d189ec5bd3850da32b99a82a0b570bd960f9b2ca53c5a55aa0f` |
| <a id="h041"></a>H041 | `Help/en/PTC_Mathcad_Help/API/Object_Inputs.html` | `80a13a9a76a5abc2a167d07dd141804e7fc3f1f5074e21039632c1660f60a65a` |
| <a id="h042"></a>H042 | `Help/en/PTC_Mathcad_Help/API/Object_InputsOutputsStates.html` | `8f96ffd1d4b132549f5e3440dac671d3759e011d7c7f7b73a4eeacad3a50f951` |
| <a id="h043"></a>H043 | `Help/en/PTC_Mathcad_Help/API/Object_Matrix.html` | `0aecf09dc148104825e6bdf3b466eaa379219fcb61fcf9514c97b79fcbc75046` |
| <a id="h044"></a>H044 | `Help/en/PTC_Mathcad_Help/API/Object_Outputs.html` | `2181686e3690a611a0784b741c0e8def5140d3d492e513f905f40b8544d37987` |
| <a id="h045"></a>H045 | `Help/en/PTC_Mathcad_Help/API/Object_SetValueResults.html` | `43655dcfe7a6f907190bd14c57d12dd492a1b63c9d455f6c77d91d01763d2b60` |
| <a id="h046"></a>H046 | `Help/en/PTC_Mathcad_Help/API/Object_ValuesSetter.html` | `8f635b97be4f4edfb1b3bf009e4df24fa7d528b386307fac122f2860a4541d3e` |
| <a id="h047"></a>H047 | `Help/en/PTC_Mathcad_Help/API/Object_Worksheet.html` | `a68f11ebad020b3a6c45bb06363983a70c37872486711fc41af5161b70bd7dda` |
| <a id="h048"></a>H048 | `Help/en/PTC_Mathcad_Help/API/Object_WorksheetReadonlyOptions.html` | `693380521a9ccf42ab0728685e109fd7cf5311832920a3cfde0c79cf02a402db` |
| <a id="h049"></a>H049 | `Help/en/PTC_Mathcad_Help/API/Object_Worksheets.html` | `47e8a143223d2668e67538c6aad96c706a4f2f8deef0c952f2f84b953aa9d921` |
| <a id="h050"></a>H050 | `Help/en/PTC_Mathcad_Help/beta_hgeom_zeta.html` | `c62f6972ac2a1c74e8470ca586ee842dd211c453b5268df451a148878432ac30` |
| <a id="h051"></a>H051 | `Help/en/PTC_Mathcad_Help/button_events.html` | `b427f646c797102d7a84918df92bb777af00e11e135dce60d077ee5fb62a0a8e` |
| <a id="h052"></a>H052 | `Help/en/PTC_Mathcad_Help/button_format_properties.html` | `11aa29204a75a04c0cf4ce66d9cf41869bf39f7a7ba74e165c59149bb169d21c` |
| <a id="h053"></a>H053 | `Help/en/PTC_Mathcad_Help/caching_referenced_worksheets.html` | `336c35ba8a6d317f44cf16db32fd3f98926a0e4e6a6c6d68efa09680a79917df` |
| <a id="h054"></a>H054 | `Help/en/PTC_Mathcad_Help/checkbox_events.html` | `3e1b9371b7edf52ba14d26f7420cd79d3c738b6dd7fa5c56d965d2c2ef57fd73` |
| <a id="h055"></a>H055 | `Help/en/PTC_Mathcad_Help/checkbox_format_properties.html` | `a4be1c5d914eb4005fa5b665953cd9c360a80c2e995d8c2b243076318ce781ad` |
| <a id="h056"></a>H056 | `Help/en/PTC_Mathcad_Help/combining_symbolic_and_numeric_calculations.html` | `e0f750f8ce7b9356f00675aa9d2e27b8ff97fdfc25badf2554d3bdce7226d35a` |
| <a id="h057"></a>H057 | `Help/en/PTC_Mathcad_Help/comparison_operators.html` | `2a71007f7b2c09714ef05fa0df76fe8fe675cba90185dbe42fe814cbee398a45` |
| <a id="h058"></a>H058 | `Help/en/PTC_Mathcad_Help/complex_conjugate_operator.html` | `fab7bb167924c3f2c742154ba158e99271abb234e74856fb965590950681f3ad` |
| <a id="h059"></a>H059 | `Help/en/PTC_Mathcad_Help/conditional_branching.html` | `c11a74ee35a4cb08d4180175755f0b9245d9d01352278e26744b86ed295844d3` |
| <a id="h060"></a>H060 | `Help/en/PTC_Mathcad_Help/conditional_operators.html` | `bf2f3a6803b0a6a69bfd87929a81ffa78bde1a3630678f246dacda400f9b1594` |
| <a id="h061"></a>H061 | `Help/en/PTC_Mathcad_Help/converting_between_RGB_and_television_images.html` | `5f07fa44348b0639130ce41b315c27de26c7dafa49017f4c6ff2c1ebbd4b8199` |
| <a id="h062"></a>H062 | `Help/en/PTC_Mathcad_Help/converting_images_grayscale_and_color.html` | `5a929487bd970df1955b4654cf4ff946a6eb7584ced514ed0bec537b6eea4bad` |
| <a id="h063"></a>H063 | `Help/en/PTC_Mathcad_Help/converting_images_or_colors_RGB_HLS_HSV.html` | `628c6cf942553f0c755ee315a141cb7c0ff26384b2bc2c4e7552e5de824fa75e` |
| <a id="h064"></a>H064 | `Help/en/PTC_Mathcad_Help/create_text_style.html` | `3c0c5478d0f7fbd54408152de62d997d682e7b0494f7da49d22565a88c3d40dc` |
| <a id="h065"></a>H065 | `Help/en/PTC_Mathcad_Help/definition_operator.html` | `4ef22f7d6b263dd7561516ba004db15ea3cfda2a8a7afcbda6287951acd601c9` |
| <a id="h066"></a>H066 | `Help/en/PTC_Mathcad_Help/degree_operator.html` | `4a71d14a212502884330b972d65ee0a6e3568a598f0fa2fa529de2ae177765dd` |
| <a id="h067"></a>H067 | `Help/en/PTC_Mathcad_Help/derivative_and_prime_operators.html` | `bd2d0c3b35c481e0c3ba5544be03d3d990109d83ab886f4b7d16f7790cb4333d` |
| <a id="h068"></a>H068 | `Help/en/PTC_Mathcad_Help/designating_input_regions.html` | `7c17175e02bb6e1390797bab52ec3bc170ed50db0923baf395ff78e1372e9d68` |
| <a id="h069"></a>H069 | `Help/en/PTC_Mathcad_Help/designating_output_regions.html` | `a031dc1aaa6abe402830d7237ade03df2dda253f80ae2ac61a1ff44e28aa60dd` |
| <a id="h070"></a>H070 | `Help/en/PTC_Mathcad_Help/determinant_operator.html` | `4962d53138883ac11b276e62897278f314e285294ba1ec5036a0e1b39ce6f53d` |
| <a id="h071"></a>H071 | `Help/en/PTC_Mathcad_Help/diracdelta_functions.html` | `5a40245b11cf6825609347999fb2e150ffa387a28b4442652d75598e5c54b93d` |
| <a id="h072"></a>H072 | `Help/en/PTC_Mathcad_Help/division_operators.html` | `c150e680b6ca6a372c5f2afd61294d60712e8fdcf6e9e6e21ca02fa18f8c1c40` |
| <a id="h073"></a>H073 | `Help/en/PTC_Mathcad_Help/elliptic_integral_functions.html` | `9b26faa3a9db07dcfb4b1705492b5bbe4b53cfdb8171c8d22e04d3711a6d3ccc` |
| <a id="h074"></a>H074 | `Help/en/PTC_Mathcad_Help/enabling_disabling_controls.html` | `7e9a54ca3efbfc81ab8c8aa0d0b0c0f749628a884475eac9f1006900e69dedb6` |
| <a id="h075"></a>H075 | `Help/en/PTC_Mathcad_Help/Entering_binar_octal_hexadecimal_expressions.html` | `867fb429bb7947585ab134f0dc083158e4e744216cf62de76abb5add5efbe7e2` |
| <a id="h076"></a>H076 | `Help/en/PTC_Mathcad_Help/evaluation_operator.html` | `cdcdcf47e1582ccfe17cd472746a2318f1a00c38e9d1f4a1d327dd9a34fdfe78` |
| <a id="h077"></a>H077 | `Help/en/PTC_Mathcad_Help/example_excel_component_formatting.html` | `bd39d51cd8fb355dfccf441c119fb41f6a24d3a77db5293065a4019ed27b926a` |
| <a id="h078"></a>H078 | `Help/en/PTC_Mathcad_Help/exec_event.html` | `cdc6e63ad21f99ce6ef7f9051bf7948b700de15b42eebd9d22a87f6349d9f847` |
| <a id="h079"></a>H079 | `Help/en/PTC_Mathcad_Help/exponential_integrals.html` | `1e7005518e20c1bcb653dd96f97e5395a5196db33aafac9b3ea9a55e3f972e8e` |
| <a id="h080"></a>H080 | `Help/en/PTC_Mathcad_Help/exponentiation_and_matrix_inverse_operator.html` | `db770c58c159b21c60464c1d729aceedf233317bd55409876edc1089de45988a` |
| <a id="h081"></a>H081 | `Help/en/PTC_Mathcad_Help/expression_type_functions.html` | `5bdcd25eb373f8b68bfc3e520048d1a114bbd5862ef4733bb0be15b94dda2e47` |
| <a id="h082"></a>H082 | `Help/en/PTC_Mathcad_Help/factorial_operator.html` | `c63febac5f13a9d7f9aaca492cf2f8bbe27dbdff5ccbc526cf3f781f388563f6` |
| <a id="h083"></a>H083 | `Help/en/PTC_Mathcad_Help/for_loop_operator.html` | `8fc355215fab1f407a9b6dcb29eab14b4aa454275dac83d78c83559804415cdc` |
| <a id="h084"></a>H084 | `Help/en/PTC_Mathcad_Help/fresnel_functions.html` | `5ba0a680a357702ac54521a2f382c1b78618c8a480440f8800df6f8db72c5285` |
| <a id="h085"></a>H085 | `Help/en/PTC_Mathcad_Help/global_definition_operator.html` | `9ed12a6c5eaea08b687ff24208cc6eb9d1b4c41944a454b8f259e09496927672` |
| <a id="h086"></a>H086 | `Help/en/PTC_Mathcad_Help/gradient_operator.html` | `f51a5bb85ce176eeb0dc1d69497f353d6e6c7326e5c3e0a8d254d726b7ddf462` |
| <a id="h087"></a>H087 | `Help/en/PTC_Mathcad_Help/grayscale_morphology.html` | `58b0c9aa4c1d249490326606cad4fee6ef27bc220cc4d43cdfdd6f8b3fa58735` |
| <a id="h088"></a>H088 | `Help/en/PTC_Mathcad_Help/hide_borders_area.html` | `a4c9f7142dd1b779351085ddd43c970bdcf689a05c0e16f3cc6077945bf3281c` |
| <a id="h089"></a>H089 | `Help/en/PTC_Mathcad_Help/image_files.html` | `4eec2f6bbb843bcbc5079b1f208394d7492aa1ce9069e66d7e6ee9d4ea915c0b` |
| <a id="h090"></a>H090 | `Help/en/PTC_Mathcad_Help/image_properties.html` | `200397a3644fbab9d2caff198f188c6ea6e3787c392c3e75e4410db19fb95045` |
| <a id="h091"></a>H091 | `Help/en/PTC_Mathcad_Help/index_operator.html` | `3d5997e15472d892c1c55026410eef52d751ccfb6d06f22d03a587d14f8cb2ad` |
| <a id="h092"></a>H092 | `Help/en/PTC_Mathcad_Help/inputs_and_outputs.html` | `caacb62f92f95d3f5152130954758135aa64c47818f0d50a0a4291e466e1e420` |
| <a id="h093"></a>H093 | `Help/en/PTC_Mathcad_Help/integral_operator.html` | `d29e9245062c97cb8b8cb58ecfb4c1d6c73a196b7ea8795740987ac928922200` |
| <a id="h094"></a>H094 | `Help/en/PTC_Mathcad_Help/integral_sin_cos.html` | `cc91659a651febde69158b047a05946c1b3d99b931c2aa76ff2e804f89cffdc1` |
| <a id="h095"></a>H095 | `Help/en/PTC_Mathcad_Help/is_element_of_operator.html` | `1a91241c9c74d37fb0181e19c0a65b85fda444c798a1c727fc0036118518ee2b` |
| <a id="h096"></a>H096 | `Help/en/PTC_Mathcad_Help/jacobian.html` | `80d96f75209f6c6c1cd44d15935f6e06bc2d62f9d8ae3a862bd2341fd7becc43` |
| <a id="h097"></a>H097 | `Help/en/PTC_Mathcad_Help/keyboard_shortcuts_for_editing_arrays_and_tables.html` | `077a67fee05e6ff532ec7a7176ef1a73fb91dbed2a2a53e33eab8d9a9ec54783` |
| <a id="h098"></a>H098 | `Help/en/PTC_Mathcad_Help/keyboard_shortcuts_for_editing_math.html` | `0a2a240d036a8306536b66d5b39d0eac2295b09af739a26785827ca2f7490392` |
| <a id="h099"></a>H099 | `Help/en/PTC_Mathcad_Help/lambert_functions.html` | `9b18e33efbaa50975074366935adf7db58e501f28b196b3f9bba4b0d9fa1ad75` |
| <a id="h100"></a>H100 | `Help/en/PTC_Mathcad_Help/limit_operator.html` | `0522e9d18ee2779ed236293b7b006b7f35f5166938e529a544596676e76b2343` |
| <a id="h101"></a>H101 | `Help/en/PTC_Mathcad_Help/listbox_events.html` | `2746aa8e4c54d4edda158d04ce025e74b0cbd35a7ed27f5fa03ef4ddb8c2adf2` |
| <a id="h102"></a>H102 | `Help/en/PTC_Mathcad_Help/listbox_format_properties.html` | `76d4f6725f72a7ffad0d6529a35c0ef1f7e7eab07480517bd61c121bfb3bf2cc` |
| <a id="h103"></a>H103 | `Help/en/PTC_Mathcad_Help/local_assignment_operator.html` | `8a66add0cd7fcb5d25f9960d0ea71d0979ff7782419bf33b14eaee81aabd1f30` |
| <a id="h104"></a>H104 | `Help/en/PTC_Mathcad_Help/logarithmic_functions.html` | `3d067718ffbe3126da23cda9d8d26d5d2e4f205e1483486abb76831b15cc4f81` |
| <a id="h105"></a>H105 | `Help/en/PTC_Mathcad_Help/Logarithmic_integrals.html` | `2d33caa17d5475ceb75eee1754a079a792c2abc80efbbe61a9c6bb2332e03bae` |
| <a id="h106"></a>H106 | `Help/en/PTC_Mathcad_Help/logical_operators.html` | `b0dc717e192e1cd18dd3db79225e3481efdf295bb210c7c55d880007a79c4bf4` |
| <a id="h107"></a>H107 | `Help/en/PTC_Mathcad_Help/matrix_operator.html` | `edf7afe704eae1ff8435d8826d66d87af53e3b15ef0e362b2fc9fcb6794b5399` |
| <a id="h108"></a>H108 | `Help/en/PTC_Mathcad_Help/moment_invariants.html` | `d8e0e0561240bf902d5d5b8052c531e830fc4fe11eb92d58a57208ab28eaa27e` |
| <a id="h109"></a>H109 | `Help/en/PTC_Mathcad_Help/multiplication_and_dot_product_operator.html` | `22b0b3f9948eac4df27c44bbcd9a5aac86600d30157c0f3e16d85451448ca819` |
| <a id="h110"></a>H110 | `Help/en/PTC_Mathcad_Help/negation_and_subtraction_operator.html` | `f976f237a4c4dfbbeec865e495a0ddfbd40130914d7d788426b026de24cfcbba` |
| <a id="h111"></a>H111 | `Help/en/PTC_Mathcad_Help/norm_operator.html` | `d02b9cc6bed63aff1b018a5270455e79048f08a589480aa890c0228956038ed5` |
| <a id="h112"></a>H112 | `Help/en/PTC_Mathcad_Help/ODE_function.html` | `ffd436daab190d0ba2dd23ce7fa7aa7e2369e7846a926a01fee50906019915b0` |
| <a id="h113"></a>H113 | `Help/en/PTC_Mathcad_Help/operators_for_exiting_loops_and_programs.html` | `71ea59d760e75ce706c4ac2c23405a53128570d6e680dc68e5e4972207cbb499` |
| <a id="h114"></a>H114 | `Help/en/PTC_Mathcad_Help/other_symbolic_functions.html` | `adc288648dc45b420045b619be57bf6281caacb022e3e0b417fdeaa050059619` |
| <a id="h115"></a>H115 | `Help/en/PTC_Mathcad_Help/pareto_plot.html` | `a3871bf93f288f0750d466627677330fa2a872541a3dd82958bb245c5427ff40` |
| <a id="h116"></a>H116 | `Help/en/PTC_Mathcad_Help/partial_differential_equations_in_solve_blocks.html` | `237b95f5ae1aa6ab9d6e32a311a2bed94dcca8cb67b76dce4cdd6cbe019c63ee` |
| <a id="h117"></a>H117 | `Help/en/PTC_Mathcad_Help/percent_operator.html` | `c01114f3dec5e3103a7ac9bfe7c0961e5dd59ce9d781c50760a56f8cc01f33bf` |
| <a id="h118"></a>H118 | `Help/en/PTC_Mathcad_Help/plot_gridlines.html` | `d0abae2849aed17e5b5ab5bdda99dde475aff59e0e54a93a78105f01dac1101c` |
| <a id="h119"></a>H119 | `Help/en/PTC_Mathcad_Help/plot_lables.html` | `2cd6af358489afcab0a0919dca7dac61f204be05e27017fe6b30fd462d951ff2` |
| <a id="h120"></a>H120 | `Help/en/PTC_Mathcad_Help/polar_operator.html` | `3c029de9676b94fb613bca48777cef7816cb5f69580cfb655124242a97a4e339` |
| <a id="h121"></a>H121 | `Help/en/PTC_Mathcad_Help/product_operator.html` | `796201f5a1a308c5e9312635bdd1d44ddb60a91449d5fe7508b25c14524896a2` |
| <a id="h122"></a>H122 | `Help/en/PTC_Mathcad_Help/program_operator.html` | `477ff512864a2355d69e206e0660b1e17b8df7c38c4ce615991b57cba96c2b28` |
| <a id="h123"></a>H123 | `Help/en/PTC_Mathcad_Help/radio_button_events.html` | `2da7d06f4e28a384e1e0d53fad4ef460e0d7153f845f0e0fc7a1c1347120f61f` |
| <a id="h124"></a>H124 | `Help/en/PTC_Mathcad_Help/radio_button_format_properties.html` | `8723c033fa2f13f390ed00e4ccba3d2ad068fa8e619985b1f8e480932ce13ad2` |
| <a id="h125"></a>H125 | `Help/en/PTC_Mathcad_Help/range_operators.html` | `88c8276521d5db8e8f53e29db0c193a9eb98c4e654c3e8021960761ed6acbe32` |
| <a id="h126"></a>H126 | `Help/en/PTC_Mathcad_Help/range_variable_vector.html` | `5908387aab9bbd01acb449d4c6a1c3ffcf33974e70c1e1bb7c658bfd402a4602` |
| <a id="h127"></a>H127 | `Help/en/PTC_Mathcad_Help/region_growing.html` | `48dc40c0211fe0a6ee4286ca48078510072a62edf99c01b5cbd0b70bf0826ee0` |
| <a id="h128"></a>H128 | `Help/en/PTC_Mathcad_Help/regions_and_using_find_and_replace.html` | `2f2732e95c50512215b61f266190f0e5fd470ddb649beb9c5a94f38714fed58a` |
| <a id="h129"></a>H129 | `Help/en/PTC_Mathcad_Help/row_and_column_operators.html` | `a870677758a3602c0fc4deabcefd06511fb08817a03a39a46b1eca9e61fa5b2e` |
| <a id="h130"></a>H130 | `Help/en/PTC_Mathcad_Help/scaling_operator.html` | `d9a0b7b9c1e0849486ff948f79405c2c5801137e41b5629ddef8da94de2d6657` |
| <a id="h131"></a>H131 | `Help/en/PTC_Mathcad_Help/selecting_one_or_more_regions.html` | `34d3c829b96e96473474c1901e1429fda3e17aee1da60c2758c2b66057096935` |
| <a id="h132"></a>H132 | `Help/en/PTC_Mathcad_Help/shape_features.html` | `78f00cd0bacb5c0a85d25974ed4983a0890029ca41211212b9624bacb8e0b1f9` |
| <a id="h133"></a>H133 | `Help/en/PTC_Mathcad_Help/slider_events.html` | `50db38c6f219160bdfd2a0b63b2eed62de8567624b2d9017726932f75f5a2900` |
| <a id="h134"></a>H134 | `Help/en/PTC_Mathcad_Help/slider_format_properties.html` | `c06ff203e81976bd16a456782030b809d1a582c8975c47e9f87c86e1ab624ca3` |
| <a id="h135"></a>H135 | `Help/en/PTC_Mathcad_Help/square_root_and_nth_root_operator.html` | `09892e43920d42d8ae151c4370d4808b04e0e9fbef941d9216b6e23fa6a1f485` |
| <a id="h136"></a>H136 | `Help/en/PTC_Mathcad_Help/start_event.html` | `12f7e031e9654649c12e39835b0e89f68192cdce640e895127d510138b2a6be9` |
| <a id="h137"></a>H137 | `Help/en/PTC_Mathcad_Help/stop_event.html` | `06e54e2faa759cc614cd715e2bcaea897f2206bdae8a9b5c2a277a6b287c338f` |
| <a id="h138"></a>H138 | `Help/en/PTC_Mathcad_Help/string_functions.html` | `a383d7b755d510ad0a971429f9680e73497c124861b1eff1d70845e519839713` |
| <a id="h139"></a>H139 | `Help/en/PTC_Mathcad_Help/summation_operator.html` | `206b5445e43096ef1a7cada63aed466bc035a58009a0f0139fa764f260ea8358` |
| <a id="h140"></a>H140 | `Help/en/PTC_Mathcad_Help/symbolic_evaluation_operator.html` | `0509012dd4312d3f40455e9d7ac2b812c08d80538c11c61e0489e99bf10816ad` |
| <a id="h141"></a>H141 | `Help/en/PTC_Mathcad_Help/symbolic_gamma_functions.html` | `8c3ebd15b5c17376c50a502e0bc80edd1795be293439bb82fcbd49a20334bd77` |
| <a id="h142"></a>H142 | `Help/en/PTC_Mathcad_Help/to_add_border.html` | `62bf697f6c182bae73ac1c6eb2318aaba7763921c02fb56e3f9156da9d08b20e` |
| <a id="h143"></a>H143 | `Help/en/PTC_Mathcad_Help/to_add_delete_y2_axis_in_chart.html` | `e81e847e14e72d125fa376f13416e8ed83d56d4ded74094465496a3e3db71ce3` |
| <a id="h144"></a>H144 | `Help/en/PTC_Mathcad_Help/to_add_format_minor_grids_ticks_in_chart.html` | `6020d4e11cf8b19ebee3622069e76aae1c6839a7886b83934355dee11340750b` |
| <a id="h145"></a>H145 | `Help/en/PTC_Mathcad_Help/to_add_or_remove_space_between_regions.html` | `a8852f305f35e8640a90d1bc16c220c142180378dac8ed3271a66a12d7c5594f` |
| <a id="h146"></a>H146 | `Help/en/PTC_Mathcad_Help/to_add_plot_legend.html` | `378de5ae5b3fe66b7350ac40b0dff6cf310b71e2e1e528a080bf147988c1d5fd` |
| <a id="h147"></a>H147 | `Help/en/PTC_Mathcad_Help/to_apply_a_label.html` | `ba5d66655736b48535f28128275e590dec9ee631ecdddd2cf5f9773ac9573173` |
| <a id="h148"></a>H148 | `Help/en/PTC_Mathcad_Help/to_apply_an_operator_to_multiple_terms.html` | `a1ebb43baa4e8f3283f5af2c2c63ad09cb26f16a2a1212381e879d55cdc0dba8` |
| <a id="h149"></a>H149 | `Help/en/PTC_Mathcad_Help/to_change_a_surface_fill.html` | `6bae18250ad87d5f71e1f6ce49888aed958185089aba27c98c0d5e532abbeb7d` |
| <a id="h150"></a>H150 | `Help/en/PTC_Mathcad_Help/to_change_the_number_of_plotted_points.html` | `cc1dcc36152ed5398485116a20b6444e37cad95a28d9839024f0f1db79c5fa1a` |
| <a id="h151"></a>H151 | `Help/en/PTC_Mathcad_Help/to_collapse_and_exapnd_an_area.html` | `d7f978ee02264f02a7d78a6be39847844dd24fddf71a902f80a2fd87e716307f` |
| <a id="h152"></a>H152 | `Help/en/PTC_Mathcad_Help/to_control_area_calculation.html` | `72ab26a4ad6ec8b7e4d054631e8b88cbac6d3ed8b0c5f949076d3016e64c9fea` |
| <a id="h153"></a>H153 | `Help/en/PTC_Mathcad_Help/to_copy_cut_and_paste_regions.html` | `4cb9597e74f02d08021148d6edaa197e143da73a6cdd1b0895770c75dde469b9` |
| <a id="h154"></a>H154 | `Help/en/PTC_Mathcad_Help/to_create_a_box_plot.html` | `ef0a92f74860ca6fed808015d711a50649a5a5c6926d97398db7792ef809320f` |
| <a id="h155"></a>H155 | `Help/en/PTC_Mathcad_Help/to_create_a_custom_function.html` | `d03fd58438fe2bd27cfe937f6b5e17c42875b7552f3806409523e314716c24ee` |
| <a id="h156"></a>H156 | `Help/en/PTC_Mathcad_Help/to_create_a_waterfall_plot.html` | `2a65e8ada8ae613223f692435e546b7b3482a36892b4e54b8c5f7d242c36abdc` |
| <a id="h157"></a>H157 | `Help/en/PTC_Mathcad_Help/to_create_an_effects_plot.html` | `642588acca28f0ece46daf169d433cf9c8db57fd761d3ba68c7123a18241dca2` |
| <a id="h158"></a>H158 | `Help/en/PTC_Mathcad_Help/to_create_an_error_plot.html` | `da1ca3907a8bc299beeb9b37cd21eb31490caa0b23a170d0b2dc275f529b6350` |
| <a id="h159"></a>H159 | `Help/en/PTC_Mathcad_Help/to_create_and_edit_headers_and_footers.html` | `e31b8565a26908b42024d6bd90cc8e6d59e303674b42ff7383d67ee067894e2b` |
| <a id="h160"></a>H160 | `Help/en/PTC_Mathcad_Help/to_customize_a_trace.html` | `09e70c87a1f0725640ffb2e07dc9d004d0372e9796632a37dea724b0a2bbad08` |
| <a id="h161"></a>H161 | `Help/en/PTC_Mathcad_Help/to_customize_the_worksheet.html` | `76cd28a3046e5d512bdc234012168e558ab4fc2b33d5635745b02cfc731b0763` |
| <a id="h162"></a>H162 | `Help/en/PTC_Mathcad_Help/to_define_a_function_by_symbolic_result.html` | `30e5e1a90434a93824ada291d35d0312158afd9f1abf1e854ba17aa445767e74` |
| <a id="h163"></a>H163 | `Help/en/PTC_Mathcad_Help/to_delete_regions.html` | `488da85002f1c562815e001c2946ab6977d81291c08e84924c5940e0737dc849` |
| <a id="h164"></a>H164 | `Help/en/PTC_Mathcad_Help/to_designate_input_and_output_regions.html` | `68fad8f04aed0aa56ad44c269e3c1af884a8686ffd9aefe246ebb1ab252447fb` |
| <a id="h165"></a>H165 | `Help/en/PTC_Mathcad_Help/to_display_3D_plots_in_perspective.html` | `c828adbbc736a2f678d5a0e11089ea2fb75d262c07faaef10278c5b98ffffc61` |
| <a id="h166"></a>H166 | `Help/en/PTC_Mathcad_Help/to_display_indices_of_a_large_matrix.html` | `e45aed6ef7e507d1a523a075f2addad5e5137ee18d16da218edde5f3babb0b13` |
| <a id="h167"></a>H167 | `Help/en/PTC_Mathcad_Help/to_dock_the_input_output_window.html` | `bf474971ac6384005c536051fa609d844eca138169b0230820c7ef80faab7863` |
| <a id="h168"></a>H168 | `Help/en/PTC_Mathcad_Help/to_edit_advanced_controls.html` | `8a1dd11515ce0f5d31ae76a268a08819a53537477d65b61c57b0ede20720cfea` |
| <a id="h169"></a>H169 | `Help/en/PTC_Mathcad_Help/to_edit_arrays_and_tables.html` | `85cc675cc698b30fc4fd20655032f63743cbf94bc196323c148a6a3ac311beb0` |
| <a id="h170"></a>H170 | `Help/en/PTC_Mathcad_Help/to_edit_plot_legend.html` | `07a2ff83e9924f5cd5c05f374800c781cc934eae21636d26f041fcf00849d376` |
| <a id="h171"></a>H171 | `Help/en/PTC_Mathcad_Help/to_edit_tick_mark_values.html` | `00c245201cda3d291acd8fa491bf543a262b04055ff60b400e90db82aca8ca17` |
| <a id="h172"></a>H172 | `Help/en/PTC_Mathcad_Help/to_enter_math_expressions.html` | `170b0557652096ad2c0942ada4ac8d1233c7d3d4ea8df0df3a75a0cbf0b350c5` |
| <a id="h173"></a>H173 | `Help/en/PTC_Mathcad_Help/to_format_an_axis_in_chart.html` | `472aa7b03a0dce9f90eff8d26e26a46eb2509a1d6115a5e4f955e69cc02937fb` |
| <a id="h174"></a>H174 | `Help/en/PTC_Mathcad_Help/to_format_label_styles.html` | `dcad1d4cf18e47e12b60067ee07c37a6a9e233b30c5809f96636388dc2b59cee` |
| <a id="h175"></a>H175 | `Help/en/PTC_Mathcad_Help/to_format_math_font.html` | `f233708c0adc2cd4cb89877be2ffb5857ff1e6d897b95d4e70350c36777d4f65` |
| <a id="h176"></a>H176 | `Help/en/PTC_Mathcad_Help/to_format_the_result_display.html` | `e047ad6a8bf5896306a18d19fb6f40fff012ae12587089e98a0a6320fc4b1901` |
| <a id="h177"></a>H177 | `Help/en/PTC_Mathcad_Help/to_format_traces_in_chart.html` | `7890d414ef02c62a852ea0edfac797d47605f00113e11269b97c35ebd97d6217` |
| <a id="h178"></a>H178 | `Help/en/PTC_Mathcad_Help/to_group_math_terms.html` | `3ba6c9b141c97220c65742dbe9c54c35d1e59f677c080755bfb50c513c17aa93` |
| <a id="h179"></a>H179 | `Help/en/PTC_Mathcad_Help/to_insert_a_symbol_or_a_constant.html` | `f7af381951bcbe695fbcd21b506a71d4efe309bdc31fa0888a4f0d08520bf431` |
| <a id="h180"></a>H180 | `Help/en/PTC_Mathcad_Help/to_insert_an_image.html` | `ce6b8c8e4fa64c8bbc216e39562fda1d39783e244803725b228ac37b686e3264` |
| <a id="h181"></a>H181 | `Help/en/PTC_Mathcad_Help/to_insert_and_format_text.html` | `750bfd1ed6fded2dc4a624fdb8b8431b6977ebba9252f3e1c804aa77b5dc1d05` |
| <a id="h182"></a>H182 | `Help/en/PTC_Mathcad_Help/to_insert_keywords_and_modifiers.html` | `1967058eeb763db1a61e1da24a6864ed1196b020da3b8f6700b968edc05f656f` |
| <a id="h183"></a>H183 | `Help/en/PTC_Mathcad_Help/to_insert_or_delete_page_breaks.html` | `35a1f56adb40b00d17cec2105b82615ad8dbdb18a374ef18d5ea1e12b299180b` |
| <a id="h184"></a>H184 | `Help/en/PTC_Mathcad_Help/to_insert_toggle_equation_break_operator.html` | `1b494830c755c3211d0f3c7c3a541d6b04108e2d667fd0ff740171e6141f25e0` |
| <a id="h185"></a>H185 | `Help/en/PTC_Mathcad_Help/to_insert_units.html` | `0e556112e24ae49cc0c02d73bce74430ec54ad140c6b10405424d357a83ab324` |
| <a id="h186"></a>H186 | `Help/en/PTC_Mathcad_Help/to_keep_a_file_on_the_recent_worksheet_list.html` | `9fe26a014c55db7fe6d4b9b5d686d9a785931b84b30b433802e2ba2457581fd8` |
| <a id="h187"></a>H187 | `Help/en/PTC_Mathcad_Help/to_move_a_trace_to_y2_in_chart.html` | `65f91a607fa63357a048e2ca800994feb64b326f2bab32513450ee2f98245032` |
| <a id="h188"></a>H188 | `Help/en/PTC_Mathcad_Help/to_move_regions.html` | `f39f7a9a9c4244bcb9bf5143071d5253e05fcaadef54232fba3c7cbed123aaa4` |
| <a id="h189"></a>H189 | `Help/en/PTC_Mathcad_Help/to_print_worksheets.html` | `35e9843a38ae3b7343a809c6aca2896de881a15a0f9f452c63b6c8cc72003768` |
| <a id="h190"></a>H190 | `Help/en/PTC_Mathcad_Help/to_protect_unprotect_an_area.html` | `825e5876e82fe32bd94e16aa5ad1e6b0b60b9e1b30af0f549704c67cfbab0f48` |
| <a id="h191"></a>H191 | `Help/en/PTC_Mathcad_Help/to_reference_worksheets.html` | `30f58508c90da18385aa7f8da6dc657b8262d804b0ee6b142e64a243fcd797ab` |
| <a id="h192"></a>H192 | `Help/en/PTC_Mathcad_Help/to_replace_or_delete_an_operator.html` | `dea0897eb9525f6b265710bb12be2a63c991ccfb95caf180db74069c1051704d` |
| <a id="h193"></a>H193 | `Help/en/PTC_Mathcad_Help/to_resize_a_large_matrix.html` | `7a8bbe684f4345ef6946dc0b6b6baae2d472aed5550bcdba59e996ef8a259c8c` |
| <a id="h194"></a>H194 | `Help/en/PTC_Mathcad_Help/to_set_template_options.html` | `fdc1d006a1e6e5187d18a3591bc996ac8c729dab758247fb50ea5d135ab7f054` |
| <a id="h195"></a>H195 | `Help/en/PTC_Mathcad_Help/to_set_the_plot_bg_color.html` | `17ab7b3f5046a971ae14711c8adebf428af58fe435f47b018c1e71d56822f5d8` |
| <a id="h196"></a>H196 | `Help/en/PTC_Mathcad_Help/to_show_hide_axis_expressions.html` | `6be2c170a85927eaedb4710f53ba070b4aa07794b2330e08b32e1776096527ed` |
| <a id="h197"></a>H197 | `Help/en/PTC_Mathcad_Help/to_type_input_expressions_excel_component.html` | `a85c9c66ad1c29a3e57ec1d90c1fd6efddbac43fc7d03395ef3f2d0a96651ca4` |
| <a id="h198"></a>H198 | `Help/en/PTC_Mathcad_Help/to_type_output_expressions_excel_component.html` | `8ab1a449165b2c1ec08d5e5bc6edeafdea47a9cb09b6f30b6f31fbbf18d3c867` |
| <a id="h199"></a>H199 | `Help/en/PTC_Mathcad_Help/to_use_a_template.html` | `0bed5873e5d6ce224e860c0f9406141c28438db87de0f2f9025b1e1a0aa8baa5` |
| <a id="h200"></a>H200 | `Help/en/PTC_Mathcad_Help/to_view_large_symbolic_results.html` | `5d834adbb6cb5d512daaf96fab7693882ea5e540e2429d400a9d2b44445b1d2f` |
| <a id="h201"></a>H201 | `Help/en/PTC_Mathcad_Help/transpose_operator.html` | `f5a759d827bee54a1d809d835d02e0560b2e0264a74c9a4797b6cd6a4b389c8a` |
| <a id="h202"></a>H202 | `Help/en/PTC_Mathcad_Help/try_operator.html` | `e91b451efe60a19f8211910d8f56223195de5e641737d6012dc5637f2fecf68b` |
| <a id="h203"></a>H203 | `Help/en/PTC_Mathcad_Help/unit_placeholders.html` | `fc79f36f68894be5f8b4b6710fc856cd5a4763746b95f6169e1555d4a8cd977c` |
| <a id="h204"></a>H204 | `Help/en/PTC_Mathcad_Help/unit_scaling_in_common_formats.html` | `8b8e621157e4e59b99581bf6cc89558c50c1ba563719fa1f8a8ecb997e89e10b` |
| <a id="h205"></a>H205 | `Help/en/PTC_Mathcad_Help/using_headers_and_footers.html` | `7e05f1c494217025f0c5377d400e6c8e1fadef0293b50892489a3f703c9f8b29` |
| <a id="h206"></a>H206 | `Help/en/PTC_Mathcad_Help/using_keywords_for_symbolic_operations.html` | `808508406ded094f2055792be088a6cc47a81a4a0b39700af63e8bcac1585700` |
| <a id="h207"></a>H207 | `Help/en/PTC_Mathcad_Help/using_literal_subscripts_for_name_definitions.html` | `dbb5635daeb5199c8ffb985c12700231f7070d2582ea9dc03c6da2ef1826fa05` |
| <a id="h208"></a>H208 | `Help/en/PTC_Mathcad_Help/variable_clearing_functions.html` | `32a935e83845c578a255a7a0bbb260a5b670a6e56be09297128062994bf4e67d` |
| <a id="h209"></a>H209 | `Help/en/PTC_Mathcad_Help/vector_cross_product_operator.html` | `b224748b8e777f527e66077b887e2051e7a9a31bf6b3ea1892d357c39d694d56` |
| <a id="h210"></a>H210 | `Help/en/PTC_Mathcad_Help/vectorize_operator.html` | `a7388a75226cbbe7e21000eb27254b08eac9e28907d43d9665c52697d9dafefc` |
| <a id="h211"></a>H211 | `Help/en/PTC_Mathcad_Help/while_loop_operator.html` | `0fa24913ef8604a974657612fc5057c3b71a585f76a8a86b1486eaaa3d4dd5b5` |
| <a id="h212"></a>H212 | `Help/en/PTC_Mathcad_Help/wn_function_analysis.html` | `57c2d05fad3d2ad671a61b5e623333f7636aaf99c3011fc42e91a4f9cb308c4c` |
| <a id="h213"></a>H213 | `Help/en/PTC_Mathcad_Help/wn_header_footer.html` | `01def9373a96b70c737ee9120630cc5badd5f4b098dd3054587c490287831cce` |
| <a id="h214"></a>H214 | `Help/en/PTC_Mathcad_Help/writing_c_programs_for_dlls.html` | `85629b5d503664b3b5331cf61a4ef459298121754b4c35fc22527df771cabe95` |
| <a id="h215"></a>H215 | `Ptc.MathcadPrime.Automation.xml` | `10445eb660ccef3710f37bf66f15e2bd56fdba758f46e08958e05f15d1404a96` |

## 유지 규칙과 조사 경계

원본 기능을 추가하거나 상태를 바꾸면 해당 행, 원본 근거, 실행 경로와 제한을 함께 갱신한다. 일부만 구현했으면 상위 기능을 완료로 올리지 않는다. 문서 전체가 아니라 명령·옵션·자료형·작성 경로별로 확인한다. 분석 기록은 조사 당시 이력이며 현재 상태는 이 목록을 따른다.

이번에 영문 최상위 도움말 HTML 1,075개를 형식 파싱해 주제/함수 후보를 찾고, 선정한 실제 기능 근거와 API 하위 도움말·공개 XML을 대조했다. 전체 도움말의 모든 예제·인자 조합·플랫폼별 동작을 조사 완료했다는 뜻은 아니다. 문서로만 존재를 확인한 기능의 실행 계약, 미조사 모듈/리소스와 native 본문, 전체 원본 런타임 동일성은 열린 항목이다. 새로운 근거가 나오면 계속 확장한다.

기존 ‘20%/50%’ 단계명·테스트 개수·현재 목록의 행 수를 계측한 제품 완료율로 쓰지 않는다. 원본의 별도 modal 결과 창은 근거가 없으며, 큰 기호 결과의 원본 방식인 영역 폭 조절은 EA-087의 미구현 항목이다.


<a id="h216"></a>
**H216** — `Help/en/PTC_Mathcad_Help/to_create_a_string.html`; SHA-256 `6062888d372e75b26926573b11bb4dcfb77cea27c3a40e0ae3712489d8af5c7d`. 설치 도움말의 quote 입력·String 내부 입력·Enter/클릭 이탈 계약.
