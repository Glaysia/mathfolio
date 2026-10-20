# 조건 분기 작성·native 왕복·원본 행 배치

2026-10-19. [0099 원본 계약](0099-conditional-branch-contract.md)의 IF pair-nary·BODY·Ctrl+J·행 배치와 [0100 XML escape 비용](0100-xml-escape-cost.md)을 연결했다. 원본 DLL·CoreLib의 정적 REA와 자체 Rust/headless 검사를 사용했다. GUI·스크린샷·원본 프로세스·어셈블리 실행은 사용하지 않았다. r98의 소스·실행 파일·측정은 별도로 보존한다.

## 작성과 저장

IF는 `branch_count`개 test/body 쌍, 선택적인 마지막 else 및 전체 `alternate` 플래그 하나다. 기존 자체 저장에 새 필드가 없으면 1쌍·ordinary로 읽는다. branch 수·포화 arity·tree node/depth·body 구조를 검증한다. native는 `<elseif>` 또는 `<alsoif>`로 후속 쌍을 기록하고 마지막 `<else>`를 구분한다. scalar then/else는 원본 reader/descriptor repair처럼 한 statement BODY로 감싸 canonical `<program>`으로 저장한다. 원래 중첩된 program을 무조건 flatten하지 않는다.

원본 reader는 mixed elseif/alsoif에서 하나라도 alsoif면 전체 alternate=true로 정규화한다. native 편집 import도 이를 따른다. 계산 준비는 정규화한 편집 tree를 직렬화한다. 기존 무편집 native archive의 export passthrough는 원문 보존 정책을 유지하므로, 모든 load/save가 혼합 원문을 다시 쓴다는 주장은 하지 않는다. 직접 raw 계산 XML의 기존 mixed 거절 정책은 유지하며 native import를 거쳐 정규화한 경로와 구분한다. 전체 XML attribute·알 수 없는 namespace·모든 원본 오류 복구까지 수용하는 것은 아니다.

리본 `if`는 빈 자리에 최소 test/body 한 쌍을 생성한다. 기존처럼 else까지 임의로 생성하지 않는다. 별도의 else if/also if/else는 확인된 IF 오른쪽 또는 미완성 IF 바로 뒤의 Program 빈 문장 경계에서 삽입하며, 그 빈 문장만 소비하고 나머지 앞뒤 문장을 보존한다. middle operator 선택에서는 IF 전체 ordinary/alternate 모드를 바꾼다. 중간 mode 혼합·중복 else·잘못된 위치는 원문·redo를 바꾸지 않는다. 일반 populated container split/merge·모든 ancestor repair는 미완료다.

Ctrl+J는 기존 5종에 `if`를 추가했다. 기본 설치 .NET 8 Dictionary의 삽입·열거 순서와 fresh descriptor table 초기화를 추적해 IF가 IF_THEN보다 먼저 선택됨을 확인했다. `else`·`elseif`·`alsoif`는 확인된 인접 Program 문장 경계에서 이름→placeholder→branch repair를 한 Undo로 묶는다. Normal mode·HasNoLabel·정확한 전체 formatted 이름·suffix·문자 선택 조건은 공유한다. 실패 시 먼저 단어를 지우지 않는다. try와 일반 위치의 branch keyword 변환은 미완료다.

## 원본 행 배치와 입력 연결

원본 VisualPairNaryNode처럼 if/else if/also if와 test를 같은 baseline에 놓고 BODY를 다음 행에 들여 쓴다. else는 자체 header 다음에 BODY를 놓는다. XML의 `then`을 화면 단어로 출력하던 기존 경로를 제거했다. operator별 hit rectangle과 part를 보존해 실제 pointer 클릭과 리본 전체 모드 변경을 연결한다. 괄호 변환·확대에서도 hit와 caret가 함께 이동한다.

PROGRAM/BODY의 원본 왼쪽 double vertical glyph와 오른쪽 minor fence의 차이를 반영했다. 왼쪽은 STIX와 독립 벡터 두 stroke의 근사이며 원본 글꼴 glyph/정확한 stroke metrics는 미완료다. BODY는 오른쪽 minor fence만 생략한다. pair-nary 오른쪽 minor fence의 0.3 opacity를 구분한다. 행별 간격·일부 기존 for/while 조판과 전체 Prime 렌더링 동등성은 남는다.

