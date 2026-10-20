# REA 분석 환경

**UI 개편 전 기능 체크포인트:** [현재 보존 범위](CHECKPOINT-2026-10-20.md). 다음 작업은 [현대적인 UI와 기존 단축키 안내 개선](UI-MODERNIZATION.md)이다. 기존 r222 고정 검증은 [입력 행렬 편집·조판·인쇄](analysis/0207-typed-source-matrix-window.md)에 보존한다. 아래 이름 붙은 이전 검증은 당시 이력이다.

이전 [0122 구조 삭제 계약](analysis/0122-structural-deletion-contract.md)은 설치 CIL의 단계별 삭제·컨테이너 재파싱·값 fusion·방향별 커서와 C의 실제 기호 hit/선택 geometry·기본 색 metadata를 연결한다. [0125](analysis/0125-structural-deletion-integration.md)에 독립 구현과 동일 소스 검증을 기록하며, 원본 런타임·CLR/COM·GUI 없이 정적 근거와 CPU 테스트를 사용한다.

2026-10-15 이 Windows PC에 REA 6.5.0을 설치했다. Node.js 24.13.1을 사용한다.

- CLI: `rea --version`, `rea --help`.
- Codex MCP 등록 완료. REA가 생성한 설정 백업은 `C:\Users\harry\.codex\config.toml.rea.backup`에 있다.
- REA workflow skill: `C:\Users\harry\.agents\skills\reverse-engineer-anything`.
- 초기 조사는 CLI로 수행했다. 2026-10-16 현재 대화에서는 REA MCP 도구도 연결되어 `binary_session`과 `inspect_managed_artifact`를 실제 사용했으며, CLI 정적 분석과 함께 기능 목록을 확장했다.
- Ghidra 12.1.4: `C:\Users\harry\.local\share\mathcraft-tools\ghidra_12.1.4_PUBLIC`.
- Eclipse Temurin JDK 21.0.12.1+1: 같은 tools 폴더의 `jdk-21.0.12.1+1`.
- 다운로드한 두 ZIP은 공식 배포 메타데이터의 SHA256과 대조했다. 경로와 SHA256은 `scripts/setup-rea-native.ps1`에 고정했다.
- `MathcadPrime.exe`의 Ghidra 정적 분석 실행도 성공했다. 계산 엔진 네이티브 본문의 지원 여부는 별개다.
- MCP 등록에 Ghidra 및 Java 경로를 반영했다. CLI는 아래 환경 변수를 설정한 터미널에서 사용한다.

```powershell
$env:GHIDRA_INSTALL_DIR = "$env:USERPROFILE\.local\share\mathcraft-tools\ghidra_12.1.4_PUBLIC"
$env:JAVA_HOME = "$env:USERPROFILE\.local\share\mathcraft-tools\jdk-21.0.12.1+1"
rea doctor --provider ghidra --json
```

## 분석 시작

```powershell
./scripts/inspect-mathcad.ps1
./scripts/inspect-mcdx.ps1 -Path 'C:\Users\harry\Downloads\detune.mcdx'
rea inspect-managed-members 'C:\Program Files\PTC\Mathcad Prime 12.0.0.1\Ptc.Calculation.dll' --json
```

`artifacts/rea/`는 Git에서 제외된 로컬 증거 저장소다. 설치 폴더를 수정하거나 원본 제품 DLL을 프로젝트에 복사하지 않는다.

Windows Ghidra P0는 순수 네이티브 PE를 지원하지만 `mceng.dll` 같은 managed/C++/CLI 혼합 DLL은 `managed_target_unsupported`로 거부한다. 이 제한은 설치 실패가 아니다. 혼합 DLL의 CIL은 REA .NET 정적 검사로 조사하고, 네이티브 본문은 지원되는 다른 호스트/제공자에서 별도로 분석해야 한다. 원본 제품 실행 검증과 정적 호출 그래프를 구분한다.

첫 결과는 [최초 분석 기록](analysis/0001-initial-inspection.md)에 정리했다.

현재 기능 조사의 범위·추가 REA 증거·미조사 경계는 [기능 목록 재조사](analysis/0031-feature-inventory-audit.md), 구현 판정은 [통합 목록](FEATURES.md)을 따른다. 목록을 확대하면서 원본 앱·GUI·스크린샷·COM·CLR 실행은 사용하지 않았다.

후속 정적 계약은 [인쇄·페이지](analysis/0033-print-and-page-contract.md), [프로그램 줄·행렬 Tab](analysis/0036-program-lines-and-matrix-tab.md), [비활성 영역](analysis/0037-disabled-regions.md), [저장 날짜](analysis/0038-saved-date-fields.md), [계산 queue·중지/재개](analysis/0039-calculation-queue-and-cancellation.md)에 기록했다. 각각 retained REA 근거·원본 해시·설치 도움말을 대조하며 원본 런타임은 사용하지 않았다. worker와 협력 취소 구현은 [0040](analysis/0040-calculation-cancellation-engine.md)/[0041](analysis/0041-background-calculation-ui.md), 실제 독립 구현의 양 OS 검사 결과는 [CONFORMANCE](CONFORMANCE.md)를 따른다.

