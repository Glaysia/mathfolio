# Try/on error 작성·계산·삭제와 표시 준비 중복 제거

2026-10-19. [0103 원본 TRY 계약](0103-try-control-contract.md)과 [0104 원본 탐색·삭제](0104-try-deletion-contract.md)를 독립 Rust 구현에 연결한다. [0102](0102-display-preparation-cost.md)는 실제 Enter의 표시 준비를 분해한다. GUI·스크린샷·원본 프로세스·CLR/COM 실행은 사용하지 않았다. 앞선 r101 소스·실행 파일·측정은 보존한다.

## 작성·native 형식·원본 조판

TRY는 순서가 정해진 두 BODY, child0 시도 본문과 child1 on-error 본문이다. 두 child 모두 inherent이며 원본의 precedence 290·primal0·두 탐색 위치를 구분한다. native는 `<tryCatch>`의 두 expression이며 canonical BODY는 `<program>`이다. scalar body는 한 statement Program으로 정규화하며 원래 중첩 Program을 flatten하지 않는다. 알 수 없는 attributes·namespace·잘못된 arity는 보존 가능한 지원 범위 밖으로 처리한다.

빈 위치 리본과 정확한 전체 단어 Ctrl+J `try`는 두 빈 BODY를 만들고 첫 본문의 placeholder로 이동한다. 기존 Normal/HasNoLabel/formatted 이름·suffix·문자 선택·scope·예산·단일 Undo 조건을 공유한다. 일반 populated container 삽입·분할을 완료 처리하지 않는다.

원본 VisualTryNode처럼 `try` header, 첫 BODY, `on error` header, 두 번째 BODY의 네 행을 쓴다. BODY는 들여 쓰고 왼쪽 double fence를 유지하며 오른쪽 minor fence는 Try가 소유한다. 글꼴 glyph·stroke·일부 DPI metrics는 독립 근사다. header 클릭은 Center caret에 NodeSide Left/Right를 쓰며 native operator-part index는 null이다. IF의 명시 part index와 혼동하지 않는다.

## 계산 경계

첫 본문이 성공하면 그 값을 반환하고 handler를 실행하지 않는다. 계산 오류가 나면 handler를 한 번 실행한다. handler 자체 오류는 같은 Try가 다시 잡지 않으며 바깥 Try의 적용 대상이 될 수 있다. Return/Break/Continue는 오류가 아닌 제어 흐름으로 전파한다. 단위 값, 반복의 continue/break와 handler return을 일반 프로그램 실행에 연결했다.

원본 도움말은 계산 오류를 포괄적으로 설명하지만 확보한 명시 오류 예제는 0으로 나누기다. 현재 엔진의 fault 분류는 **독립 구현의 제한된 정책**이다. Unsupported/미지원 구현, 이전 영역의 실패 연결, CANCELLED, 호스트 instruction budget·control/no-value·미지원 recurrence는 대체 결과로 숨기지 않는다. 나머지 지원하는 계산 fault는 handler로 보낸다. 원본의 모든 정적 타입/이름 검사·오류 taxonomy 동등성을 주장하지 않는다.

현재 interpreter의 순차 local state를 유지하며 실패 전 대입을 임의 rollback하지 않는다. 이 정책은 자체 회귀로 검증하고 원본의 부분 상태 rollback은 미확정으로 남긴다. 프로그램 바깥 워크시트 binding은 기존대로 clone된 실행 scope로 보호한다. expression-class Try를 직접 수식에 놓는 경로는 local Program 한 명령으로 실행한다. 이는 schema·descriptor·값 반환 도움말에 근거한 제한된 추론이며, 확보한 원본 native 예제는 모두 Program 내부였다. 이를 다른 모든 제어문의 standalone 동등성으로 확대하지 않는다.

## 삭제와 확정

원본 Delete pipeline에서 확인한 빈 Try 바깥 안쪽 삭제, 직접 BODY 상태, header 중심의 두 본문 병합과 이전 Program/확인된 마지막 inherent BODY 연결을 지원한다. header 삭제는 두 BODY 순서와 주변 문장을 보존한다. 빈 header 중심은 무조건 placeholder로 축약하지 않고 원본 repair 결과인 두 빈 문장 Program과 구분한다. 실패하는 편집은 원문·caret·redo를 보존한다. 일반 커서/다른 descriptor의 전체 repair는 미완료다.

실제 key·ribbon·pointer 검사는 draft 중 기존 결과를 유지하고 Enter로 확정하는 계약을 따른다. 정상 값 17의 시도 본문을 0으로 나누기로 바꾸면 입력 중에는 17을 유지하고 Enter 후 handler의 43이 된다. header 삭제로 본문을 합친 경우도 확정 전 결과를 바꾸지 않는다.

native probe는 5개 영역의 17, 43, 12, 3, 73 V를 저장·재수입·재계산으로 확인한다. 자체 저장 재열기는 계산 0회로 표시 5개를 복원한다. 정상 Try의 handler lazy, handler 오류의 바깥 전파, 미지원·상위 실패·호스트 한도·실제 cancellation/fresh retry를 별도로 검사한다.

