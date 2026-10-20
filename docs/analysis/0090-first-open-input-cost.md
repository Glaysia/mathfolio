# 실제 저장 문서를 연 직후의 입력 준비 비용

2026-10-19. [0088](0088-calculation-signature-cost.md)의 4,000영역 stress fixture는 4,000개의 editor를 미리 보유했다. 실제 문서 open은 editor를 비우고 viewport의 영역만 만든다. 이번 검사는 **save→새 앱 상태 open→빈 frame→편집 focus→첫 이벤트→Enter**를 실제 경로로 거쳐 두 경우를 구분한다. GUI·스크린샷·원본 런타임·일반 앱 main을 실행하지 않았다.

## 검사 경로와 판정

test-only `open_input_latency.rs`는 기존 `input_latency.rs` 하위 모듈이다. 기존 r89 제품 manifest 123개를 대조해 일치함을 확인한 뒤 probe만 추가했다. 변경 전 harness는 ignored `artifacts/rea/open-input-before-r89-20261019.exe`로 복사했으며 SHA-256은 `73f29ce4d968d303290079db4fcee9f1af968ead886fd217686de926fb5a5109`다. 연결한 test-only mod를 제외한 기존 122개 manifest 파일은 그대로였다. 새 테스트 파일 identity와 baseline archive `ed5f005baafecc34b2595c9acded5edb87923ca7736384dd540014e663fb5ca0`는 같은 basename JSON에 기록했다.

synthetic fixture는 4,000 scalar 정의와 16×16 행렬 정의 100개다. 두 문서 모두 자체 저장 캐시가 있는 파일과 없는 파일을 별도로 생성한다. 캐시는 자체 계산기로 계산한 뒤 기존 저장 경로로 작성하며 파일 생성·초기 계산은 측정에 포함하지 않는다. 이 fixture의 각 정의는 결과 표시 query와 별개다. 저장 결과 표시가 있는 query는 아래 별도 일반 회귀에서 검사한다. 원본 1015/1016은 복사된 원본 MCDX를 읽기만 한다.

각 문서는 fresh app 상태에서 여섯 번 독립적으로 연다. ArrowLeft press, ArrowLeft release, Text, Paste, IME preedit, IME commit을 각각 첫 이벤트로 보낸다. open 시 editor 0개, 첫 viewport 이후에는 synthetic 23개·원본 29개만 생성됨을 확인했다. focus는 실제 page UI의 `focus_new` 경로를 쓰고 Number atom의 caret은 fixture helper로 정한다. 물리 마우스 클릭의 비용을 측정한 것은 아니다.

open·빈 frame·focus frame·첫 이벤트·Enter frame·worker 잔여 시간을 분리한다. 각 frame의 준비 호출 수, editor/draft 개수, 계산 generation·실행 여부·global calculation error도 기록한다. 준비 호출 수는 기존 test-only phase collector를 사용한다. 저장 표시 수와 Pending은 open 뒤 비편집 frame에서 유지되어야 한다. Text/Paste/IME commit은 draft만 만들고 generation을 바꾸지 않아야 한다. IME preedit와 Arrow는 draft를 만들지 않는다. Enter 이후 변경 case만 한 generation을 제출한다.

이 검사는 전체 앱 데스크톱 frame rate나 GPU 품질을 측정하지 않는다. CPU test profile은 자체 코드 opt-level 1·dependency opt-level 2다. 폰트 설정·fixture 파일 생성은 측정 밖이며, 첫 빈 UI frame의 font/layout 비용은 별도 `empty.cpu_us`에 나타난다. `calculation_error`는 global 오류이고, 수식별 fault의 표시 시점은 별도 일반 회귀로 확인한다.

## 변경 전 관찰

probe 36건은 r89 제품 동작에서 통과했다. 단일 첫 표본에서는 4,000 scalar 캐시 없는 문서의 첫 Text frame이 59.276 ms, Arrow release가 59.107 ms였다. 캐시 있는 문서에서는 각각 59.046 ms·58.513 ms였다. 모든 Arrow release와 IME preedit도 첫 이벤트에서 준비를 한 번 호출했다. 실제 원본 1015의 Arrow release는 5.438 ms·준비 1회였다.

