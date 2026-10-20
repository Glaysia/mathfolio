# XY inline 식의 draft·commit 연결 제안

2026-10-19. r152 검증 소스를 읽고 작성한 **구현 설계**다. 원본 동작을 새로 실행하거나 REA로 확정한 문서가 아니다. 원본 근거는 [0042](0042-edit-commit-contract.md)와 [0149](0149-native-xy-domain-and-commit-contract.md)에 한정한다. 아래 baseline과 제안에 이어 별도 소유 모듈에서 구현한 경계를 끝에 구분한다.

## 확인된 현재 연결

`plots.rs::draw`(606 부근)는 `&mut PlotSpec`의 primary x/y 문자열을 `TextEdit`에 직접 전달한다. 따라서 타이핑부터 실제 Canvas source가 바뀐다. 다만 frozen `worksheet_ui.rs` Plot arm(4642 부근)은 반환 Response를 그대로 전달하고 Text arm의 `changed |= response.changed()` 같은 연결이 없다. 공통 응답 부분에도 이를 대신하는 처리가 없다. 따라서 현재 공백은 매 키 계산만이 아니라 **source는 변하지만 dirty/history/commit 경계가 누락될 수 있음**이다. baseline을 실제 키 입력으로 먼저 고정해야 한다.

`PlotDraft`(worksheet_ui.rs 83)는 별도의 설정 창용 전체 spec 사본이다. `commit_plot_draft`(3348)는 validate→전체 region 교체→remember→calculation_changed를 수행한다. 이것은 inline text buffer와 별도 수명 주기를 가진다. 두 편집면을 같은 Option에 넣으면 popup 진입/취소가 inline 편집을 덮을 수 있다.

`HistoryEntry`(37)는 전체 문서 또는 단일 committed region과 현재 editor/cursor만 보존한다. `input_snapshot`(685)의 단일 region 빠른 경로에는 현재 Plot이 없다. `snapshot`은 ZIP/cache를 빼도 모든 regions를 복제한다. 이 경로에 inline Text를 그대로 연결하면 매 키마다 전체 문서를 복제하는 회귀가 된다.

## 가장 작은 권장 모델

권장하는 최소 구현은 owner별 `Option<draft>`와 작은 borrowed `egui::TextBuffer` adapter다. `as_str`은 draft가 없으면 committed primary x/y 문자열을 빌리고, 최초 실제 insert/delete에서만 spec을 한 번 복제한 뒤 String TextBuffer에 문자 offset 처리를 위임한다. 빈 insert/빈 range 삭제에는 복제하지 않는다. 프레임 끝 candidate가 원문과 같으면 draft도 제거해 no-op/redo를 보존한다. 모든 idle plot의 문자열이나 PlotSpec·curve points·native ZIP·Undo 배열을 프레임마다 복제하지 않는다. 한 owner의 x↔y 전환은 같은 draft를 사용한다. 여러 trace 설정은 기존 modal을 유지한다.

`draw`는 committed spec을 읽고 TextEdit에는 adapter를 전달한다. 응답을 expression_changed / commit_requested / focused_field / resize_requested로 구분한다. pan/zoom은 여전히 source 변경이 아니다. resize를 식 buffer와 섞지 말고 기존 단일 gesture history를 유지한다. full-spec draft를 사용한다면 resize 시작·modal 진입 전에 text를 확정하고 그 뒤 geometry/config를 변경해야 한다. 또는 commit할 때 현재 committed spec에 primary x/y만 덮는다. 오래된 draft가 외부에서 바뀐 trace 스타일·크기를 되돌리지 않게 해야 한다.

타이핑은 dirty 표시와 화면 문자열만 바꾼다. document source/signature, calculator generation, pending scheduling, saved native marker, 표시 중 curve는 건드리지 않는다. 편집 중 흐리게 표시하려면 inline dirty를 직접 paint에 전달한다. `pending_regions`에 넣어 Automatic이 이전 식을 쓸데없이 실행하게 하지 않는다. IME preedit는 draft 문자열 변경이나 history로도 취급하지 않는다.