새 Ctrl+J/ribbon Try의 최초 placeholder에서도 별도 바깥 선택 없이 Backspace로 빈 Try를 제거한다. 같은 위치 Delete는 원본처럼 두 빈 문장의 Program으로 합치고 handler 쪽 placeholder에 caret를 둔다. 둘째 placeholder Delete를 억지로 같은 축약으로 처리하지 않는다. 이 실제 fresh 경로는 native BODY Center 상태를 사용하며, 편집자가 표현할 수 있는 모든 BODY boundary가 원본에서 도달 가능한 상태라는 뜻은 아니다.

## 표시 준비의 구조적 비용

실제 cold 4k의 표시 준비 약 14 ms에서 전체 worksheet DOM 재파싱 약 5.4 ms, tree decode/검증 약 4.2 ms가 주된 비용이었다. 작은 문자열/geometry 복사만 없애면 전체가 해결된다고 판단하지 않았다. 계산용 fragment를 labelling할 때 이미 파싱한 context에서 같은 labelled tree를 만들어 앱에 전달한다. 별도 전체 DOM decode와 바이트·오류를 oracle로 대조했으며 일반 no-solve-block 경로의 중복 DOM 파싱을 제거했다. solve block은 기존 authoritative decode를 유지한다. 구체 적용 범위와 최종 성능·메모리 제한은 [0102](0102-display-preparation-cost.md) 및 아래 통합 검사에 기록한다.

## 최종 통합 검증

Windows/peets 각각 **workspace 862 + backend 2 = 864개 통과·실패 0개·36개 ignored**다. fmt·workspace all-target Clippy·backend library/test Clippy(`-D warnings`)·workspace 빌드와 원본 참조 saved/input 각각 38/38·차이 0을 확인했다.

양 OS 각각 native 13건·PDF 11건을 검사했다. MCDX **26개·XML 104파트**의 설치 XSD 오류는 0개다. 원본 표 계산·색상·64개 저장 결과·27개 XamlPackage 원문 보존 회귀를 유지했다. PDF **22개 모두 이미지 0개**이며 두 독립 reader로 검색 텍스트·단위·원본 표 기준점과 색상을 검사했다. 새 Try PDF는 OS별 2쪽, 복원 결과 5개, drawing 299개다. 원본 전체 조판·glyph·DPI·물리 프린터의 동등성은 미검증이다.

source tar는 ignored `mathcraft-try-control-20261019-r105.tar`, SHA-256 `c6188ccd11d16f3a250c9b66e285985c23789da2dd85b000938bc098a9301c6d`, 7,823,360 bytes다. 제품 Rust 119개·vendor Rust 5개를 포함한 manifest 142파일을 archive·양 호스트와 대조했다. peets root는 `/home/peets/Projects/mathcraft-try-control-20261019-r105`다. Windows release `target/release/mathcraft.exe`는 **22,123,520 bytes**, SHA-256 `4ab45a53a76b5886f27f99f3e3722ff5d3ec61623b6e9f8a6d336dac9bb0a564`다. 일반 앱 main은 실행하지 않았다.

`source-validation.json`에 소스/release/성능 identity, `schema-validation.json`과 `pdf-verified/validation.json`에 native/PDF 검사를 기록했다. `peets/verification.json`의 산출물 57개·표준 로그 5개·별도 actual-open 로그를 수집 후 size/SHA로 재검증했다. 표준 로그 prefix는 `try-control-`, 날짜 suffix는 `-20261019.log`다.

## 실제 입력·Enter의 r101→r105 비교

같은 PC에서 보존한 r101과 최종 r105 harness를 각각 5회 순서 교대했다. capture I/O를 끄고 실제 save→fresh open→empty→focus→첫 이벤트→Enter를 측정했다. 단위 ms, 각 열은 중앙값이며 합 열은 실행별 Text+Enter를 먼저 더한 값의 median이다. [0102](0102-display-preparation-cost.md)의 계측 baseline/candidate 비교와 별개인 최종 제품 source 비교다.

| 문서 | 첫 Text r101→r105 | Enter r101→r105 | 실행별 Text+Enter r101→r105 |
| --- | ---: | ---: | ---: |
| 4,000 scalar, cache 있음 | 2.383→2.264 | 48.448→42.447 | 50.871→44.542 |
| 4,000 scalar, cache 없음 | 2.176→2.248 | 48.299→42.963 | 50.438→45.366 |
| 100개 16×16 행렬, cache 있음 | 5.008→5.137 | 35.278→30.167 | 40.341→35.077 |
| 100개 16×16 행렬, cache 없음 | 7.756→7.858 | 39.474→34.633 | 47.453→42.491 |
| 원본 1015 | 0.466→0.467 | 3.828→3.294 | 4.274→3.739 |
| 원본 1016 | 0.488→0.498 | 3.605→3.274 | 4.093→3.768 |

