# 페이지 나누기 앱 연결

2026-10-19. 원본 계약은 [0141](0141-page-break-and-space-contract.md)의 설치 도움말·XSD·정적 CIL/BAML 증거를 따른다. 이번 단계는 Mathcraft 자체의 헤드리스 앱 경로를 구현·검증했으며 GUI·스크린샷·원본 프로그램을 실행하지 않았다. 제품 전체의 완성도나 모든 배치의 호환성을 뜻하지 않는다.

## 명령과 한 번의 실행 취소

빈 root 워크시트 삽입점에서 정확한 Ctrl+Enter는 페이지 나누기 요청을 만든다. 활성 수식이나 선택된 영역의 기존 Ctrl+Enter 분기는 바꾸지 않았다. Ctrl+Shift+Enter와 Ctrl+Alt+Enter를 새 페이지 명령으로 소비하지 않는다. 리본은 `문서 → 간격 → 페이지 나누기`와 원본 Remove Space Above/Below에 대응하는 `공백 제거 → 위쪽 공간 / 아래쪽 공간`을 사용한다. 삭제는 현재 구현한 단일 인접 break 양성에서만 활성화하며 일반 공백 삭제 전체를 구현했다고 주장하지 않는다.

요청은 바로 문서를 변경하지 않고 page frame 끝에서 처리한다. 입력 위치가 바뀌거나 영역이 선택되었거나 draft·dialog·popup·drag가 생기면 폐기한다. New/Open/Undo/Redo도 보류 요청을 제거한다. 원본의 빈 crosshair 문맥과 활성 수식 문맥을 분리한다.

앱은 모든 non-break 영역의 실제 표현 bounds를 구한 뒤 [문서 트랜잭션](../../crates/mathcraft-document/src/page_break.rs)을 호출한다. 성공한 candidate만 설치하고 영역 이동·break 추가/삭제·삽입점 변경을 하나의 전체 문서 history 항목으로 보존한다. 실패하면 source·좌표·선택·history를 바꾸지 않고 status에 이유를 남긴다. 계산 갱신은 기존 위치 이동과 같은 `calculation_changed(false)`를 사용해 Automatic/Manual과 Pending 처리 계약을 따른다.

빈 삽입점 Backspace/Delete는 각각 위/아래 방향의 한 grid 구간에서만 제거를 요청한다. 앱의 일반 region clipboard Delete가 키를 먼저 없애므로 blank-only 페이지 명령을 그보다 앞에 배치했다. 키 테스트도 실제 global shortcuts → document history → region clipboard → page 순서로 실행한다. 활성 수식의 문자 삭제나 선택 영역 삭제를 이 경로로 바꾸지 않는다.

## Bounds와 좌표

오래된 region response height를 정답으로 재사용하지 않는다. 명령 시 화면 밖 영역까지 빠짐없이 검사한다.

- 수식: committed PortableTree를 읽기전용 Editor로 만들고 현재 결과·저장 결과·라벨·서체로 geometry와 baseline을 함께 구한다. native anchor에는 같은 baseline offset을 적용한다.
- 자체 텍스트: source 복사본의 비대화형·숨겨진 TextEdit를 현재 typography/width/frame 설정으로 측정한다. 원래 텍스트나 caret·선택을 편집하지 않는다.
- native rich text/table: 기존 native presentation의 실제 measure와 alignment offset을 사용한다. 미지원 native visual 오류는 거절한다.
- Area: 독립 Area child source와 현재 저장/실행 결과 표현을 읽어 기존 AreaUI의 높이 규칙을 재현한다. expanded height, `child.top − baseline + height + 4`, native child의 `top + native.height + 4`, 최소 높이를 함께 적용한다. 접힌 Area의 편집 높이는 22 DIP이다. 일반 native decoder가 Area owner에 남기는 예상된 unsupported visual 오류는 무시하되, 실제 child의 미지원·위치 불명·보호 상태는 거절한다. 이 owner 우회가 일반 수식의 native visual 오류까지 숨기지는 않는다.
- Plot: 현재 명세의 height를 사용한다. 그 밖의 geometry를 확정할 수 없는 opaque 영역은 보수적으로 거절한다.

Area는 page-wide bounds로 전달한다. 삽입점이 내부 높이를 가르는 경우 현재 문서 API가 거절한다. 복잡한 보호·겹침·다중 break 삭제의 원본 guard 전체를 대체하지 않는다.

`PageGeometry::body_height_exact()`는 기존 내부 f64 값을 읽기만 하는 getter다. 페이지 명령과 marker page-index 계산은 f32 변환 없이 이 값을 사용한다. native top을 UI 선의 위치로 덮어쓰지 않는다.

## 별도 수동 표시선

검증된 빈 native `<pageBreak/>`만 별도 painter에서 표시한다. 일반 region loop·선택·Tab 순회·remove_selected 대상으로 취급하지 않는다. Draft view에서는 표시선을 숨긴다. 기본 PDF에는 root 통합의 print snapshot 경로를 통해 편집선을 넣지 않는다. 원본의 별도 collection/인쇄 제외 근거는 0141 후속 절에 있다.

