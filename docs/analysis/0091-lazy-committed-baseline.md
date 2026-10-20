# 첫 입력 전 확정 계산 입력의 지연 준비

2026-10-19. `retain_committed_input`가 첫 draft mutation 전에 이전 문서 전체를 MathML로 직렬화·label 처리하고 표시 트리를 복원하던 비용을 조사했다. 일반적인 입력→Enter에서는 새 문서만 계산하며, 먼저 만든 이전 XML은 사용하지 않았다. 이번 변경은 원본 기능을 추가하지 않고 기존 Enter 확정·Manual·F5 계약을 유지하면서 해당 작업을 필요할 때까지 미룬다. GUI·스크린샷·원본 Mathcad 실행·새 계산 worker는 사용하지 않았다.

## 이전 경로와 변경

이전 cold 경로는 `committed_input=None`이고 draft가 없을 때 `prepare_calculation_input().ok()`를 호출했다. 여기에는 presentation signature, 모든 수식 직렬화, opaque payload/XML 검증, 전체 worksheet label 처리, 표시용 트리 복원이 포함됐다. 첫 편집에서 이전 값을 보존하려는 목적이었지만, 이전 XML이 실제 필요한 경우는 draft를 남긴 채 다른 영역을 계산하거나 자동 계산 설정을 바꿀 때다.

이제 `PreparationCache::committed_baseline`에 **계산 입력에 필요한 raw snapshot만** 보관한다. retain은 source를 hash하거나 MathML/worksheet를 생성하지 않고 engine update/worker dispatch/계산도 하지 않는다. `WorksheetApp`에 별도 상태 필드를 추가하지 않았다.

| snapshot에 보관 | 제외 |
|---|---|
| `CanvasRegion`의 source tree/opaque XML·id·좌표·원본 순서·format·source_region·disabled 및 enabled override | plain `CanvasContent::Text` |
| width·height·grid의 원래 값 | 제목·next_id·page header/footer·별도 saved presentation cache |
| native archive의 **모든 바이트를 소유 복제** | 기존 결과·표시 캐시·Undo/Redo·editor history/caret |
| 각 live math editor의 mutation 전 revision과 검증 오류 결과 | editor 객체 자체와 AST 이외의 편집 상태 |

이 선택은 현재 `CanvasDocument::active_calculation_signature`와 준비 함수가 읽는 source 범위에 맞춘 것이다. plain Text만 제외하며, signature에 참여하는 opaque `text`/`image` 등은 원문과 metadata를 보존한다. disabled source도 snapshot의 region 상태에 남고 실제 준비 때 기존 규칙대로 실행에서 제외한다. header/footer와 saved 결과는 계산 입력 생성에 참여하지 않으므로 복제하지 않는다. snapshot은 저장·native export용 문서가 아니다.

native archive는 pointer·길이·부분 digest로 동일성을 추정하지 않는다. 같은 길이로 한 바이트만 바뀐 archive도 다음 snapshot에 정확히 들어간다. snapshot 이후 live document·editor·archive를 바꿔도 과거 입력은 변하지 않는다.

## 확정·명령·수명

- 첫 잠재 편집 이벤트는 raw baseline만 만든다. 첫 key에서 전체 XML 준비 오류나 새 계산 결과를 공개하지 않는다.
- 실제 draft가 시작되면 baseline을 고정한다. Enter/명시 평가가 새 문서를 확정하면 사용하지 않은 baseline을 버리고 현재 입력만 준비한다. disabled item의 commit처럼 계산을 생략하는 경로도 해당 commit helper에서 baseline을 폐기한다.
- draft를 유지한 F5/다른 영역 계산/Auto 설정 변경이 과거 입력을 요구할 때만 frozen snapshot을 준비하여 기존 `committed_input`으로 승격한다. 성공한 입력은 재사용하므로 같은 draft의 후속 명령이 다시 materialize하지 않는다.
- 잠재 편집 이벤트가 거절된 명령·IME preedit·실제 text가 없는 key처럼 no-op일 수 있다. draft가 없고 committed input도 없으면 **retain 때마다 최신 raw snapshot으로 교체**한다. 그 사이 format·source·order·archive가 바뀌었는데 첫 snapshot을 계속 쓰는 정책을 두지 않았다.
- Open/New는 기존 preparation reset/restore 경로가 raw baseline도 함께 폐기한다. 이전 문서 baseline이 다음 문서로 넘어가지 않는다.

