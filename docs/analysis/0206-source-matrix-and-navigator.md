# 원본 입력 행렬 계산과 navigator 내부 resize

2026-10-20. r221의 제한된 기능·검증 기록이다. 테스트 수를 완성률로 환산하지 않는다.

## 원본과 구현 범위

[원본 source/result 형식 구분](../../artifacts/rea/source-matrix-format-next-20261020/README.md)은 source matrix의 첫 `<ml:display offset="r,c" size="rows,cols"/>`와 뒤따르는 전체 column-major 셀을 확인한다. source display에는 결과의 Show/Origin 필드가 없다. [source editor 계약](../../artifacts/rea/source-matrix-editor-viewport-contract-next-20261020/README.md)은 원본의 전체 source AST·보이는 셀 배열·caret/offset mutation 경로를 별도로 기록한다. 모두 설치 IL/BAML/XSD의 정적 근거이며 원본 실행 관찰이 아니다.

이번 계산기 변경은 raw Math50 source의 첫 display를 엄격히 분리하고, 숨겨진 셀까지 전체 계산한다. display 없는 기존 경로에는 새 전체 태그 순회를 추가하지 않았다. display가 뒤에 있거나 foreign namespace·잘못된 범위/속성/구조가 있으면 거절한다. 기존16384셀 제한은 유지한다. typed source viewport IR/렌더러는 아직 연결하지 않았으므로 해당 문서는 Opaque/read-only source를 보존한다. 일반 Matrix 저작·결과 formatting과 혼동하지 않는다.

실제 App으로 plain 및 metre source를 명시적으로 계산하고 전체2x3 비대칭 값을 검사했다. metre source의 canonical SI 값과 cm output의100배 display 값을 구분하며, own/native cache는 전체 display carrier임을 확인한다. 저장·cold reopen에서 추가 실행이나 이중 cm 변환이 없고 source XML·모든 숨겨진 셀이 남는다. 숨겨진 셀의 오류/단위 불일치도 성공한 부분 결과로 덮지 않는다. 새 source packet은 MCDX4·own2·observation1이며 source PDF/편집 가능성을 주장하지 않는다.

[navigator resize 계약](../../artifacts/rea/matrix-navigator-resize-contract-next-20261020/README.md)과 [첫 개별 서식 clone](../../artifacts/rea/matrix-resize-first-local-clone-next-20261020/README.md)은 내부8개 edge/corner·최초 margin snapshot·양 축 원자 갱신·Size 다음 Offset 명령을 확인한다. 내부 resize를 해당 full pair로 연결하고 상속 요청값의 size/offset/show가 확인된 첫 개별 서식을 복사한다. 요청30x30이 실제20x20으로 clamp된 사례에서도 navigator가 보낸 offset0,0을 명시해 오래된7,8이 재적용되지 않게 한다. 기존 imported sparse DTO 및 outer/pan 정책은 별도다. 모션 없음/원위치 복귀는 history를 만들지 않고 source/owner/scale 변경·Esc·capture loss는 완료되지 않은 편집을 취소한다. 원본 Escape focus route·중첩 전환 전체 및 독립 UI sizing/cancel policy의 동일성은 미확인이다.

[retained 비용 감사](../../artifacts/rea/matrix-retained-cost-next-20261020/v1/AUDIT.md)에 따라 성공한 source authority와 같은 window+MathFormat key에는 preflight 숫자 포맷을 재실행하지 않는다. Show 전환144셀 재포맷과 Preview182셀 이후 Commit182셀 재포맷을 제거했다. 새 source/크기/스타일/오류 회복은 다시 검증한다. 기존 formatted-cell counter의 red/green 검사이며 FPS·GPU·실제 GUI 지연 개선률로 환산하지 않는다.

## 검증

[채택 receipt](../../artifacts/rea/source-matrix-navigator-r221-integration-20261020/validation.json): 같은683파일의 Windows 2131·peets 2132 정상 검사(backend2 포함), ignored 103/101, 각48harness, fmt·workspace/backend Clippy·build·reference38·Windows release 통과. [빌드 파일](../../artifacts/rea/source-matrix-navigator-r221-integration-20261020/windows/mathcraft.exe)은 실행하지 않았다.

양 OS packet 합계 MCDX20·own12·PDF8·선택 Mathcad XML part의 XSD120건이다. source packet은 위2x3 plain/unit 사례이고, 기존 asymmetric 큰 결과 및 live ORIGIN 인덱스 packet을 함께 재생성했다. 기존8 PDF는 fitz/pypdf 숫자/crop과 vector paths·image0을 확인했다. 새 source viewport의 PDF 또는 원본 전체 인쇄 동일성 검증으로 세지 않는다. r216의192MCDX/120PDF 전체 packet은 재생성하지 않았다.

[독립 source 검사기](../../artifacts/rea/source-matrix-display-validator-r221-20261020/README.md)와 [15개 오류 복사본 검토](../../artifacts/rea/source-matrix-display-validator-final-review-next-20261020/README.md)는 전체 source·원본 보관 archive·own signature/query 역할·숫자 type·전체 cache·OPC refs/status를 검사한다. 초기 검사기가 허용한 단위 AST/Boolean/정의 wrapper/VARIABLE 라벨 오류는 이전 결과를 보존하고 보강했다. 현재 checker는 양성 및 XSD24개를 통과하고15개 부정 사례를 모두 거절한다. SI 및 실행 횟수는 실제 Rust producer assertion 근거이며 파일 검사기가 source를 재실행한 것은 아니다.

archive SHA `9b7c1ae94927868083820c0b460c4bb67107d6615c5093e66b7a2949daa60a47`; manifest SHA `e06cc3b86b1f5d0aaaa4a7e880ec36edcee63ace97b2c48474c6ca9b1686f4b6`. 문서 갱신은 채택 후 별도 receipt로 보관한다. source viewport 편집/조판/경계 caret·전체 nested/Area·unknown cold ORIGIN·전체 원본 형식/print 계층·체감 지연은 남아 있다.555행(부분159/미구현396),87가족(구현4/부분69/미구현14)을 유지한다. GUI·스크린샷·원본 CLR/Mathcad 실행은 사용하지 않았다.