commit controller는 계산을 직접 호출하지 않는 작은 transaction으로 둔다.

1. dirty draft가 있는 실제 Plot owner를 확인하고 현재 spec으로 candidate를 만든다.
2. PlotSpec 구조/문자열 byte 한계를 검증한다. 실패하면 문서·curve·draft·redo를 모두 보존한다. 여러 owner를 확정할 명령은 전부 preview한 뒤 설치한다.
3. 원래 문자열로 돌아온 draft나 순수 focus/caret 이동이면 아무 작업도 하지 않는다.
4. source 변경이 있으면 단일 Region history를 한 번 보존하고 candidate를 설치한다. draft를 새 baseline으로 정리한다. 키마다 owner history를 쌓지 않는다.
5. 호출자가 마지막에 한 번만 계산 통지를 한다. Enter/focus commit은 자동/수동 설정을 따르고 명시 계산 명령은 자체 명령을 한 번 실행한다.

불완전한 식 `sin(`은 구조 문자열 한계 내에서 committed source가 될 수 있다. 이를 직전의 유효한 식으로 되돌리면 사용자 입력이 사라진다. 계산을 요청한 시점에 parser 오류로 처리하고 오래된 curve는 Pending presentation으로 유지한다. 반면 byte 한계 위반처럼 PlotSpec 자체가 저장 불가능한 경우는 source 설치 전에 실패시킨다. `TextEdit::char_limit(4096)`는 Unicode UTF-8 **byte** 한계를 보장하지 않는다.

`native_plot::same_source`는 계산 없는 구조 비교에 사용할 수 있지만 오류가 난 식을 무조건 commit 거절하는 validation 대용으로 쓰지 않는다. 원시 문자열은 달라도 식 구조가 같은 경우 source 표현 저장과 계산 필요 여부를 구분할 수 있다. 첫 구현에서 이를 최적화하지 않아도 계산은 키마다가 아니라 commit 한 번으로 제한한다.

## 반드시 연결할 경계

| 현재 위치 | 필요한 연결 |
| --- | --- |
| `plots.rs::draw` 606–767 | Enter/lost-focus를 해당 field 응답으로 포착하고 expression dirty와 geometry를 구분한다. 기존 y/x stable IDs를 유지한다. |
| `page_ui` 4642 / 5020–5056 | 실제 문서 대신 inline buffer를 그린다. field typing을 generic changed→remember/calculation 경로에 넣지 않는다. |
| `page_ui` 마지막 previous_editing 검사 5310 | plot owner를 실제로 떠날 때 dirty draft를 commit한다. 같은 plot x↔y 이동·popup/menu_resume은 따로 판단한다. |
| worksheet Enter 5165 | 현재 Math 전용이다. plot Enter를 이 경로에 억지로 넣어 새 Math/삽입점을 만들지 않는다. PlotResponse의 commit을 먼저 소비한다. |
| `sync_editors` 1081 / Save 1095 | inline source를 preview/commit한 뒤 저장한다. 저장 취소/실패 때 draft를 잃지 않는다. Save는 완료된 전체 재계산을 강제하는 명령으로 바꾸지 않는다. |
| native export 2234 | 같은 source commit 경계를 거친다. 새 식에 이전 curve가 Synchronized로 결합되지 않도록 Pending/source-signature 규칙을 유지한다. |
| `calculate_command_with_active` 1252–1340 | Automatic 이외의 명령은 inline commit을 **Now의 early return보다 먼저** 수행한다. commit helper에서 계산 후 이 함수가 다시 계산하는 이중 실행을 피한다. Automatic은 미확정 inline text를 commit하지 않는다. |
| `open_print` / `begin_print` / `print_snapshot` | 사용자 인쇄 명령의 편집 종료 경계에서 source를 확정할지 명시한다. snapshot/render 자체는 계산·commit을 하지 않는 pure presentation 경로로 유지한다. 수동 모드의 이전 curve 출력을 허용한다. 원본 print가 모든 plot draft를 확정하는지까지는 0042/0149의 확정 사실이 아니다. |
| `EditPlot` 2800 / `commit_plot_draft` 3348 | 설정 창을 열기 전 inline source를 확정하거나 같은 candidate를 명시적으로 전달한다. modal Cancel이 inline 입력을 버리지 않게 한다. Apply no-op이면 추가 history/계산을 만들지 않는다. |
| Undo/Redo 865–907 / restore 744 | field가 focus를 가진 동안의 편집 Undo는 TextEdit의 로컬 history로 보내고 owner history를 동시에 pop하지 않는다. committed Undo/Redo는 해당 inline state를 복원된 source로 reset한다. field focus/caret 복원은 작은 cursor state만 기록한다. |
| New/Open/Close 및 remove/cut/duplicate/copy | dirty inline text도 닫기 확인에 포함한다. 소유 region이 없어지면 state를 제거한다. region 작업과 텍스트 Ctrl+C/V를 구분하고 source를 확정할 명령의 순서를 정한다. |
| 비동기 계산 결과 | draft 중에는 committed 식이 그대로이므로 기존 job 결과가 그 source에 대응한다. commit 후에는 기존 generation/signature 필터가 이전 job을 거절해야 한다. |

