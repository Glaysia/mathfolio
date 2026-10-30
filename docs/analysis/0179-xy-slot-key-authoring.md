# XY 축 슬롯 키 작성과 입력 순서

2026-10-19. 원본 정적 계약과 현재 제품 소스를 대조한 단계 문서다. r184의 최종 archive·양 OS gate·native/PDF 검증은 [0180 통합](0180-xy-slot-key-integration.md)에 확정했다. 이 문서는 원본 전체 일치나40% 달성을 주장하지 않는다. 원본 분석은 GUI·스크린샷·원본 runtime/CLR 실행 없이 수행했다.

원본 근거는 [0157 슬롯 작성](0157-native-xy-slot-authoring-contract.md), [0158 모델](0158-xy-slot-model-design.md), [0161 식 배치](0161-native-xy-expression-layout-contract.md), [후속 계획](../../artifacts/rea/xy-slot-key-next-plan-20261019.md)과 [후속 CIL ledger](../../artifacts/rea/xy-slot-key-next-method-ledger-20261019.json)다. Ledger의24개 메서드 수는 조사 범위이며 제품 진척률이 아니다. 토큰은 각 설치 PE의 SHA/MVID에 묶인 위치다. 이번 단계가 원본 모든 routed command를 복원했다는 뜻은 아니다.

## 원본 명령과 포커스 양성

| 조작·조건 | 확인된 원본 동작 | 주요 위치 |
|---|---|---|
| Shift+Enter | InsertRow gesture. 정확한 Shift-only 입력이며 CtrlShiftEnter와 구분한다. | EquationCommands0025; Core Gestures04d6 IL019c..01a3 |
| 식의 실제 왼쪽 경계이고 식이 정확히 PH가 아님 | 현재 식 앞에 슬롯 삽입 | EquationList00d3 |
| 나머지 상태·PH 식·scaling 칸 포커스 | 현재 식 뒤에 삽입. 문자열을 나누거나 복사하지 않음 |00d3; PlotEquation0340 |
| 새 슬롯 | expression/scaling PH, 새 equation identity, 새 expression 컨트롤에 focus 위임 |030c/0336; callback149f→035c |
| expression과 scaling 모두 정확히 PH에서 Backspace/Delete | 빈 슬롯 제거 요청. scaling에 단위·Group(PH)·불완전 연산자가 있으면 빈 슬롯이 아님 | PlotEquation031c/031d |
| 빈 슬롯 하나만 남음 | 더 제거하지 않음 | EquationList 삭제 경로 |
| 빈 슬롯 제거 후 Backspace | predecessor 우선, 없으면 successor의 scaling/end | RemoveItemAt00c2→NavigateRegionEnd0360 |
| 빈 슬롯 제거 후 Delete | successor 우선, 없으면 predecessor의 expression/start |00c2→NavigateRegionStart0361 |

AtLeftEdge는 단순 selection의 최소 offset이 아니다. EquationControl00a1→EquationModel00e6/00e8은 canonical 첫 NavigatorState와 현재 node·nullable CaretPosition·CaretSide·NodeSide를 비교한다. Normal과 Scope의 상태 취득 경로도 다르다. NavigatorState1325는 ValueRange의 **caret focus**를 읽는다. 따라서 forward 전체 선택의 시작이0이더라도 focus=end이면 왼쪽 경계라고 단정할 수 없다. native UTF16 범위와 egui char index의 일반 변환도 구분해야 한다.

새 슬롯의 expression focus는 확인했지만, 최초 PH의 원본 선택 폭·전체선택 여부·픽셀 caret 위치는 이 경로만으로 확정하지 않았다. 앱의 새 식/start는 CPU 편집기에 맞춘 제한된 대응이다.

## 내부 행 명령이 먼저다

scaling은 독립 수식 편집기다. EquationControl015d는 InsertRow→01a5에 CanPerformRowOperations01ab를 붙이며, 모델0193/0196은 read-only/scriptable/empty 조건 뒤 GetNearestNaryAncestor 존재를 확인한다. 따라서 원본 우선순위는 단순히 “행렬이면 내부, 아니면 축 슬롯”이 아니다.

