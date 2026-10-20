# 곡선별 결과의 확정·무효화·저장 수명

2026-10-19. [0162 설계](0162-per-trace-plot-result-design.md)와 [0165 원본 오류 캐시](0165-native-xy-error-cache-contract.md)를 앱의 실제 명령 경로에 연결한다. GUI·스크린샷·원본 실행 없이 Rust headless 입력·계산·파일 경로를 검증한다.

`PlotData`는 원래 ordinal 순서의 `TraceOutcome` 목록만 갖는다. 예전 첫 곡선 `.points` alias를 제거하고 `first_curve()`는 명시적 Option으로 남긴다. Ready·Pending(previous)만 좌표를 보유하며 Failed에는 과거 좌표가 없다. 성공 두 개 사이의 실패를 제거하여 뒤 곡선의 스타일/resultRef가 앞으로 당겨지지 않는다.

실제 source/context 무효화는 `mark_regions_pending`에서 Ready를 Pending(previous)로 바꾸고 native 원본 그대로 보존 표시를 해제한다. 단순 cold-open의 aggregate Pending은 무효화 명령이 아니다. v1의 aggregate Pending은 예전 모든 곡선에 적용하고, v2의 Ready/Pending 혼합은 각 슬롯을 유지한다. 계산 비활성화는 표시 전체를 Pending으로 보관한다. 정상 결과를 가진 sibling과 실패가 섞여도 오류 이동 대상에 포함하고 곡선 번호와 실제 진단을 표시한다.

토폴로지가 바뀌면 이전 좌표를 같은 개수라는 이유만으로 재사용하지 않는다. AxisLists는 X/Y equation ID 순서를 비교한다. Legacy 설정 창의 삭제/추가에는 별도 dirty flag를 두어 같은 개수로 되돌아와도 이전 좌표 연결을 폐기한다. 기존 명령 경로의 불변식이며 다중 inline 슬롯 구조 편집 UI가 완성됐다는 뜻은 아니다.

문서의 `SavedPlotSnapshot` DTO가 bounded v1/v2 parsing을 담당한다. 필수 variant key, 중복/미지원 key, 2MiB·16slots·curve당 2,000 finite points·16KiB 진단 제한을 검증한다. `Pending.previous` 누락은 null과 구분하여 거절한다. Incomplete는 strict empty struct variant로 읽는다. all-Ready는 기존 v1 wire를 유지하고 혼합만 v2로 쓴다. renderer용 prepared 좌표는 저장하지 않는다.

Native 결과 writer의 typed action은 Synchronized, PendingLiteral, PendingKeepPayload, PendingClearPayload를 구분한다. 이전 좌표가 있는 Pending은 그 좌표를 Pending으로 저장한다. fresh Failed/Incomplete와 좌표 없는 Pending은 해당 ref의 옛 payload를 제거한다. Unavailable 원문은 보존한다. 실패의 앱 문자열로 원본 EngineError 번호·NodeHandle을 만들지 않는다. 따라서 own 파일은 Failed 진단을 보존하지만 native fresh reopen은 빈 Pending으로 읽는 제한이 있다. 이는 live 원본 ResultManager clear 동등성을 주장하지 않는다.

Native reader는 missing/empty Pending 슬롯도 그 자리에 남긴다. 정상 curve 사이의 missing ref 때문에 정상 곡선이 사라지거나 ordinal이 압축되지 않는다. 여전히 지원하지 않는 원본 diagnostic/payload/namespace와 association 모호성은 기존 안전 경계에 남으며 print도 지원하지 않는 원본 결과를 조용히 생략하지 않는다.

새 리본 XY 삽입만 `blank_xy()`를 사용한다. PH X/Y 각 한 개와 한 trace, explicit Bottom/Right legend source다. 기존 Default/sample API와 old JSON은 유지한다. native 한 쌍 source는 기존 Legacy carrier로 읽히고 MathML multiplication은 괄호를 정규화하므로 app-private equation ID나 plain text bytes의 native 왕복 동일성은 주장하지 않는다. legend source codec 지원과 실제 legend 위치 layout 구현은 별도이며, 현재 후자는 미완료다.

통합 검증과 남은 범위는 [0167](0167-per-trace-plot-integration.md)에 기록한다. [0168](0168-native-xy-graph-region-geometry-contract.md)의 graph-size/outer region 분리는 다음 단계다.