전체 키 이벤트·ribbon·pointer 검사는 수식 draft 중 기존 계산 결과를 유지하며 Enter로 확정하는지 확인한다. ordinary→alternate 변경은 1을 유지하다가 Enter 후 111이 된다. Undo도 편집 상태와 확정 경계를 구분한다. native probe는 5개 영역에서 1, 111, 43, 73 V, 7을 확인하고 saved reopen의 계산 0회/표시 5개 복원을 검사한다.

## 비용과 남은 범위

escape의 다섯 번 연속 replace를 한 번의 문자열 순회로 바꿨다. 이름·suffix·라벨·숫자·imaginary symbol의 최종 XML을 legacy helper에 바이트 단위로 대조한다. 미세 측정과 실제 captured serializer, cold/warm builder의 개선·악화를 [0100](0100-xml-escape-cost.md)에 함께 기록한다. 전체 Enter 효과는 최종 같은 source harness의 r98→r101 비교로 별도 확인한다.

## 최종 통합 검증

Windows/peets 각각 **workspace 822개 + backend 2개 = 824개 통과·실패 0개·33개 ignored**다. app 346·compat 284·document 70·editor 109·core 13개이며 fmt·workspace all-target Clippy·별도 backend library/test Clippy(`-D warnings`)·workspace 빌드를 통과했다. 원본 참조 saved/input 각각 38/38·차이 0이다.

양 OS 각각 native 12건·PDF 10건을 검사했다. MCDX **24개·XML 96파트**의 설치 XSD 오류는 0개다. 원본 표 계산·색상·64개 저장 결과·27개 XamlPackage 원문 보존 회귀를 유지했다. PDF **20개 모두 이미지 0개**, 두 reader에서 검색 텍스트·원본 표 숫자/색상/기준점을 확인했다. 새 조건문 PDF는 OS별 2쪽·복원 결과 5개·drawing 218개이며 visible then이 없다. 원본 전체 배치·폰트 glyph·DPI·물리 프린터의 동등성은 미검증이다.

source tar는 ignored `mathcraft-conditional-branches-20261019-r101.tar`, SHA-256 `c8c15d671a9728e3f0dfa162c1c386c67c8d7096b22109e6f85662cc66b9e078`, 7,700,480 bytes다. 제품 Rust 114개·vendor Rust 5개를 포함한 manifest 137파일을 archive·양 호스트와 대조했다. peets root는 `/home/peets/Projects/mathcraft-conditional-branches-20261019-r101`다. Windows release `target/release/mathcraft.exe`는 **22,081,536 bytes**, SHA-256 `8434a844dff41f6b76613d8ab54ada43673611eca72694e712ea06da066cbd92`다. 일반 앱 main은 실행하지 않았다.

`source-validation.json`에 소스/release/성능 identity, `schema-validation.json`과 `pdf-verified/validation.json`에 native/PDF 검사를 기록했다. `peets/verification.json`의 산출물 52개·표준 로그 5개·별도 실제 열기 로그는 수집 후 size/SHA를 재검증했다. 표준 로그는 `conditional-branches-{windows-fmt,windows-clippy,windows-tests,backend-windows,backend-windows-clippy,windows-build,windows-reference,windows-probes,peets-check,peets-backend-clippy,peets-reference,peets-probes,peets-pdfs}-20261019.log`다.

## 실제 입력·Enter의 r98→r101 비교

같은 PC에서 보존한 r98과 최종 r101 harness를 각각 5회 순서 교대했다. capture I/O는 끄고 실제 save→fresh open→empty→focus→첫 이벤트→Enter를 측정했다. 단위 ms, 각 열은 중앙값이며 합 열은 실행별 Text+Enter를 먼저 더한 값의 median이다. 구 r94→r98 결과와 합치지 않는다.

