# Typed XY 편집의 중복 소스 검증 비용

2026-10-19. [r193](0182-xy-typed-expression-integration.md)의 typed 첫 수식 입력에서 변경되지 않은 모든 축을 키마다 다시 MathML·JSON으로 검증하는 비용을 분리했다. 원본 기능의 새 구현이나 전체 앱 속도 측정이 아니다. GUI·스크린샷·원본 runtime·CLR 실행은 없다.

## 실제 경로와 변경 범위

`expression_field::stage_expression`은 변경한 Editor를 PortableTree로 변환하고 `PlotExpression::from_tree`로 검증한다. 이어서 draft 전체를 검증하는데 각 축의 validate가 수행된 뒤 aggregate loop에서 같은 축의 validate와 source JSON 직렬화가 반복됐다. 16X/16Y·2080노드 fixture에서 한 Text 이벤트에 validate_tree65회, source_bytes32회가 관찰됐다. idle은 두 검사 모두0회였으며 Editor revision·draft 생성·원문을 바꾸지 않았다. 이 구간의 idle 문제가 매 프레임 전체 tree clone이라는 가정은 확인되지 않았다.

[소스 carrier](../../crates/mathfolio-document/src/plot_expression.rs)의 private Box에 검증된 node 수와 정확한 source JSON byte 수만 추가한다. `from_tree`와 Deserialize는 기존 PortableTree·문법·텍스트·MathML·깊이/노드 예산 검사를 먼저 모두 수행한다. JSON 길이는 같은 serde writer로 한 번 계산하며 JSON 버퍼를 보관하지 않는다. typed tree의 mutable 접근자는 없으므로 같은 immutable carrier의 구조 검증 결과만 재사용한다. mutable LegacyText는 계속 매 admission에서 검사한다.

aggregate는 같은 source 순서로 정확한 node/JSON byte 수를 더하고 기존 node-before-byte 오류 순서를 유지한다. serialization 오류의 `usize::MAX` 처리도 그대로다. receipt는 wire에 쓰거나 외부 입력에서 받지 않는다. label/context 해석·prepared query·결과·print cache의 증명이 아니며, Arc·공유 tree·hash/pointer cache·평가 memo는 추가하지 않았다. tree 복제 정책은 그대로다.

## 측정 고정과 범위

별도 scratch workspace는 r192 archive SHA `f467d5decc95476668027c7b7938f3ed77fed80c95d7adc39069f5c17a115ee6`에 고정했다. r193은 compat module 순서 fmt만 바뀐 별도 완료 revision이다. isolated target/executable을 사용했으며 당시 제품에는 수정하지 않았다. 새 String 수식 통합을 포함한 다음 revision 전체 성능으로 확대하지 않는다.

확정 측정은 root/A/C가 Cargo·프로파일 idle을 확인한 뒤 **12:26:07.390188–12:26:10.522996 UTC**, before/candidate5교대다. allocation/counter feature 없는 하네스의 실제 `plots::draw`를 측정한다. 각 process의20 idle/10 연속 Text 표본 median을 구한 뒤5 process median을 사용했다. 첫 Text와 cold draw는 process당1개다. 모든 지표를 합산해 프레임 시간을 만들지 않았다.

fixture는 tiny=1X/1Y Number1, one_matrix=1X/1Y32×8행렬/514노드, many_matrix=16X/16Y8×8행렬/2080노드다. curve rendering 없는 authoring-only 오류 presentation이므로 tessellation·GPU·실제 곡선 계산을 포함하지 않는다. cold draw는 새 egui/fonts를 포함하지만 fixture 생성 비용은 제외한다. 출력 파괴는 타이머 밖이다.

|실제 draw CPU median, ms|tiny before→candidate|one_matrix|many_matrix|
|---|---:|---:|---:|
|첫 Text|0.1775→0.1713|1.0031→0.5356|2.5100→0.8329|
|연속 Text|0.07130→0.04710|0.77880→0.41345|1.90765→0.45550|
|warm idle|0.02405→0.02265|0.06145→0.06705|0.30725→0.30955|
|draft idle|0.0360→0.0242|0.0708→0.0697|0.3471→0.3225|
|ArrowLeft|0.0534→0.0365|0.0834→0.0800|0.3731→0.3455|
|cold draw|10.0885→9.6728|9.5535→8.8631|11.4130→11.2670|

