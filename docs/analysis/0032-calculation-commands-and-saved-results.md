# 저장 결과 복원과 원본 계산 명령 — 2026-10-16

[통합 목록](../FEATURES.md)의 W03/W04, HD001/HD004/HD005/HD006/HD009를 먼저 연결한 구현 기록이다. 원본의 모든 계산 상태·의존성·문서 결과를 완성했다는 뜻은 아니다.

## 확인한 원본 계약

설치 도움말 `about_controlling_region_calculation.html` SHA-256 `e9c3025f61e9a9bde4092c91613789eb3cf457814d92ce43e86f176d6773a005`와 기존 REA 증거의 원본 어셈블리/CIL 해시를 재검증했다. GUI·스크린샷·원본 앱·COM·CLR 실행은 사용하지 않았다.

| 원본 경로 | REA 근거 | 확인한 동작 |
| --- | --- | --- |
| WorksheetViewModel.FireCalculateNow `0x06000a75` | `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b`; IL SHA `fd778e4ee882d03bd0e9db5e97de63111a58586c2a92f41a9f97b72072e327ed` | 보이는 활성 item을 먼저 강제 갱신하고, 이어서 마지막 보이는 계산 item까지의 일반 계산을 요청한다. 활성 영역만 계산하고 끝나는 명령이 아니다. |
| GetLastVisibleCalculatable `0x06000a01` | 같은 REA 증거; IL SHA `bf5f266b7ae1246d677a825ecf6b0288c97ad16ec4cc704d612e1fc648178fce` | 보이는 페이지의 위치순 item을 뒤에서 찾으며 실제 뷰 교차·계산 가능 여부를 검사한다. |
| FireRecalculateAll `0x06000a74` | 같은 REA 증거; IL SHA `7dae7f8b606ba25e97aea4d9b4469650137db0ae99999c7b04fbb583fde118f6` | 활성 편집을 commit한 뒤 전체 재계산 명령을 보낸다. |
| Calculation.HandleCalculateNow `0x06000016` | `ev_af7f38985f06b57c13d5262826e5f45f5f437b81377dcd93df83bd763278c4b7`; IL SHA `32cafac351652bbdb8c248ed8a245fc6c688197b5c30241f238863fe19de0fc8` | 유효한 마지막 계산 대상이 없으면 계산하지 않는다. |
| 자동 계산 재개 | 보존한 dependency-manager IL `0x060003f4` 및 도움말 | Auto 활성화는 미계산 작업을 재개한다. 수동 편집은 이전 결과를 지우는 대신 영향받은 식을 흐리게 한다. |

Worksheet 어셈블리 SHA-256은 `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd`, Calculation 어셈블리는 `88680b1a29536c632d0b705e71f5e727a0b02c64aa95af2157eedb2ada5e6885`다. ignored `artifacts/rea/feedback/calc-command-findings.md`, `calc-command-cil.json`, `calc-command-decode.py`에 해시 검증과 해독 위치를 보존한다. 원본 구현 본문을 Rust나 저장소 문서로 옮기지 않았다.

## 구현한 상태 전이

문서 열기는 이제 계산을 호출하지 않는다. 지원되는 MCDX의 저장된 literal 결과를 resultRef로 연결하고 namespace가 완결된 결과 MathML만 읽는다. 실수·복소수·행렬·단위와 제한된 기호 결과는 입력 수식을 실행하지 않고 조판한다. 숫자 행렬은 원본의 column-major 저장을 표시의 row-major로 옮긴다. 명시 결과 단위가 source RHS에 있으면 이미 단위가 선택된 저장 숫자를 다시 변환하거나 단위 glyph를 중복으로 붙이지 않는다. 비어 있거나 미지원인 저장 결과는 0으로 바꾸지 않는다.

