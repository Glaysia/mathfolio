# 구조 삭제·연산자 선택과 서명 직렬화

2026-10-19. [0122 원본 계약](0122-structural-deletion-contract.md)과 [0123 비용 분석](0123-signature-emission-cost.md)을 r125 독립 Rust 구현에 연결한다. GUI·스크린샷·원본 런타임·CLR/COM을 사용하지 않았다. r121의 소스와 측정은 보존한다.

## 원본의 삭제 단위

기존에는 피연산자의 글자 경계에서 Backspace/Delete가 거절됐고 일부 구조는 마지막 leaf를 임의로 남겼다. 확인한 연산자 중심·피연산자 경계에서는 연산자 핸들을 제거하고 해당 컨테이너를 재파싱한다. `12+34`는 `1234`, `5^2`는 `52`, `x+y`는 이름 `xy`, `1.2+3`은 `1.23`이 된다. `1.2+3.4`는 유효한 하나의 수가 아니므로 두 값을 보존하는 Scale이 된다. 이때 Backspace는 오른쪽 값의 시작, Delete는 왼쪽 값의 끝에 둔다. 도움말의 정수 예시를 모든 비정수에 대한 곱셈 규칙으로 일반화하지 않았다.

Group의 확인된 양쪽 경계/괄호 상태에서는 괄호 쌍을 제거한다. Fraction·Power의 중심과 일부 안쪽 빈 자리, 확인된 단항·후위 상태도 피연산자를 보존한다. Conjugate·Vectorize는 일반 연산자 핸들 삭제와 구분해 자식 식을 보존하고 더 강한 부모 아래에서 필요한 Group을 복구한다. 후보 검증 실패는 소스·커서·redo를 보존하며 성공한 삭제는 한 번의 Undo로 복원한다.

일반 이항 기호의 실제 x 범위를 클릭하면 연산자 중심 커서를 선택한다. 선택된 기호는 원본 기본 CaretColor `#0080FF`의 글자 geometry로 표시하고 피연산자 bounds에는 DarkGray·불투명도0.25의 사각형 하이라이트를 사용한다. 공유 galley를 유지하며 CPU mesh 검사는 opacity·disabled tint·clip을 대조한다. 이 구현은 원본의 현재 사용자 색상 설정이나 모든 선택 상태의 레이어/기하 일치를 관찰한 결과는 아니다.

실제 CPU 앱 Key/Pointer 이벤트와 Enter·Undo/Redo를 검사한다. `x=2`, `y=3`, `xy=7`에서 `x+y`의 연산자를 지우는 동안 기존 결과5를 유지하고 Enter에서7로 바뀐다. 자동 곱셈6으로 잘못 바꾸지 않는다. 새 native fixture는 정의3개·query11개, 총14개 영역이며 자체 재열기 실행0회·결과 표시11개를 검사한다.

Formatted/contextual 이름 결합, 일반 outer Typeover의 두 단계 선택, double-placeholder, 특수 연산자 전체 경계, 일부 동일 우선순위 단항 부모 복구와 전체 부분 Scope 삭제는 미완료다. 미확정 조합을 임의로 한 피연산자를 버리는 정책으로 대체하지 않는다. EA-010은 부분 구현을 유지한다.

## 정확한 계산 서명

CanvasRegion/PortableTree의 고정 JSON framing만 직접 출력하며 값·문자열·float·enum·서식은 기존 serde serializer를 사용한다. 앱의 정확한 snapshot memo와 예산·fallback은 유지한다. 문서의 uncached 서명은64KiB buffer를 통해 SHA로 보내 큰 JSON 임시 Vec를 줄인다. 기존 저장 cache의 literal golden과 독립 구 serializer/SHA oracle로 바이트·해시를 대조한다. 작은 문서에는64KiB buffer가 더 클 수 있으며 warm 비교와 snapshot 복제 비용은 남아 있다.

## 동일 소스 최종 검증

Windows와 peets 각각 **workspace1009 + backend2 = 1011개 통과·실패0개·51개 ignored**다. 앱406개·editor195개·document75개를 포함한다. fmt·workspace all-target Clippy·backend library/test Clippy(`-D warnings`)·build 및 Windows release가 통과했다. 원본 reference saved/input 각각38/38·차이0이다. GUI·일반 앱 main은 실행하지 않았다.

native18건/PDF16건/OS, MCDX36개·XML144파트 설치 XSD 오류0개, PDF32개 이미지0개다. 두 독립 PDF reader로 검색 텍스트·단위·표 색상/기준점을 검사했다. 새 fixture는14영역·query 표시11개·자체 재열기 계산0회·Pending0개·MCDX 재수입 재계산 오류0개이며, 새 PDF는 OS별2쪽·drawing162개다. XML은 define3/eval11/scale1/mult0이며 지운 plus/pow/div는 남지 않는다. 기존 원본의 표42편집 셀·80계산 행·64결과 표시·27XamlPackage도 검사한다. 두 원본의107영역/visual·80saved row 전체를 구 import oracle와 양 OS에서 별도로 대조했다.

