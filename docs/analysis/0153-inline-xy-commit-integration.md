# Inline XY 입력·확정·계산 상태 연결

2026-10-19. 사용자 평가15%에서40% 목표로 진행하는 후속 묶음이다. [0042 일반 확정](0042-edit-commit-contract.md)·[0149 plot 확정](0149-native-xy-domain-and-commit-contract.md)의 원본 근거를 [0150 독립 UI 설계](0150-native-xy-inline-commit-design.md)에 따라 연결한다. GUI·스크린샷·원본 Mathcad/CLR/COM을 사용하지 않았다. 전체 원본 EquationControl/IME/routed command 구현 완료라는 주장은 아니다.

## 입력과 결과의 분리

r152의 inline x/y TextEdit는 실제 PlotSpec 문자열을 직접 바꾸었지만 page의 changed/history 경로에 연결되지 않았다. 현재는 owner별 optional draft와 borrowed TextBuffer로 분리한다. 원문 읽기·no-op은 clone0, 첫 실제 수정 때만 PlotSpec 한 번을 복제한다. 그 뒤 문자열 편집은 Unicode 문자 인덱스를 String TextBuffer에 위임한다. 타이핑은 문서 원문·저장 캐시·곡선·Pending 상태·Undo·계산 preparation/baseline을 변경하지 않는다. root 입력 의도 판단도 plot Text 경로를 전체 문서 snapshot에서 제외한다. 전체 프레임/GPU 속도 수치를 새로 주장하지 않는다.

plain Enter는 draft source를 한 번 확정한다. Automatic이면 계산하고 Manual이면 이전 곡선과 Pending을 유지한다. 같은 owner의 y→x Tab/x→y Shift+Tab은 같은 draft를 유지하며 바깥 방향은 일반 traverse로 둔다. Escape는 미확정 입력만 취소한다. source를 원문으로 되돌린 no-op은 history/redo를 지우지 않는다. 원본의 Shift+Enter 식 추가와 독립 축 목록/공유 identity는 [0151](0151-native-xy-axis-pairing-contract.md)에 별도 확인했으며 아직 구현하지 않았다.

IME Preedit는 draft에만 보관하고 조합 중 Enter 또는 Commit과 같은 frame의 Enter를 owner 확정으로 취급하지 않는다. 한글 조합 중 explicit Save/F5는 미확정 source를 게시하지 않는다. 첫 nonempty preedit에서 활성 축 문자열과 작은 caret checkpoint만 보관한다. 실제 focus 이동으로 조합이 중단되면 그 미확정 조합만 되돌리고 앞서 확정된 draft 편집과 다른 축/크기를 유지한다. 이는 egui adapter의 독립 조합 정책이며 원본 모든 OS IME 동작의 증거가 아니다. 다른 owner의 IME 종료 이벤트를 이전 owner가 가로채거나 composing flag를 남겨 저장을 막지 않는다.

## 확정 이후의 안전한 수명

source 확정 즉시 해당 곡선 상태를 Pending으로 표시하고 이전 background generation을 무효화한다. 계산 명령이 invalid table/Area 또는 빈 Now 범위로 중간 종료해도 새 source에 옛 points를 Synchronized로 저장하지 않는다. 취소는 engine epoch/preparation을 통째로 초기화하지 않으며 다음 실제 명령이 한 job을 제출한다. 기존 points는 inert historical presentation으로 남고 source binding으로 실행하지 않는다.

F5/Ctrl+F5는 Now의 early return 전에 plot source를 확정한다. Automatic refresh는 편집 중 draft를 확정하지 않는다. Save/MCDX export는 같은 editor 확정 경로를 거치며 native export도 최신 계산 캐시를 명시적으로 수집한다. 수동 Pending의 old points는 own 저장/재열기에 유지하고, 재계산한 fresh points는 native cold reopen에서 실행0으로 복원한다. print snapshot/render는 commit/계산을 하지 않고 마지막 확정 source/curve를 사용한다.

