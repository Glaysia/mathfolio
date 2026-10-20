# 일반 연산자의 문자 범위 입력과 문서 열기

2026-10-19. [0118 원본 계약](0118-normal-infix-entry-contract.md)의 설치 CIL을 독립 Rust 입력에 연결한다. GUI·스크린샷·원본 실행·CLR/COM은 사용하지 않는다. r117 소스와 실행 파일·측정은 보존한다.

## 수식 중간에서 입력한다

기존 입력은 숫자 `1234`의 가운데에서 `+`를 눌러도 전체 숫자를 감싸 `1234+□`를 만들었다. Normal 텍스트 범위에서는 원본처럼 `12+34`로 분할하며, `1[23]4`에서 입력하면 선택한 글자만 제거해 `1+4`로 만든다. 분수·거듭제곱·곱셈·뺄셈·비교·논리 명령도 해당 범위를 사용한다. 완성된 두 조각이 있으면 커서는 오른쪽 조각의 시작에 둔다. 기존 구조에서의 컨테이너·우선순위와 새 연산자의 실제 fixity를 함께 사용한다.

Normal 전체 문자 선택은 원래 문자를 버리고 빈 연산자 틀로 교체한다. Scope 선택은 기존 식을 피연산자로 보존하는 별도 경로다. `-`는 Normal 시작·빈 자리에서는 음수, 두 피연산자 사이에서는 뺄셈이며 Scope의 방향별 계약을 별도로 유지한다. 기존 명시적 wrapper API와 자동 Scale 입력을 무조건 이 경로로 합치지 않는다.

실제 Text/Key CPU 앱 테스트에서 비교·논리 단축키도 같은 입력 경로를 쓰는지 확인한다. 수정 중에는 기존 계산 결과를 유지하며 평가 경계 Enter에서만 새 값과 의존 영역을 갱신한다. Undo/Redo와 자체 저장·MCDX 내보내기·재수입·명시 재계산을 검사한다. 원본에 없는 명령은 추가하지 않는다.

Ctrl+-의 원본 동작은 literal subscript 삽입 모드 전환이며, 원본의 전체 formatted 문자열 내 문자 위치를 이동시키지 않는다. 단일 base+suffix 모델에서 suffix 끝을 나갈 때는 같은 전체 이름 끝을 표현하는 NodeRight로 매핑한다. base 끝으로 잘못 돌아가 새 연산자를 거절하고 뒤 숫자를 이름에 합치던 회귀를 검사한다. suffix 중간을 무조건 이름 전체 끝으로 보내지 않으며 여러 rich run을 교대로 두는 이름의 편집 공백은 남아 있다. 실제 전체 앱 키 라우팅 검사에서도 확대율을 유지한다.

원본 전체 FormattedString lexer·모든 navigator/repair·Call callee의 APPLY/ARGUMENT_LIST 내부 재구성·모든 수치 표기/metadata 변환은 완성 범위에 포함하지 않는다. 지원하지 않는 조합은 다른 AST로 조용히 치환하지 않으며 후보 검증 실패 시 source·caret·history를 보존한다. 원본 실행의 수치 결과와 Undo 내부 저장 형태를 관찰했다고 주장하지 않는다.

## 문서 열기 비용

[0119](0119-native-open-cost.md)는 open·첫 페이지를 분해하고 패키지 재사용 후보를 독립 측정한 기록이다. 직접 native import가 이미 검증한 NativePackage를 같은 open 작업의 visual/settings·저장 표시 복원에 빌려준다. 자체 저장 파일도 visual 단계의 패키지를 restore에 재사용하며 지속 캐시는 추가하지 않는다. 새로 가져온 정상 표는 동일 defaults와 같은 parser로 이미 완전히 읽은 visual을 재사용한다. 편집된 자체 파일·local/alias 표·Err·예산 초과의 기존 refresh는 유지한다. notes importer·regions/cache XML 해석은 별도로 남아 있다.

기존 import 구현을 보존한 독립 oracle로 document·원본 archive bytes·좌표·모든 visual/색상/표/XAML·서식·saved cache/status/pending·오류와 무계산 열기를 대조한다. UTF16·namespace alias·열 font/defaults·폭·배경색·잘못된 표와 stale cache도 포함한다. 최종 소스에서 원본1015/1016 각각107개 region/visual과80개 saved row의 대조를 양 OS에서 추가 실행했다.

첫 페이지의 큰 비용은 glyph layout이다. 별도 단일 진단에서는 원본 두 문서에서497회·13.546/13.825ms, editor 생성은 .087/.107ms였다. 이를 모두 중복이나 제거 가능한 비용으로 분류하지 않는다. viewport 내용을 생략하거나 준비를 다음 프레임으로 옮기지 않았다. GUI/GPU·메모리 peak·원본 실행의 체감 성능은 이 CPU 측정으로 판정하지 않는다.

## 최종 통합 검증

Windows와 peets 각각 **workspace981 + backend2 = 983개 통과·실패0개·47개 ignored**다. 앱397개·editor181개를 포함한다. fmt·workspace all-target Clippy·backend library/test Clippy(`-D warnings`)·workspace build와 Windows release build가 통과했다. 원본 reference saved/input은 각각38/38·차이0이다. 일반 앱 main은 실행하지 않았다.

native17건/PDF15건/OS, MCDX34개·XML136파트 설치 XSD 오류0개, PDF30개 이미지0개다. 두 독립 reader로 검색 텍스트·단위·표 색상/기준점을 검사했다. 새 infix fixture는7개 query/result이며 자체 재열기 실행0회·Pending0개·MCDX 재수입 재계산 오류0개다. 새 PDF는 OS별2쪽·복원7개·drawing163개다. 기존 원본 표42개 편집 셀·80행 계산·64개 결과 표시·XamlPackage27개의 회귀도 유지한다.

