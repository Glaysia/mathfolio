# 암시적 배율 연산과 확정 준비

2026-10-19. [0110 원본 계약](0110-implicit-adjacency-contract.md)의 설치 도움말·descriptor·정적 IL을 독립 Rust 입력·계산·저장에 연결한다. GUI·스크린샷·원본 프로세스·CLR/COM은 실행하지 않았다. 성능 변경의 설계와 채택 범위는 [0111](0111-commit-preparation-design.md)에 기록한다. 이전 r109 소스와 바이너리·원시 측정은 보존한다.

## 배율과 명시적 곱

원본의 `5y`는 `5*y`와 다른 SCALE descriptor다. `IsImplicit=true`, XML 이름 `scale`, Ctrl+Shift+U, 활성 수식에서만 보이는 U+2218 ∘를 확인했다. 이전 독립 구현은 Scale 노드를 수입·저장했지만, 자동 입력에서는 Multiply를 만들고 계산에서는 일반 곱과 합쳐 처리했다. 기존 노드를 사용해 이 차이를 교정했다.

숫자 뒤의 단위/이름과 완전한 식 뒤의 다음 피연산자에 기존 인접 입력 경로가 Scale을 만든다. 명시적 `*`는 Multiply를 유지한다. `xy`는 한 이름이며 과학 표기 `2e-3`와 허수 `2i`는 배율로 쪼개지 않는다. `2m^2`는 `scale(2,power(m,2))`다. 일반 부분 문자열을 원본 parser처럼 분할·repair한다는 주장은 하지 않는다.

Ctrl+Shift+U는 정확한 modifier의 Key press에서만 배율을 넣는다. Ctrl+U, Alt 추가, release는 해당 명령을 실행하지 않는다. Ctrl+마이너스의 이름 아래첨자는 [0057](0057-shortcut-and-subscript-contract.md)을 유지한다. 자동 입력/명령은 초안이며 기존 결과를 유지하고 바깥 평가 경계 Enter에서 확정한다.

∘는 활성 편집 수식의 paint에만 나타난다. 비활성 표시와 readonly/PDF에는 나타나지 않으며 focus 전환으로 intrinsic 폭/높이를 바꾸지 않는다. 현재 구현은 기존 예약 공백 안의 overlay로, 원본 SymbolMargin -1·전용 font·모든 배율에서의 glyph 위치/선택 geometry가 같다는 증거는 없다. 결과 단위 RSCALE는 별도 readonly 구조이며 사용자 Scale로 바꾸지 않는다.

## 오른쪽 함수의 의미

오른쪽이 이름으로 해석되는 함수면 `scale(x,f)`는 기존 `f(x)` 호출 경로로 한 인자를 전달한다. 값과 단위는 곱셈을 유지한다. 기존 Call의 인자 평가·arity·captured scope·local function guard·깊이·ORIGIN·builtin dispatch를 공용 함수로 추출했다. 인자는 한 번 평가한다. 명시적 Multiply는 함수를 자동 호출하지 않는다.

일반 이름의 lookup 및 정확히 같은 label의 binding을 먼저 확인한다. 같은 label의 실패한 함수 정의를 builtin으로 되돌리지 않는다. 값으로 재정의한 `cos`와 명시적 FUNCTION `cos`, UNIT `min`의 300s를 구분한다. 사용자 함수 capture·함수 안의 Scale·range·array builtin·vectorize는 기존 Call과 같은 제한 안에서 회귀 검사한다. 이 검사는 새로운 원본 배열 실행 표본이 아니다. 일반 함숫값을 반환하는 식과 higher-order parameter는 현재 Value IR에 없으며 지원 완료로 세지 않는다.

## Ctrl+J가 오른쪽 식을 보존하는 경우

