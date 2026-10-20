# 괄호·입력 컨테이너와 직렬화 비용

2026-10-19. [0114 원본 계약](0114-parenthesis-adjacency-contract.md)의 설치 CIL·descriptor·도움말을 독립 Rust 입력에 연결한다. GUI·스크린샷·원본 프로세스·CLR/COM은 사용하지 않았다. [0115 직렬화 비용](0115-fused-serialization-cost.md)의 독립 serializer oracle과 실제 전체 Enter 비교를 함께 사용한다. 이전 r113 소스와 실행 파일·원시 측정은 보존한다.

## 새 피연산자를 기존 값 안에 합치던 오류

기존 `2` 뒤의 `(`는 Group(2)로 감싸고 커서를 기존 숫자의 왼쪽으로 보냈다. 이어 `3)`을 입력하면 Group(32)가 되어 6 대신 32를 계산했다. 수정 전 CPU 테스트에서 이 값과 AST를 재현했다. 이제 Normal 숫자 끝에서는 Scale(2,Group(PH)), 이름 끝에서는 Call(name,PH)를 만든다. 이름의 UNIT/FUNCTION label을 함수 호출 생성의 조건으로 삼지 않는다. 완성 Group 뒤의 새 괄호도 별도 배율이다. 명시적 Multiply는 바꾸지 않는다.

`sqrt(`와 `abs(` 단어를 각각 Root·Absolute 템플릿으로 강제 치환하던 독립 규칙도 제거했다. 일반 이름의 호출과 원본 전용 연산자 명령을 분리한다. `sqrt`의 사용자 정의와 label은 이름 호출 경로를 따르며 `\\`/√·절댓값 명령은 기존 전용 노드를 사용한다. 이 변경은 모든 함수 타입/오버로드의 원본 동일성을 뜻하지 않는다.

임시 OP200이 Scale142로 바뀐 뒤 다시 parse되는 원본 순서를 반영해 동일 우선순위 Scale 사슬과 더 강한 Neg, Power base 뒤의 괄호도 구분한다. Power base의 숫자 끝에 넣으면 Scale(2,Power(Group(PH),기존지수)), Power 자체 Right에 넣으면 Scale(Power(2,기존지수),Group(PH))다. 기존 지수와 이름 metadata·노드 관계를 보존한다. 부분 이름에서 나온 숫자 조각은 factory의 descriptor 분류에 따라 Number로 재분류하며, 원래 이름이었다는 이유로 Identifier("12")를 만들지 않는다. 일반 radix·hex·복잡한 formatted 조각의 전체 factory는 아직 지원 완료로 세지 않는다.

## 원본의 컨테이너 안에서 입력한다

원본 GetContainerRoot는 괄호만의 특별 처리 없이 일반 연산자·피연산자 삽입에도 적용된다. Power exponent, Fraction numerator/denominator, Index indices와 Call의 원본 ARGUMENT_LIST 등 확인한 슬롯이 선형 parse의 경계다. 기존 prepare_infix는 우선순위가 높다는 이유로 Power/Fraction 밖으로 올라가 이 경계를 잃었다. 공통 슬롯 조건을 먼저 적용하도록 교정했다.

따라서 커서를 그대로 둔 `2^3+4`는 2^(3+4)=128, `2/3+4`는 2/(3+4)=2/7이다. 지수 안에서 입력한 `2^3m`을 바깥 단위로 옮기지 않으며 차원 있는 지수 오류를 유지한다. 지수·분모에서 ArrowRight로 나간 뒤 `+4`/`*2`/`m`을 입력하면 바깥 식에 붙는다. 기존 배열 좌변 검사도 실제 ArrowRight로 `i^2` 밖에 나온 후 `+j`를 입력해 기존 11이라는 계산 시나리오를 보존한다. UI 입력의 범위를 고친 것이며 이미 저장된 native AST의 계산 의미를 다시 해석한 것이 아니다.

## Normal·Scope와 닫기

Scope 선택을 감싸는 동작과 Normal 텍스트 입력을 분리한다. 빈 자리, 명시 전체 값 문자 선택, 중간 UTF16 범위 제거 후 prefix/suffix, 값 시작/끝, 식 경계별 삽입과 커서를 구분한다. 원본 문자열 범위와 실제 native 경계를 표현할 수 없는 조합을 다른 wrapper로 조용히 바꾸지 않는다. 후보 전체의 구조·예산·커서 검증 뒤 한 번 checkpoint하며 실패 시 source·caret·scope·redo를 보존한다. 임시 OP_PLACEHOLDER를 영구 AST로 발명하지 않는다.

