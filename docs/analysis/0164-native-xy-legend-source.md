# Native XY legend 위치·폭의 좁은 source 지원

2026-10-19. [0161](0161-native-xy-expression-layout-contract.md)·[0163](0163-native-empty-and-partial-plot-contract.md)의 정적 REA를 문서 모델과 native source codec에 연결했다. GUI·스크린샷·원본 runtime·CLR은 실행하지 않았다. 이 변경은 위치 source/blank constructor이며 renderer 배치나 원본 전체 layout 구현 완료가 아니다.

## 원본 필드와 기본값 구분

AxisModel.GetXmlNode1068은 축 root의 `legend-position`에 축별 enum.ToString을 쓰고, `axisLine/@legendWidth`에 InvariantCulture double을 쓴다. `axisLine/@position`은 축 선의 위치이며 식 목록 위치가 아니다. 1065 raw reader는 legend-position이 있을 때만 current 값을 바꾸고,1069는 축별 enum을 구분한다. schema absent default와 fresh control의 명시 state를 혼동하지 않는다.

| source | X | Y |
| --- | --- | --- |
| worksheet50.xsd의 absent 기본값 | PlotBoundaryBottom | PlotBoundaryLeft |
| 이번 typed boundary subset | Bottom / Top | Left / Right |
| 새 blank_xy의 명시 source | PlotBoundaryBottom | PlotBoundaryRight |

fresh YRight는 constructor/presenter 및 새 빈 plot 도움말의 양성이다. normal typed deserializer에서 XSD defaults가 언제 augmentation되는지는0163의 미확정 범위로 유지한다. raw setter가 absent 값을 바꾸지 않는다는 사실만으로 모든 absent native Y가 오른쪽이라고 주장하지 않는다. 문서 모델의 None은 source absent를 보존하며 후속 renderer가 참조할 schema 기준 위치는 X Bottom/Y Left다. 명시 Some(Bottom/Left)도 None과 source 형식이 다르므로 same_source는 구분한다.

native lexical은 정확히 `PlotBoundaryBottom/Top/Left/Right`다. `XBottom` 등의 약식 이름, 다른 대소문자, 축이 다른 boundary, AxisAttached enum은 지원하지 않는다. unknown native attribute/child나 marker·tick·axis geometry를 새 default로 추정하지 않는다. 기존 strict 검사에서 원문 보존이 필요한 Err로 남긴다.

## 모델·wire·비교

`canvas::PlotLegendPosition::{Bottom,Top,Left,Right}`는 typed enum이며 JSON은 snake_case다. PlotAxisSpec에 optional legend_position과 optional f64 legend_width를 추가했다. 두 필드 모두 serde default/None skip을 적용하여 기존 r160의 `{"label":"","range":null}` byte 순서와 기존 plot JSON/cache SHA를 바꾸지 않는다. 새 source 필드는 range 뒤에 직렬화한다. 공통 PlotSpec validate가 X/Y orientation과 width를 검증한다.

legend_width는 이 앱의 UI 폭 subset으로 **finite·nonnegative**만 지원한다. xs:double 전체 범위 지원 주장이 아니다. negative/NaN/Inf native는 Err/raw 보존이며 width를0으로 보정하지 않는다. f32로 줄이거나 pixel 폭으로 임의 환산하지 않고 f64 source 값을 보존한다. ±0, subnormal, 최대 finite 값도 writer→reader의 bits를 대조한다. width absent와 explicit0, +0과−0을 same_source 및 signature에서 구분한다. 원본 숫자의 lexical spelling이 다른 경우 payload writer는 roundtrip 가능한 숫자 문자열을 쓰며, source가 그대로인 native export는 기존 원문 bytes를 복사한다.

수정 경로는 canvas.rs의 optional source types, 새 plot_legend.rs, plot_source_model.rs의 legend validation 호출1줄, native_plot.rs의 reader/writer/same_source와 회귀다. PlotSpec의 Legacy/AxisLists 권위나 native geometry 변환은 바꾸지 않았다.

## blank_xy는 Default 샘플과 분리한다