direct `Scale(keyword,rhs)`의 왼쪽 무라벨 단어는 While/If의 BODY1, For의 BODY2, Try의 attempt BODY0, Return의 operand0로 오른쪽 식을 보존한다. 기존 Program은 재사용하며 scalar는 필요한 BODY로 감싼다. Return의 첫 native entry와 Try의 교체된 BODY Left caret를 따로 처리한다. 입력 단어와 Scale 부모를 제거한 후보 전체를 검증한 뒤 한 번 checkpoint하므로 실패 시 source·caret·scope·redo를 보존한다.

명시적 Multiply, Scale의 오른쪽 단어, Break/Continue와 branch keyword는 이 특수 보존 경로에 포함하지 않는다. 원본의 임시 OP_PLACEHOLDER는 native 저장 타입으로 확인되지 않았으므로 새 AST를 추측해 추가하지 않는다. [0106](0106-populated-program-insertion-contract.md)과 [0109](0109-program-insertion-integration.md)의 당시 implicit 미지원 설명은 이 direct Scale 경로에 한해 해소됐으며 일반 parser의 공백은 남아 있다.

새 CPU 앱 테스트는 실제 Key/Text 이벤트로 자동 단위, Ctrl+Shift+U의 함수 적용, 다섯 Ctrl+J 변환·Undo를 검사한다. native probe는 f(x):=x+1 정의와 결과 **3m, 1, 5, 3, 6, 17, 43, 73V**를 저장한다. 자체 저장 재열기는 계산 없이 8개 표시를 복원하고 MCDX 재수입 뒤 같은 값을 재계산한다. 프로그램 제어문은 Program 안에서 검증하며 직접 root IF의 원본 계산 성공으로 확대하지 않는다.

## 최종 통합 검증

Windows/peets 각각 **workspace 918 + backend 2 = 920개 통과·실패 0개·43개 ignored**다. fmt·workspace all-target Clippy·backend library/test Clippy(`-D warnings`)·workspace 빌드와 원본 reference saved/input 각각38/38·차이0을 확인했다. 일반 앱 main은 실행하지 않았다.

native15건/OS·PDF13건/OS, MCDX30개·XML120파트 설치 XSD 오류0개, PDF26개 이미지0개다. 두 PDF reader로 검색 텍스트·단위·원본 표 기준점/색상을 검사했다. 새 implicit-scale PDF는 OS별3쪽·복원 결과8개·drawing318개이며 편집 전용 ∘는 출력에 없다. 원본 전체 glyph·조판·실제 GPU/프린터 동등성은 미검증이다.

동결 archive `mathcraft-implicit-scale-20261019-r113.tar`의 SHA-256은 `998f776187615f6146aff66ccb44da79f70c1f967b0b657a11e2d8bdd2a7895b`, 7,997,440bytes다. tar의 모든 regular file 172개와 제품 Rust129개·vendor Rust5개를 양 호스트에서 대조했다. peets root는 `/home/peets/Projects/mathcraft-implicit-scale-20261019-r113`다. Windows release `target/release/mathcraft.exe`는 22,177,280bytes·SHA-256 `e5e61efa3b5ffd456f6f9bb32ff631b9e7f166b84a12c3cbcfb4620d222e010c`다. 원격 산출물67개·표준 로그5개·별도36case actual-open 로그를 size/SHA로 재검증했다.

전체 보고서는 `implicit-scale-integration-20261019/source-validation.json`, native/PDF는 그 아래 `schema-validation.json`, `pdf-verified/validation.json`에 보존한다. 이전 r109 증거는 덮어쓰지 않는다.

## 실제 열기·입력 r109→r113

보존한 r109와 동결 r113 CPU 앱 harness를 동일 PC에서 각각5회 순서 교대했다. 단위ms, 각 열은 중앙값이다. 합계는 실행별 구간을 먼저 더한 뒤 median을 구한다. page-frame CPU이며 tessellation/GPU·프로세스 시작·전체 계산 완료 시간은 포함하지 않는다.