many_matrix의 독립 `spec.validate` micro median은1.3917→0.0013ms다. 비용이 사라진 이유는 재검증을 생략한 admission 약화가 아니라 이미 검증된 불변 carrier의 재사용이다. 반대로 constructor에 JSON 길이 계산이 추가되어257노드의 from_tree+입력clone은0.0731→0.0806ms,65노드는0.0188→0.0211ms로 늘었다. 전체 source serialization0.3110→0.3154ms와 clone0.0996→0.0959ms는 여전히 남는다. idle/cold font 개선은 주장하지 않는다.

actual WorksheetApp 입력 프로브도 별도로0/4000 offscreen Math와 작은 typed plot에서 signature 불변·실행0·background generation 불변·draft 유지로 확인했다. 이 값은 phase observer가 켜져 있어 위 draw 표와 구분한다. 4000개일 때 첫 Text1.3004→1.3484ms, 다음 Text1.1843→1.1390ms, idle1.1715→1.0852ms다. input_snapshot은 약3–4µs였으며 O(N) page scan은 이 패치로 제거되지 않는다.

최초 5교대는 A의 이전9.31초 compile과 절대 시각을 대조할 수 없어 **provisional로 제외**했다. 첫 raw와 tiny 첫 Text0.1723→0.2013ms/cold10.3557→10.8181ms, one_matrix cold9.5729→10.5397ms/draft idle0.0705→0.0843ms 등의 반대표본도 보존한다. 확정 결과와 합치지 않았다.

## 별도 allocation/counter와 보존 비용

관찰 feature를 켠 별도 실행에서 many_matrix의 Text 검증 횟수는65→1이다. source_bytes32호출은 남지만32 JSON Vec 생성 대신 receipt read이며 변경한 tree만 한 번 JSON 길이를 센다. 연속 Text의 Rust allocation9659→1019회, alloc+realloc requested bytes1,382,566→277,750였다. 첫 Text는13904→5264회와1,828,076→724,292bytes다. idle allocation353회/124,651requested bytes는 그대로다.

이 값은 Rust allocator 요청 합과 요청된 live high-water이며 RSS·OS/non-Rust memory·realloc의 물리적 겹침을 나타내지 않는다. receipt는64bit에서 typed source당16bytes,32축 source 복사본당 최대512bytes다. 기존 clone/history 복사본도 각각 그 비용을 가진다. 전체 앱의512bytes 상한이 아니다. 첫 draft의 many_matrix 요청 live peak 증분은519,988→521,012bytes(+1024)였고 연속 Text의76,757bytes는 그대로였다. 새 전역 보관 cache는 없다.

## 독립 oracle와 산출물

[receipt tests](../../crates/mathfolio-document/src/plot_expression_receipt_tests.rs)는 r193의 이전 validator·aggregate loop를 독립 보존한다. Unicode/escaping/native label/context/literal suffix·-0·invalid Number·제어문자·잘못된 arity/statement·text/node/MathML 예산·mutable legacy 변경·동일 JSON·deserialize 재검증을 대조한다. forged receipt/unknown/duplicate key·StringLiteral typed-plot 거절·깊이와 aggregate node-before-byte 우선순위도 검사한다. 계산 source/hash 예산을 낮추지 않는다.

제품 채택 전 scratch document229pass/7ignored였으며, 제품 독립 oracle4pass와 document전체235pass/7ignored를 확인했다. 최초 native_calculation 옵션 컴파일 실패와 수정 뒤 정상 재시도 로그를 분리 보존했다. document all-target Clippy `-D warnings`도 통과했다. 최종 조합의 양 OS/native/PDF/release 검증은 다음 통합 revision의 책임이다. 이 문서의 검사 수를 기능 완성률로 사용하지 않는다.

원시 자료는 `artifacts/rea/typed-authoring-cost-probe-20261019/`의 README·`probe-identities.json`·`candidate-source-manifest.json`·`confirmed-boundary.json`·`confirmed-paired-*`·`counter-summary.json`·`worksheet-summary.json`에 있다. 초기 `unmodified-source-manifest.json`은 tar 추출 전의 좁은 복사본이므로 전체 baseline manifest로 사용하지 않는다. authoritative baseline은 고정 archive다.

관찰 feature 없는 baseline exe SHA `282236b51939bc662947d456e4c7eee867517779dae3687431e6918a7e085bf4`, candidate SHA `4e4c547abf0e9e02bc410ea09e24bb17253c822778d7fe7601450b8fe93df6d8`다. counter exe·source·diff의 나머지 SHA는 identity JSON에 보존했다.
