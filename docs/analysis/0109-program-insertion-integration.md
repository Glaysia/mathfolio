# 선택한 식의 프로그램 삽입과 실제 프레임 비용

2026-10-19. [0106 원본 삽입 계약](0106-populated-program-insertion-contract.md)의 정적 REA 근거를 독립 Rust 편집·계산·저장에 연결한다. 프레임 비용의 분해·최적화 근거는 [0107](0107-frame-interaction-cost.md)을 따른다. GUI·스크린샷·원본 프로세스·CLR/COM 실행은 사용하지 않았다. r105 소스·실행 파일·원시 측정은 보존한다.

## 선택 상태와 BODY 보존

이전 while/for/return 리본은 현재 target 전체를 감쌌다. While은 이를 조건 child0, For는 iterable child1에 넣었으며 커서·문자 선택·구조 Scope를 구분하지 않았다. 원본의 모드 분기와 다르므로 공통 후보 기반 삽입으로 교정했다.

Scope로 선택한 완전한 식은 While/IF의 BODY child1, For의 BODY child2, Try의 attempt BODY child0에 보존한다. 조건·반복 변수/range는 새 빈 입력칸이다. 기존 Program을 선택하면 그 노드와 statement children을 BODY로 재사용하며 Program을 불필요하게 한 겹 더 만들지 않는다. 원래 식의 ID·이름/label/contextual·literal subscript를 유지한다.

원본 Scope Left/Right를 그대로 구분한다. Special 네 종류는 양방향을 허용하며, Try Right는 역순 첫 placeholder를 고른다. Return은 Scope Left에서만 선택한 식을 child0에 보존한다. Right는 이 확정 경로에서 거부한다. 반환 식의 커서는 원본 Return Center에서 한 번 Right 이동한 첫 navigable state를 사용한다. 숫자는 UTF-16 offset0, `73·V`는 첫 숫자의 offset0이며, 바깥 fence와 prefix 종류는 확인한 boundary를 사용한다. 모든 노드를 무조건 첫 leaf로 이동시키지 않는다.

자체 `Selection::Subtree`는 원본의 임의 segment 전체를 표현하지 않는다. 현재 지원은 완전한 subtree/statement Scope다. 실제 Space 경로는 Key+Text 공백 이벤트를 통해 Scope를 만들며 같은 삽입·한 Undo·원래 scope history 복원을 검사했다.

## 전체 문자 선택은 다른 명령 경로다

Normal 모드에서 명시적으로 값의 UTF-16 범위 전체를 선택하면 해당 값을 보존하지 않고 새 빈 연산자 템플릿으로 교체한다. While/For/IF/Try/Return/Break/Continue에 연결했다. 앞뒤 방향의 전체 선택을 모두 검사한다. Break/Continue는 0-arity native leaf이며 새 커서는 Right다.

root atom·평가 왼쪽의 독립 atom·Program/BODY의 독립 statement atom을 지원한다. 원본의 Range 없는 atom Center는 collapsed text caret가 될 수 있으므로 전체 값 선택으로 해석해 지우지 않는다. 중간 문자/부분 범위·rich suffix만의 선택·산술식의 인접 term·Ctrl+J의 원본 implicit-parent 특수 보존은 일반 parser를 더 복원해야 한다. 현재 표현할 수 없는 경우를 임의 wrapper 또는 명시 곱셈으로 바꾸지 않는다. 이 미지원 경로의 무변경을 원본도 거부한다는 주장으로 사용하지 않는다.

빈 자리 템플릿과 정확한 단어 Ctrl+J는 기존 계약을 유지한다. IF 리본은 최소 test/body 한 쌍을 만들며 else는 별도 명령이다. 비UI `insert_if_else` 편의 API는 기존 내부 정책으로 남아 있으며 원본 단일 IF 명령의 재현으로 세지 않는다.

## 원자적 편집·계산 경계

scope/문자 모드, 방향·구조 조건을 확인하고 history 없는 후보에서 삽입·BODY repair·caret를 계산한다. 후보 전체의 node/depth/문자/binder 검증이 성공한 뒤에 한 번 checkpoint한다. 독립 자원 한도인 node16,384·depth128 등을 원본의 수치 제한이라고 주장하지 않는다. 실패 시 원래 source·revision·caret·selection·Undo/Redo를 보존한다.

앱 리본은 기존 동일 core API를 사용한다. 실제 입력 검사는 Program 안에서 결과17을 표시한 상태로 그 식을 IF 본문에 넣고 조건0·else43을 작성한다. 초안 동안 결과17을 유지하고 평가 바깥 경계의 Enter에서만43으로 바꾼다. Scope 양방향·메타데이터·기존 Program 재사용·Return 거부 방향·Normal 전체 값 선택을 별도로 검사한다.

