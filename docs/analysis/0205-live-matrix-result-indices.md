# 계산된 행렬 결과 인덱스와 개별 서식

2026-10-20. r220의 제한된 지원 범위와 양 OS 검증 기록이다. 완성률로 환산하지 않는다.

## 원본 근거

[결과별 ORIGIN](../../artifacts/rea/matrix-result-origin-next-20261020/REPORT.md), [리본 명령](../../artifacts/rea/matrix-show-indices-next-20261020/README.md), [커서 대상 선택](../../artifacts/rea/matrix-indices-context-next-20261020/README.md), [Clone/serializer 및 source 행렬 구분](../../artifacts/rea/source-matrix-format-next-20261020/README.md)을 사용한다. 설치 managed IL·BAML·XSD의 정적 근거이며 원본 실행 결과가 아니다. 원본은 source MatrixNodeInfo와 ResultMatrixFormat을 커서에 따라 구분하고, 첫 개별 result 명령은 상속받은 요청 서식을 복사한다. 확인한 MatrixFormat writer는 Origin을 쓰지 않는다. shared engine context와 임의 native 문서의 ORIGIN 실행 순서 전체를 입증한 것은 아니다.

## 연결한 동작

Rust 계산기가 실제로 성공한 행렬 결과의 Scope에서 SYSTEM ORIGIN을 캡처하고 재사용 행과 함께 유지한다. 실패·Pending은 이를 현재 결과로 게시하지 않는다. App의 invalidation이 callback보다 앞서도 이전 행의 origin으로 명령을 켜지 않는다. 저장 cache/Value/문서 원문에 새 origin 필드를 추가하지 않았다.

root Evaluate의 완전한 flat 결과와 확인된 서식/ORIGIN에 리본 표시·숨기기를 연결했다. 입력 행렬 내부 커서·셀 선택·다른 owner·진행 중 drag는 제외한다. 문서 source commit이나 계산 요청을 하지 않고 한 결과의 view history만 추가한다. 첫 개별 Show 변경은 확인된 요청 size/offset/show를 materialize하며 실제20x20으로 clamp된 결과 대신 상속된 요청30x30을 유지하는 회귀 검사가 있다. 기존 imported sparse DTO는 이 명령이 자동 정규화하지 않는다. 첫 resize의 전체 Clone 동일성까지 주장하지 않는다.

표시 축에 숨겨진 셀이 있을 때 ORIGIN+offset의 라벨과 시작/끝점을 조판한다. Show=false라도 수식 focus 중 임시 라벨을 그린다. focus 변경 시 source caret/IME·값·geometry가 바뀌지 않는 범위를 검사한다. 안전한 i32/heading budget와 고정 gutter는 독립 정책이며 원본 unchecked overflow·폰트·print 배치 동일성을 주장하지 않는다.

PrintSnapshot은 원래 숫자 projection과 확인된 live 인덱스 token을 함께 고정해 측정·paint에 사용한다. 실제 source에 ORIGIN:=1000, M:=identity(20), M=를 작성하고 Rust로 한 번 계산한 후 show/hidden 저장·PDF·cold reopen을 검사했다. 각 PDF144셀(1은12/0은132), 표시 상태26개 인덱스·숨김0개를 fitz/pypdf의 query crop과 전체 텍스트 양쪽으로 대조했다. 위쪽 원문1000과20은 별도 crop에서 센다. identity는 대칭이므로 전치 검증으로 세지 않으며 이전 asymmetric packet을 별도로 다시 검증했다.

## 채택 검증과 제한

[검증 receipt](../../artifacts/rea/matrix-indices-r220-integration-20261020/validation.json): 동일671파일로 Windows 2093·peets 2094 정상 검사(각 backend2 포함), ignored 102/100, 각48harness, fmt·workspace/backend Clippy·build·reference38·release(Windows)를 통과했다. 새 packet 합계 MCDX12·own8·PDF8·XSD72parts다. PDF는 전부 벡터 paths가 있고 bitmap image0개이며 픽셀 비교는 하지 않았다. [Windows 실행 파일](../../artifacts/rea/matrix-indices-r220-integration-20261020/windows/mathcraft.exe)은 빌드만 했다.

archive SHA `b7112f8905bea8cf68de2b473bc2358174ff6938c4c2aa83dac2a8a28249aef1`, source manifest SHA `7d91baf5cf968a210302485d12d50c91541bd0d35b58b082b323c548482be01f`. 문서 갱신은 검증 후 별도 receipt로 보관하므로 archive의 당시 문서는 그 시점 이력이다. r216의192MCDX·별도10XML·120PDF 전체 packet은 재생성하지 않았다.

r219 첫 전체 gate는 양 OS에서 리본 fixture1개가 실패해 채택하지 않았다. [clock 교정](../../artifacts/rea/matrix-indices-ribbon-clock-r220-20261020/receipt.json)은 테스트가1초 press를 클릭으로 기대했으나 pinned egui max_click_duration이0.8초였음을 기록한다. 또한 [Pending collector 진단](../../artifacts/rea/matrix-origin-callback-layout-next-20261020/v3/REPORT.md)은 실제 조판에 라벨이 모두 있지만 이전 Context response rect로 수집한 오류를 구분한다. 제품 label golden/첫 프레임 조건을 줄이지 않았다. [print focus fixture](../../artifacts/rea/matrix-live-indices-print-packet-v2-next-20261020/README.md)는 정상 worksheet-background focus와 실제 source/IME focus를 구분한다. 이전 실패 파일은 보존한다.

원본/GUI/CLR/WPF·스크린샷은 실행·사용하지 않았다. cold native/own에는 결과별 ORIGIN의 검증된 authority가 없어 라벨을 추정하지 않는다. source 행렬 편집/viewport, Area/nested 결과, 전체 ribbon 다중선택·원본 print hierarchy·첫 resize Clone·체감 지연은 미완료다. 이번 단계는 EA-066만 부분 구현으로 올려555행(부분159/미구현396), 상위87가족(구현4/부분69/미구현14)을 유지한다. 테스트 수는40% 달성 근거가 아니다.