기존 `uncommitted_math`에 plot ID만 넣어 재사용하지 않는다. Math는 실제 문서 AST가 이미 바뀌므로 CommittedBaseline으로 이전 입력을 복원하는 설계지만, 제안한 inline plot은 문서가 마지막 committed source를 그대로 보유한다. 이 둘을 섞으면 불필요한 전체 preparation·baseline materialization이 생긴다.

Undo의 세부 원본 granularity와 plot 하위 x/y field 사이의 focus commit 우선순위는 0149에서 미확정이다. TextEdit 로컬 Undo와 plot owner를 떠날 때 한 transaction을 만드는 것은 이번 독립 UI 구현 제안이며 원본 전체 routed-command 재현 완료라는 주장이 아니다.

## 의미 있는 headless 검사

* cold native plot에서 실제 focus/선택/Text를 여러 프레임 입력한다. draft만 변하고 document source·curve points·calculator generation·last_executed_regions·owner Undo 길이는 그대로인지 확인한다. IME preedit도 포함한다.
* 자동 모드 Enter는 한 번만 commit/계산하고 새 curve를 만든다. 수동 모드 Enter는 source만 확정하고 이전 curve/Pending을 남긴다. unchanged Enter와 원래 문자열로 돌아온 draft는 redo와 history를 보존한다.
* 다른 math/plot/Area/빈 곳을 실제 클릭하여 focus exit가 한 번만 commit하는지 검사한다. 동일 plot의 x↔y Tab은 정한 aggregate 정책을 따른다.
* F5/Ctrl+F5가 dirty inline source를 early return 전에 확정하고 exactly-once 계산하는지, Automatic/무관한 UI frame은 이를 확정하지 않는지 검사한다.
* 저장·own cold reopen·native export에서 수정된 source와 Pending/새 curve가 정확히 결합되는지 검사한다. old native-origin marker를 source 변경 후 동기 결과로 승격하지 않는다.
* draft local Undo와 owner Undo/Redo를 실제 Ctrl+Z/Y로 분리 검증한다. committed source 복원 후 stale buffer가 다시 덮지 않아야 한다.
* 긴 Unicode paste의 byte-limit 실패, 없어진 owner, modal Cancel/Apply/no-op, resize 중 draft, pan/zoom, print snapshot의 무계산을 검사한다.
* warm idle와 각 Text stimulus에서 기존 frame counters로 preparation/evaluator 제출 0을 확인한다. 절대 시간 비교 전에 per-frame 문자열/문서/곡선 복제 여부를 먼저 검증한다.

## drawing 모듈의 반영 경계

`plots/inline_edit.rs::InlinePlotEdit`는 optional PlotSpec draft와 borrowed TextBuffer를 제공한다. buffer 읽기·빈 insert/delete·동일 replace는 draft를 만들지 않는다. 최초 실제 변경에 한 번만 clone하고 이후 Unicode 문자 offset 편집은 String TextBuffer에 위임한다. `normalize`는 원래 source로 돌아온 candidate를 버린다. 이 모듈은 source XML·수식 parser·계산기·root history를 호출하지 않는다.