동결 archive `mathcraft-infix-entry-20261019-r121.tar`는8,202,240bytes·SHA256 `faa6743cf5904e0acabdd604ff3a3b5ecfa5dabae1f6701ecf2a2c8f6aee1fe6`다. 모든 regular source181파일(제품 Rust138·vendor Rust5)을 archive/양 호스트와 대조했다. 초기 source manifest SHA는 `8e7bbc5b8cb68d5ca1da807cefae5d14031ebd58206951a0f01389197f7ada19`이며 최종 검증 메타데이터를 추가한 JSON과 구분한다. peets root는 `/home/peets/Projects/mathcraft-infix-entry-20261019-r121`다. 원격 산출물77개·표준 로그5개·별도36case profile을 size/SHA로 대조했다. 추가 독립 open oracle 로그도 양 OS의 최종 소스에 연결한다.

Windows release `target/release/mathcraft.exe`와 보존한 `mathcraft-infix-entry-r121-20261019.exe`는22,223,872bytes·SHA256 `349afcadaf632c59b338c8640a0b87ee8b039481164e1d16a9f51074e7da1940`다. 최종 보고서는 `artifacts/rea/infix-entry-integration-20261019/source-validation.json`, 그 아래 schema/PDF validation에 있다. 수집 tar는2,846,720bytes·SHA256 `47c74f5770cfa295fbf01075a4051d6cf74ad63d6fff784406e8d536c29144ad`다.

## 최종 r117→r121 열기·입력

동결된 같은 PC의 최적화 CPU 앱 harness를 각각5회 순서 교대했다. 아래는 ms·중앙값이다. 합은 각 실행의 구간을 먼저 더한 중앙값이며 별도 열의 중앙값을 더한 값이 아니다. source 준비/첫 페이지의 CPU 시간으로 프로세스/폰트 구성·fixture 생성·프레임 사이 대기·tessellation/GPU·전체 계산 완료 시간은 포함하지 않는다.

| 문서 | 열기 | 첫 페이지 | 열기+첫 페이지 | Text+Enter |
| --- | ---: | ---: | ---: | ---: |
| 4,000 scalar/cache |27.597→27.273|9.952→10.018|37.644→37.291|35.143→34.306|
| 4,000 scalar/no cache |14.494→14.386|10.246→9.987|24.798→24.804|35.407→34.121|
| 100개16×16/cache |17.153→17.952|15.610→17.003|32.763→35.624|25.671→26.213|
| 100개16×16/no cache |12.646→12.439|16.399→16.356|29.048→28.876|26.803→25.661|
| 원본1015 |19.250→16.446|23.312→24.107|42.565→39.727|2.565→2.724|
| 원본1016 |19.252→15.848|23.171→23.406|42.423→39.632|2.682→2.526|

| 문서 | Enter | worksheet+display | 기록한 열기→Enter CPU 합 | worker residual |
| --- | ---: | ---: | ---: | ---: |
| 4,000 scalar/cache |32.800→31.984|16.324→15.792|73.608→72.984|26.259→27.666|
| 4,000 scalar/no cache |33.052→31.837|16.527→15.640|61.372→59.750|28.582→26.993|
| 100개16×16/cache |20.833→21.033|6.504→6.172|58.766→63.285|11.615→12.248|
| 100개16×16/no cache |21.429→20.506|6.121→5.738|57.812→56.360|11.770→11.273|
| 원본1015 |2.103→2.228|.962→1.049|45.312→42.996|1.975→1.831|
| 원본1016 |2.188→2.076|.967→.934|45.614→42.573|1.675→1.659|

worker residual은 Enter와 동시에 끝난 계산을 제외하므로 Enter와 더해 완료 지연으로 부르지 않는다. 원본 열기는 줄었지만1015 입력과 cache 행렬의 첫 화면/전체 합 등 증가도 있다. 36개 이벤트 쌍 중16개 Text/이동/IME+Enter 중앙값이 증가했다. 모든 입력이 빨라졌다고 주장하지 않는다. 원시 값과 증가 전체 목록은 `infix-entry-open-input-alternating-20261019.{json,log}`·`infix-entry-open-input-phase-summary-20261019.{json,log}`에 보존한다.

행렬 cache 증가를 확인하려 같은 실행 파일을 추가5회씩 교대했으며 첫 비교를 덮어쓰지 않았다. 추가 비교에서 해당 열기+첫 페이지는33.613→33.684ms, Text+Enter는26.664→26.061ms였다. 첫 비교의2.861ms 증가가 이 추가 측정에서 그대로 반복되지는 않았다. 이를 원인이 해결됐다는 증거로 쓰지 않는다. 원본1015 Text+Enter 증가는 추가 비교에서도2.706→2.880ms였다. 추가 비교와 모든 다른 증가도 `infix-entry-open-input-confirmation-20261019.{json,log}`에 남긴다. 두 비교 총720case에서 draft/open 실행0·첫 이벤트 준비0·dirty Enter 준비1·generation/오류 계약을 통과했다. 별도 양 OS36case도 동일 계약을 확인했다.

before harness SHA는 `df7f803114021833686cd39c7754f2b0644d3a3339bddda672b579f9a256e7b6`, after는 `956afee2c8c456d78dccb0e3497724480ddb81e30e31f12198c8b854a81ac495`다. 큰 문서의 signature·직렬화·worksheet/표시 준비와 cold glyph 비용, 일반 formatted 이름·parent repair/삭제·전체 navigator는 후속 과제다. 세부553행 부분138/미구현415의 분류를 유지하며 이 작업이나 테스트 수를 전체20% 달성으로 환산하지 않는다.