이 값은 기존 stress fixture의 cold retain 69.770 ms와 다른 경로다. 실제 open의 소수 editor로도 non-edit 입력에서 큰 문서 전체 준비가 발생한다는 근거다. 제품 변경 후 같은 probe와 실행 파일을 순서 교대해 비교하며, 첫 Text의 비용이 Enter로 옮겨지는 경우 두 시간을 함께 보고한다.

## 최종 제품 변경 뒤의 5회 교대 비교

[0091](0091-lazy-committed-baseline.md)의 lazy baseline과 non-edit 입력 분리, 첫 focus의 navigation 교정을 포함한 최종 source로 다시 측정했다. 초기 pre-navigation 후보는 별도 로그로 보존하고 아래 수치에는 쓰지 않았다. 최종 after harness는 `open-input-after-r93-final-20261019.exe`, SHA-256 `5710c0c2cae8b968903503b8a0969d990f4930128a053e79f31590002e66f80a`다. 두 실행 파일을 같은 PC에서 각각 5회 실행하며 선행 순서를 번갈아 바꿨다.

| 실제 열린 문서 | 첫 Text frame 전→후 | Enter frame 전→후 | Text+Enter 합의 median 전→후 |
|---|---:|---:|---:|
| 4,000 scalar, 자체 캐시 있음 | 66.081→2.954 ms | 15.196→69.195 ms | 81.277→71.854 ms |
| 4,000 scalar, 자체 캐시 없음 | 70.319→3.362 ms | 14.932→74.848 ms | 85.190→78.210 ms |
| 16×16 행렬 100개, 캐시 있음 | 53.458→6.254 ms | 5.551→48.588 ms | 58.857→54.400 ms |
| 16×16 행렬 100개, 캐시 없음 | 55.984→11.413 ms | 10.513→54.207 ms | 66.474→65.663 ms |
| 원본 1015 | 6.017→0.592 ms | 1.447→4.737 ms | 7.464→5.349 ms |
| 원본 1016 | 5.488→0.637 ms | 1.318→4.645 ms | 6.764→5.227 ms |

합 열은 각 실행에서 두 frame 시간을 먼저 더한 후 구한 median이다. 앞의 두 median을 단순히 더한 값과 다를 수 있다. 계산 입력을 실제 확정하는 Enter에 준비 비용이 남으므로 **계산이나 확정이 수 ms 안에 끝난다는 의미가 아니다.** 특히 큰 행렬 문서는 첫 Text도 아직 11 ms가 넘는 표본군이다.

open·빈 frame·focus·Text·Enter의 기록된 단계 합 median은 캐시 있는 4,000영역 **125.606→116.474 ms**, 캐시 없는 경우 **115.427→109.058 ms**였다. 행렬은 각각 96.079→92.411 ms·111.343→109.876 ms, 원본 1015/1016은 55.465→54.771 ms·54.106→53.085 ms였다. 단계 사이의 caret fixture 설정과 폰트 configure 등은 포함하지 않으므로 전체 앱 시작 wall time으로 확대하지 않는다. worker의 남은 시간은 JSON에 별도 기록했다.

non-edit Arrow release는 캐시 있는 4,000영역 **64.528→0.942 ms**, 캐시 없는 경우 **67.088→1.142 ms**였다. 첫 이벤트 준비 호출은 1→0이다. 최종 after의 180개 관측 case에서 empty·focus·첫 이벤트의 준비 호출은 모두 0이었다. Arrow press/release·IME preedit는 Enter까지 포함해 총 0회, Text/Paste/IME commit은 Enter에서만 1회 준비했다. 이 계약의 위반은 0개였고 global calculation error도 0개였다. draft 중 계산 generation/실행 0, 저장 표시 유지와 Enter 단일 generation이라는 probe의 기능 assertion도 전부 통과했다.

이 비교는 같은 PC의 5회 median이며 다른 백그라운드 작업·파일 cache·CPU clock을 완전히 통제한 성능 보장이 아니다. 첫 key에서 준비를 제거한 것과 비용 일부가 Enter로 이동한 것을 함께 보고한다.

## 기존 raw log의 Enter 준비 단계 분해

