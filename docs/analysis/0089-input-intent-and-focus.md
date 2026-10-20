# 편집이 아닌 입력의 준비 비용과 첫 수식 포커스

2026-10-19. [0042](0042-edit-commit-contract.md)의 원본 일반 수식 확정 계약, [0036](0036-program-lines-and-matrix-tab.md)의 프로그램 Enter·행렬 Tab, [0057](0057-shortcut-and-subscript-contract.md)의 정확한 modifier 경계를 유지하며 자체 입력 경로를 교정했다. 원본 기능을 추가한 작업이 아니다. GUI·스크린샷·원본 프로세스는 사용하지 않았다.

기존 `page_ui`는 모든 Key 이벤트에 이전 계산 입력 준비를 실행했다. 키를 떼는 이벤트와 수식의 커서 이동에도 MathML 생성·전체 문맥 처리·표시 트리 복원이 발생했다. 실제 save→fresh open→viewport focus 검사에서는 4,000영역의 Arrow press/release에 각각 약 60ms를 쓰면서 문서·계산 결과는 바뀌지 않았다. 측정 방법·전체 표본과 최종 전후 결과는 [0090](0090-first-open-input-cost.md)을 따른다.

`input_intent::may_edit`는 키 해제·방향키·Home/End/PageUp/PageDown·일반 Copy/SelectAll/Paste 요청 키를 실제 편집과 구분한다. actual Paste·Cut·Text·IME 이벤트는 수정 전 baseline을 확보한다. Tab은 마지막 행렬 셀에서 행을 늘릴 수 있으므로 잠재 편집으로 유지한다. Ctrl+Shift+C도 원본 Column 연산자이므로 일반 Copy와 구분해 baseline을 보존한다. Ctrl+- literal 아래첨자의 경로도 유지한다.

리본에서 Copy·Paste 요청·보기 설정·열기/새 문서 요청·설정 draft만 여는 명령 등은 수식 입력을 미리 준비하지 않는다. 실제 clipboard 이벤트나 설정 적용은 기존 변경 경계에서 처리한다. Undo/Redo·Cut·수식 명령·서식 변경 등 나머지 명령의 기존 baseline 경계는 유지했다. 표의 pointer 확정·행렬 구조 명령도 수정 전 snapshot을 계속 보존한다. 문서가 실제로 움직이는 영역 방향키는 기존 `nudge`의 이력·확정 경로를 사용한다.

## 첫 Tab의 별도 오류

보호 회귀를 추가하다가 첫 포커스 직후 행렬 Tab이 새 행을 만들지 않고 다른 위젯으로 이동하는 실패를 재현했다. 단독 formula 테스트는 매 frame `focus=true`를 전달해 이 실패를 놓쳤다. 실제 worksheet처럼 첫 frame에만 포커스를 요청하는 검사가 필요했다.

현재 OSS 의존 `egui 0.36.2`의 `Memory::set_focus_lock_filter`는 `had_focus_last_frame(id)`와 현재 focus가 모두 맞아야 필터를 적용한다. 첫 focus frame의 설치는 무시될 수 있고, 다음 frame 시작의 기본 Tab/arrow traversal이 formula가 키를 받기 전에 작동했다. 이 원인은 자체 라이브러리 소스의 직접 확인이며 원본 Mathcad의 내부 구현으로 주장하지 않는다.

formula는 이전 frame에 자신이 소유한 navigation이 framework traversal로 빠지려 하면 포커스를 회수하고 해당 traversal을 중지한다. pointer press나 열린 popup에서는 새 회수를 적용하지 않는다. 이후 기존 수식 키 handler가 Tab·Shift+Tab·arrow를 처리한다. 이 변경은 원본의 행렬/수식 키 동작을 다른 위젯의 기본 탐색으로 바꾸지 않기 위한 것이다. OS 전역 포커스·접근성·전체 팝업 조합의 동등성은 미검증이다.

worksheet 회귀 네 개는 커서·선택·release의 무작업, 리본 Copy·보기·draft dialog의 무작업, Column 및 첫 Matrix Tab의 수정 전 baseline과 계산 실행 0회, 첫 Shift+Tab의 이전 셀 이동·포커스·문서 불변을 검사한다. Tab 실패 원시 로그는 ignored `matrix-tab-input-intent-initial-20261019.log`, 성공 검사는 `input-intent-focused-20261019.log`에 있다.

첫 실제 편집도 이전 XML을 미리 만들지 않도록 [0091](0091-lazy-committed-baseline.md)의 raw baseline을 연결했다. 대규모 첫 Enter와 실제 계산 비용은 여전히 남으며 전체 제품 지연이나 완성률 20%가 해결됐다는 뜻은 아니다.