| 문서 | Text r109→r113 | Enter r109→r113 | Text+Enter r109→r113 | worksheet+display r109→r113 |
| --- | ---: | ---: | ---: | ---: |
| 4,000 scalar/cache | 2.547→2.792 | 45.682→38.968 | 48.283→41.353 | 26.801→19.725 |
| 4,000 scalar/no cache | 2.662→2.499 | 44.221→37.379 | 46.635→39.951 | 26.365→19.585 |
| 100개16×16/cache | 5.350→5.210 | 32.237→28.344 | 37.577→33.953 | 13.046→10.080 |
| 100개16×16/no cache | 5.486→5.500 | 32.689→30.212 | 38.282→35.333 | 13.175→10.167 |
| 원본1015 | 0.552→0.518 | 3.671→2.914 | 4.477→3.485 | 2.242→1.380 |
| 원본1016 | 0.514→0.495 | 3.668→2.663 | 4.209→3.121 | 2.077→1.266 |

| 문서 | Arrow release+Enter r109→r113 | IME preedit+Enter r109→r113 | 기록한 열기→Enter CPU 합 r109→r113 | worker residual r109→r113 |
| --- | ---: | ---: | ---: | ---: |
| 4,000 scalar/cache | 1.601→1.602 | 1.604→1.699 | 89.722→85.893 | 28.269→29.402 |
| 4,000 scalar/no cache | 2.095→1.544 | 1.907→1.929 | 74.904→67.661 | 29.171→27.848 |
| 100개16×16/cache | 0.951→0.914 | 0.968→1.018 | 72.185→68.692 | 11.923→11.438 |
| 100개16×16/no cache | 1.197→1.279 | 1.219→1.216 | 70.542→67.197 | 11.018→11.400 |
| 원본1015 | 0.581→0.546 | 0.534→0.560 | 49.103→49.092 | 1.816→2.131 |
| 원본1016 | 0.564→0.567 | 0.559→0.531 | 49.631→47.174 | 1.768→1.727 |

worker residual은 Enter와 동시에 끝난 계산을 제외하므로 Enter와 더해 전체 완료 지연으로 부르지 않는다. 기록한 열기→Enter는 font/fixture setup·보조 caret 이동·frame 사이 대기·tessellation을 제외한다.

전후 총360case와 양 OS 별도36case가 첫 이벤트 준비0회·수정 Enter1회·비편집0회·draft/generation/global-error 계약을 통과했다. 이벤트 합의36개 중앙값 중 9개가 증가했다. 모든 입력이 빨라졌다고 주장하지 않는다.

증가 표본(ms): `4k-scalars-with-cache/ime-preedit` 1.604→1.699, `100-matrices16-without-cache/arrow-release` 1.197→1.279, `1015_LCC_S/arrow-press` 0.538→0.608, `100-matrices16-with-cache/ime-preedit` 0.968→1.018, `1016_LCC_S/arrow-press` 0.535→0.579, `1015_LCC_S/ime-preedit` 0.534→0.560, `4k-scalars-without-cache/ime-preedit` 1.907→1.929, `1016_LCC_S/arrow-release` 0.564→0.567, `4k-scalars-with-cache/arrow-release` 1.601→1.602.

원시 자료는 `implicit-scale-open-input-alternating-20261019.{json,log}`와 `implicit-scale-open-input-phase-summary-20261019.{json,log}`다. before SHA `b11903f67869d2a07d420c0aa0c0214a94d40e2e37c99de8962b035ebe080bd0`, after SHA `7455dff0ec57b8e7984a774c1f1408ec9ca3b9e2eb3650767fae333703f41d73`를 source report에 묶었다.

EA-028/080의 지원 깊이와 `op_scale`의 부분 구현을 갱신했다. 세부553행 부분138/미구현415, engine874행 부분127/미구현747이다. `mc_Scale` 등 다른 함수를 이름 유사성으로 지원 처리하지 않는다. 일반 parser·함숫값·전체 native 조판·도표/풀이/상징의 공백과 실제 사용성 검증이 남아 있어 전체20% 달성은 판정하지 않는다.
