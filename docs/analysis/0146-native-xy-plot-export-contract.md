# Native XY plot의 source-only 저장 계약

2026-10-19. Mathcad Prime 12.0.0.1 설치 관리 코드·스키마·문서 패키지를 정적으로 조사했다. GUI·스크린샷·원본 프로세스·CLR 실행은 하지 않았다. 제품 코드는 변경하지 않았다. [0023](0023-xy-plots.md)·[0027](0027-multi-trace-xy-plots.md)의 native 저장 공백을 실제 writer/reader로 좁힌 후속 계약이다.

## 결론과 범위

**독립적으로 작성한 단일 XY trace의 식과 서식을 native XML에 저장하는 근거가 확보됐다.** `<plot><xyPlot>` 아래 trace style, graph-size, 두 축의 plotEquations와 domain을 기록하는 원본 writer가 확인됐다. 두 Math 자식의 역할도 식→사용자 scaling 순서로 확정됐다. trace cached points와 가짜 engine binary를 만들지 않는 source-only 저장은 별도 원본 writer와 reader에서 허용한다.

이것은 원본 Mathcad로 새 파일을 열어 재계산까지 실행한 검증이 아니다. 실제 원본 저장 패키지의 XY 표본, cached curve의 binary 형식, native를 다시 Mathcraft의 editable PlotSpec으로 읽는 parser는 별도 공백이다. 처음 구현은 식 한 쌍·일반 lines trace·dimensionless 축·default axis state·source-only 결과로 제한할 수 있다. 실제 export 이후 XML schema, 순수 native-reader, Mathcraft 재계산·PDF를 따로 검사해야 한다.

## 원본 파일과 새 REA

대상 MVID는 `cecd333a-bf3e-4423-b74b-7ff799618db1`이다. `Ptc.Controls.Plots.dll` SHA-256=`4e5067e58e69c68e8152ce677d5ac77a6587e00f6295bda5cc8ddff18f6685eb`. 새 focused REA는 다음과 같다.

| 범위 | Evidence |
| --- | --- |
| XYPlotControl/PlotAxis/PlotEquation/PlotTrace 등 | `ev_f99518cf39a0873adf0c6f18e58723f4190b3385ac5d484c6de83e7a69d89360` |
| PlotControl persistent provider | `ev_166611f06fa5fa948b7b75fe5f682053b846df6b0548c72059ee342948f7a3b6` |
| PlotModel | `ev_256c745d6d9a7a0938ec0889acdf0d6c9c561fc63b4b8e8a7c932895b5de7909` |
| AxisModel/TraceModel | `ev_1960b1fae2c5d467b6e1af9ef42c1c838180c712c96e94815909d70aa2ee3b25` |
| PlotDataObjectCreator | `ev_ef08f9c3e9cec0f55bdcf38f4b276387c860a1deaecc0989647f44c1528a2708` |
| AxisModelDefinitionCollection | `ev_7e12a86e2ea1576c70f678d88ebbca2082901caa5bc0016142a7539a279c0` |

ignored `artifacts/rea/native-xy-export-method-ledger-20261019.json`와 `feedback/native-xy-export-*-cil-20261019.txt`에 선택 본문의 token·IL SHA를 보존했다. decoder는 REA가 보고한 assembly/IL SHA를 설치 bytes와 다시 비교하며 dnfile/dncil로 해독한다. code loading/reflection을 사용하지 않는다.

설치 트리의 worksheet.xml을 가진 mcdx/mctx 77개를 조사했지만 plot payload는 없었다. `native-xy-export-shipped-inventory-20261019.json`은 조사 경로와 worksheet SHA를 기록한다. 따라서 아래 derived fixture를 원본이 실제 출력한 표본으로 부르면 안 된다.

## XML 조립 순서

`PlotControl.GetRegionPersistentData 0x060002be`는 model의 PlotXmlNode child들을 읽어 `xyPlotType.InnerXml`에 넣고, plot wrapper의 background/origin-positioning/show-axis-expressions/disable-calc를 저장한다. IL SHA=`913433eee884e332af9e65649ac4a5890b1e1a9d36f6f2282835801e39e3714f`.

