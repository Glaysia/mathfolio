# 프로그램 제어문 작성과 cold 워크시트 준비

2026-10-19. 원본의 프로그램 제어문 입력은 [0096](0096-program-control-authoring-contract.md), 준비 비용의 원인 분해는 [0095](0095-worksheet-preparation-cost.md), 실제 최적화와 fallback 경계는 [0097](0097-cold-worksheet-preparation.md)에 기록했다. GUI·스크린샷·원본 프로세스·어셈블리 실행 없이 설치본 REA와 자체 Rust 테스트를 사용했다.

## 원본 입력·저장·계산 연결

Ctrl+J는 정확한 소문자 전체 식별자 `while`, `for`, `return`, `break`, `continue`를 구조로 바꾼다. 원본의 명시 라벨·contextual/None·literal suffix·선택 모드 조건을 구분한다. while/for/return은 첫 placeholder로, 자식이 없는 break/continue는 새 leaf 오른쪽으로 이동하며 한 번의 Undo로 복원한다. 원본의 나머지 if/else/elseif/alsoif/try 및 pair repair는 미완료다. 지원하지 않는 단어를 임의의 구조로 바꾸지 않는다.

break/continue는 원본처럼 자식 없는 노드와 `<ml:break/>`·`<ml:continue/>`로 저장한다. 리본의 빈 placeholder 삽입을 연결했으며, 이미 값이 있는 위치의 일반 삽입·분할은 완료로 세지 않는다. 원본에서 확인되지 않은 루프 내부 전용 편집 제한은 추가하지 않았다. 잘못된 계산 위치는 계산 오류로 처리한다. leaf 오른쪽 Backspace와 왼쪽 Delete는 그 자리를 placeholder로 바꾸고 한 Undo로 복원한다. 반대 방향의 이웃/줄 병합은 별도 미완료다.

실제 전체 키 라우팅에서 modifier·key release·라벨·Undo/Redo·focus를 검사한다. 수식 작성 중에는 기존 결과를 유지하고 Enter로 확정한다. continue를 break로 수정하는 사례는 결과 12를 유지하다가 확정 후 3으로 바뀐다. 반복문·중첩 반복문·while·return 및 단위 반환을 native 저장→재수입→재계산에 연결했다. 새 probe는 5개 영역·질의 5개에서 12, 3, 8, 3, 7 V를 검사하며 자체 저장 재열기는 계산 0회로 결과 5개를 복원한다.

프로그램 descriptor precedence도 원본 상수에 맞췄다. PROGRAM/WHILE_LOOP/FOR_LOOP/IF는 290, RETURN은 40, LOCAL_ASSIGNMENT는 41이다. For/While의 descriptor 이름은 원본 FOR_LOOP/WHILE_LOOP이며 자체 저장 enum 이름은 유지한다. 모든 파서·입력 모드의 원본 동등성을 주장하지 않는다.

## 버벅임 원인과 수정

최초 Enter의 worksheet/라벨 단계에서 XML 조각별 wrapper 파싱과 출력 문자열 재조립이 반복됐다. 빈 cache의 작은 조각이 많은 경우에만 공유 context batch로 한 번 파싱하고, label patch는 해당 context 범위만 복사한다. 출력은 큰 임시 wrapper 문자열을 만들지 않고 직접 조립한다. 기존 cache가 있는 warm 경로는 유지한다.

적용 조건은 256개 이상·평균 조각 1 KiB 이하·최대 조각 8 KiB 이하·batch 2 MiB 이하 및 node/reserve 제한이다. 조건·XML·context 범위를 충족하지 않으면 기존 경로로 돌아간다. malformed 입력의 첫 오류·부분 cache·전역 정의·namespace·Unicode·좌표/생성 metadata와 최종 bytes를 기존 builder oracle에 대조한다. 보존 cache payload는 동일하지만 공유 DOM의 일시 메모리 증가 가능성은 남는다.

동일 PC의 recorder 비활성 5회 교대 측정에서 cache 있는 4,000영역 cold builder는 28.376→21.794 ms, 없는 경우 27.519→21.292 ms였다. 큰 행렬에 무조건 batch를 적용해 느려진 후보는 제외했다. 원본 1016의 1.804→1.814 ms 차이는 개선으로 세지 않는다. 이는 builder 구간이며 앱 전체 Enter나 GUI 체감 속도가 아니다. source label walk와 표시 준비 등의 비용은 남는다.

## 통합 검증

Windows/peets 각각 **workspace 791개 + backend 2개 = 793개 통과·실패 0개·31개 ignored**다. app 340·compat 274·document 70·editor 94·core 13개이며 fmt·workspace all-target Clippy·별도 backend Clippy(`-D warnings`)·workspace 빌드를 통과했다. 원본 참조는 saved/input 양쪽 38/38·차이 0이다.

양 OS 각각 native 11건·PDF 9건을 검사했다. MCDX **22개·XML 88파트**의 설치 XSD 오류는 0개다. 원본 표 계산·색상·64개 저장 결과와 27개 XamlPackage 원문 보존 회귀도 유지했다. PDF **18개 모두 이미지 0개**, 두 reader에서 검색 텍스트·원본 표 숫자/색상/기준점을 확인했다. 새 프로그램 PDF는 OS별 2쪽·복원 결과 5개·drawing 178개다. 원본 전체 배치·모든 글꼴·물리 프린터 동등성은 미검증이다.

