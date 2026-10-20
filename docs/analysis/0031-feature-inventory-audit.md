# 원본 기능 목록 재조사 — 2026-10-16

이 문서는 기능 조사 직후의 이력이다. 이후 저장 결과 복원·F5/Ctrl+F5 계산 명령의 구현은 [0032](0032-calculation-commands-and-saved-results.md)에 기록했으며 현재 상태와 개수는 [통합 목록](../FEATURES.md)을 따른다.

기존 87행은 분야를 너무 크게 묶었고 원본의 문서·편집·설정·연동 기능을 충분히 찾지 않았다. 현재 상태의 단일 목록은 [FEATURES](../FEATURES.md)다. 이번에는 제품 기능을 추가하지 않고 조사 범위와 판정을 바로잡았다.

## 조사 경계와 결과

GUI·스크린샷·원본 앱·COM·CLR·스크립트·사용자 DLL을 실행하지 않았다. REA의 정적 artifact 검사, 이전 REA 리본/편집 증거, 설치된 도움말의 HTML/표와 공개 자동화 XML 계약, 현재 Rust 소스 및 기존 회귀 근거를 사용했다. 원본 도움말 설명은 기능 존재/의미의 근거이며 새 실행 관찰과 다르다.

| 층 | 결과 | 합산 규칙 |
| --- | --- | --- |
| 분야 요약 | 87묶음 | 아래 구체 항목을 압축한 개요. 합산 제외 |
| 편집·연산자·프로그램·미적분 저작, 상징 키워드/수정자 | 162행 | 큰 글꼴/스타일 묶음 2개는 더 세부적인 문서 행과 중복되어 제외 |
| 문서·계산 제어·출력·페이지·도표·컴포넌트 | 219행 | 전체 기능 수를 단정하는 숫자가 아님 |
| 공개 자동화·컨트롤 스크립팅·사용자 DLL | 126행 | 내부 Rust 계산과 공개 API 제공은 별도 |
| 추가 도움말 사용자 함수/상징 overload | 46행 | 내부 이름 별칭과 이미 추가한 EfiAffine 이름 제외 |
| 위 세부 목록 합계 | 553행: 부분 구현 103 / 미구현 450 | 상위 묶음·내부 바인딩·단위와 중복 합산 금지 |
| 계산 바인딩 | 21개 모듈·874행 | 기존 868 + EfiAffine 6. 사용자 함수 개수 아님 |
| 사용자 이름 대조 | 34개 | 기존 바인딩의 encoding/overload 이름과 연결; 신규 기능으로 세지 않음 |
| SI vocabulary | 단위 212·물리 상수 15 | 지원 값/차원 lookup 목록; 별도 기능 완료 수 아님 |

영문 최상위 도움말 HTML 1,075개를 파싱해 제목·함수 후보를 만들고, 실제 기능의 본문/표와 API 하위 문서·공개 XML을 좁혀 확인했다. 선택한 세부 행과 별칭의 근거 파일은 215개다. 각 파일의 설치본 상대 경로·SHA-256, 행별 구현 경로·미구현 사유는 [근거 JSON](../spec/mathcad12-feature-audit.json)에 있다. 이 JSON에는 원본 본문이나 구현 코드가 없다.

동일 이름의 숫자 함수와 상징 수정자, 또는 앱 명령과 자동화 API는 적용 문맥이 다르다. 반면 `READ_IMAGE`와 `mc_READ_5F_IMAGE` 같은 사용자 이름/내부 encoding은 같은 기능이므로 합산하지 않는다. `f/g/y`, `1/ln`, Custom Functions 가이드의 예제 함수 `multiply`는 새 내장 함수 후보에서 제외했다.

## REA 추가 근거

설치본 `efiaffine.dll`은 기존 20개 모듈 목록에서 빠져 있었다.

- REA artifact: `ev_3f13b1f7832b6c2672e420855bd0fdb79c6b9b0a6fc2b45fc664b97cad97fe75`.
- SHA-256: `301664074ff4497aaebde9ffca0ae5bc7b6c4cb2b5a2990abd4b852f6fb25979`.
- MVID: `b7b5fd8f-355a-4db2-95c0-8a97e430d6d2`.
- ModuleInterfaceAttribute: `0x0c000003`, blob SHA-256 `d253cfb397651e5c446443c0bad3c645a8a32d6eea2b7d510bf00e162213a1bb`.

REA가 확인한 파일/속성 해시를 검증한 뒤 기존 `extract-engine-catalog.py`로 이름·인자 구조·routing 사실만 추출했다. `mc_DMS`, `mc__2F_DMS`, `mc_FIF`, `mc__2F_FIF`, `mc_hhmmss`, `mc__2F_hhmmss`의 6개는 모두 미구현이다. 원본 인터페이스/IL 본문은 저장소에 복제하지 않았다.

추가로 `efistatsci_.dll`의 REA artifact `ev_ed0361e22d27f3afa517cbf10c59266464563a6c6f74ef532aef480d8267d71c`, SHA-256 `b17b40be192e4383f5c6fea0faab0c44909567ccaa7e807ff2cbb37c52d869b5`는 CLI metadata가 없는 native PE임을 확인했다. 이 결과로 해당 라이브러리의 기능이 없다고 판정하지 않는다. native 본문/등록 경로는 추가 조사 대상이다.

## 원본과 다른 현재 동작의 정정

설치 도움말 `about_controlling_region_calculation.html`, SHA-256 `e9c3025f61e9a9bde4092c91613789eb3cf457814d92ce43e86f176d6773a005`는 다음을 구분한다.

- 문서 열기 시 Auto Calculation이 켜져 있어도 자동 재계산하지 않는다. 명시 계산과 저장 결과의 수명 주기가 있다.
- Calculate Now는 처음부터 마지막 보이는 영역까지 계산하며 활성 영역에서는 그 영역을 갱신한다.
- Calculate Worksheet는 문서 전체를 계산하는 별도 동작이다.

현재 앱은 `worksheet_ui.rs`의 열기/계산 경로와 리본의 두 계산 버튼이 같은 `Recalculate` action을 사용한다. 독립 계산 결과가 보이게 만든 수정은 원본의 열기/캐시 수명 주기를 완성했다는 뜻이 아니다. HD005–HD009에 공백으로 기록했다. 기존 F9 shortcut을 원본 단축키를 복구한 것으로 부르지 않는다.

`to_view_large_symbolic_results.html`은 큰 기호 결과를 선택하면 오른쪽 resize handle이 나타나며 이를 드래그해 영역 폭을 늘리는 동작을 설명한다. 이는 미구현 EA-087이다. 제거한 자체 결과 확장 modal과는 별개다. [결과 분석 0029](0029-result-visibility.md)의 원본에 없는 modal 제거 기록은 유지한다.

## 검증과 남은 조사

업데이트된 원본 바인딩 JSON을 실제 release CLI `features --all`로 읽어 21개 모듈·874개 항목을 확인했다. 세부 목록 553개의 고유 ID·상태 합계, 근거 파일 215개의 경로/해시, 구현 경로, 원본 바인딩 순서/이름, SI 227개와 문서 내부 참조를 검사했다. 이 검사는 기능이 새로 구현됐다는 근거가 아니다.

공개 API/설정의 모든 오류·자료형·버전 차이, 도움말 예제 조합, native 본문, BAML의 모든 context command, 전체 단위계·상징 엔진의 모든 이름, 원본 런타임 동일성은 미완료다. 파일 수·목록 행 수를 전체 조사 완료나 제품 완료율로 사용하지 않는다.