자체 문서는 계산 표현을 `SavedCalculationCache`에 저장한다. 이 캐시는 평가 환경이나 변수 값이 아니라 표시 구조·오류·경고·동기화 상태다. SHA-256 source signature가 수식·배치·서식·원본 archive와 일치할 때만 복원한다. 제목·머리글·본문 주석 변경은 이 수학 캐시의 의미를 바꾸지 않는다. 형식이 틀린 표시·오래된 signature·없는 캐시는 Pending으로 남기고 자동 계산하지 않는다. 도표 점/스타일도 표현 캐시로 보존하므로 재열기 때 재계산하지 않는다.

계산 세션의 `update`는 입력을 파싱하고 무효화만 한다. `ThroughRegion`은 경계까지 Pending인 식을, `ActiveRegion`은 지정 식을 강제로, `Worksheet`는 전체를 계산한다. 깨끗한 선행 정의는 계산된 binding delta를 재사용한다. 각 결과마다 전체 Scope를 복사하지 않는다. 늦게 배치된 global은 앞선 식의 선행 조건이 될 수 있지만, 경계 밖 표시 결과를 미리 새 결과로 게시하지 않는다. 나중에 그 결과가 게시될 때는 그 요청에서 다시 실행되지 않았어도 이전 저장 표현을 교체한다.

도표는 `context_before`로 이미 계산한 전역/선행 binding을 복원하고 모든 trace·표본에 같은 읽기 전용 문맥을 사용한다. 표본마다 선행 식을 다시 실행하지 않는다. 아직 Pending인 선행 정의가 있으면 문맥 조회에서 계산을 숨겨 실행하지 않고 거부한다. 첫 수식보다 위에 있는 도표의 전역 선행 조건은 내부 `Prerequisites` 요청으로 준비하되 화면 밖 전역 결과를 게시하지 않는다. 이는 새 사용자 명령이 아니다.

앱의 F5와 리본 ‘지금 계산’은 보이는 활성 수식을 먼저 반영하고, 화면과 실제 교차하는 마지막 계산 영역까지 필요한 구간을 처리한다. 화면 밖 활성 영역은 강제 대상이 아니다. Ctrl+F5와 ‘워크시트 계산’은 활성 편집을 반영한 전체 재계산이며 수동/자동 설정을 바꾸지 않는다. 이전 자체 F9 shortcut은 원본 F5/Ctrl+F5로 교체했다.

수동 상태에서는 마지막 결과를 유지하고 Pending인 영역만 흐리게 그린다. 동기화된 다른 영역의 결과·오류·경고를 전부 숨기지 않는다. 자동 계산을 켜면 미계산 구간을 재개한다. 잘못된 native math/solveblock도 실패용 엔진 행으로 남겨 마지막 보이는 대상이라는 이유로 앞선 정상 계산을 중단시키지 않는다.

## 검증과 남은 경계

검증 수와 두 OS 결과는 [CONFORMANCE](../CONFORMANCE.md)에 기록한다. 기존 원본 38개 실행 표본은 유지하되, 이번 캐시/계산 상태 검사는 원본을 새로 실행한 비교가 아니다. 독립 fixture와 headless 이벤트·문서·수치/오류 상태를 검사하며 화면 캡처를 사용하지 않는다.

변경 지점 뒤의 무효화는 보수적인 순서 기반이며 원본의 세밀한 의존성 그래프 전체는 아니다. 저장 표현으로부터 원본 엔진 환경이나 난수/파일 실행 상태를 복원하지 않는다. 재열기 후 첫 강제 계산/변경에서는 선행 문맥을 새로 구성해야 한다. 원본의 모든 저장 문자열·range·특수 기호·컴포넌트/도표 cache 형식과 외부 IO/컨트롤 수명 주기는 미완료다. 평면 영역 비활성화는 이후 [0037](0037-disabled-regions.md), 협력 취소·백그라운드 계산은 [0040](0040-calculation-cancellation-engine.md)/[0041](0041-background-calculation-ui.md)에 연결했다. Area 상위 전파·원본 위치별 queue와 영역별 즉시 결과 게시·외부 IO 중단은 남아 있다. 가시 경계 뒤 global 선행 조건의 전체 원본 정책도 아직 미확정이다.