`PlotSpec::blank_xy()`는 각각 PH인1X/1Y의 AxisLists, owner-local equation ID1/2와 next3, 명시 XBottom/YRight를 만든다. 첫 trace는 [0157](0157-native-xy-slot-authoring-contract.md)의 DarkBlue·Solid·LineWeight1.0을 표현한다(RGBA[0,0,139,255]). native source는 PH를 실제 ml:placeholder로 유지하며 가짜0 또는 sin curve를 만들지 않는다. 이 constructor 자체는 계산기·cache·worker를 호출하지 않는다.

기존 PlotSpec::default의 x/sin(x), 기존 sample/legacy JSON은 그대로다. blank의 width/height는 현재 앱 기본420/260을 재사용할 뿐 원본 최종 geometry라고 고정하지 않는다. 원본은 PageBodyWidth/2와 후속 measure를 사용한다(0163). marker의 미지원 추가 metadata·선택 focus·scaling layout 값을 constructor에 추정하여 넣지 않는다. 앱의 새 plot 생성/화면 배치는 별도 후속 연결이다.

## 독립·native 회귀

신규 normal6개는 old axis/plot JSON exact bytes, 기존 sample 불변, blank PH·위치·첫 trace style, wrong-axis/width validation, optional source 및 signed-zero signature 차이, finite f64 width roundtrip, unsupported enum/metadata 거절, native MCDX 저장→source 재해석→unmodified 재출력의 worksheet byte 일치를 확인한다. 원본 코드 실행으로 결과를 계산한 테스트가 아니다. 기존 persisted cache golden과 독립 old derive/SHA oracle도 전체 document 회귀에 포함한다.

명시 ignored fixture1개는 `MATHCRAFT_NATIVE_XY_LEGEND_OUTPUT` 아래4plot package/own문서/worksheet.xml을 만든다. absent sample, fresh blank, Top/Left+decimal width, Bottom/Right+signed-zero/subnormal width를 포함한다. 생성 자료는 `artifacts/rea/native-xy-legend-generated-20261019`, 검사기는 `validate-native-xy-legend-20261019.py`다. 로컬 원본 worksheet50.xsd와 표준 XML namespace schema를 network/entity 없이 로드하고 원본 XSD bytes는 수정하지 않았다.

XSD 오류0이며 wrong-axis boundary는 XSD도 거절했다. AxisAttached와 negative width는 XSD가 허용하지만 Rust의 좁은 subset이 Err라는 차이도 검사했다. XSD 통과를 실제 measure/렌더링 parity 또는 전체 원본 source 지원으로 확대하지 않는다. 결과는 `artifacts/rea/native-xy-legend-schema-validation-20261019.json`; worksheet SHA=`8f581b37e9256b5f681c22df822cec4650e650986ed20aace3ab04bb1f744591`이다.

전체 document182개 PASS/실패0/ignored4, document all-target Clippy `-D warnings` PASS를 확인했다. focused6개와 명시 실행한 ignored fixture1개도 PASS다. 전체 로그는 `native-xy-legend-document-tests-20261019.log`, `native-xy-legend-document-clippy-20261019.log`다. 로그는 `native-xy-legend-tests-20261019.log`, `native-xy-legend-fixture-20261019.log`다.

## 원시 증거와 한계

설치 PE identity/메서드 관찰은 `artifacts/rea/native-xy-empty-method-ledger-20261019.json` 및0163을 재사용했다. AxisModel1068 IL SHA=`1bd856d4a56449acb77e732052acf3d84f5adf1617ff04c06562d4ac56722aa9`. 원시 파일 SHA는 writer `47d7b7a945e268d589989c4bdb0c1d47ab5e46305d10474b0e660841ce8d1949`, parser `38df5fa7897d92c2d253d0c88ee0e2b65a739ae6e4e2db3e8c76c685c87d981c`, enum reader `750e74dc03ad6c8a5dab5b4304f62da8f5ac57b256dcda80e5cc27a0c40e5853`다. worksheet50.xsd SHA=`b4a73084e11db4dbcba52c603e2791a3026b088e150cb482114b9a315f355ba3`를 검사했다.

axis-attached 위치, 숨김/다중축/marker defaults, 정확한 legendWidth의 WPF 폭 변환·adaptive columns, 모든 origin/default geometry, 원본 blank Enter의 native engine error와 partial trace 표시 timing은 완료 범위가 아니다. 새 renderer 배치나 완성도 수치로 이 source patch를 환산하지 않는다.