닫는 `)`는 무조건 가장 가까운 컨테이너로 이동하지 않는다. 원본의 구조상 한 번 Right 뒤 Family1 오른쪽 경계에 도달하는 경우에만 한 번 더 이동하며 문서 변경과 Undo를 만들지 않는다. `Group`과 마지막 Call argument의 확인한 경계, 선형 infix의 tail, 명시 DegreeRight 뒤 POLAR tail을 구분한다. Power/Fraction/Index 자신의 오른쪽 상태를 건너뛰지 않는다. 그 외 확인한 Postfix GROUPING 삽입은 실제 문서 편집이다.

예를 들어 `(2^3)`을 그대로 입력하면 첫 `)`는 지수 3을 괄호로 감싼다. PowerRight로 먼저 ArrowRight를 보낸 다음 `)`를 입력해야 바깥 Group을 벗어난다. `f()`의 닫기에서 인자 PH를 지워 arity0을 만들던 독립 규칙도 이 텍스트 경로에서 제거했다. 원본 CloseParen은 그 경계에서 navigation-only이며 확인한 native writer는 PH를 보존한다. 일반 zero-argument 함수의 불가능성을 주장하는 것은 아니다. `]`와 기존 비UI wrapper API는 이번 원본 `)` 명령으로 소급 분류하지 않는다.

원본의 전체 navigator/handle repair, 모든 부분 선택·rich run·symbolic/program 조합을 완성한 것은 아니다. 확인한 분기와 남은 범위는 0114 및 아래 최종 검증과 함께 읽어야 한다.

## 계산 확정·저장과 출력

CPU 앱 테스트는 실제 Text/Key 이벤트로 2 뒤에 `(3)`을 입력해 초안 동안 기존 결과 2와 계산0회를 유지하고 평가 경계 Enter에서 6으로 확정하는지 검사한다. Undo/Redo, 함수 정의를 유지한 호출 수정, 실제 ArrowRight의 바깥 연산자/단위 입력을 검증한다. 값이 같아도 AST와 native Scale·명시 Multiply·괄호를 구분한다.

새 native probe는 함수 정의 1개와 수치·단위·지수/분모/이동 시나리오를 저장하고, 자체 파일 재열기의 무계산 표시 복원과 MCDX 재수입·재계산을 검사한다. 출력에서도 새 소스를 그대로 보존하며 기존 표·색상·결과 캐시·벡터 PDF 회귀를 유지한다.

## 직렬화 변경

원래 재귀 String/format/escape 임시 복사를 plain subset의 한 출력 buffer로 줄였다. unsupported forms와 Call/Index의 fallible 인자→callee/base 순서는 독립 legacy serializer fallback으로 유지한다. raw proof와 labelled emission 양쪽에 적용하며 validation·exact source identity·표시 tree·캐시 payload·8MiB output/32MiB cache 예산은 유지한다. 새 proof를 만든 직후 그 proof와 원본 tree를 다시 비교하던 첫 equality만 생략하며 occupied source guard는 유지한다.

0115의 함수 단독 측정은 행렬 raw+labelled 9.400→2.559ms, 4k scalar 7.092→4.884ms였지만 proof 생성·validation/clone·앱 bookkeeping·worker는 포함하지 않는다. 실제 입력 전체와 악화 표본을 최종 동결 비교에서 판정한다. 이 수치만으로 16ms 또는 제품20% 달성을 주장하지 않는다.

## 최종 통합 검증

Windows/peets 각각 **workspace 954 + backend 2 = 956개 통과·실패 0개·45개 ignored**다. fmt·workspace all-target Clippy·backend library/test Clippy(`-D warnings`)·workspace release 빌드와 원본 reference saved/input 각각38/38·차이0을 확인했다. 일반 앱 main은 실행하지 않았다.

native16건/OS·PDF14건/OS, MCDX32개·XML128파트 설치 XSD 오류0개, PDF28개 이미지0개다. 두 독립 PDF reader로 검색 텍스트·단위·표 기준점/색상을 확인했다. 새 parenthesis PDF는 OS별4쪽·복원 결과14개·drawing998개다. 원본 전체 glyph·조판·GPU/실기 프린터 동등성은 미검증이다.

archive `mathcraft-parenthesis-20261019-r117.tar`는 SHA-256 `fea698f24e4af0ba087d2399574a554064df6c895aa3c0693a683afad42ab34d`, 8,099,840bytes다. 모든 regular source177파일(제품 Rust134·vendor Rust5)을 archive/양 호스트와 대조했다. peets root는 `/home/peets/Projects/mathcraft-parenthesis-20261019-r117`다. Windows release `target/release/mathcraft.exe`는 22,199,808bytes·SHA-256 `c6618d181df084726fdf16e8d9e40bcdbe9d142af73835fa3eef45b02068c783`다. 원격 산출물72개·표준 로그5개·별도 actual-open36case 로그를 size/SHA로 대조했다.

