# 표 복사·잘라내기·내용 지우기 앱 연결

2026-10-16. [0061](0061-spec-table-clipboard-contract.md)에서 helper의 기능과 실제 원본 호출 분기가 다름을 확인해 이전 앱 동작을 교정했다. GUI·스크린샷·원본 실행은 사용하지 않는다.

## 원본에 맞춘 payload 분기

본문만 선택해 Copy하면 구조 행렬을 전달한다. 이름/단위 행을 포함하면 원본의 생성 규칙에 맞춘 새 표를 전달한다. 헤더만 복사한 표는 헤더와 빈 단위·빈 데이터 한 행을 가지며, 단위부터 선택한 표는 빈 헤더를 갖는다. 범위의 수학 표현은 보존하고 원본 resultRef·열별 서식·영역 metadata를 새 표에 복제하지 않는다.

독립 앱의 운반 표현은 각각 기존 `mathcraft-tree:`와 `mathcraft-table:`이다. 이는 Mathcad의 OS clipboard binary format을 구현했다는 뜻이 아니다. 본문 수식은 구조 편집기의 Paste로 전달하며, 이름/단위의 새 표 객체는 활성 수식에 XML을 입력하지 않고 워크시트에 새 표 영역으로 삽입한다. 대상 문서의 기본 수식 서식을 상속하며 source ID를 재사용하지 않는다.

전체 표 영역의 Copy/Cut도 별도로 지원한다. 이 경로는 영역의 기존 서식을 보존하고 붙여넣은 영역에 새 ID를 부여한다. 임의의 미지원 opaque XML을 붙여넣도록 허용하지 않고 SpecTable 구조 검증을 통과한 표만 허용한다. 붙여넣기 후 새 visual cache를 만들고 Undo/Redo에서도 복구한다. 삭제한 로컬 표의 셀 편집기·표시 cache를 해제한다.

## 내용 삭제와 확정 경계

범위 Cut은 복사가 성공한 다음 원본 Backspace/Delete의 행/열 삭제·부분 clear·최소 슬롯 보존 규칙을 적용한다. Backspace도 같은 내용 삭제 경로를 사용한다. 전체 유일 열을 잘라내면 복사는 되지만 원본처럼 표 내용 삭제는 no-op일 수 있다.

Clear Cells는 행/열 삭제와 다르다. 표의 크기·열 서식과 metadata를 유지하면서 선택 내용을 비우고 선택을 해제한다. 범위가 없으면 활성 셀의 수학 root를 비운다. 행렬/표 탭 Rows and Columns의 명령으로 연결했으며, 이번 앱 지원은 Spec Table에 한정한다. 일반 행렬의 다중 셀 Clear는 남아 있다.

한 번의 Cut/Clear는 한 영역의 Undo와 한 번의 계산 확정으로 연결된다. 일반 수식 Paste는 초안을 수정할 뿐이며 Enter/이탈 이전에 다른 셀·계산 결과를 바꾸지 않는다. 잘못된 표 payload는 활성 셀 초안을 먼저 확정하지 않고 거부한다. 물리적 클립보드·IME·원본 앱 상호 운용은 이 검사 범위 밖이다.

## 제거한 잘못된 동작

이전 앱은 복사한 행렬을 표 body의 직사각형에 나눠 덮어썼다. 이 동작은 `CreateMatrixFromTableCells` helper와 독립 모델의 `paste_matrix` 기능만 보고 연결한 것이며, 원본 Paste 호출에서 근거를 찾지 못했다. 해당 앱 경로를 제거했다. 독립 모델의 범위 편집 helper는 내부 유틸리티로 남지만 원본 사용자 기능으로 세지 않는다. 헤더/단위 선택을 행렬로 복사하던 앱 경로도 새 표 객체 분기로 교정했다.

## 검증

Headless 실제 이벤트 검사는 이름/단위/본문을 포함한 드래그 Copy, 평행 이동·2배율에서 Ribbon Copy 유지, 본문 Cut의 행렬 payload와 한 번의 삭제/계산/Undo/Redo, 크기를 유지하는 범위 Clear, 활성 셀 Clear와 불완전 숫자 초안, 새 표 객체의 활성 셀 밖 삽입과 대상 기본 서식 상속, invalid payload의 원본·초안 보존을 검사한다.

구조 행렬 Paste 후 활성 셀의 초안만 행렬로 바뀌고 인접 셀은 유지되며, Enter 뒤에도 행렬을 인접 셀에 나누지 않는 것을 확인한다. 중첩 배열 값의 전체 원본 계산 의미는 별도 미완료다. 전체 영역 Copy/Paste의 새 ID·서식·native visual과 Undo/Redo, 삭제 뒤 로컬 cache 해제/복구도 검사한다. 최신 동일 소스의 양 OS 결과와 XSD 검증은 [CONFORMANCE](../CONFORMANCE.md)를 따른다.
