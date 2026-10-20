# 원본 작성 흐름과 20% 목표의 검증 경계

2026-10-16. 사용자가 평가한 11%에서 20%를 목표로 진행하는 작업이다. 항목 수나 테스트 통과 수를 전체 Mathcad 완성률로 환산하지 않는다. GUI·스크린샷·원본 앱·COM·CLR 실행 없이 설치 도움말, 정적 REA와 headless 입력/문서 검사를 사용한다.

## 이번에 연결한 흐름

- **이름 작성:** Ctrl+-와 리본 아래첨자는 변수 이름의 suffix를 편집한다. 배열 index와 구분하며, suffix 입력·선택·삭제·커서·Undo를 원본 MathML에 연결한다. 임의의 이름 중간 rich-run은 여전히 지원하지 않는다.
- **구조 수식:** 비교 6종과 논리 AND/OR/XOR/NOT의 AST·MathML·키 입력·조판을 연결한다. `=` 평가와 Ctrl+= 비교를 구분한다. Ctrl+Shift+Z의 ∞ 및 Ctrl+Shift+2의 OR를 전역 Undo/도표 명령이 가로채던 충돌을 제거한다.
- **명령 경계:** 수정 키의 추가 Shift/Alt를 무시하던 전역 키 소비를 정확한 modifier 검사로 바꾼다. F1/Ctrl+F1, F5/Ctrl+F5도 구분한다. 원본에 없는 Ctrl+-/+/0 확대 단축키는 제거한다.
- **표:** 새 1×1 데이터 표, 이름·단위·데이터 셀 편집과 행/열 명령을 문서 모델·리본·계산·저장에 연결한다. 마지막 행/열의 명시 삭제는 원본처럼 셀을 비우고 최소 슬롯을 유지한다. 선택 범위 Delete는 별도 원본 계약을 따른다.
- **리본 편집 대상:** 표 셀에 식별자·단위·첨자·구조 수식을 넣을 때 새 워크시트 수식 영역을 만들지 않는다. 셀의 draft를 수정하고 확정 시 저장·계산한다. 일반 수식의 index 템플릿도 기존 피연산자를 보존한다.
- **영역 이동:** 드래그 중에는 표시 좌표만 변경하고 drop에서 원본 좌표·Undo·계산을 한 번 확정한다. 마지막 release 위치와 한 frame의 press→빠른 이동을 놓치지 않는다. 이동의 Undo/Redo는 관련 영역만 복원하며 다른 영역의 수정·편집기를 지우지 않는다.
- **새 표 폭:** 원본의 측정 폭이 없는 표는 셀 자연 폭을 사용한다. 가져온 표의 측정 폭과 구분해 빈 표가 뷰포트 전체로 늘어나는 것을 방지한다.

각 원본 근거와 제한은 [0057](0057-shortcut-and-subscript-contract.md), [0058](0058-input-and-calculation-latency.md), [0059](0059-spec-table-structural-commands.md)에 있다. WorksheetDragger를 REA MCP로 다시 조회한 부모 evidence는 `ev_7e6782733b36e145c3635b0b4f0ff55e3376c95a96b01ff5519d5c9a548bc1f6`, DLL SHA-256은 `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd`, MVID는 `44f31187-dcfc-4afd-a7d6-50fbee8dc361`이다. 선택한 39개 메서드의 metadata/CIL 분석이며 원본 실행 관찰은 아니다.

## 성능 판정

핵심 병목은 영역마다 앞선 모든 정의를 복제하는 O(N²) 문맥 라벨 처리였다. bound-variable overlay로 바꾸고 검증·계산 XML을 revision별로 재사용하며 worker에는 `Arc<str>`을 전달한다. 4천 정의 cold 준비가 약 984→110ms로 줄었다. 같은 입력 retain은 약 6.2ms다.

실제 입력 이벤트 검사에서 4천 영역 드래그 delta 60회는 평균 1.60ms/frame, drop은 73.60ms였다. 중간 계산 제출 0회, drop 1회와 원본 좌표 불변을 assert한다. 큰 문서의 확정·첫 계산에는 전체 문맥/XML 준비가 남아 있다. 이 CPU headless 수치를 실제 GPU FPS·글꼴 품질이나 전체 입력의 지연 보장으로 설명하지 않는다.

## 20% 목표에서 남은 공백

실제 원본의 모든 편집 전이와 rich text, 풀이 블록 저작, Area, 단위/배열/기호 계산, 도표·인쇄·native 저장 표현 전체가 완성된 것은 아니다. 두 사용자 문서의 성공도 전체 문서 호환율이 아니다. 전체 기능 판정은 [FEATURES](../FEATURES.md), 동일 소스의 Windows/peets 통합 결과는 [CONFORMANCE](../CONFORMANCE.md)를 따른다. 검증한 작성 흐름과 남은 공백을 함께 갱신하고, 이 묶음의 완료를 전체 완성률 20% 달성으로 표시하지 않는다.

후속 교정: [0061](0061-spec-table-clipboard-contract.md)/[0064](0064-table-clipboard-application.md)에서 원본 Copy의 실제 호출 분기를 확인했다. 헤더/단위 Copy는 새 표이며 본문만 행렬이다. 이전 body 직사각형 분배 Paste는 원본 근거가 없어 앱에서 제거했다. Copy/Cut/Clear와 활성 수식 Paste의 현재 지원은 후속 기록을 따른다.
