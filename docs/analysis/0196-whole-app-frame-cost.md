# 전체 앱 프레임과 텍스트 서식 요청 비용

r208 guard/일반 회귀의 양 OS 고정 검증은 [0197](0197-rich-paragraph-source.md)에 있다. 아래 CPU 측정은 별도 동결된 개발 cohort r208-a이며 최종 제품의 재측정이 아니다. GUI·스크린샷·설치 Mathcad 실행은 사용하지 않는다.

## 기존 측정의 경계

[0191 LCC CPU](0191-lcc-workspace-cpu.md)는 실제 workspace·idle·scroll을 측정했지만 전체 App의 리본·상태 표시·단축키·raw 입력 hook을 포함하지 않았다. 그때의 빠른 CPU 결과로 사용자가 보고한 지연이나 GPU 문제를 부정할 수 없다.

[전체 소스 감사](../../artifacts/rea/whole-app-ui-audit-20261020/README.md)는 `App::ui`와 chrome을 따로 추적했다. 서식 있는 Text의 단축키 처리 함수가 Ctrl+B/I 이벤트를 찾기 전에 source context를 요청했다. chrome과 page의 필요한 요청 외에도, 포커스된 Text의 빈 입력에서 추가 요청이 생겼다. 준비된 editor는 원문 plain 문자열을 대조하며, 첫 admission에는 package/source 검증도 가능하다. 기존 캐시가 있는 idle에서 ZIP/XML을 매번 다시 해석한다는 주장은 아니다.

## 수정과 검사

`rich_text_shortcuts`는 현재 Text focus를 확인한 뒤 정확한 pressed COMMAND+B/I 이벤트가 있는지 read-only로 먼저 본다. 없으면 source context를 요청하지 않는다. 실제 명령에서는 기존 context 재검증·키 소비·B/I 순서를 유지한다. stale selection/source나 미지원 context의 키 소유권을 바꾸지 않는다.

[수정 전 재현](../../artifacts/rea/whole-app-guard-r208-before-20261020.log)은1 PASS/1 FAIL이다. 포커스된 native Text owner의 archive를 의도적으로 유효하지 않은 bytes로 만들어, 불필요한 source 준비가 cached error를 생성하는지를 관측했다. 빈 입력에서도 package를 준비해 실패했다. 이 fixture는 Mathcad 원본 동작이나 실제 LCC 지연 측정이 아니라 shortcut 경로의 불필요한 호출을 드러내는 회귀다.

[수정 후 검사](../../artifacts/rea/whole-app-guard-r208-after-20261020.log)는2 PASS다. 빈 입력·pointer movement·IME preedit·키 해제·unmodified/추가 modifier 입력은 package/editor cache를 만들지 않고 events/source/history를 유지한다. 정확한 B는 live context를 계속 요청하며 context가 미지원이면 이벤트를 소비하지 않는다. [기존 실제 rich Text 통합](../../artifacts/rea/whole-app-guard-r208-integration-20261020.log)의10 PASS/1 ignored도 유지했다. 첫 테스트의 deprecated IME Enabled 사용 경고는 실제 Preedit 사례로 바꿨다.

## 후속 검증

사용하지 않는 `eframe::Frame` 인자와 UI 본문을 분리해 실제 생산 App 본문을 headless로 호출한다. [독립 추출 감사](../../artifacts/rea/whole-app-r208-extraction-audit-20261020.json)는 r207 본문과 분리된 본문이 동일함을 확인한다. raw 입력 hook은 입력 주기당 한 번, egui multipass 본문·tessellation·출력 해제는 실제 순서대로 실행한다. 문서 원문·저장 결과·Pending·Undo·계산 제출/실행 상태를 검증하고 출력 해제 후 assertion한다. startup open/worker 초기화와 warm 프레임을 구분한다.

동결된 개발 측정 r208-a는 [독립 보고서](../../artifacts/rea/whole-app-profile-20261020-r208-a/independent-audit-v2/REPORT.md)에 있다. off/on 각각16사례·688프레임, 합계32사례·1376프레임이다. 1015/1016 원본 복사본, OS PPP1/2, zoom1/2, fresh/reopen을 검사했다. off 측정의 fresh 첫 프레임22.725–33.593ms, 같은 Context 재열기1.170–4.152ms, idle 사례별 중앙값0.388–0.807ms, scroll 후0.424–0.762ms다. 각 프로세스 idle320프레임의 texture upload는0이며 on idle의 glyph/formula/native-layout 재생성도0이다.

이 측정은 Cargo test profile의 CPU 자료이며 shipping release·GPU·OS 표시 지연을 측정하지 않는다. 두 프로세스 한 번씩의 자료로 최적화 전후 차이를 주장하지 않는다. 동결 archive와 harness/hash·입력·불변식은 검증했지만 측정 후 문단 구현을 더한 현재 제품과 archive는 다르다. 개발 측정으로 양 OS r208 채택을 대체하지 않는다.

chrome의 오류·경고 중복 순회는 다른 조사 대상으로 남아 있으며 검증되지 않은 cache를 추가하지 않았다. [headless 어댑터 조회](0198-headless-graphics-adapter.md)도 별도로 기록한다. 단축키 요청 감소를 실제 체감 지연 해소나 전체40% 완성으로 환산하지 않는다.