전후 각 180case, 총 360case와 양 OS 별도 36case가 첫 이벤트 준비 0회·비편집 total 0회·수정 Enter 1회·draft/generation/global error 계약을 통과했다. CPU headless test profile 자료이며 실제 GUI/GPU 지연·전체 앱 시작 wall time·계산 완료 시간으로 해석하지 않는다. 36가지 case의 stimulus/Enter 72개 비교 중 26개 중앙값은 증가했으며 모든 조작이 빨라졌다고 주장하지 않는다.

원시 자료는 `try-control-open-input-alternating-20261019.{json,log}`다. frozen r101 SHA-256 `0808f7bf04d0891a976a583d8b4972875b6071b2eaf533e97fcf3761bdc53cf7`, r105 `d7d2cbecd4a50af429e392ee6e70a5cac2afef7de84f14613a4407d29d6e077a`이며 최종 현재 source의 app harness와 after가 같다.

악화된 중앙값(ms): `100-matrices16-with-cache/arrow-release` enter 4.766→4.867, `100-matrices16-with-cache/ime-commit` stimulus 4.961→5.228, `100-matrices16-with-cache/ime-preedit` stimulus 3.487→3.508, `100-matrices16-with-cache/paste` stimulus 4.822→4.912, `100-matrices16-with-cache/text` stimulus 5.008→5.137, `100-matrices16-without-cache/arrow-press` enter 7.251→7.508, `100-matrices16-without-cache/arrow-release` stimulus 2.479→2.803, `100-matrices16-without-cache/arrow-release` enter 6.948→7.338, `100-matrices16-without-cache/ime-preedit` enter 6.298→7.200, `100-matrices16-without-cache/paste` stimulus 7.499→7.838, `100-matrices16-without-cache/text` stimulus 7.756→7.858, `1015_LCC_S/ime-preedit` stimulus 0.449→0.476, `1015_LCC_S/ime-preedit` enter 0.435→0.451, `1015_LCC_S/text` stimulus 0.466→0.467, `1016_LCC_S/arrow-release` stimulus 0.246→0.254, `1016_LCC_S/ime-preedit` stimulus 0.451→0.466, `1016_LCC_S/ime-preedit` enter 0.437→0.453, `1016_LCC_S/text` stimulus 0.488→0.498, `4k-scalars-with-cache/arrow-press` enter 2.225→2.412, `4k-scalars-with-cache/ime-preedit` enter 2.711→3.326, `4k-scalars-without-cache/arrow-press` stimulus 0.837→0.874, `4k-scalars-without-cache/arrow-press` enter 2.291→2.307, `4k-scalars-without-cache/arrow-release` stimulus 0.765→0.804, `4k-scalars-without-cache/arrow-release` enter 2.050→2.142, `4k-scalars-without-cache/ime-preedit` enter 2.834→2.929, `4k-scalars-without-cache/text` stimulus 2.176→2.248.


## 준비 단계의 이동과 남은 지연

표시 tree decode를 계산 준비 쪽으로 옮겼으므로 표시 시간만 비교하지 않는다. 같은 실행에서 worksheet+display를 먼저 더한 중앙값(ms)은 다음과 같다. `try-control-open-input-phase-summary-20261019.{json,log}`가 360case의 phase 대응·binary/source identity를 확인한다.

| 문서 | worksheet+display r101→r105 |
| --- | ---: |
| 4,000 scalar, cache 있음 | 31.527→25.635 |
| 4,000 scalar, cache 없음 | 31.099→25.673 |
| 100개 16×16 행렬, cache 있음 | 17.241→12.447 |
| 100개 16×16 행렬, cache 없음 | 17.372→12.482 |
| 원본 1015 | 2.470→1.933 |
| 원본 1016 | 2.221→1.957 |

비편집 stimulus+Enter의 실행별 합 중앙값은 10개 case에서 증가했다. 가장 큰 증가는 cache 없는 행렬의 Arrow release 9.407→10.567ms다. 이 case들도 준비·계산은 0회이며 증가 원인은 아직 특정하지 않았다. 기록한 열기→Enter frame CPU 합은 6개 문서 중 5개에서 줄었지만 1016은 44.792→46.631ms로 증가했다. 이 합에는 fixture setup과 frame 사이 대기·프로세스 시작·GUI/GPU 시간이 포함되지 않는다. 전체 열기나 모든 입력이 개선됐다고 해석하지 않는다. phase summary의 scope 문자열은 r101→r105로 정정했으며 원시 측정·binary identity는 변경하지 않았다.

EA-079를 부분 구현으로 옮기고 EA-080의 10개 원본 단어 지원 경계를 갱신했다. 세부 553행은 부분 구현 138·미구현 415다. 일반 프로그램/선택 repair·상징/풀이·도표 등 남은 공백을 포함한 전체 20% 달성을 아직 판정하지 않는다.