새 측정 없이 최종 `open-input-alternating-20261019.log`의 `PREP_PHASE`를 바로 뒤의 `OPEN_INPUT` case와 연결했다. 아래는 after의 첫 Text→Enter case 각각 5회의 CPU median이다. phase 계측은 `prepare_input` 내부이며 editor 동기화·명령 routing·frame의 나머지는 포함하지 않는다.

| 문서 | signature | validation | MathML 직렬화 | worksheet 생성/라벨 | 표시 준비 | prepare 전체 | Enter frame |
|---|---:|---:|---:|---:|---:|---:|---:|
| 4,000 scalar, 캐시 있음 | 8.566 ms | 0.214 ms | 10.571 ms | 31.014 ms | 16.077 ms | 66.700 ms | 69.195 ms |
| 4,000 scalar, 캐시 없음 | 8.166 ms | 0.205 ms | 11.541 ms | 36.751 ms | 15.615 ms | 71.918 ms | 74.848 ms |
| 원본 1015 | 0.396 ms | 0.067 ms | 0.933 ms | 2.010 ms | 0.809 ms | 4.230 ms | 4.737 ms |

각 열의 median은 서로 다른 표본에서 나올 수 있어 합이 전체 median과 같지 않다. Enter에서 prepare를 뺀 나머지 시간의 median은 4,000영역 캐시 있음/없음 2.800/2.930 ms였다. 이미 소수 editor만 존재하는 이 경로에서는 검증 약0.2 ms보다 **전체 문맥 라벨/출력과 cold 표시 DOM**이 크다. 계산 worker의 별도 잔여 시간은 캐시 있는 4,000영역에서 35.169 ms였다. 이를 Enter 준비 비용이나 새 수치 계산의 완료 시간과 섞지 않는다.

`prepare_input`은 signature→editor fault→MathML→worksheet 순서 뒤에 geometry template과 표시 tree까지 만든 후에야 `CalculationInput`을 반환한다. `calculate_command_with_active`는 이 input의 region IDs로 scope를 정하고, 그 후 `dispatch_background`가 generation을 올려 기존 worker를 대체한다. 따라서 현재 약70 ms의 대부분은 이미 worker에서 하는 수치 계산이 아니라 **worker에 제출하기 전 UI thread의 source/presentation 준비**다.

분석상 나눌 수 있는 경계는 **source 준비와 표시 tree 준비**다. 현재 표시용 `labelled_trees`는 canvas와 print scene에서 사용하며 engine 실행 입력은 완성된 XML이다. 동일 XML의 cold DOM decode를 generation을 가진 worker의 표시 sidecar로 옮기면 기존 계산 request/scope 생성 규칙을 유지하면서 UI의 표시 준비 일부를 분리할 수 있다. 다만 표시 단계에는 geometry capture와 cache 정리도 섞여 있으므로 16 ms 전부를 절감한다고 주장할 수 없다. print scene은 전체 label을 사용하므로 visible 영역만 만들고 나머지를 생략해서는 안 된다. 현재 source에 맞는 전체 표시 snapshot과 기존 full-DOM 차등 계약도 유지해야 한다. 따라서 이것은 당장 적용할 작은 패치로 확정한 제안은 아니다.

더 큰 31–37 ms worksheet 단계와 직렬화/signature까지 옮기려면 현재 `CalculationJob`의 source-only 입력을 owned calculation snapshot으로 확장해야 한다. UI에서는 Enter 확정, live editor tree/validation 동결, visible/active 명령 의도와 epoch/generation 확보를 먼저 수행한다. worker가 frozen source에서 XML·region IDs를 만든 뒤 scope를 결정해야 하며, 기존처럼 source 생성 전에 현재 draft를 다시 참조하거나 UI의 live Arena/cache를 worker에서 빌려서는 안 된다. 이 변경은 단순 함수 이동보다 크다.

어느 worker 분리든 최신 대기 job만 유지하고 submit 시 이전 실행을 취소하는 기존 정책, Open/New의 epoch, 늦은 reply 거절, Manual/Stop에서 명시 명령을 보존하는 정책을 유지해야 한다. 표시 sidecar에도 같은 source/generation 검사를 적용하고 오류를 open/draft 시점으로 앞당기지 않는다. 전체 XML 파싱의 취소 확인 지연과 큰 archive의 snapshot 복사 비용도 남는다. 위 내용은 다음 후보에 대한 소스 분석이며 새 구현이나 성능 달성 수치가 아니다.