현재 [slot_keys.rs](../../crates/mathfolio-app/src/plots/slot_keys.rs)의 inner_row_owner는 지원하는 atom/산술 경로만 owning axis로 넘기고, Matrix나 미지원 컨테이너의 live caret 경로는 내부 명령에 남긴다. 내부 명령이 예산·지원 범위 때문에 실패해도 축 슬롯 삽입으로 우회하지 않는다. 이 보수적 경계는 모든 nary/함수/미적분 편집의 완전한 원본 구현과 구분한다. IME composition 중에는 구조 명령을 실행하지 않으며, 원본 IME routed priority 전체를 복원했다고 주장하지 않는다.

## 현재 source·draft·history 연결

[slot_edit.rs](../../crates/mathfolio-app/src/plots/inline_edit/slot_edit.rs)는 검증된 aggregate draft가 있으면 그것을 기준으로 candidate를 만든다. 실제 첫 draft에서 원본 PlotSource baseline을 보관하고, 외부 same-ID source 변경을 borrowed equality로 검사한다. geometry resize는 source와 별도로 처리한다. 삽입은 새 empty expression/scaling=None이며 기존 식·typed scaling은 identity별로 보존된다. 실패·상한·allocator overflow는 source/draft/redo/포커스·palette counter를 부분 변경하지 않는다.

[slot_keys.rs](../../crates/mathfolio-app/src/plots/slot_keys.rs)는 포커스 ID와 field part를 확인하고 expression의 `CCursorRange.primary`를 사용한다. `sorted_cursors()[0]`를 왼쪽 경계 판정에 사용하지 않는다. scaling은 항상 뒤 삽입이다. plain Backspace/Delete는 양쪽이 빈 슬롯일 때만 owning axis가 소비하고, 채워진 식은 원래 편집기에 남긴다. 삭제 후 focus target의 part와 edge는 위 표처럼 구분된다.

구조 Undo/Redo는 source·scaling 편집 상태·focus checkpoint를 복원하는 로컬 draft transaction이다. 최초 PH 및 일반 텍스트 편집을 포함한 원본 전체 Undo 경계와 같다는 주장은 하지 않는다. Cmd+Z/Cmd+Y는 구조와 그 뒤 필드 수정의 혼합 로컬 이력을 순서대로 처리하고, Cmd+Shift+Z를 redo로 바꾸지 않는다.64개 entry는 독립 앱 이력 개수 제한이며 측정된 byte/RSS 상한이 아니다. owning plot의 Enter/focus commit 후에는 중앙 문서 이력과 계산 수명 경로가 적용된다.

## trace appearance와 identity

원본은 새 trace가 필요한 경우에만 ApplyNextStyle을 호출한다. 확인한 palette는 DarkBlue, Black, Red, Yellow, Orange, Green, Blue이며 선 두께1·실선·심볼 없음이다. 짧은 축의 슬롯 삽입은 기존 trace들을 재연결할 수 있으므로 모든 삽입에서 새 appearance를 만들면 틀린다. 원본은 longer-axis 순서의 SortedTraces로 저장하며 shared equation membership과 전역 append 순서는 별개의 개념이다([0157](0157-native-xy-slot-authoring-contract.md)).

현재 어댑터는 순수 document 모델의 trace 생성 판정/삽입·삭제를 재사용하고, 새 trace일 때만 runtime palette cursor를 전진시킨다. palette cursor와 equation ID floor는 Undo/Redo/Cancel로 되감지 않는다. ID floor는 제거된 슬롯의 숫자 ID를 같은 편집 세션에서 재사용하지 않게 하는 **독립 앱 정책**이다. 원본 managed object identity와 앱의 owner-local u64 ID가 동일한 직렬화 필드라는 주장은 하지 않는다.16 슬롯 상한도 앱 예산이며 원본 최대치로 제시하지 않는다.