template의 Height 2, Center, StrokeThickness 4, DashArray 2,2와 PageWidth binding을 사용한다. WPF dash 수치는 stroke 배수이므로 이 renderer의 절대 길이로는 8 DIP 간격을 사용한다. 별도 원본 ItemsControl의 transform에 있던 `−2×collectionIndex`를 절대 Canvas Y에 중복 적용하지 않는다. 현재 absolute mapping은 `stride × page − gap/2 + 1 DIP`이며 그중 +1은 2 DIP control 중앙이다. 원본 panel의 모든 layout 세부와 픽셀 동등성을 실행으로 검증한 것은 아니다. brush의 원본 색은 정적 증거에서 아직 확정하지 못했으므로 중성 회색을 사용하며 원본 색 일치를 주장하지 않는다.

표시선 source 검증은 owner ID와 정확한 XML bytes를 함께 보존하는 cache다. 정상·오류 모두 source가 바뀔 때만 파싱하고 제거된 owner는 매 frame prune한다. Open/restore는 cache를 지우며 Draft 전환도 보존 entry를 해제한다. 단순 ID 재사용이나 source hash만으로 유효하다고 간주하지 않는다. XML source 전체를 매 키마다 다시 파싱하지 않는다.

계산 준비의 opaque fallback도 별도로 교정했다. 정확히 검증되는 pageBreak만 inert로 건너뛰며 잘못된 payload는 기존 unsupported fault를 유지한다. 화면에서 선만 숨기면서 계산 오류를 남기는 경로를 방지한다.

## 헤드리스 검증

일반 회귀는 빈 Ctrl+Enter의 Y/X suffix 이동과 한 번의 Undo/Redo, 활성/선택 Ctrl+Enter 불변, 추가 modifiers 거절, deferred 요청 취소, 인접 Backspace와 Delete의 서로 다른 이동, 미지원 geometry와 잘못된 native payload의 원자적 실패, passive marker의 Tab/선택/삭제 제외, eager Area bounds와 page-wide 교차 거절, imported Area owner visual 오류와 실제 child 검사, source cache의 warm 재파싱 0회/변경 시 재검증/prune/Draft 해제, 읽기전용 수식·텍스트·native rich bounds와 실제 paint height의 일치, 유효 break의 계산 fault 0개/잘못된 break fault 보존을 검사한다. 일반 geometry 비교 후 서체를 바꾸면 이전 response height 대신 새 측정값을 사용한다.

명령: `cargo test -p mathcraft-app page_break_ui -- --test-threads=1`. 최종 집중 검사 **13 passed / 0 failed / 1 ignored**, 앱 all-target Clippy `-D warnings` 통과. 아래 ignored native probe도 최종 harness에서 별도로 실행해 **1 passed**를 확인했다.

ignored `page_break_authoring_save_native_export_reopen_probe`는 `MATHCRAFT_PAGE_BREAK_OUTPUT` 아래에 다음 파일을 만든다.

- `page-break.mathcraft`: 숫자 7/14의 두 Eval과 실제 Ctrl+Enter로 만든 break, 한 번의 Undo/Redo를 거친 저장 문서.
- `page-break.mcdx`: 새 native pageBreak와 재배치한 수식 좌표, 원본 형식의 저장 결과.
- `page-break-reimported.mathcraft`: native 파일을 다시 연 뒤 저장한 자체 문서.

fixture 설정의 명시적 계산과 파일 재열기를 구분한다. 자체 문서/native 파일을 열 때는 계산 실행 0회, Pending 0개, fresh 결과 2개이며 원래 Canvas와 native 재수입의 좌표 쌍이 정확히 일치해야 한다. 저장된 cache를 재계산해 맞췄다고 주장하지 않는다. 최종 로컬 probe 로그는 [page-break-app-final-probe-20261019.log](../../artifacts/rea/page-break-app-final-probe-20261019.log), 출력은 [page-break.mcdx](../../artifacts/rea/page-break-app-final-probe-20261019/page-break.mcdx)와 [page-break.mathcraft](../../artifacts/rea/page-break-app-final-probe-20261019/page-break.mathcraft)에 보존했다. 양 OS 전체 검증·PDF/XSD 검사와 최종 source archive identity는 0145 통합 기록이 생성된 뒤 그 기록을 기준으로 확인한다.

## 변경 경로와 남은 범위

앱 구현과 회귀는 [page_break_ui.rs](../../crates/mathcraft-app/src/worksheet_ui/page_break_ui.rs), 좁은 field/default/request/shortcut/painter/Tab 연결은 [worksheet_ui.rs](../../crates/mathcraft-app/src/worksheet_ui.rs), 리본 enum/context와 메뉴는 [ribbon.rs](../../crates/mathcraft-app/src/ribbon.rs)이다. f64 getter는 [page.rs](../../crates/mathcraft-app/src/page.rs), 계산 inert whitelist는 [edit_commit.rs](../../crates/mathcraft-app/src/worksheet_ui/edit_commit.rs)이다. 문서 pure API와 exporter/print 구현은 별도 root 소유 범위다.

현재 삭제는 단일 인접 break의 안전한 배치만 지원한다. 일반 공백 삽입/제거 전체, 여러 break를 한꺼번에 제거하기, 교차하는 page-wide item을 native처럼 재배치하기, 임의 marker drag, 원본 색과 모든 panel pixel layout은 남은 범위다. source에 없는 명령이나 native payload 속성을 추가하지 않았다.