| 문서 | 첫 Text r98→r101 | Enter r98→r101 | 실행별 Text+Enter r98→r101 |
| --- | ---: | ---: | ---: |
| 4,000 scalar, cache 있음 | 2.472→2.292 | 56.137→49.928 | 58.404→52.768 |
| 4,000 scalar, cache 없음 | 2.505→2.665 | 56.264→50.175 | 58.392→53.138 |
| 100개 16×16 행렬, cache 있음 | 4.910→5.160 | 43.224→36.036 | 48.065→41.196 |
| 100개 16×16 행렬, cache 없음 | 8.806→9.009 | 47.946→41.262 | 56.577→49.763 |
| 원본 1015 | 0.481→0.475 | 4.083→3.928 | 4.564→4.388 |
| 원본 1016 | 0.512→0.570 | 4.369→4.041 | 4.929→4.635 |

첫 Text 자체는 일부 증가했다. 전체 36종의 이벤트+Enter 합 중 7종도 증가했다. 예를 들어 cache 없는 행렬의 IME preedit는 13.239→14.815 ms, cache 없는 4k의 preedit는 5.313→5.822 ms다. 이들은 준비·실행 0회를 유지한 비편집 사례이며 원인은 미확정이다. 원본 1016의 기록한 open~Enter CPU 단계 합도 48.894→49.866 ms로 증가했다. 그 합에는 font setup·fixture 생성·caret helper·대기·worker remaining이 없으며 전체 앱 시작 시간이 아니다.

전후 각 180case, 총 360case의 counter/draft/generation/global error 계약 위반은 0개다. 첫 이벤트 준비 0회·비편집 total 0회·수정 Enter 1회를 유지했고 양 OS 별도 36case도 통과했다. CPU headless test profile 자료이며 실제 GUI/GPU 지연·전체 앱 시작 wall time·계산 완료 시간으로 해석하지 않는다. 악화된 표본도 그대로 기록하고 모든 문서가 빨라졌다고 주장하지 않는다.

원시 자료는 `conditional-branches-open-input-alternating-20261019.{json,log}`, runner는 `measure-conditional-branches-open-input-20261019.py`다. frozen r98 SHA-256 `af0569c3ce8335a6999a88a77656744ca2a8812a8e1ff3f66ee779e16175fe2d`, r101 `0808f7bf04d0891a976a583d8b4972875b6071b2eaf533e97fcf3761bdc53cf7`이며 최종 현재 source의 app harness와 after가 같다.


기능 목록의 기존 EA-071/072/073/080 범위를 갱신했다. 부분 구현 137·미구현 416, 총 553행이며 전체 완성률로 환산하지 않는다. 전체 프로그램 편집·상징/풀이·도표 등 남은 공백을 포함한 20% 달성은 아직 입증하지 않았다.

## Enter 준비 단계와 남은 병목

같은 360case의 PREP/OPEN 기록을 fixture·이벤트·실행 순서로 대조했다. 단위 ms, 각 열은 중앙값이며 더해서 전체 median으로 쓰지 않는다.

| 문서·버전 | 서명 | 직렬화 | worksheet | 표시 준비 | 준비 전체 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 4k cache r98 | 7.606 | 9.929 | 21.408 | 14.173 | 53.700 |
| 4k cache r101 | 7.415 | 7.768 | 18.901 | 14.046 | 47.626 |
| 행렬 cache r98 | 6.249 | 15.138 | 7.454 | 10.220 | 40.288 |
| 행렬 cache r101 | 5.944 | 8.388 | 7.368 | 10.152 | 33.227 |
| 원본 1016 r98 | 0.363 | 0.777 | 1.856 | 0.847 | 3.984 |
| 원본 1016 r101 | 0.364 | 0.584 | 1.709 | 0.838 | 3.636 |

직렬화 감소는 독립 escape 측정과 방향·규모가 일치한다. 4k의 worksheet 감소와 달리 표시 준비 약 14 ms와 서명 비용은 남는다. 표시 준비는 source 보존·geometry snapshot·DOM·tree decode/검증을 포함하므로 원인을 더 나눠야 한다. worker remaining은 frame과 겹친 부분을 포함하지 않으므로 전체 계산 완료 시간으로 합산하지 않는다. 별도 요약은 ignored `conditional-branches-open-input-phase-summary-20261019.json`이다.