`PlotModel.get_GraphRegionXmlNode 0x06001154`는 xyPlot을 만들고 optional legend, traces, graph-size를 순서대로 붙인다. 각 trace는 appearance 순으로 `ITraceModel.RegionXmlNode`를 사용한다. `get_PlotXmlNode 0x0600114e`는 graph node에 plotTitle과 axes를 붙이고 rangeAxis(x), primaryResultAxis(y)를 순서대로 넣는다. `GetAxisXmlNode 0x06001152`는 axis XML의 domain 바로 앞에 plotEquations를 삽입하고 축 equations를 순회한다. 최소 payload의 순서는 다음과 같다.

```xml
<plot>
  <xyPlot>
    <traces><trace><traceStyle ...>lines</traceStyle></trace></traces>
    <graph-size width="..." height="..."/>
    <axes>
      <xAxis rank="1">axisLine, axisGrid, plotEquations, xyDomain</xAxis>
      <yAxis rank="1">axisLine, axisGrid, plotEquations, xyDomain</yAxis>
    </axes>
  </xyPlot>
</plot>
```

생략한 optional title/legend/markers/formatting을 추가할 때도 schema 순서를 지켜야 한다. 두 축 rank는 모두1이며 trace와 축 식을 연결하는 독자 ID는 이 writer에 없다. PlotModel.createPlot1150은 x/y plotEquation 목록을 순번대로 AddEquation하고, GraphRegionXmlNode1155는 이미 생성된 trace와 저장된 trace 목록을 순서대로 대응한다. 단일 x/y/trace에 숨은 별도 relationship이 필요하다는 근거는 없다. 여러 trace의 식 재사용 규칙은0027의 도움말 계약과 함께 보되 임의 deduplication을 만들지 않는다.

## 두 math는 식과 scaling이다

`PlotEquation.GetRegionXmlNode 0x0600033d` IL0023–0036은 Expression.GetRegionXmlNode를 먼저, 0037–0051은 GetUserScalingExpression.GetRegionXmlNode를 다음에 추가한다. formattingOverride는 설정이 있을 때만 붙인다. IL SHA=`475b9559473f2316e291f04b29a6403e624be0a0155ce35aa265e6b24708c6f4`.

reader0338도 child0을 Expression, child1을 Scaling에 전달한다.0339는 scaling math의 첫 child가 MathML placeholder면 설정을 바꾸지 않고, 아니면 사용자 scaling을 복원한다. 따라서 무차원 축의 최소 형식은 `plotEquation / math / structural-expression`과 **별도의** `math / ml:placeholder`다. math 하나를 생략하지 않는다. 식은 기존 editor_bridge의 native MathML로 구조화한다. vector·range·function도 이 식에 기록하며 XY 좌표 샘플을 축 식에 끼워 넣지 않는다.

## 축의 필수 상태와 기본값

AxisModel.GetXmlNode1068은 axisLine, axisGrid, optional title/markers/numberFormat, domain을 만든다. PlotModel1152가 plotEquations를 domain 앞에 삽입한다. reader1065는 schema에서 optional인 항목도 직접 속성을 읽는 경우가 있으므로 schema만 보고 더 줄이지 않는다.

| 항목 | 확인한 계약 |
| --- | --- |
| axisLine | position을 명시한다. constructor0fef의 Position은 enum0, writer는 lowercase, schema 기본값은 origin이다. positionticmark/legendWidth에는 optional reader guard가 있다. |
| axisGrid | gridFrequency, gridLabels, gridLines, tickMarks 순서다. labels/ticks display를 명시한다. 기본 생성자는 numbers/major ticks=true, minor ticks=false, grid lines=false다. |
| linear default | DefinitionCollection1085의 linear 분기는 start−10/end10/max100/major11/threshold2를 전달한다.0fef가 major count와 position tick(defaultCount/2)을 설정한다. schema의 gridFrequency 기본2와 원본 model 기본11을 혼동하지 않는다. |
| xyDomain | scale-type=linear, auto-scale=true다.1068 writer는 auto-scale을 항상 true로 쓴다. startValue/endValue는 필수이며 기본 limit에는 직접 ml:placeholder를 넣는다. |
| explicit limits | 기본값이 아닌 limit은 해당 MathML과 resultRef를 저장한다. axis start/end 속성은 현재 표시 경계를 별도로 저장하므로 사용자 limit과 같은 뜻으로 취급하지 않는다. |