동결 archive `artifacts/rea/mathcraft-structural-deletion-20261019-r125.tar`는8,304,640bytes·SHA256 `0fa555837dead845b3291464d016434ff4d18933d661d228432cf9ad618a61ff`다. 모든 regular source187파일(제품 Rust144·vendor Rust5)을 archive/양 호스트와 대조했다. 초기 source manifest SHA는 `4f10dd763b8bef12f1fc707516e0b38d86f410c0692c900cab8a33d9912df4ce`이며 최종 검증 metadata를 추가한 JSON의 해시와 구분한다. peets root는 `/home/peets/Projects/mathcraft-structural-deletion-20261019-r125`다. 원격 산출물82개·표준 로그5개·raw36case profile과 별도 original-open oracle를 size/SHA로 대조했다.

최종 보고서는 `artifacts/rea/structural-deletion-integration-20261019/source-validation.json`이다. 수집 tar는2,959,360bytes·SHA256 `2a2cf94da817f901137e03e424df94c6a73478516514e539fc7523aa00feb245`다. Windows release `target/release/mathcraft.exe`와 보존 `mathcraft-structural-deletion-r125-20261019.exe`는22,262,784bytes·SHA256 `194f119538f75a64c2ca7f295365a159a8dd053442b2134c3ed549e49d0c8d8c`다.

## r121→r125 실제 열기·입력 비교

동결 CPU test executable을5회 교대로 실행한360case와, 같은 두 executable의 별도 확인360case를 모두 보존한다. 시간은ms, 각 열은5개 표본의 중앙값이며 합산 열은 각 표본의 합을 먼저 계산한 값이다. ‘첫 페이지’는 실제 open 직후 empty page frame, ‘입력+Enter’는 Text 이벤트와 확정 frame의 합이다. 생성·폰트 설정·caret helper·프레임 사이 간격은 포함하지 않으며 GUI/GPU·전체 시작 시간으로 해석하지 않는다.

| Fixture | open | 첫 페이지 | open+첫 페이지 | 입력+Enter |
|---|---:|---:|---:|---:|
| 4k cache |28.008→23.092|10.590→10.617|38.598→33.817|36.493→36.097|
| 4k no cache |14.609→12.981|11.147→10.890|25.756→23.173|37.519→36.355|
| 100×16×16 cache |17.789→12.259|16.104→16.934|34.030→29.157|26.256→25.385|
| 100×16×16 no cache |13.234→10.072|16.942→16.387|30.489→26.252|26.992→25.140|
| 원본1015 |16.463→17.122|24.489→24.398|41.053→42.181|2.823→3.031|
| 원본1016 |16.405→16.305|23.375→23.889|39.851→40.792|2.916→2.833|

첫 비교에서36event pair 중13개의 입력+Enter 중앙값이 증가했다. 가장 큰 증가는4k cache IME commit +.815ms다. 1015의 입력+Enter +.208ms와 원본 두 문서의 open+첫 페이지 증가도 보존한다. 행렬 cache 첫 페이지 +.830ms를 개선된 open 시간과 섞어 숨기지 않는다.

추가 확인 측정은 첫 비교를 대체하지 않는다:

| Fixture | 확인 open+첫 페이지 | 확인 입력+Enter |
|---|---:|---:|
| 4k cache |40.434→33.305|38.088→35.914|
| 4k no cache |26.324→23.303|37.562→35.719|
| 100×16×16 cache |35.093→29.388|26.851→24.549|
| 100×16×16 no cache |30.860→26.630|28.310→24.980|
| 원본1015 |41.436→40.687|3.535→2.810|
| 원본1016 |42.589→41.053|2.889→2.674|

확인 비교에서도5/36event pair가 증가했다. 가장 큰 증가는4k no-cache IME commit +1.525ms다. 1015의 앞선 증가가 같은 방향으로 반복되지는 않았지만 원인을 해결했다고 단정하지 않는다. 전체720case에서 오류0개, open/draft 실행0회, 첫 event 준비0회, dirty Enter 준비1회와 generation 전이를 유지했다. worker remaining은 Enter와 동시 진행한 시간을 제외하므로 Enter frame에 더해 전체 완료 시간이라고 보고하지 않는다.

첫 비교의 Text Enter 준비 중 signature는4k cache7.572→5.896ms, matrix cache6.302→4.847ms다. 4k의 worksheet+display16.433→17.903ms처럼 다른 단계의 증가는 남아 있다. snapshot clone·warm source 비교, worksheet/표시 준비·직렬화와 cold glyph 조판 비용도 계속 조사한다. JSON 시간만 줄었다고 전체 버벅임을 해결했다고 주장하지 않는다.

측정 파일은 `structural-deletion-open-input-{alternating,confirmation}-20261019.{json,log}`, 세부 phase는 `structural-deletion-open-input-phase-summary-20261019.json`, 두 비교의 모든 adverse pair는 `structural-deletion-open-input-confirmation-summary-20261019.json`이다. before harness SHA는 `956afee2c8c456d78dccb0e3497724480ddb81e30e31f12198c8b854a81ac495`, after는 `bbb9766e65f0c90c451479fbe2fb182bedfdf8bdac7fc24e682709a90f4d03fc`다.

목록553행은 부분138/미구현415를 유지한다. EA-010의 구현 깊이를 갱신했고 설치 도움말215개를 재해시했으며 로컬 링크430개 오류0개다. 기능 행/테스트 수를 전체20% 달성으로 환산하지 않는다.
