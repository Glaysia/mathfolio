# 긴 문자열 결과 보존과 도표 첫 수식

2026-10-19. [0185 문자열 통합](0185-string-value-integration.md)의 r200은 일반 검사에 통과했지만 후속 검토에서 두 공백이 발견되어 완료 checkpoint로 채택하지 않았다. GUI·스크린샷·원본 runtime 없이 설치 정적 근거와 실제 Rust 컨트롤러를 검사한다. r202 양 OS 통합 검증을 완료했다. Windows/peets 각각1541검사(workspace+backend), 122MCDX와 별도10XML의 XSD 742parts·60PDF 구조 검증을 통과했다. r201 Windows 전체 검사에서 기존 plot context 테스트의 임시 파일 충돌이 나와 그 기록을 보존했다. 시간값에만 의존하던 임시 폴더에 프로세스 내 순번과 배타적 생성으로 소유권을 부여하고 새 snapshot에서 검사한다.

## 결과 데이터와 표시 예산

다섯 개의 14,000자 literal을 concat한 70,000자 결과는 편집 atom 예산과 계산 String 예산 안에 있다. r200의 native cold open은 64KiB 표시 예산 때문에 결과를 그리지 못하면 inert numeric source까지 버리고 Synchronized를 Pending으로 바꿨다. own cold open도 값은 남겼지만 상태를 Pending으로 바꿨다. native copy의 결과 본문 70,000자는 그대로였으며 실제 변경은 상태 속성뿐이었다. 이를 결과 payload 삭제로 기술하지 않는다. [분리 재현](../../artifacts/rea/string-cache-audit-20261019/literal-status-proof.json)에 원문 길이·SHA·상태를 별도로 보존했다.

[검증된 String carrier](../../crates/mathcraft-app/src/worksheet_ui/inert_string_result.rs)는 표시 가능성과 분리해 원문 데이터의 타입·구조·예산을 확인한다. [복원 경로](../../crates/mathcraft-app/src/worksheet_ui.rs)는 이 근거로 같은 입력에 저장된 상태와 inert 값을 유지한다. 계산하지 않으며 기존 Pending, 입력/context signature 변경, 비활성 상태를 Synchronized로 승격하지 않는다. 임의 JSON·알 수 없는 metadata·중복 필드·잘못된 XML을 유효한 계산 값으로 인정하지 않는다. 저장 presentation/native 표시 변환의 64KiB와 계산 String 1MiB는 독립 앱 예산으로 유지한다. live/JSON 값의 DisplayMath 생성은 다른 경로이며 화면 전체에 64KiB 제한이 적용된다는 뜻은 아니다. 이 수정은 큰 결과의 화면·인쇄 배치 지원을 새로 보장하지 않는다.

## 도표의 expression-only 입력

[설치 CIL ledger](../../artifacts/rea/string-plot-expression-only-ledger-20261019.json)는 STRING 삽입 경로와 AllowExpressionsOnly의 정의·평가문 차단을 구분한다. STRING descriptor의 IsEvaluatable 기본 false를 확인했으므로 첫 math에서 String literal을 차단하던 [PlotExpression](../../crates/mathcraft-document/src/plot_expression.rs) 문법을 확장했다. 빈 문자열도 완성된 String이며 placeholder가 아니다. wire 형식·원문·CR·따옴표를 보존하고 기존 immutable validation receipt를 유지한다.

[실제 키 회귀](../../crates/mathcraft-app/src/plots/string_source_tests.rs)는 quote·연산 문자 입력, Enter 전 draft, Undo/Redo와 native source를 검사한다. [계산 회귀](../../crates/mathcraft-app/src/plots/calculation/typed_tests.rs)는 숫자처럼 보이는 String도 축 숫자로 변환하지 않고 타입 오류로 남기는지 검사한다. STRING을 implicit domain이나 가짜 곡선으로 대체하지 않는다. scaling source는 이 변경 전에도 별도 문법에서 String을 허용했으며 이번에 정책을 넓히지 않았다. 실제 숫자/배율 계산은 계속 숫자 타입을 요구한다.

## 문자열 인덱스 설정의 실제 XML 위치

추가 XSD 대조에서 r200 reader와 테스트 fixture가 함께 잘못된 root 속성을 사용한 것을 확인했다. 원본 위치는 `calculation/calculationBehavior/@strings-use-origin`이다. root 속성은 원본 계약의 근거가 아니므로 거절 사례로 옮기고, 고유 direct child의 실제 boolean을 읽도록 교정한다. duplicate·중첩·다른 namespace·잘못된 boolean·root 위장을 거절하고, child/속성 부재의 기본 false와 UTF-16/비표준 part 경로를 유지한다. 과거 병렬 반복 검사는 당시 잘못된 fixture의 안정성 기록이며 원본 설정 호환성을 증명하지 못한다.

[설정 컨트롤러](../../crates/mathcraft-app/src/worksheet_ui/string_settings_tests.rs)가 올바른 구조의 native·own·worker 계산을 검사하고, 생성된 원본/copy/own 재수출 세 MCDX의 실제 settings part를 설치 XSD로 검증한다. 원본 전체 문자열·도표·편집 동작의 완료 판정과 구분하며 테스트 수를40% 목표 달성으로 환산하지 않는다.

## 고정 소스와 검증

고정 archive SHA-256 `6090b27fa0163abd511d973cb91f693e8821448cab5f4ebac6fc0540b67a5795`, 501파일/253제품 Rust다. [manifest](../../artifacts/rea/string-values-integration-20261019-r202/source-manifest.json)와 [검증 집계](../../artifacts/rea/string-values-integration-20261019-r202/validation.json)에 원본 로그·runner·출력 해시를 연결했다. Windows workspace1539+backend2, peets1541검사, 실패0/ignored83이다. fmt·workspace/backend Clippy 경고금지·build·reference38saved/38input·release를 확인했다.

양 OS 122MCDX와 별도10XML, 총 742XSD parts 오류0이다. 60PDF는 이미지 객체0·두 reader의 텍스트 추출을 검사했고 화면/출력 시각적 동등성을 판정하지 않았다. String 설정3개 native 출력/OS는 실제 calculationBehavior 속성을 XSD와 대조했다. 큰 결과는 일반·Area 각3개 native 출력/OS의 전체 result.xml byte equality와 own Pending/disabled/source 변경 경계를 검사했다.

[Windows release](../../artifacts/rea/mathcraft-string-values-r202-20261019.exe)는 24,762,368bytes, SHA-256 `2588894a913d228b41efe6e3112aaf0968d2ef485cd6aa5f8f45e3becc825767`다. 앱 창을 실행하지 않았다. r201 Windows 실패 archive·로그와 r200 검토 보류 기록은 그대로 보존했다. 이는 해당 소스의 검증 범위이며 제품 완성률을 뜻하지 않는다.