최종 보고서는 `parenthesis-integration-20261019/source-validation.json`, native/PDF는 그 아래 `schema-validation.json`, `pdf-verified/validation.json`에 기록했다. r113 원본 archive·실행 파일·측정은 보존했다.

## 실제 열기·입력 r113→r117

동결 r113/r117 CPU 앱 harness를 같은 PC에서 각각5회 순서 교대했다. 단위ms, 각 열은 중앙값이며 합은 실행별 구간을 먼저 더한 median이다. page-frame CPU로 tessellation/GPU·프로세스 시작·전체 계산 완료 시간을 뜻하지 않는다. 새 괄호/컨테이너 변경과 직렬화 변경이 함께 들어간 통합 비교다.

| 문서 | Text r113→r117 | Enter r113→r117 | Text+Enter r113→r117 | worksheet+display r113→r117 |
| --- | ---: | ---: | ---: | ---: |
| 4,000 scalar/cache | 2.800→2.462 | 37.320→32.927 | 39.779→35.236 | 19.265→16.949 |
| 4,000 scalar/no cache | 2.271→2.167 | 35.687→32.756 | 38.062→35.103 | 18.372→16.513 |
| 100개16×16/cache | 5.320→5.131 | 28.084→20.506 | 33.073→25.698 | 9.815→5.874 |
| 100개16×16/no cache | 5.425→5.435 | 28.156→20.766 | 33.581→26.247 | 9.311→5.624 |
| 원본1015 | 0.466→0.495 | 2.599→2.264 | 3.106→2.770 | 1.177→1.056 |
| 원본1016 | 0.466→0.459 | 2.515→2.017 | 2.979→2.465 | 1.133→0.884 |

| 문서 | Arrow release+Enter r113→r117 | IME preedit+Enter r113→r117 | 기록한 열기→Enter CPU 합 r113→r117 | worker residual r113→r117 |
| --- | ---: | ---: | ---: | ---: |
| 4,000 scalar/cache | 1.502→1.557 | 1.547→1.553 | 79.472→74.311 | 29.943→26.930 |
| 4,000 scalar/no cache | 1.622→1.569 | 1.518→1.639 | 63.380→60.535 | 27.164→26.584 |
| 100개16×16/cache | 0.832→0.799 | 0.857→0.799 | 66.107→59.359 | 11.160→11.557 |
| 100개16×16/no cache | 1.262→1.234 | 1.111→1.171 | 62.456→54.934 | 11.333→11.547 |
| 원본1015 | 0.519→0.526 | 0.507→0.509 | 45.228→45.696 | 1.848→1.721 |
| 원본1016 | 0.538→0.521 | 0.498→0.566 | 43.629→43.804 | 1.632→1.837 |

worker residual은 Enter와 동시에 끝난 계산을 제외하므로 Enter와 더해 완료 지연으로 부르지 않는다. 기록한 열기→Enter 합은 font/fixture setup·caret 보조 이동·frame 사이 대기·tessellation을 제외한다. 전후360case와 양 OS 별도36case가 draft/prepare/generation/global-error 계약을 통과했다. 이벤트 합36개 중앙값 중10개는 증가했다. 모든 입력이 빨라졌다고 주장하지 않는다.

증가 표본(ms): `4k-scalars-without-cache/ime-preedit` 1.518→1.639, `1016_LCC_S/ime-preedit` 0.498→0.566, `100-matrices16-without-cache/ime-preedit` 1.111→1.171, `4k-scalars-without-cache/arrow-press` 1.483→1.543, `1016_LCC_S/arrow-press` 0.511→0.567, `4k-scalars-with-cache/arrow-release` 1.502→1.557, `100-matrices16-with-cache/arrow-press` 0.801→0.821, `1015_LCC_S/arrow-release` 0.519→0.526, `4k-scalars-with-cache/ime-preedit` 1.547→1.553, `1015_LCC_S/ime-preedit` 0.507→0.509.

원시 자료는 `parenthesis-open-input-alternating-20261019.{json,log}`와 `parenthesis-open-input-phase-summary-20261019.{json,log}`다. before SHA `7455dff0ec57b8e7984a774c1f1408ec9ca3b9e2eb3650767fae333703f41d73`, after SHA `df7f803114021833686cd39c7754f2b0644d3a3339bddda672b579f9a256e7b6`를 source report에 연결했다.

세부553행 부분138/미구현415, engine874행 부분127/미구현747이다. 지원 깊이를 갱신했으며 일반 lexer·rich-run·전체 navigator/repair·도표/풀이/상징의 공백과 실기 사용성 검증을 포함한 전체20% 달성은 판정하지 않는다.
