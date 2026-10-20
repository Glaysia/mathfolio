# 개별 영역의 계산 비활성화

2026-10-16. 원본 GUI·스크린샷·Mathcad 런타임 실행 없이 [0033 원본 인쇄·계산 계약](0033-print-and-page-contract.md)의 설치 XSD·도움말·REA CIL 증거를 적용한 독립 구현이다.

## 원본 계약

`disable-calc`는 기본 false인 계산 항목 속성이다. 영역 비활성화는 선택 영역을 즉시 흐리게 표시하며 편집을 막지 않는다. 자동 계산, 지금 계산, 워크시트 계산은 해당 항목을 제외한다. 비활성 정의는 유효한 바인딩을 제공하지 않으며, 그 정의에 의존하는 후속 계산은 다른 유효한 정의가 없으면 오류가 된다. 비활성 항목을 편집하는 것만으로 후속 계산을 실행하지 않는다.

원본 상태 변경 이벤트의 정적 근거는 `EquationControl.HandleCalcIsDisabledChanged` (`0x06000220`)와 SpecTable의 대응 메서드 (`0x0600029b`)다. Area 전체 토글과 내부 개별 항목 토글은 별도 범위다.

## 구현

- `CanvasRegion.calculation_disabled`는 기본 false이며 false는 직렬화에서 생략한다. 따라서 이전 `.mathcraft` 문서가 그대로 열리고, 기능을 사용하지 않은 문서의 기존 결과 캐시 서명도 유지된다. 계산 가능한 원본 항목 외의 텍스트에는 비활성 상태를 저장할 수 없다.
- 계산 리본과 영역 우클릭 메뉴의 **영역 비활성화**는 같은 명령을 호출한다. 변경은 문서 Undo/Redo에 포함된다. 복제와 내부 클립보드 복사도 상태를 보존한다.
- 활성 수식만 계산 세션 입력에 넣는다. 비활성 수식·도표는 전역 선행 정의, 자동 계산, F5, Ctrl+F5, 직접 `=` 계산에서 실행하지 않는다. 제외된 정의가 이전 세션의 바인딩으로 남지 않도록 기존 세션의 입력 갱신·무효화 경로를 사용한다.
- 마지막 결과는 표시 캐시로 보존하고 비활성 표시와 함께 흐리게 렌더링한다. 저장 상태는 항상 `Pending`이며 계산 완료된 새 결과로 취급하지 않는다. 비활성 항목을 전역 미계산 작업 수에 넣어 워크시트 전체가 영구 대기 상태가 되게 하지 않는다.
- 활성 계산 입력의 서명은 비활성 수식·위치·서식을 제외한다. 저장 표시 서명에는 이를 포함한다. 저장 결과만 복원하고 엔진이 아직 초기화되지 않은 문서에서도 비활성 항목의 편집 때문에 다른 수식·도표를 재실행하지 않는다.
- MCDX 가져오기는 계산 항목의 `disable-calc="true"` 또는 `"1"`을 읽는다. 새 Mathcraft 수식 내보내기와 원본 MathML·불투명 계산 항목 보존 내보내기에서 속성을 쓰거나 해제한다. 원본의 다른 실행 속성·알 수 없는 XML은 이 상태 변경을 위해 삭제하지 않는다.
- 이 기능 이전에 저장한 `.mathcraft`도 원본 archive의 source-bound 항목을 확인해 누락된 비활성 상태를 복원한다. 이전 캐시 서명이 정확히 맞는 경우 마지막 표시는 보존하되 후속 결과까지 모두 `Pending`으로 전환한다. 이미 맞지 않던 캐시는 되살리지 않는다. 명시적으로 다시 활성화한 항목에는 `calculation_enabled_override`를 저장해 원본 archive의 옛 상태가 사용자 변경을 뒤집지 않게 한다.

## 회귀 확인

다음 테스트를 추가했다. 최종 운영체제별 통합 결과는 [CONFORMANCE](../CONFORMANCE.md)를 기준으로 한다.

| 검사 | 보장하는 경계 |
| --- | --- |
| `calculation_disable_is_backward_compatible_and_only_active_source_affects_dependencies` | 이전 JSON 기본값, 비활성 원본 편집의 활성/표시 서명 분리, 지원하지 않는 항목 검증 |
| `legacy_native_disable_migration_retains_pending_presentations_and_honors_explicit_reenable` | 이전 자체 문서 원본 속성 복원, 캐시 Pending 이전, 명시적 재활성화 보존, 이미 불일치한 캐시의 복구 금지 |
| `calculation_disable_exports_new_math_and_can_reenable_preserved_native_math` | 새 MCDX 비활성 속성 및 원본 보존 후 재활성화·동시 수식 수정 |
| `disabled_definition_is_excluded_from_all_calculation_scopes_and_can_be_reenabled` | 수동 전환, 직접 `=`·지금·전체·자동 계산 제외, 후속 undefined, 재활성화 |
| `disabled_edits_after_reopening_keep_pending_saved_results_without_waking_a_cold_engine` | 저장·열기, 이전 표시 Pending 유지, 엔진 초기화 없는 비활성 편집, 재활성화 시 재검증 |
| `disabled_plot_preserves_points_but_never_runs_or_invalidates_other_regions_when_edited` | 저장 도표 점 유지, 비활성 도표 실행 제외, 편집 후 다른 수식 재실행 없음 |
| `imported_native_disabled_math_keeps_its_saved_result_pending_and_never_executes` | 원본 속성과 저장 결과를 읽고, 유효하지 않은 원본 식도 비활성 상태에서는 실행하지 않음 |

## 남은 범위

평면 수식과 자체 도표의 계산 비활성화 경로를 구현했다. Area의 상위 상태 전파·중첩 토글, 모든 원본 include·MathBoard·SpecTable의 실행 엔진은 완료하지 않았다. 불투명 계산 항목의 속성 보존은 해당 원본 기능 자체가 편집·계산 가능하다는 뜻이 아니다. 비활성 상태를 바꿀 때 후속 무효화는 현재 계산 세션의 보수적 전파를 이용하며 원본의 정밀 의존성 그래프 전체를 복원했다고 주장하지 않는다. 원본 인쇄에서 비활성 결과에 적용하는 정확한 opacity도 이번 정적 증거로 확정하지 않았다.