`plots::draw`는 `&mut PlotSpec` 외에 `&mut InlinePlotEdit`를 받고 `DrawResponse { response, expression_changed, commit_requested, resize_started }`를 반환한다. 실제 source 수정은 resize의 width/height뿐이고 축 문자열은 언제나 draft에 쓴다. field focus 중 plain Enter는 commit 요청만 반환하며 field focus를 유지한다. Shift/수정자 Enter와 전역 Enter를 가로채지 않는다. Escape는 draft와 composing 상태만 취소한다. pending draft에도 최신 resize dimensions를 복사하므로 뒤 commit이 geometry를 되돌리지 않는다. root가 source transaction·계산 모드·history·수명 주기를 연결한다.

같은 aggregate 안의 y→x plain Tab과 x→y Shift+Tab은 해당 field focus를 같은 frame에 요청하고 반환 response ID도 함께 바꾼다. x에서의 outward Tab과 y에서의 outward Shift+Tab은 기본 traversal에 남긴다. 이 구분으로 egui의 한 frame focus gap이 plot 전체를 떠난 것으로 오인되는 것을 막는다. 이 독립 UI 대응을 원본 전체의 routed-command 우선순위가 확정됐다는 뜻으로 해석하지 않는다.

egui TextEdit는 IME preedit도 buffer에 임시 삽입하므로 이것은 draft 안에만 남긴다. `InlinePlotEdit::is_composing()`으로 root가 명시 Save/F5와 focus 확정을 유예할 수 있다. 활성 preedit 중 plain Enter는 TextEdit의 기본 focus 해제에도 전달하지 않고 owner commit도 요청하지 않는다. 최종 IME Commit과 Enter가 같은 frame이어도 owner commit을 유예하고, 뒤의 plain Enter에서 요청한다. Commit/빈 preedit/Disabled/cancel은 composing flag를 지운다.

늦은 focus 변경에 대한 대응은 독립 IME 통합 정책이다. 최초 nonempty preedit에서 활성 축 문자열 하나와 조합 전 caret 문자 offset만 보존한다. 실제 focus가 그 축을 떠나거나 조합이 취소되면 그 축의 미확정 preedit만 복원하고 composing을 해제한다. 다른 축과 조합 이전에 확정된 draft 편집은 유지한다. 복원 시 TextEdit caret를 조합 전 위치로 접어 남아 있던 preedit selection이 확인된 문자열 뒤쪽을 지우지 않게 한다. 이 과정에서 다른 owner의 IME 이벤트를 소비하거나 해석하지 않는다. 전체 PlotSpec·곡선·TextEdit Undo 배열을 checkpoint로 복제하지 않는다. 이는 플랫폼별 원본 routed-command 동작 전체를 재현했다는 주장이 아니다.

buffer 검사는 idle/no-op clone0, 실제 Unicode 편집 clone1, materialized 다른 trace/source metadata 보존, 원문 복귀·취소·resize dimensions 보존을 검사한다. CPU frame 검사에서는 실제 Text/Enter와 수정자 Enter/Escape의 source 비변경·독립 commit 요청, 다른 plot이 프레임 뒤쪽에서 focus를 얻은 후 Escape가 그 대상만 취소하는 경우를 검사한다. egui가 Escape 전에 focus를 해제하므로 `Memory::had_focus_last_frame`의 정확한 field ID를 사용하고 자체 draw 시점의 오래된 bool로 대체하지 않는다. 실제 y→x Tab과 x→y Shift+Tab, Enabled/Preedit/Enter/Commit/후속 Enter, 늦은 focus 이동 뒤 Disabled·빈 Preedit, 문자열 중간 조합 취소 뒤 정상 입력도 검증한다. 소유 plot 검사 22개(기존12+inline10)와 app all-target Clippy가 통과했다. 통합/최종 gate 결과는 이후 통합 기록과 구분한다.