source tar는 ignored `mathcraft-program-controls-20261019-r98.tar`, SHA-256 `dd484fe491cc8ed3c3973fb17bb512570e478bdc64f471c434c9560c658c0b79`, 7,608,320 bytes다. 제품 Rust 110개·vendor Rust 5개를 포함한 manifest 133파일을 archive·양 호스트와 대조했다. peets root는 `/home/peets/Projects/mathcraft-program-controls-20261019-r98`다. Windows release `target/release/mathcraft.exe`는 **22,050,304 bytes**, SHA-256 `aece40fecd31dc2d5b289dec718f28a6ade193175e310a9d0e5204e0cea8a9d0`다. 일반 앱 main은 실행하지 않았다.

검증 root의 `source-validation.json`은 소스·release·성능 identity를, `schema-validation.json`과 `pdf-verified/validation.json`은 native/PDF 검사를 기록한다. `peets/verification.json`의 산출물 47개·표준 로그 5개·별도 실제 열기 로그는 수집 후 size/SHA를 재검증했다. 표준 로그는 `program-controls-{windows-fmt,windows-clippy,windows-tests,backend-windows,backend-windows-clippy,windows-build,windows-reference,windows-probes,peets-check,peets-backend-clippy,peets-reference,peets-probes,peets-pdfs}-20261019.log`다.

## 실제 입력·Enter의 r94→r98 비교

같은 PC에서 보존한 r94와 최종 r98 harness를 5회씩 순서 교대했다. 기존 r89→r94 표와 섞지 않고 이번 변경 전후만 비교한다. 각 실행은 실제 save→fresh open→empty→focus→첫 이벤트→Enter이며 capture I/O는 끈다. 단위 ms, 각 열은 중앙값이다. 합 열은 각 실행의 Text+Enter를 먼저 더한 값의 median이다.

| 문서 | 첫 Text r94→r98 | Enter r94→r98 | 실행별 Text+Enter r94→r98 |
| --- | ---: | ---: | ---: |
| 4,000 scalar, cache 있음 | 2.484→2.724 | 61.947→55.662 | 64.471→58.583 |
| 4,000 scalar, cache 없음 | 2.331→2.467 | 61.909→55.473 | 64.375→57.878 |
| 100개 16×16 행렬, cache 있음 | 5.184→5.316 | 44.786→44.982 | 49.915→51.402 |
| 100개 16×16 행렬, cache 없음 | 8.446→9.491 | 49.767→48.796 | 59.394→59.412 |
| 원본 1015 | 0.495→0.543 | 4.319→4.448 | 4.859→4.991 |
| 원본 1016 | 0.484→0.569 | 4.098→4.434 | 4.593→5.162 |

행렬의 Text+Enter 합은 거의 같거나 증가했고 원본 1015/1016도 이 표본에서는 증가했다. 전체 문서에서 일관되게 빨라졌다고 결론내리지 않는다. batch가 적용되지 않는 경로의 작은 차이를 최적화 효과로 주장하지 않으며, 4k Enter의 감소와 별도로 남은 비용을 추적한다.

전후 각 180case에서 계약 위반·global error는 0개다. 첫 이벤트 준비 0회·비편집 total 0회·수정 Enter 1회를 유지한다. 양 OS 별도 36case에서도 동일하다. CPU headless test profile 자료이며 GUI/GPU frame rate·앱 시작 전체 wall time·계산 완료 시간과 다르다. label walk·표시 준비 등 나머지 확정 비용과 전체 프로그램 저작은 남아 있다.

원시 자료는 `program-controls-open-input-alternating-20261019.{json,log}`, runner는 `measure-program-controls-open-input-20261019.py`다. r94 SHA-256은 `5a133ab4715dcd3697539727fae6caba24881a2c089a988ef2e491c5aa87c255`, r98은 `af0569c3ce8335a6999a88a77656744ca2a8812a8e1ff3f66ee779e16175fe2d`이며 각 frozen harness와 현재 source harness의 identity를 확인했다. r93/r94 기존 측정은 보존했다.


EA-076/077의 입력·삭제 범위와 EA-080의 지원 5개 키워드를 갱신했다. 세부 553행은 부분 구현 137·미구현 416이며 기능의 깊이·원본 완성도를 항목 비율로 환산하지 않는다. 이번 범위만으로 전체 20% 달성을 판정하지 않는다.

## Enter 준비 단계의 남은 비용

같은 360case의 phase 기록을 fixture·이벤트·실행 순서로 대응시켰다. 단위 ms이며 각 열의 중앙값을 더해 전체 median으로 쓰지 않는다.

| 문서·버전 | 서명 | 직렬화 | worksheet | 표시 준비 | 준비 전체 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 4k cache r94 | 7.727 | 10.214 | 27.822 | 13.651 | 59.769 |
| 4k cache r98 | 7.758 | 10.067 | 20.817 | 13.939 | 53.284 |
| 행렬 cache r94 | 6.808 | 15.558 | 7.879 | 10.371 | 41.834 |
| 행렬 cache r98 | 6.291 | 16.031 | 7.851 | 10.430 | 41.493 |
| 원본 1016 r94 | 0.377 | 0.805 | 1.771 | 0.687 | 3.680 |
| 원본 1016 r98 | 0.387 | 0.849 | 1.826 | 0.802 | 3.840 |

4k worksheet 단계의 감소는 독립 builder 측정과 방향이 일치한다. 표시 준비 약 14 ms, 직렬화 약 10 ms와 서명 약 8 ms가 남는다. 원본 1016은 worksheet·표시·직렬화의 증가가 관측됐으며 원인을 확정하거나 잡음이라고 단정하지 않는다. Enter와 겹쳐 실행되는 worker remaining도 원시 JSON에 남기고 전체 계산 시간으로 합산하지 않았다. 별도 요약은 ignored program-controls-open-input-phase-summary-20261019.json이다.