계산 generation은 draft마다 늘지 않으므로 표시 sidecar의 채택에는 generation 외에 현재 source/editor revision 확인이 필요하다. 특히 미확정 F5의 frozen baseline 표시가 live draft의 표시 cache를 교체하면 안 된다. 초기 구현은 기존 최종 reply에 표시 payload를 원자적으로 포함하는 편이 작다. 별도 중간 reply를 보내려면 현재 `accept_background_reply`가 reply를 받자마자 `running=false`로 만드는 규칙부터 분리해야 한다.

현재 자료만으로 효과가 큰 최소 제품 패치를 확정하기는 어렵다. 다음 작은 조사 후보는 기존 test-only 계측에서 worksheet 31–37 ms를 순서/global prepass, fragment parse/label, cache payload 생성, 출력 조립으로 더 나누는 것이다. cold `serialize_math`가 새 snapshot을 넣은 직후 같은 tree와 비교하는 중복은 소스상 제거 가능하지만, 기존 equality primitive 측정은 4k 약0.08 ms 수준이라 주요 지연의 해결책으로 과장할 수 없다. 이번 조사에서는 추가 계측·제품 변경을 하지 않았다.

## 저장·수동 계산·오류 시점의 일반 회귀

일반 검사 네 개는 작은 유효한 `.mathcraft` 파일을 실제 저장한 뒤 새 app에서 연다.

1. Manual에서 첫 Text는 저장 표시를 유지하고 계산을 제출하지 않는다. Enter로 확정한 뒤에도 실행은 없으며, 명시적 Worksheet 계산에서 바뀐 값이 query에 반영된다.
2. IME preedit는 document·저장 표시·generation·오류 상태를 바꾸지 않는다.
3. 첫 불완전 입력은 draft로 남고 Enter 전에 fault를 공개하지 않는다. Enter 이후 dependent query에 오류가 나타난다.
4. fresh saved open의 첫 Text draft를 남긴 채 active 영역을 offscreen으로 두고 F5를 호출하면 이전 committed 값이 계산되고 draft가 유지된다. Enter 후 Worksheet 명령에서는 새 값이 계산된다. 이미 계산한 fixture의 warm cache로 이 경계를 대신하지 않는다.

처음 세 검사는 제품 변경 전에도 통과했으며, 최종 after harness로 네 검사 모두 다시 실행해 **4개 통과·실패 0개**를 확인했다. Auto OFF, disabled·표 commit·native archive·미확정 F5의 추가 계약은 관련 기존 검사와 통합 검증에서 함께 확인한다.

## 재현과 산출물

일반 회귀는 `cargo test -p mathcraft-app actual_saved_open_ -- --nocapture`, 전체 lifecycle probe는 `cargo test -p mathcraft-app actual_open_first_input_profile -- --ignored --nocapture`다. scratch는 ignored `artifacts/rea/open-input-latency-20261019/<pid>-<nonce>`에만 생성한다. `MATHCRAFT_OPEN_INPUT_OUTPUT`으로 별도 절대 경로를 지정할 수 있다. 원본 문서는 수정하지 않는다.

`artifacts/rea/measure-open-input-20261019.py --after <변경 후 harness> --runs 5`는 변경 전 보존 실행 파일과 변경 후 실행 파일의 선행 순서를 번갈아 실행한다. 두 실행 파일 SHA, 모든 36case×각5회 표본, median과 원시 로그를 `open-input-alternating-20261019.{json,log}`에 보존한다. 단일 초기 로그는 `open-input-before-first-20261019.log`, 일반 회귀 로그는 `open-input-regression-20261019.log`다.

최종 원시 로그/JSON은 위 일반 basename이며, 사용하지 않은 pre-navigation 자료는 `open-input-pre-nav-{alternating,summary}-20261019.*`에 별도 보존했다. `open-input-after-final-first-20261019.log`에는 최종 source의 별도 36case 1회 실행도 기록했다. 전체 Windows/peets 검증은 통합 기록에서 별도로 확인한다. 측정된 key·Enter 준비 비용을 전체 Mathcad 기능 완성률로 환산하지 않는다.