일반 수식의 편집/확정 경계는 [0042](analysis/0042-edit-commit-contract.md), 결과 서식 버튼·NumberFormat/ComplexStyle과 MCDX 직렬화는 [0045](analysis/0045-result-format-contract.md)를 따른다. 정적 IL·리소스의 실제 설정과 독립 구현의 cache/표시 구조를 구분한다. 사용자 PDF는 원본 실행 없이 읽기 자료로만 비교했다.

실제 1015 원본에서 누락된 표는 `spec-table`임을 확인했고 [0048](analysis/0048-native-table-and-colors.md)에 열·단위·리터럴·빈칸 및 텍스트/수식 색상을 연결했다. 원본 Whiteboard/Equation/RichText/SpecTable control의 정적 정렬점 계약은 [0052](analysis/0052-native-alignment-and-print-integration.md), SI 단위 system metadata의 대표 유도단위는 [0050](analysis/0050-unit-regressions.md)에 기록했다. 벡터 PDF·독립 글꼴 subset은 [0049](analysis/0049-vector-pdf-output.md)의 독립 구현이다. 현재 양 OS 검사와 실제 파일 비교의 최신 기준은 [CONFORMANCE](CONFORMANCE.md)이며, 원본 실행 성공과 혼동하지 않는다.

후속 표의 셀 편집·탐색·직렬화 계약은 [0053](analysis/0053-spec-table-editing.md), 열별 단위 scale·정의와 독립 엔진의 checkpoint 경계는 [0054](analysis/0054-spec-table-calculation.md)에 있다. 원본 native padding lowering과 개별 오류/취소 게시 시점의 미확인 부분을 명시했다. 화면 glyph 밀도/캐시 비용의 [0055](analysis/0055-screen-rendering-cost.md)와 자체 JSON 저장의 [0056](analysis/0056-native-cache-float-roundtrip.md)은 현재 Rust 구현의 결함 조사이며 원본 Mathcad가 같은 내부 알고리즘을 사용한다는 주장이 아니다.

후속 [원본 단축키·이름 아래첨자·비교/논리 입력](analysis/0057-shortcut-and-subscript-contract.md), [이동 중 transform과 drop 확정](analysis/0058-input-and-calculation-latency.md), [표 구조·범위 명령](analysis/0059-spec-table-structural-commands.md)은 설치 도움말 및 SHA/MVID/token에 결합한 REA metadata·CIL을 근거로 한다. 원본 DLL을 실행하거나 배포물에 포함하지 않는다. 통합 범위와 남은 공백은 [0060](analysis/0060-original-workflow-integration.md)을 따른다.

이어진 구현과 원본 근거: [배열 원소 정의](analysis/0063-indexed-definition-contract.md), [표 Copy caller·Cut/Clear](analysis/0061-spec-table-clipboard-contract.md), [앱 연결 교정](analysis/0064-table-clipboard-application.md)를 반영했다. 지원 범위와 미확정 동작은 각 기록 및 FEATURES/CONFORMANCE를 따른다.

후속 [일반 행렬 선택 mode](analysis/0065-matrix-selection-contract.md)는 fresh REA evidence와 실제 concrete mode·가상 메서드 상속을 대조해 Delete/Clear/Paste guard를 구분한다. [입력 연결](analysis/0066-matrix-selection-application.md)과 [저장 통합](analysis/0068-matrix-workflow-integration.md)은 독립 구현 검사다. [배열 참조 복제 비용](analysis/0067-array-reference-cost.md)은 Rust 계산기의 병목 조사이며 원본의 메모리 구조를 추정 복제한 것으로 표시하지 않는다.

후속 [연산자 계약](analysis/0069-structural-array-operators-contract.md)은 원본 descriptor 이름뿐 아니라 localized BAML 명령·factory의 실제 선/괄호 방향·금지 자식 규칙을 교차 확인한다. Norm과 determinant의 UI/내부 명칭 차이, `<norm/>` 비표준 alias 제거, `op_det`의 미복원 타입 lowering을 구분했다. [입력](analysis/0070-structural-array-operators-input.md)/[계산](analysis/0071-structural-array-operators-calculation.md)/[저장 통합](analysis/0072-structural-operators-integration.md) 검사는 독립 구현의 증거다. OSS 입력 backend의 좁은 수정은 Mathcad 원본 코드·자원과 분리해 provenance와 라이선스를 보존한다.

전역 정의·벡터화·외적의 descriptor·XML writer·실제 키·화살표 factory 및 표 셀 AllowExpressionsOnly는 [0073](analysis/0073-global-vector-operator-contract.md)·[0074](analysis/0074-global-vector-operator-input.md), 계산 native 정적 근거는 [0075](analysis/0075-global-vector-operator-calculation.md)에 보존했다. 새 원본 런타임 검증과 구분하며 GUI/COM/CLR을 실행하지 않았다.

Percent·Degree·Polar의 원본 descriptor·키·writer는 [0077](analysis/0077-scalar-notation-contract.md), 계산 타입·배열/단위 경계는 [0078](analysis/0078-scalar-notation-calculation.md), inline suffix factory는 [0080](analysis/0080-scalar-notation-integration.md)에 연결했다. Degree와 DMS 함수를 혼동한 기존 목록명을 교정했다.

