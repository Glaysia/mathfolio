# Typed source 행렬의 표시 범위·편집·인쇄

2026-10-20. r222는 source 행렬을 Opaque로만 계산하던 r221 뒤의 제한된 입력/저장 통합이다. 완성률로 환산하지 않는다.

## 원본과 연결한 범위

[source 형식](../../artifacts/rea/source-matrix-format-next-20261020/README.md), [source caret/format](../../artifacts/rea/source-matrix-editor-viewport-contract-next-20261020/README.md), [control 알림](../../artifacts/rea/source-matrix-notification-next-20261020/README.md), [dirty/계산 경계](../../artifacts/rea/source-matrix-subscriber-next-20261020/README.md)의 설치 CIL·BAML·XSD를 근거로 했다. 원본 실행 관찰이 아니다. Source의 전체 셀과 첫 display offset/size는 결과 MatrixFormat·ORIGIN과 별개다. ContentChanged가 문서 dirty로 이어지는 경로는 양성이지만 source pan 후 실제 계산 요청/Undo granularity 전체는 입증되지 않았다.

[IR 후보](../../artifacts/rea/source-matrix-ir-next-20261020/v1/README.md)에 따라 기존 Matrix variant에 선택적 SourceMatrixDisplay를 추가했다. None의 기존 JSON/두 MathML writer bytes를 유지하고 empty display 및 속성 presence를 보존한다. offset/size는 unsigned row,column tuple이며 null/unknown/duplicate/비정상 bounds·숨은 unsupported 셀은 거절한다. Full arity/모든 column-major 원문 셀은 그대로이며 계산은 전체 값을 사용한다. source Show/Origin/result-view 필드는 추가하지 않았다.

[source renderer/caret](../../artifacts/rea/source-matrix-editor-render-next-20261020/README.md)는 보이는 실제 AST 자식만 조판하고 full NodeId/주소를 유지한다. 숨은 셀은 hit가 없고 continuation glyph는 데이터가 아니다. 방향키·Tab이 숨은 셀로 갈 때 source offset을 바꾸고, import/paste 진입은 원문 변경 없이 보이는 caret로 clamp한다. 셀 내부 fraction 이동을 보존하고 같은 RawInput 배치의 뒤쪽 geometry 키 전에 hits를 갱신한다. 실제 source pan 하나를 editor Undo 하나로 기록하는 것은 독립 정책이다. source 직접 resize/drag/heading·전체 nested navigator는 미완료다.

[full-window 보정](../../artifacts/rea/source-matrix-editor-render-v2-next-20261020/README.md)은 Some display가 실제 전체 범위일 때 기존 Matrix 조판을 그대로 사용한다. metadata의 존재만으로 여백이 늘던 제품 오류를 old2실패→새 실제 geometry/hit/readonly helper 검사로 교정했다. readonly allocated rect와 measured size 사이에는 기존 egui pixel snap 허용치를 적용하되, None/Some 변형 간 geometry·값·hits 비교는 exact로 유지했다. 이 fixture 보정과 제품 guard를 구분한다.

[실제 App 편집](../../artifacts/rea/source-matrix-typed-edit-app-next-20261020/v1/README.md)은 visible full cell2에 KeyNum0+Text0을 보내20으로 수정하고 Enter·CmdZ/Y·명시 계산·own/native cold를 검증한다. 숨은5셀과 display가 남고, metre source의 SI 값 및 cm 표시 `[100,2000,300,400,500,600]`을 분리한다. Native 재작성의 UNIT 식별자는 기존 canonical xml:space를 붙이며, 원본 ZIP의 불변 검사는 따로 유지한다. 원문 lexical spelling의 bit-exact 편집 주장은 하지 않는다.

[dense trace](../../artifacts/rea/source-matrix-dense-trace-next-20261020/v1/README.md)는272셀의 실제 traced writer/DOM label oracle·warm 재사용 뒤 display prefix 변경과 숨은 label/전체 주소 보존을 검사한다. 이전 [preflight reuse](../../artifacts/rea/matrix-retained-cost-next-20261020/v1/AUDIT.md)도 유지한다. [r221 실제 LCC CPU](../../artifacts/rea/lcc-full-app-r221-analysis-next-20261020/README.md)는 이전 고정 binary의 CPU envelope이며 새 sourceSome/GPU/체감 지연 개선률이 아니다.

## 파일·양 OS 검증

[채택 receipt](../../artifacts/rea/source-matrix-editor-r222-integration-20261020/validation.json): 같은695파일로 Windows 2164·peets 2165 정상 검사(backend2 포함), ignored 104/102, 각48harness, fmt·workspace/backend Clippy·build·reference38·Windows release 통과. [실행 파일](../../artifacts/rea/source-matrix-editor-r222-integration-20261020/windows/mathcraft.exe)은 빌드만 했다. Archive `6706c655fa436ce978546bfdf026a4ff8bf3f10dc495baef704862f4bf9675aa`, manifest `e2a2ad24181604e5f0206614ac055f36705225ce87831d704836c48061ad220a`. 문서 갱신은 채택 후 별도 receipt다.

새 packet 합계 MCDX20·own12·PDF12·선택 Mathcad XML XSD120건이다. [typed source checker](../../artifacts/rea/source-matrix-display-validator-r222-20261020/README.md)는 실제 전체 typed source·native/own/cache를 검증하고16개 rebound 오류 복사본을 거절한다. Saved carrier는 전체 변환된 display quantity이며 SI는 실제 Rust producer assertion 근거다. Source clipboard/shape는 아직 Some일 때 원자적 거절이고, 같은 shape의 셀 편집과 전체 subtree 복사는 보존된다.

[새 source PDF checker](../../artifacts/rea/source-matrix-print-validator-r222-20261020/README.md)는 양 OS의 plain/unit 각1페이지에서 입력2·3과 결과전체6개를 서로 다른 crop으로 fitz/pypdf 양쪽에서 대조했다. 각93 vector paths와 bitmap0이며 m/cm 글자도 구분한다. 전체 숫자 multiset을 보존한 source/query swap을 포함한 실제 오류 PDF3개와 synthetic crop8개를 별도로 검사했다. Native seed 좌표의 정상 grid snap을 원문 region attrs와 직접 대조하도록 producer fixture를 고쳤다. 픽셀/원본 전체 print hierarchy 동일성은 주장하지 않는다.

첫 preflight의 private test helper 호출, bare matrix를 worksheet top expression으로 사용한 fixture, UNIT xml:space raw 비교, readonly pixel snap, native seed 좌표 가정, Clippy test tuple type alias 수리 로그는 보존한다. 원본 source/숫자 golden을 줄이거나 프로덕션 serializer를 테스트 때문에 바꾸지 않았다. r216 전체192MCDX/120PDF packet은 재생성하지 않았다.

기능 상태555행(부분159/미구현396),87가족(구현4/부분69/미구현14)을 유지한다. EA-061/063/065의 지원 깊이와 sourceSome 한계를 갱신했다. [빈 셀 Ctrl 경계](../../artifacts/rea/matrix-empty-term-next-20261020/README.md)는 도움말/정적 분기 불일치가 있어 EA-064 미구현을 유지한다. [factory 조사](../../artifacts/rea/source-matrix-authoring-factory-next-20261020/README.md)에 따라 native None/일반13행 성장에 일괄12 제한을 새로 넣지 않았다. GUI·스크린샷·원본 CLR/Mathcad/COM은 실행하지 않았고40% 달성은 미완료다.