[plot_slot_lifecycle.rs](../../crates/mathfolio-app/src/worksheet_ui/plot_slot_lifecycle.rs)는 기존 계산 곡선을 `(xEquationId,yEquationId)` 쌍이 그대로 남았을 때만 유지한다. 단순 ordinal·같은 식 문자열·같은 trace 개수를 identity 증거로 쓰지 않는다. 새로 생기거나 재연결된 쌍은 이전 이웃 곡선을 옮기지 않고 Pending으로 둔다. 사용자 scaling·자동/원본 단위 presentation의 소유권 역시 식/계산 결과에 남으며 슬롯 작성이 단위를 재해석하지 않는다.

## 배치 입력 순서와 raw hook

원본과 별개인 앱 입력 통합 문제다. 예를 들어 `[Text(prefix), ShiftEnter, Text(suffix)]`를 한 번에 넘기면, 구조 명령을 너무 일찍 실행할 때 prefix가 새 슬롯으로 가거나, 너무 늦게 실행할 때 suffix까지 이전 슬롯에 들어갈 수 있다. 글로벌 F5/save가 page draw보다 먼저 실행되는 중앙 순서도 고려해야 한다.

새 [plot_input_queue.rs](../../crates/mathfolio-app/src/worksheet_ui/plot_input_queue.rs)는 공식 eframe raw_input_hook에서 **egui InputState 계산 전** 이벤트를 이동해 순서를 나눈다. `prepare(&mut RawInput, active_plot)`은 backlog를 새 OS 이벤트 앞에 이어 붙이고, mutation prefix·명령·후속 mutation을 별도 delivery로 보낸다. 앞 segment의 TextEdit가 저장한 최신 caret를 다음 command segment에서 사용하므로 현재 구조 명령은 prelayout이어도 prefix보다 먼저 실행되지 않는다. 문자열·물리키·per-event modifiers·IME payload는 복제/재작성하지 않는다.

- prefix/ShiftEnter/suffix는 순서대로 세 segment다. replay 중 F5/CtrlF5·저장/열기/새 문서/인쇄와 텍스트도 분리해 글로벌 prelayout scan이 뒤쪽 명령을 먼저 실행하지 않게 한다.
- 단순 ShiftEnter+keyup+modifier 해제와 일반 PointerMoved+Press+Release는 같은 delivery다. 뒤따르는 benign key release/modifier/hover는 함께 보낸다. 일반 클릭·단일 명령에 불필요한 프레임을 추가하지 않는다. Touch는 확인되지 않은 채 benign으로 취급하지 않는다.
- Cmd+A+Text는 같은 segment를 유지한다. Cmd+Shift+Z는 redo barrier가 아니다. 비활성 plot이며 backlog가 없으면 RawInput을 그대로 통과시킨다.
- 중앙 [WorksheetApp](../../crates/mathfolio-app/src/worksheet_ui.rs)은 raw_input_hook→prepare_plot_input을 연결하고, backlog가 남으면 request_repaint한다. 열기 적용 시 clear, 새 문서 적용 시 앱 Default 재생성으로 이전 문서의 deferred tail을 취소한다. 큐 자체는 equation owner를 추측하거나 다른 필드에 강제로 focus하지 않는다.

egui0.36.2 Context::run_dyn은 pass마다 RawInput.take로 events를 이동하며, request_discard가 입력을 자동 재생하거나 모델을 rollback하지 않는다. 따라서 이 큐는 discard에 의존하지 않는다. eframe hook은 native input cycle당 실행되고, headless frame helper도 같은 prepare 경로를 호출한다. InputState의 key-down/repeat/modifiers는 실제로 전달된 이벤트 순서로 계산된다. 이 버전 RawInput에는 별도 modifiers snapshot 필드가 없고, ModifiersChanged와 각 Key/Pointer의 modifiers를 보존한다. 반면 focused·viewport·time 등 window metadata는 최신값이며 과거 window 상태를 재생하지 않는다.