PlotSpec의 pan/zoom이 source range를 바꾸지 않는 기존 계약을 유지한다. 처음에는 range=None만 지원하거나, 명시 range를 정확한 domain expression으로 변환해야 한다. 이를 auto-scale=false로 기계적으로 바꿀 근거는 이 writer에 없다.

## Trace style, identity, cache

TraceModel.get_RegionXmlNode1277은 traceStyle text에 lowercase trace type(lines 등)을 쓰고 color, symbol, line-weight, line-style을 붙인다. 점 개수를 바꿀 수 있는 trace만 num-of-points를 쓰며 마지막에 ResultManager.CreateResultRef를 호출한다. 별도 style ID/이미지 relationship은 없다. symbol은 **none**(소문자), line-style은 **Solid/Dot/Dash/DashDot/DashDotDot/None**이다. RGBA는 schema의 ARGB/RGB hex로 변환하며 line-weight는0.25–9 범위다.

PlotSpec의 marker size/frequency와 rich axis/trace labels는 최신 writer1277에서 대응하는 전체 항목을 확인하지 못했다. 조용히 버리지 말고 처음에는 default/no marker/no label로 제한하거나 개별 serializer 계약을 조사한다. graph-size는 graph model 크기이며 바깥 region 측정 크기·식 입력란을 포함한 크기와 같다는 증거는 없다. 기존 Mathcraft420×260을 원본 기본값이라고 기록하지 않는다.

**캐시 없는 source-only trace는 원본 자체의 경로에 있다.** PlotDataObjectCreator.CreateTrace001d IL0027–0029는 resultRefSpecified=false를 명시하고 style/label/point count를 만든다. TraceModel reader1278 끝의 UpdateResultRef도 CalculationCore.ResultManager0161에서 resultRef 속성이 없으면 false를 반환하며 강제 cache 참조나 예외를 만들지 않는다. XSD도 trace/value/math의 resultRef를 optional로 둔다. 따라서 가짜 trace value,0으로 채운 좌표, 추측한 engine part가 필요하지 않다.

일반 writer가 cache를 가질 때는 trace·기본값이 아닌 domain limit·Math equation마다 공유 ResultManager에서 참조를 만든다. source-only export에서는 이들을 생략하고 기존 source의 resultRef를 다른 plot에 재사용하지 않는다. 저장된 Trace2DResults binary/enginecache의 완전 재구성과 원본 재열기 자동 계산 완료는 미검증이다.

## Derived fixture와 초기 구현 검사

`artifacts/rea/native-xy-export-derived-minimal-20261019.xml`은 보존한 Presentation worksheet envelope에 독립적으로 조립한 한 trace(x대x) payload를 넣은 것이다. 원본 writer 출력은 아니다. 두 math, 기본 placeholder domains, origin axes, major11, style, graph-size가 있고 resultRef는 없다. bundled lxml과 기존 로컬 schema 일체로 worksheet50 검증을 통과했다. record는 native-xy-export-derived-schema-validation-20261019.json, fixture SHA는 동명 metadata JSON에 있다. XML 구조 검사이며 원본 실행 검증으로 간주하지 않는다.

초기 제품 구현은 새 native writer/reader helper로 분리하여 PlotSpec 구조식을 각 축에 하나씩 기록하고 정의·위치·단위·disabled source를 유지한다. 새 package-global region ID는 기존 export allocator를 사용한다. 앞서 미지원인 style/labels/range는 atomic unsupported로 거절하여 기존 destination을 덮지 않는다. import도 알려진 payload만 PlotSpec으로 복원하고 unknown plot은 Opaque source-owned로 보존한다. 이 범위로 기본 native 저장 공백을 좁힐 수 있다.

## 독립 source helper의 구현 범위

`mathcraft_document::native_plot::payload(&PlotSpec)`는 namespace-complete `<plot>`을 반환한다. `parse_with_references(owning_region_xml)`는 한 번의 XML parse로 `ParsedPlot { spec, trace_result_refs }`를 반환한다. 기존 `parse`와 `trace_result_refs` wrapper도 유지한다. `same_source(&PlotSpec, &PlotSpec)`는 크기·축 상태·trace 서식을 비교하고 각 식을 구조 tree로 비교하며 XML을 생성/재파싱하지 않는다. 지역 identity·위치·disabled 상태·결과 ID 할당은 소유 문서의 import/export transaction 책임이다. 이 helper는 결과 part를 열거나 curve 데이터를 계산하지 않는다.