Undo/Redo·열기·owner 삭제는 해당 inline state를 정리한다. 키보드 Ctrl+Z는 focus된 TextEdit의 문자 이력에 전달하고 document Undo를 동시에 pop하지 않는다. toolbar Undo는 남은 해당 draft를 취소한 뒤 committed region history와 구분한다. 설정 창 진입 전 확정 실패는 창을 열지 않으며 Cancel/no-op Apply가 이전 source를 되돌리거나 새 history를 만들지 않는다. 이전 plot을 떠나 새 math draft를 같은 frame에 시작했을 때 그 math까지 자동 확정하지 않는다.

크기 변경은 gesture 시작의 치수만 보관하고 첫 실제 크기 변화에서 region checkpoint를 만든다. clamp로 원래 크기가 유지된 drag는 source/history/계산/generation을 바꾸지 않는다. 여러 drag frame은 한 Undo이며 pending text가 함께 확정되더라도 최신 width/height를 옛 draft가 덮지 않는다. 조합 중에는 resize가 preedit source를 확정하지 않는다.

## 통합 검사

실제 CPU key/focus/pointer 경로로 typing inertness, Manual/Automatic Enter, F5/전체 계산, focus exit/inbound Tab, Escape/no-op, Unicode byte budget, 문자 Ctrl+Z/region Undo/Redo, modal, 실제 resize와 clamp, IME Enter·종료·focus 이동을 검사한다. 빈 Now 범위 이후 source/old curve는 Pending이고 synthetic 이전 generation reply는 거절됨을 확인한다.

독립 fixture의 실제 SelectAll+Text+Enter→Manual Pending/old points→own save/reopen→명령 계산 y=2x→MCDX→native cold open 실행0→vector PDF를 연결한다. 입력 중 PDF는 이전 sin(x), 계산 후 PDF는2*x의 확정 source/곡선을 그리며 snapshot이 draft를 확정하지 않는다. 원본 fixture/사용자 파일을 변경하지 않는다. r154 고정·Windows/peets gate·MCDX/XSD/PDF 최종 검증 결과는 아래에 추가한다. 이 검사 수를40% 완성으로 환산하지 않는다.

## r154 최종 통합 검증

Windows workspace1280+backend2, peets workspace+backend1282가 같은 고정 소스로 통과했다. 양 OS 실패0/ignored66, fmt·workspace/backend Clippy·build·reference38saved/38input와 Windows release가 통과했다. 기존 원본1015/1016·Area/행렬/native XY/결과/페이지 나누기와 새 inline 입력 fixture를 함께 검사했다. 양 OS 합산26MCDX/130XML parts XSD 오류0,34PDF 이미지0·PyMuPDF/pypdf 독립 추출을 확인했다. 새 inline before PDF에는sin(x), 계산 후 PDF에는2*x가 남고 원래 곡선은Manual Pending으로만 유지된다. GUI·원본 runtime·실물 프린터 결과와 구분한다.

archive11,827,200bytes SHA `fc2e5583afb425f60f332948685c01fe10fc4e24bb3167563a28abe57eb2d57f`, manifest SHA `b88560a11ce294a5e6b0f115df9f51a5a3a5549aba030408b0f6f536c303e8e0`,411files/195product Rust다. peets 경로 `/home/peets/Projects/mathcraft-inline-plot-20261019-r154`에서 archive/manifest/각 source bytes를 검증했다. 회수 evidence1,843,200bytes SHA `3f46424814347a5a6227c85b9332da52b4b3781fc43b48259d43e2641c71d689`는 양끝 hash가 같다. 최종 record `artifacts/rea/inline-plot-integration-20261019-r154/validation.json`은 current product=archive를 확인했다. archive에는 준비 상태 문서가 들어 있으며 이 최종 문서와 후속 조사 기록은 고정 제품 소스를 바꾸지 않는다.

고정 release23,347,712bytes SHA `ff7721a0b705f1f3b212219cd62eccc35b590c645fce4f6c2645e288be41a18f`, CPU harness24,591,872bytes SHA `9091f3726297dec7c65464d5079bbe7f03911691374cdb4a5e851e11c57be07c`다. release는 r152보다43,008bytes 증가했다. 작성 흐름과 state lifetime을 검증했으며 전체 GPU/앱 지연의 새로운 절대 수치는 측정하지 않았다. 40% 달성으로 환산하지 않는다.