추가 source 읽기 근거와 대안/한계는 [입력 순서 설계](../../artifacts/rea/plot-slot-input-order-design-20261019.md), [dependency/source identities](../../artifacts/rea/plot-slot-input-order-source-identities-20261019.json)에 있다. 해당 설계의 after-draw 실행 제안보다 현재 구현은 **직전 segmented draw의 caret를 이용한 prelayout 명령**으로 좁혀졌다.

## 검증 범위와 남은 한계

현재 source에는 [어댑터 회귀](../../crates/mathfolio-app/src/plots/inline_edit/slot_edit_tests.rs), [실제 키/포커스 회귀](../../crates/mathfolio-app/src/plots/slot_key_tests.rs), [문서 통합 회귀](../../crates/mathfolio-app/src/worksheet_ui/plot_slot_edit_integration.rs), [identity 수명 회귀](../../crates/mathfolio-app/src/worksheet_ui/plot_slot_lifecycle.rs)와 큐 순수 테스트가 있다. selection 방향/보조 문자·식/scaling 빈 판정·내부 행 우선·예산 실패·이력·팔레트·batch ordering 등 검증 대상을 소스에서 확인했다. 최종 gate 결과와 산출물 수·해시는 [0180 통합](0180-xy-slot-key-integration.md)에 기록했다.

명시적 한계는 다음과 같다. plain String expression 편집이 native 구조 NavigatorState·Scope·literal suffix 전체를 표현하지 못한다. 미지원 nary 내부 편집을 owning slot 삽입으로 대체하지 않는다. 원본 전체 IME/preview routed priority·최초 PH 선택 폭·모든 특수 축/legend reorder·native cap은 미확정이다. 큐의 backlog에는 무음 drop cap이 없으며 필요하면 별도 admission 정책이 필요하다. source reset clear는 의도적인 취소이고 window focus/viewport/time snapshot을 복원하지 않는다. 이 제한을 원본 기능이 금지한다는 뜻으로 바꾸지 않는다.

## r184 검증 준비와 실제 키 경계 보완

단계 이름은 slot-keys-r184다. 별도 freeze/windows/probes/peets/validate/finalize 준비 스크립트는 기존 r180 파일을 보존한다. 실행 전 준비 기록과 실제 source archive·최종 gate 수치는 [0180](0180-xy-slot-key-integration.md)에서 구분한다. 새 실제 배치 키 probe는 source 1X3과 factors3/4/5, own/native cold 복원, vector PDF를 대상으로 한다. pending 저장/복사에서는 모든 resultRef가 Pending이며 새 가운데 곡선은 비우고 기존 첫/마지막 곡선을 보존하는 별도 수명 검사도 통과했다.

실제 키 회귀에서 literal suffix를 포함한 scaling 식 끝 이동은 base 문자열 offset이 아닌 전체 식별자의 Right 경계로 교정되었다. IME preedit 중 ShiftEnter는 구조 삽입으로 전달하지 않고 단일행 TextEdit의 focus 종료도 막는다. geometry resize로 commit하는 경로 역시 source guard와 식 identity 쌍 기반 결과 재연결을 적용한다. 이들은 앱 통합 보완이며 원본 전체 IME/geometry routed command의 동일성을 주장하지 않는다.

추가 회귀에서 IME Commit과 같은 OS 묶음의 plain Enter는 composition 확인 키로 함께 전달해야 했다. 이를 다음 input cycle로 나누면 guard가 풀린 뒤 소유 영역을 잘못 확정한다. 큐는 이 확인 묶음을 유지하고 뒤의 다른 텍스트/명령은 계속 분리한다. 값이 구조 checkpoint와 같아졌다는 이유만으로 가장 최근 글자 삭제를 건너뛰는 문제는 [field_journal](../../crates/mathfolio-app/src/plots/inline_edit/field_journal.rs)의 좁은 혼합 이력으로 보완한다. 자세한 현재 수명과 검증은 [0180](0180-xy-slot-key-integration.md)을 따른다.
