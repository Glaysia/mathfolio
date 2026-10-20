# 편집 교체·formatted 삭제·Typeover 통합

2026-10-19. 사용자가 평가한 현재15%에서40%를 목표로 계속 개발하는 중이다. 이 묶음은 원본 작성 흐름의 한 단계이며, 통과 수나 기능 행 수를 완성도로 환산하지 않는다. GUI·스크린샷·원본 프로그램·CLR/COM 실행 없이 설치본 REA/CIL, Rust 코어와 CPU 앱 이벤트, 원본 문서·MCDX·벡터 PDF로 검증한다.

## 원본 계약과 실제 변경

- [0129](0129-operator-replacement-contract.md): 연산자 중심에서 입력하면 가까운 두 피연산자를 보존하며 기호를 교체한다. 일반 이항·Fraction·Power, 인접 항 재배치와 기본 grouping을 연결했다. `1+2+3` 두 번째 `+`→`*`는 `1+2*3`이다. Power descriptor의 Infix와 분석 목록의 EffectivePostfix를 구분한다.
- [0127](0127-formatted-deletion-contract.md): 이름의 literal suffix와 라벨·contextual metadata를 보존하며 삭제/결합한다. cached Unit 라벨을 명시 Unit으로 승격하지 않는다. `V_d` 마지막 suffix 문자 삭제의 pending Subscript와 suffix 전체 선택 삭제의 Normal caret을 구분한다. 마지막 명시 라벨/Native 이름 삭제는 빈 식별자 대신 PH를 만든다.
- [0126](0126-typeover-deletion-contract.md): 실패한 첫 삭제는 빨간 임시 선택만 만들고 문서 변경·Undo checkpoint·계산을 만들지 않는다. 다음 삭제/입력은 한 명령이며 Undo는 임시 선택까지 복원한다. Fraction은 전체 opaque handle, Power는 base를 제외한 script handle이다. Power의 두 번째 삭제는 atomic base를 남긴다. 지원되는 Fraction 입력 교체는 Eval.left/Define·GlobalDefine.RHS의 부모와 형제를 보존한다.
- [0128](0128-worksheet-preparation-cost.md): 출력 조립·표시 XML 복사를 줄이고 byte/error/budget/cache oracle을 유지했다. identifier trace는 반복 개선이 확인된 큰 행렬에만 사용한다. scalar trace의 악화와 측정 한계를 보존한다.

CPU 앱 검사는 문자/기호 Text 이벤트, Backspace/Delete, 빨간 bounds와 caret, 실제 Enter, 초안 동안 이전 결과 유지, Undo/Redo를 포함한다. `=`는 이미 Evaluation인 식에서는 선택/본문을 유지하고 계산만 요청한다. bare Fraction Typeover에서는 Eval(PH)를 만든다. `:`의 bare Fraction→blank Define만 연결했으며 다른 wrapper 재파싱은 미완료다.

## 통합 검증 상태

최종 r132 검증용 소스는 작업 트리와 분리한 `mathcraft-r130-validation`에서 고정했다. 행렬/Area 후속 개발이 검증 바이너리를 바꾸지 않게 아카이브·초기 manifest·보존 실행 파일 SHA를 대조했다.

Windows와 peets 각각 workspace1056+backend2=**1058개 통과, 실패0,56ignored**다. 앱422·editor220·compat76·document75를 포함한다. fmt·workspace/all-target Clippy·backend Clippy `-D warnings`, Windows release와 Linux build, 원본 reference saved/input 각각38/38을 통과했다. 같은198파일(제품 Rust155/vendor5)을 검사 전후 대조했다. 각 OS native20/PDF18, 전체 MCDX40/XML160파트의 XSD 오류0, PDF36의 이미지0과 두 독립 텍스트 추출기를 확인했다. 원본 두 문서의107regions/107visuals/80saved와 수정 표·자체 cache의 absent/valid/stale 추가 oracle도 양 OS에서 통과했다. 제공 원본은 수정하지 않았다.

새 native fixture는 연산자 교체10regions/10queries와 formatted/Typeover17regions/13queries다. 실제 입력→계산→저장→실행 없이 결과 복원→MCDX export→독립 import/계산을 확인한다. XML은 Power의 Group/피연산자, 명시 VARIABLE와 contextual 폐기, literal suffix XAML·imaginary `j`까지 검사한다. 이전18개 fixture도 유지했다.