일반 commit은 이전 baseline을 materialize하지 않는다는 회귀를 별도로 둔다. 이는 첫 입력의 작업을 Enter로 단순 이동시켜 두 문서 모두 준비하는 구현과 구분된다. 과거 문서가 실제 필요한 특수 명령에서는 이전의 준비 비용을 여전히 지불한다.

## 공통 순수 준비 경로

기존 준비 본문을 `prepare_input(cache, document, EditorInputs::Live|Frozen, labelled_trees)`로 분리했다. source 서명·직렬화·opaque 검증·오류 placeholder·정렬/label 처리·geometry cache·표시 트리 복원은 같은 구현이다. live 경로는 기존 editor validation cache를 사용하고 frozen 경로는 mutation 전에 보존한 revision/오류를 사용한다. validation을 위해 필요한 `to_portable` 검사는 retain에 남지만, editor history나 worker 환경은 복제하지 않는다.

frozen materialization에는 **독립된 임시 cache와 임시 labelled tree map**을 사용한다. 이 결과의 `CalculationInput`만 승격하며 현재 draft의 preparation cache·labelled_trees를 이전 문서 값으로 교체하지 않는다. 현재 문서에서 이미 준비된 XML/표시 트리가 있으면 그 소유와 내용은 그대로다. helper는 계산기의 update/execute를 호출하지 않고 `CalculationInput`과 caller가 제공한 cache/label map만 만든다. 실제 명령 처리 시 기존 `refresh_calculation_input`의 update/dispatch 경계는 유지한다.

전체 XML 생성 오류도 첫 key에서는 보이지 않는다. frozen 입력을 실제로 요청하면 공통 helper가 원래 준비 오류를 반환하며, 유효한 committed input으로 가장하지 않는다. 오류가 난 raw baseline은 남아 후속 요청에서도 같은 입력을 판정한다. invalid live math는 기존처럼 failed statement와 faults로 표현되어 이전 정상 binding을 조용히 되살리지 않는다.

## 검증

`worksheet_ui/lazy_baseline.rs`의 8개 순수/headless 회귀와 기존 `edit_commit` 준비 회귀를 함께 실행했다.

- cold retain에 prepared XML·DOM decode·labelled tree·계산 결과가 생기지 않고 saved 결과 payload도 snapshot에 들어가지 않는다.
- 같은 source를 Live/Frozen으로 준비한 **XML·faults·signature·region ids·labelled trees가 같다**. 서명은 별도로 `CanvasDocument::active_calculation_signature`와도 비교한다.
- draft 변경 후에도 이전 baseline은 그대로이며, frozen materialization이 현재 live XML Arc/label map을 바꾸지 않는다.
- 일반 Enter는 이전 baseline 준비 횟수 0, 다른 영역 F5/Auto toggle은 과거 값과 draft를 유지한다. Manual commit 뒤에는 기존 계약대로 명시 Worksheet 계산이 새 결과를 공개한다.
- invalid editor와 Undo, disabled source, no-op retain 뒤 format/order/settings/동일 길이 archive 교체, 전체 XML 오류, Open/New reset을 검사한다.
- SpecTable의 column definition/body XML을 snapshot 뒤 바꿔도 frozen 입력에 바뀐 셀 값이 유입되지 않는다.

focused `edit_commit` 검사는 **29 passed / 0 failed / 5 ignored**였고, app all-targets Clippy `-D warnings`도 통과했다. 로그는 ignored `lazy-baseline-focused-tests-20261019.log`, `lazy-baseline-clippy-20261019.log`다. 여기에는 별도 담당자가 만든 **실제 save→fresh open** lifecycle 회귀 4개도 포함된다. 이 cold fixture는 이미 한 번 계산한 문서를 사용하는 회귀와 구분하여, 첫 Text draft→offscreen F5의 과거 값→Enter/명시 Worksheet의 새 값을 확인한다.

전체 Windows/peets 동일 소스 검사와 actual-open 첫 입력 시간의 before/after는 상위 통합 검증을 따른다. 이 문서는 headless 입력 경로의 비용 제거를 설명하며 실제 창의 GPU/OS 프레임 시간이나 제품 전체 지연 해결을 주장하지 않는다. retain의 tree/archive 복제와 editor validation 비용은 남는다.