행·열 LHS 허용 whitelist와 writer는 [0081](analysis/0081-array-slice-definition-contract.md), MatVal setter·front12 lowering·scalar index 경계는 [0082](analysis/0082-array-slice-definition-calculation.md), 앱 확정·native 왕복은 [0083](analysis/0083-array-slice-integration.md)에 연결했다. 원본 프로세스 실행은 사용하지 않았다.

프로그램 함수 LHS·Call coercion·원본 `{`와 실제 shipped XML 예제는 [0085](analysis/0085-local-function-authoring-contract.md), capture 계약·미확정 변경 경계는 [0086](analysis/0086-local-function-calculation-contract.md), 작성·저장 통합은 [0087](analysis/0087-local-function-integration.md)에 연결했다.

기존 일반 수식 확정·단축키·행렬 Tab 계약을 적용한 [0092](analysis/0092-committed-input-integration.md)는 자체 입력 경로의 첫 포커스 누락·불필요한 준비를 교정한다. 원본 내부 구현이라고 추정하지 않고, 자체 egui 소스와 실제 fresh-open headless 검사의 근거를 구분한다.

Ctrl+Q의 정확한 enum 순서·None/contextual·XML writer와 RL fallback·Scope/Typeover·override guard·Undo는 [0093](analysis/0093-label-cycle-contract.md)에 기록했다. 설치 Controls.Core/EquationEditor/EquationControl의 identity와 REA method 근거를 구분하고 [0094](analysis/0094-label-cycle-integration.md)의 독립 구현·지원 한계에 연결했다.

[0096](analysis/0096-program-control-authoring-contract.md)은 원본 Ctrl+J dictionary·HasNoLabel·전체 formatted 이름 비교·placeholder focus·zero-arity leaf와 삭제 handler 순서를 정적 CIL 근거로 추적한다. [0098](analysis/0098-program-controls-integration.md)의 독립 구현과 검사를 연결하며, 메뉴 일반 삽입/분할이나 if pair repair를 단순 교체로 추측하지 않는다. [0095](analysis/0095-worksheet-preparation-cost.md)·[0097](analysis/0097-cold-worksheet-preparation.md)은 자체 코드의 비용 조사이며 원본 실행 성능 측정으로 표시하지 않는다.

[0099](analysis/0099-conditional-branch-contract.md)은 원본 reader/writer와 BODY repair, 기본 .NET 8 Dictionary의 Ctrl+J if 우선순위, 조건문 header/body 좌표·토큰·fence를 정적 근거로 연결한다. [0101](analysis/0101-conditional-branch-integration.md)의 독립 구현과 구분하며 [0100](analysis/0100-xml-escape-cost.md)의 수치는 자체 코드 CPU 측정이다.

[0103](analysis/0103-try-control-contract.md)·[0104](analysis/0104-try-deletion-contract.md)는 TRY descriptor·원본 header/caret·삭제 pipeline·native와 도움말 근거를 구분한다. 원본 오류 taxonomy/rollback 미확정과 standalone inference를 명시한다. [0105](analysis/0105-try-control-integration.md)의 독립 구현 및 [0102](analysis/0102-display-preparation-cost.md)의 자체 CPU 측정과 구분한다.

[0106](analysis/0106-populated-program-insertion-contract.md)은 원본 Scope/Normal/implicit 변환·BODY repair·방향별 cursor와 설치 IL SHA를 구분한다. [0109](analysis/0109-program-insertion-integration.md)의 독립 구현과 [0107](analysis/0107-frame-interaction-cost.md)의 자체 page/tessellation CPU 측정·mesh 동등성 검사를 연결한다. 원본 standalone IF 성공은 동봉 native82개 조사에서도 확보하지 못했다.

[0110](analysis/0110-implicit-adjacency-contract.md)은 설치 descriptor/CIL/도움말에서 Scale·OP_PLACEHOLDER·RSCALE·명시적 Multiply를 분리하고 implicit-parent Ctrl+J와 native XML을 추적했다. [0113](analysis/0113-implicit-scale-integration.md)에 독립 구현/회귀를 연결하며 일반 parser·함숫값·원본 화면 일치는 미확정이다.

[0114](analysis/0114-parenthesis-adjacency-contract.md)는 설치 원본의 일반 GetContainerRoot·GROUPING Prefix/Postfix·Family1 닫기·두 단계 Scale 재파싱과 문자열 factory를 추적한다. [0117](analysis/0117-parenthesis-integration.md)에 독립 구현을 연결하며 전체 navigator/lexer/repair는 미완료다.

[0118](analysis/0118-normal-infix-entry-contract.md)은 일반 연산자의 Normal 문자 분할·전체 교체·Scope·fixity/alternate와 서로 다른 cursor 경로를 추적한다. Ctrl+-의 concrete override와 formatted 위치 보존도 추가 확인했다. [0121](analysis/0121-infix-entry-integration.md)에 독립 구현·검증 경계를 기록한다.