고정 소스: `mathcraft-editing-repair-20261019-r132.tar`,8,478,720bytes,SHA256 `4038ff96cae3c6a617e713a991da2f5ac6e6751839a5fec6fbe2ecc49f87af0e`. 초기 manifest SHA `51edfcb0a123d61ea8e203558fc0260711aedec51844ceb32f4b237065b7b7ca`. 원격 `/home/peets/Projects/mathcraft-editing-repair-20261019-r132`의92산출물·5로그·36case raw profile와 검사 후 소스를 재해시했다. evidence tar3,174,400bytes SHA `39fcae577cfbb989b9da0a7deee406e1110504793ddf3c975f7accfa941a935e`를 보존했다. Linux shell의 첫 CRLF 준비 오류는 별도 로그에 남기고 LF로 교정한 뒤 실행했다.

Windows 보존 release `artifacts/rea/mathcraft-editing-repair-r132-20261019.exe`는22,333,952bytes,SHA `e80df927270995b60d325479c6df56907f2a16b17a646c4a4b6b5270f909d786`다. 측정 harness SHA `9a862bdf3e03787030bdd0d6d2baf7183a3dbe7307dca09a9bd329c32020f445`, baseline r125는 `bbb9766e65f0c90c451479fbe2fb182bedfdf8bdac7fc24e682709a90f4d03fc`다. GUI 앱을 실행한 검증은 아니다.

이전 r130 시도는 기존 앱 검사의 suffix 전체 선택 삭제 기대값이 원본과 달라 전체 검사를 통과하지 못했다. CIL0435→1328/1329→130d로 원본의 Normal offset을 확인했고, 그 뒤 실제 빈 NativeIdentifier 버그를 발견해 수정했다. r131은 이 후속 preflight 실패를 보존한다. 기존 기대값을 단순히 삭제하지 않고 방향별 선택·마지막 문자·두 단계 Undo와 실제 CPU 앱 경로를 추가했다. 최종 revision은 r132로 분리한다.

## 전체 입력 비용

r125→r132를 각5회 교대로 실제 저장→새 앱 open→첫 입력→Enter로 검사하고 별도5회 확인을 보존했다. 두 비교 합720case의 오류·counter 계약 위반0, open/draft 실행0, 첫 event 준비0, dirty Enter 준비1과 generation 전이를 유지했다. CPU optimized 테스트 profile이며 GPU/실제 창/프로세스 기동 시간은 아니다. 첫 비교 초기에 다른 작업의 종료 확인이 늦어 그 영향은 배제할 수 없다. 확인 비교는 모든 작업의 quiet 확인 후 수행했다. 별도 열의 median을 합하지 않고 합산 구간은 표본마다 더한 뒤 median을 계산한다.

| Fixture | 첫 비교 open+첫 페이지,ms | 첫 비교 입력+Enter,ms | 확인 open+첫 페이지,ms | 확인 입력+Enter,ms |
|---|---:|---:|---:|---:|
|4k cache|33.452→35.036|34.370→35.398|32.333→33.862|35.496→33.572|
|4k no cache|22.060→21.692|34.156→31.610|22.286→22.545|33.893→34.503|
|100×16×16 cache|29.122→30.132|24.779→22.849|29.723→30.303|25.095→23.869|
|100×16×16 no cache|28.029→26.657|25.661→24.490|28.329→26.808|25.921→24.208|
|원본1015|40.480→40.245|2.793→3.112|40.670→39.372|2.726→2.672|
|원본1016|38.497→38.989|2.501→2.579|37.182→40.306|2.466→2.750|

첫 비교16/36event pair, 확인14/36이 증가했다. 가장 큰 증가는 각각4k cache Paste+2.610ms와4k no-cache Paste+1.234ms다. 행렬의 입력+Enter 감소가 반복됐지만 전체 지연 개선으로 일반화하지 않는다. 첫 비교의 worksheet+display는 matrix cache6.172→4.852ms/no-cache6.514→4.849ms,4k cache16.793→17.269ms로 증가했다. 단계의 median을 Enter와 더하거나 worker remaining을 더해 전체 완료 시간으로 보고하지 않는다.

근거는 `artifacts/rea/editing-repair-integration-20261019/source-validation.json`, `editing-repair-open-input-{alternating,confirmation}-20261019.{json,log}`, `editing-repair-open-input-{phase,confirmation}-summary-20261019.json`이다. 원본 도움말215파일과 목록553행(부분138/미구현415)을 재검증했으며 EA-010의 구현 깊이만 갱신했다.

## 남은 범위

EA-010과 관련 행은 부분 구현을 유지한다. 일반 native LinearSegment/APPLY·PairNary·특수 구조, Power Typeover의 값/트리 재파싱, 모든 rich-run·같은 우선순위 container 해제, ParenthesizeOperands=false, 임의 중첩 definition의 키 명령은 남아 있다. 큰 문서의 전체 Enter/첫 화면 CPU 비용과 cold glyph 비용도 별도로 남는다. 접이식 영역과 행렬 작성은 다음 원본 계약·실행 경로로 진행하며, 이번 묶음 검증에 섞지 않는다.