단독 `eval(if)`의 schema/저작 허용과 원본 계산 성공은 구분한다. 동봉 도움말 native IF82개는 모두 Program 안이었다. 직접 IF 계산의 원본 성공 표본이나 parent 거부의 양성 근거는 확보하지 못했으며, 자체 엔진의 standalone IF는 이번에 추가하지 않았다. Program 내부 IF 계산을 직접 root 수치 동등성으로 확대하지 않는다. ignored `standalone-if-contract-20261019.md`와 `standalone-if-help-native-survey-20261019.json`에 조사 경계와 원문 SHA를 남긴다.

새 native probe는 실제 리본으로 기존 statement에 제어문을 넣고 header를 채운 뒤 5개 결과 **3, 6, 17, 43, 73 V**를 검사한다. 자체 저장 후 재열기는 계산0회로 표시5개를 복원하며 MCDX 재수입·재계산도 같은 값을 낸다. 헤더와 본문이 바뀌지 않았는지 native 구조와 원본 XSD를 별도로 확인한다.

## 프레임의 중복 작업

계산 준비0회인 이동/IME/Enter에서도 입력 스냅샷·선택 수식 재조판·pending opacity paint의 별도 비용이 있었다. [0107](0107-frame-interaction-cost.md)은 실제 page-frame과 열기 단계를 분해하며, 이전 r101→r105의 소수 악화 표본과 현재 확인한 비용을 구분한다. 미계산 표시·글자/선/커서의 색상·clip을 유지하는지 tessellation mesh를 대조한다. paint 단계에서 tessellation으로 비용을 옮겼다면 전체 개선으로 세지 않도록 page+tessellation도 측정한다.

## 최종 통합 검증

Windows/peets 각각 **workspace 889 + backend 2 = 891개 통과·실패 0개·40개 ignored**다. fmt·workspace all-target Clippy·backend library/test Clippy(`-D warnings`)·workspace 빌드와 원본 reference saved/input 각각38/38·차이0을 확인했다.

양 OS 각각 native14건·PDF12건을 검사했다. MCDX **28개·XML112파트**의 설치 XSD 오류는0개다. 원본 표 계산·색상·저장 결과64개·XamlPackage27개 원문 보존 회귀를 유지했다. PDF **24개 모두 이미지0개**이며 두 독립 reader로 검색 텍스트·단위·원본 표 기준점/색상을 검사했다. 새 program-insertion PDF는 OS별2쪽·복원 결과5개·drawing193개다. 원본 전체 glyph/조판·DPI·물리 프린터의 동등성은 미검증이다.

source tar는 ignored `mathcraft-program-insertion-20261019-r109.tar`, SHA-256 `d537366ab6fa8760982709d99f82a0bc9f357ed7f243e0d7411866ffdc1057d3`, 7,915,520bytes다. 이번 manifest는 Rust/config만이 아니라 tar의 **모든 regular file 166개**를 기록한다. 제품 Rust123개·vendor Rust5개와 embedded assets·spec·tests/reference·examples·선정 MCDX까지 archive/양 호스트에서 대조했다. peets root는 `/home/peets/Projects/mathcraft-program-insertion-20261019-r109`다. Windows release `target/release/mathcraft.exe`는 **22,135,296bytes**, SHA-256 `005779c9d49018c0d15f6dcd297374a7f391388085e3d373d38022f1db1eed4d`다. 일반 앱 main은 실행하지 않았다.

`source-validation.json`에 소스/release/성능 identity, `schema-validation.json`과 `pdf-verified/validation.json`에 native/PDF 검사를 기록했다. 원격 산출물62개·표준 로그5개·별도 actual-open 로그를 수집 후 size/SHA로 재검증했다. 표준 로그 prefix는 `program-insertion-`, 날짜 suffix는 `-20261019.log`다.

## 실제 열기·입력의 최종 r105→r109 비교

보존한 순수 r105와 최종 r109 harness를 같은 PC에서 각각5회 순서 교대했다. 단위ms, 각 열은 중앙값이며 합은 실행별 frame CPU 시간을 먼저 더한 값의 median이다. 이 표는 기존 standard actual-open workflow의 **page-frame CPU**이며 tessellation/GPU·프로세스 시작·계산 완료 시간은 포함하지 않는다. [0107](0107-frame-interaction-cost.md)의 snapshot/height 및 opacity 별도 비교, page+tessellation 합과 구분한다.