현재 구현은 1–16개 trace와 두 축의 산술·일반 이름·named call 식, graph 크기, RGBA 색, 선 두께, Solid/Dot/Dash/DashDot/DashDotDot에 한정한다. trace가 N개면 양쪽 축에 각각 N개 equation을 순서대로 저장하고 동일한 x식도 생략하거나 합치지 않는다. parser도 정확히 N/N인 source만 읽으며 원본의 한 x식/여러 y식 재사용 배치는 아직 Opaque로 보존한다. 두 번째 math와 domain limit는 placeholder이고 축은 rank1/origin/grid11/기본 표시 상태다. label·명시 range·기본값 아닌 marker 설정·모르는 속성/자식·표현식 서식은 실패로 반환한다. 이는 원본의 금지가 아니라 현재 `PlotSpec`으로 정보를 잃지 않고 표현할 수 있는 지원 경계다. 원본 참조가 있는 trace와 두 math의 `resultRef`는 정수 검증 후 허용하지만 payload writer는 이를 새 plot에 복사하지 않는다.

식은 기존 core parser→PortableTree→native MathML 경로를 사용한다. 읽을 때도 plain 문자열로 변환한 식을 다시 구조화하여 비교한다. 유한 숫자의 동등한 표기(`2.000`/`2`)는 값의 비트로 비교하며 식별자 label/contextual flag/리터럴 아래첨자나 미지원 구조를 지우면서 성공하지 않는다. XML은 256KiB/8192 nodes/깊이64, 식은 기존 parser의 4096 bytes/256 tokens 제한을 따른다. 이 제한은 제품 자원 정책이다.

원본 graph 크기와 선 두께는 double이지만 `PlotSpec`은 f32다. parser는 f32의 재출력 숫자가 원본 double 값과 같은 경우만 허용하며, 달라지는 고정밀 값은 Opaque로 보존한다. 이는 원본 렌더링의 비트 단위 동등성 주장이 아니다. comment/processing instruction으로 식별자나 속성 본문이 여러 text node로 분할되면 첫 조각만 읽어 성공하지 않고 raw 보존 대상으로 거절한다.

추가 size 정적 검토에서 `PlotControl.GraphSizeChanged 0x060001d9`(IL SHA `5903654f64b0dfae4ef0da55ca766f8467c03519a2933a2c42465f2936d8720a`) IL019b–01bc가 model.GraphSize에 `get_Graph().ActualWidth/ActualHeight`를 넣음을 확인했다. `OnApplyTemplate 0x06000217`(SHA `84dc6e282e73d87903e0ed1f7d237c8013ae8b0ad2fda80658acc6954eecaf83`) IL0094–00d4는 이 Graph를 별도 `PART_PlotGraph` ContentControl에 넣는다. 외곽 PlotControl의 높이와 같다는 근거가 아니며 원본 전체 영역/legend 여백의 고정 변환은 미확정이다. 새 raw는 `feedback/native-xy-size-{control,xy,model,graph-template}-cil-20261019.txt`이고 기존 REA의 method body/hash를 설치 bytes와 다시 검증했다.

현재 Mathcraft `plots.rs`는 `PlotSpec.height`를 전체 영역 높이로 사용하고 graph widget에는 `(height−50).max(45)`를 요청한다. native adapter는 이 **기존 Mathcraft UI 정책**으로 graph-size.height를 저장하고 읽을 때 자체 UI 여유 50을 더한다. 50/45는 원본 상수가 아니다. 예를 들어 자체 420×260은 native graph 420×210으로 저장되며 native graph 높이260은 자체 전체 높이310으로 읽는다. f32 덧셈/뺄셈 및 재출력 수치가 모두 왕복해야 하고 실패하거나 자체 크기 범위를 넘으면 raw/Opaque로 보존한다.

소유 단위 검사 11개는 식/스타일 왕복, 참조 정수 범위와 비실행, 미지원 source의 원자적 실패, 숫자 표기와 추가 서식 거절, namespace 및 크기/노드 예산, 16개 trace의 독립 source/style/reference 순서, 실제 graph-size 값과 전체 높이 왕복, 정밀도 거절, 함께 반환되는 parse 결과, 모든 모델 필드의 source 동등성을 검사했다. 실제 저장 패키지와 XSD·재계산·PDF 통합은 후속 0147 통합 문서에서 별도로 기록할 예정이다.