| 문서 | 첫 Text r105→r109 | Enter r105→r109 | 실행별 Text+Enter r105→r109 |
| --- | ---: | ---: | ---: |
| 4,000 scalar, cache 있음 | 2.214→2.331 | 41.960→43.077 | 44.540→45.179 |
| 4,000 scalar, cache 없음 | 2.141→2.126 | 41.921→42.763 | 44.149→44.920 |
| 100개16×16 행렬, cache 있음 | 5.054→5.063 | 30.324→30.948 | 35.490→36.085 |
| 100개16×16 행렬, cache 없음 | 6.941→5.227 | 34.405→30.959 | 41.716→36.305 |
| 원본1015 | 0.457→0.499 | 3.306→3.486 | 3.771→3.975 |
| 원본1016 | 0.468→0.451 | 3.352→3.259 | 3.822→3.707 |

수정 없는 이벤트 뒤 Enter도 포함해 실행별 합으로 비교한다.

| 문서 | Arrow release+Enter r105→r109 | IME preedit+Enter r105→r109 |
| --- | ---: | ---: |
| 4,000 scalar, cache 있음 | 3.020→1.530 | 4.876→1.567 |
| 4,000 scalar, cache 없음 | 3.036→1.548 | 5.169→1.526 |
| 100개16×16 행렬, cache 있음 | 5.428→0.824 | 7.414→0.854 |
| 100개16×16 행렬, cache 없음 | 9.936→1.103 | 13.561→1.255 |
| 원본1015 | 0.667→0.515 | 0.818→0.492 |
| 원본1016 | 0.670→0.519 | 0.818→0.495 |

전후 각180case, 총360case와 양 OS 별도36case가 첫 이벤트 준비0회·비편집 total0회·수정 Enter1회·draft/generation/global-error 계약을 통과했다. 36가지 이벤트별 합 중앙값 중11개는 증가했다. 모든 입력·열기가 빨라졌다고 주장하지 않는다. 별도 frame profiler·tessellation oracle은 기존 경계를 넓힌 증거로 구분한다.

원시 자료는 `program-insertion-open-input-alternating-20261019.{json,log}`다. frozen r105 SHA-256 `d7d2cbecd4a50af429e392ee6e70a5cac2afef7de84f14613a4407d29d6e077a`, r109 `b11903f67869d2a07d420c0aa0c0214a94d40e2e37c99de8962b035ebe080bd0`이며 최종 현재 source의 app harness와 after가 같다.

증가한 이벤트 합 중앙값(ms): `100-matrices16-with-cache/ime-commit` 35.598→35.818, `100-matrices16-with-cache/paste` 35.286→35.469, `100-matrices16-with-cache/text` 35.490→36.085, `1015_LCC_S/paste` 3.849→3.866, `1015_LCC_S/text` 3.771→3.975, `1016_LCC_S/ime-commit` 3.695→3.738, `4k-scalars-with-cache/ime-commit` 44.043→44.914, `4k-scalars-with-cache/paste` 44.118→45.885, `4k-scalars-with-cache/text` 44.540→45.179, `4k-scalars-without-cache/ime-commit` 44.543→45.160, `4k-scalars-without-cache/text` 44.149→44.920.


## 남은 계산 준비와 열기 비용

같은 실행에서 worksheet+display를 먼저 더한 중앙값(ms)은 다음과 같다. 입력/paint 중복을 없애도 계산용 직렬화·label·signature 준비가 남는다는 증거다. 이 단계는 일부 문서에서 증가했다. `program-insertion-open-input-phase-summary-20261019.{json,log}`에 전체360case의 대응과 frozen binary/archive identity를 기록했다.

| 문서 | worksheet+display r105→r109 | 기록한 열기→Enter frame CPU 합 r105→r109 |
| --- | ---: | ---: |
| 4,000 scalar, cache 있음 | 25.089→26.104 | 81.545→82.394 |
| 4,000 scalar, cache 없음 | 25.331→25.747 | 69.083→69.472 |
| 100개16×16 행렬, cache 있음 | 12.530→12.705 | 68.663→70.041 |
| 100개16×16 행렬, cache 없음 | 12.410→12.486 | 76.088→64.432 |
| 원본1015 | 1.942→2.075 | 45.282→47.371 |
| 원본1016 | 2.013→1.939 | 45.385→44.057 |

열기→Enter 합은 기록한 CPU 구간의 합이며 fixture setup·frame 사이 대기·프로세스 시작·tessellation/GPU를 포함하지 않는다. 별도 opacity 비교의 page+tessellation 개선과 합산하지 않는다. 원본1015의 이 합은2.089ms 증가했고1016은1.328ms 감소했다. 원본 native cold alignment·restore 비용은 위치를 확인했지만 제거한 상태가 아니다.

EA-071/074~079의 지원 깊이를 갱신했다. 세부553행의 부분 구현138·미구현415라는 상태 수는 바뀌지 않았으며 이를 제품 완료율로 환산하지 않는다. 일반 문자 분할·implicit parser·전체 구조 repair·도표/풀이/상징·원본 전체 조판과 실제 GPU 사용성 등 남은 공백을 포함한 20% 달성은 아직 판정하지 않는다.
