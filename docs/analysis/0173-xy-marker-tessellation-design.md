# XY marker tessellation 비용과 정확 출력 보존 설계

2026-10-19. r172 통합 중의 읽기·설계 기록이다. 제품 소스, registry 소스, 기존 성능 아티팩트를 수정하지 않았고 새 빌드·성능 실행·GUI·원본 runtime을 수행하지 않았다. 원본 제품의 구현 방식을 추정하는 REA 주장이 아니라, 현재 Rust renderer와 잠금된 egui/epaint source에 대한 성능 설계다. [0169](0169-xy-geometry-source-design.md)의 geometry source와 표시 allocation 분리는 유지한다.

## 기존 측정이 말하는 범위

`artifacts/rea/xy-render-cost-probe-20261019`는 r156 기반 renderer 좌표 projection 후보를 비교한 독립 Windows headless probe다. 10 warm frames 뒤 120 frames, 5회 before/after timing 교대를 기록했다. candidate는 CurvePoints의 Arc/raw + lazy PlotPoint projection/sparse projection이다. 아래 수치는 **당시 candidate의 각 run median에 대한 median**이며 현재 r172 제품 전체·GPU·문서 열기 시간으로 확대하지 않는다.

| points × traces | 이벤트 | marker frequency | draw ms | tessellation ms |
| --- | --- | --- | --- | --- |
| 500 ×16 | idle |1|0.4351|10.1118|
| 2000 ×16 | idle |0(마커 없음)|0.2815|2.5876|
| 2000 ×16 | pointer hover |0|0.5934|3.1337|
| 2000 ×16 | inline typing |0|0.3096|2.7043|
| 2000 ×16 | idle |1|2.1895|40.4642|
| 2000 ×16 | idle |10|0.4192|7.9810|

두 median 열의 합은 frame별 draw+tessellation 합의 median이 아니다. 당시 marker1 before tessellation도40.7815ms여서 좌표 projection cache가 이 지배 비용을 제거하지 않았음이 보인다. marker1의 hover/typing, 모든 marker 모양/크기/zoom, native partial/Pending, 현재 geometry layout은 이 probe에서 측정하지 않았다.

probe는 fixture 생성/raw Arc wrapping/output destruction을 시간에서 제외한다. cold draw는 context/font/layout과 최초 projection을 포함하고 cold tessellation을 별도 결론으로 제공하지 않는다. FNV64 geometry fingerprint/finite mesh/inert source 유지 검사는 통과했지만 fingerprint가 모든 출력의 독립 동등성 증명은 아니다. allocator 계측은 별도 실행이며 RSS가 아니다.

## 반복되는 작업의 원인

현재 `plots/render_points.rs`의 Storage는 raw f64 쌍, OnceLock의 full PlotPoint 벡터, 현재 frequency 하나의 sparse projection만 보존한다. 마커1에서는 full projection을 빌리고, frequency>1에서만 **사용자가 선택한 빈도**의 `.step_by(frequency)`를 사용한다. 추가 sampling/decimation이 아니다. 이 cache에는 화면 좌표·Shape·Mesh가 없다.

`plots.rs::draw_trace`는 각 trace의 선과 marker를 별도 PlotItem으로 추가한다. egui_plot0.37 Points::shapes는 모든 marker 좌표에 transform.position_from_point를 호출하고 각 marker에 Shape를 만든다. 빈 Circle은 fill Transparent, stroke width=radius/5다. 현재 default marker_size3/marker_filled=false를 사용한 old probe의 경로는 다음과 같다.

1. 2000×16=32,000개의 CircleShape를 매 draw 생성한다.
2. epaint0.36.2 `tessellate_circle`은 fill이 투명하므로 prerasterized-disc 경로를 사용하지 않는다.
3. `Path::add_circle(radius3)`는 CIRCLE_16의 닫힘 끝점을 포함한17개 path point를 만든다.
4. 기본 ppp1/feather1에서 stroke0.6은 thin AA branch로 간다. path point마다3vertices, closed segment마다4triangles다.
5. marker 하나당51vertices/204indices, 전체 marker는1,632,000vertices/6,528,000indices를 추가한다. 각 frame에서 이 path/AA/color/index 생성이 반복된다.

이 산식은 old raw counter와 일치한다. probe의 finite_vertices는 **120 measured +10 warm, 총130frames 합**이다. marker1의228,812,220과 marker0의16,652,220 차이를130으로 나누면 정확히1,632,000vertices/frame이다. 단일 frame에228Mvertices가 있다는 뜻이 아니다. 선/축/텍스트 등을 포함한 전체는1,760,094vertices/frame이다.

기존 Vertex layout은 pos8+uv8+color4=20B, index는u32다. 이 marker mesh만 합계58,752,000B(56.03MiB)의 길이 payload가 된다. allocator raw `candidate-alloc.json`의 이 case는 tessellation realloc 요청합이 frame당155,067,904B(147.88MiB)다. 여러 성장 요청을 더한 수치라 최종 live payload/peak/RSS가 아니다. summary의 alloc-requested 열338,048B만 보고 tessellation 전체 할당이 작다고 판단하면 realloc을 놓친다.

소스는 path/AA 반복과 큰 output 확장을 확인시켜 주지만, 해당40ms를 path연산/색변환/메모리복사/allocator 각각으로 측정 분해한 자료는 없다. 여기서 특정 함수가 몇ms라고 새로 주장하지 않는다.

## 선행 기각: 위치와 마커 template만 저장하는 앱 cache

원점 marker mesh를 한 번 만들고 각 중심으로 translate하는 방식은 bit-exact output 보존안이 아니다. 기존 circle path는 `center + radius*n`, AA는 그 결과에 feather를 더한다. 원점에서 AA까지 만든 mesh를 translate하면 연산의 결합·반올림이 달라진다. 얇은 선의 opacity/gamma rounding, pixel rounding도 원본 Shape를 처리한 순서와 같다고 볼 수 없다.

더 중요한 실제 앱 경계가 있다. `worksheet_ui.rs`의 page 경로는 origin translation × zoom을 `Context::set_transform_layer`에 적용한다. `egui/layers.rs`는 최종 drain에서 Shape를 global 좌표로 바꾸고 **그 다음** Context가 tessellate한다. Circle은 center/radius/stroke를 바꾸지만 Mesh는 완성된 vertices를 바꾼다. AA 폭은 physical pixel에 묶이므로 확대된 mesh와 확대된 Circle의 후속 tessellation은 동일하지 않다. 순수 translation도 f32 연산 순서가 달라질 수 있다.

따라서 PlotItem::shapes에서 local mesh를 만들어 보관하는 안을 그대로 채택할 수 없다. identity layer/opacity1에만 제한하면 독립 probe에는 hit해도 실제 worksheet의 origin/zoom layer에서는 대부분 fallback할 수 있다. 이것을 제품 버벅임 해결이라고 부르면 안 된다. inverse-transform으로 mesh를 되돌려 넣는 방법, 별도 global layer에 그려 원래 overlap 순서를 바꾸는 방법도 정확성 우회다.

## 권장 후보: 최종 global Shape 그룹의 bounded mesh cache

정확성에 가장 유리한 위치는 **Painter 색변환과 layer transform 뒤**, epaint가 원래 Shape를 tessellate하는 경계다. stock Context::tessellate는 매 호출 새 Tessellator를 만들므로 현재 앱의 CurvePoints cache만으로 이 경계를 대체할 수 없다. 이것은 향후 좁은 egui/epaint adapter 또는 vendored patch를 요구하는 설계이며 이번 구현 승인이 아니다.

첫 지원 범위는 실제 지배 case인 **unfilled Circle marker run**으로 제한한다. 다른 marker/filled/native/print는 기존 경로로 계속 모두 그린다. 제안 순서:

1. 앱의 작은 PlotItem wrapper가 실제 `egui_plot::Points`를 보유한다. bounds/geometry/base/base_mut/initialize/color/find_closest/on_hover를 원래 Points에 위임한다. 자동 bounds·legend ID/hover/선택 의미를 재구현하지 않는다.
2. wrapper의 shapes는 Points가 만든 동일 순서의 marker Shapes를 하나의 `Shape::Vec` 그룹으로 묶는다. 마커를 생략하거나 새 좌표를 계산하지 않는다. 선 item/각 trace/hover overlay와의 상대 순서도 보존한다. stock 경로에서 grouping 자체가 mesh/parallel 동작을 바꾸지 않는지 먼저 독립 oracle을 통과시킨다.
3. 이 그룹은 기존 Painter·layer transform을 그대로 지난다. 후단 cache는 **최종 global Circle 필드들**과 최종 clip/ppp/options를 읽는다. source 좌표의 약한 hash 또는 viewport 평균값으로 대체하지 않는다.
4. miss는 원래 tessellator의 circle 처리 순서를 그대로 사용한다. 생성한 그룹 mesh의 local indices를 보존한다. hit는 그 mesh를 원래 clip/texture/batch 순서에서 append한다. 기존 preceding mesh의 vertex count만큼 index를 rebase하여 원래 최종 stream과 대조한다.
5. cache는 Context/viewport renderer 수명에 속하고 저장 문서·계산 source·CurvePoints JSON과 분리한다. owner tag는 hit 후보를 빨리 찾기 위한 힌트일 뿐이며, exact transformed fields를 확인하지 않고 승격하지 않는다. 재사용된 region/trace ID도 데이터 검증을 우회하지 않는다.

그룹이 공용 Shape::Vec라는 사실만으로 모든 Vec를 영구 cache하지 않는다. marker-only bounded signature/run 길이와 지원 predicate를 검사하고, unknown child·혼합 texture·NaN/비유한 좌표·debug paint mode·미지원 옵션에서는 원래 전체 경로를 사용한다. cache 오류·budget exhaustion은 정확 출력 fallback이지 marker 숨김 사유가 아니다.

캐시 입력은 Circle(center/radius/fill/stroke) 필드의 **bit 배열을 순서대로** 비교한다. hash는 후보 검색에만 쓰고 collision 시 exact 비교한다. 이 O(N) 비교/키 복사 비용은 남지만 대상은 입력32kcircles이며 output1.6Mvertices/path-AA 반복보다 작은 양이라는 구조적 근거만 있다. 실제 순이득은 새 측정으로 확인해야 한다. Context 소스의 “전체 shapes 비교가 tessellation의 약50%여서 cache하지 않는다”는 주석은 일반 장면의 주의점이지 이 marker workload의 새 benchmark가 아니다.

## 무효화와 그대로 남겨야 하는 상호작용

| 변화 | 검증/처리 |
| --- | --- |
| 새 계산·native cache 복원·Pending previous 교체·Undo/Redo·owner reset | curve/source generation 교체로 후보 폐기; 최종 Circle field도 exact 확인. 계산 결과·source signature를 cache에서 복원하지 않음 |
| viewport pan/zoom/resize/스크롤/graph/legend layout·축 inversion/range | 최종 global centers/radius/stroke와 clip 비교로 miss. 이전 mesh를 이동·스케일해 재활용하지 않음 |
| marker shape/size/fill/frequency/color/line appearance | 지원 run을 다시 검증. style/frequency 변경은 기존 source가 정한 전체 marker 목록을 다시 생성. old frequency 하나만 보관 |
| hover·legend highlight/선택·trace 숨김 | Points geometry와 기존 hit test 그대로. highlight는 radius를√2배로 바꾸므로 miss. hover tooltip/crosshair/nearest-point dot은 별도 매 frame, mesh cache에 고정하지 않음. legend hidden은 원래 PlotItem 제거만 따름 |
| inherited opacity/disabled tint/스타일/색상·AA options | Painter를 지난 최종 fill/stroke와 tessellation options 전체를 exact 확인. pre-AA와 post-AA gamma multiplication 교환 금지 |
| clip 변경·feather/ppp/DPI·다중 viewport | 최종 clip/ppp/options/context generation을 key에 포함. 경계 밖 circle의 기존 coarse culling 조건/확장값을 그대로 사용 |
| print/PDF/다른 graphics context | 별도 경로/수명. 화면 mesh를 재사용하지 않음. print Shape 유지, raster preview는 원래 tessellation |

화면 source 기반 key를 따로 쓰려면 transform.frame/bounds의 exact bits와 dpos_dvalue의 부호(inversion 포함), layer transform, full style/clip가 필요하다. 그러나 **최종 Shape 필드 exact 대조를 생략하는 근거로 쓰지 않는다**. 데이터 revision/Arc identity만 맞아도 viewport·opacity가 달라질 수 있다.

opacity를 source key에 넣기만 하면 되는 것도 아니다. egui는 Painter에서 source color를 바꾼 다음 AA의 thin-stroke opacity를 계산한다. cached AA vertex에 opacity를 나중에 곱하면 rounding/색이 달라질 수 있다. 후단 접근이 이 순서를 유지한다.

rayon feature가 켜져 있을 때 `Shape::Vec(len>4)`는 parallel tessellation 후보가 되는 반면 개별 Circle은 아니다. grouping이 parallel prepass와 coarse culling/mesh grouping을 바꾸므로 feature activation을 확정한 oracle이 필요하다. 이 검증 전에는 단순 “Vec는 동등”이라고 채택하지 않는다. parallel/debug 옵션에 대해서는 처음에 fallback하거나 grouping 전 상태의 경로를 보존한다.

## memory, cold 비용, 남는 비용

제안 예산은 측정 전의 정책 후보다: Context 전체64MiB, entry당4MiB, 최대16entries, trace/run별 현재key 한 개, LRU eviction. 실제 Vec **capacity**·indices·vertices·exact key 배열·Arc/entry metadata를 합산하고 checked arithmetic/u32 index 범위를 검사한다. byte 한도를 넘으면 기존 renderer로 그린다. cap을 맞추기 위해 마커 수를 줄이지 않는다. Context/drop/owner reset에 비우고 이전 generation/source의 큰 payload를 계속 붙잡지 않는다.

old marker3 unfilled-circle 조건의 trace 한 개 mesh 길이 payload는3,672,000B다. 최종 Circle key를 단순24B×2000으로 보관한다면48,000B 추가, 16traces는 합계59,520,000B(56.76MiB) 정도다. **이는 길이 산식이고 실제 retained capacity/metadata/RSS가 아니다.** Vec의 성장 여유가 있으면64MiB를 넘을 수 있어 정확 capacity 측정이 필수다. bounded final-capacity reserve/compact 여부와 그 cold copy 비용도 별도로 비교한다. 큰 marker·두꺼운 AA·더 많은 Shape가 들어오면 이 예산에 맞지 않고 정상 fallback할 수 있다.

cold miss에는 원래 tessellation + exact key 생성 + cache storage/compact 비용이 추가된다. warm에서도 epaint `Shape::Mesh` 경로는 `calc_bounds()`의 vertices scan과 `Mesh::append_ref()`의 index rebase/vertices copy를 수행한다. Arc는 이 복사를 없애지 않는다. 화면별 clip invalidation과 pan 중 miss도 남는다. cache 자체58MiB와 매 frame output mesh는 동시에 존재할 수 있다. 고정 RSS/최종16ms 달성을 이 설계로 약속하지 않는다.

GPU upload·draw는 여전히 같은 triangles를 처리하므로 CPU tessellation cache가 GPU workload를 없애지 않는다. shader sprite/instancing/filled disc 대체, 동일 위치 marker dedup은 현재 AA/alpha overlap/색·coverage를 바꿀 수 있어 exact-output 후보로 채택하지 않는다.

더 작은 별도 후보는 output batch의 vertices/indices **reserve**다. 지금은 frame마다 새 output mesh가 커진다. 지원 Circle run에서 기존 vertex/index 생성 순서를 바꾸지 않고 bounded count를 미리 예약하면 realloc 반복을 줄일 수 있다. 이것은 retention 없이 byte-exact 가능성이 높은 선행 후보지만 path/AA 계산 자체는 남으며, 현재40ms 중 절감량은 미측정이다. coarse culling·empty shapes·texture batch·u32 overflow·allocation failure 경계와 cold 과예약을 반드시 비교한다. marker budget 상한만큼 항상 거대한 Vec를 잡는 방식은 피한다.

## print와 정확성 oracle

현재 printing.rs는 두 번째 layout pass의 원래 Shape를 vector PDF에 저장하고, raster preview가 필요할 때만 같은 shapes를 tessellate한다. vector_pdf는 Circle/Path를 vector로 처리하지만 Mesh는 untextured WHITE_UV 삼각형만 지원한다. filled-disc atlas mesh나 PaintCallback을 print로 흘리면 PDF가 거절되거나 출력 형태가 달라질 수 있다. 따라서 제안 cache는 **화면 후단 전용**, draw_print/vector snapshot은 기존 Circle/Path 경로로 유지한다. print용 캐시를 별도 도입하려면 DPI/page clip/context별 새 동등성 증거가 필요하다.

향후 구현/채택 gate:

1. cache없는 old path를 독립 oracle로 보존. 모든 vertices의 pos/UV/color bits, indices 순서/범위, texture ID, primitive/clip 순서를 비교한다. FNV hash만 통과 기준으로 삼지 않는다.
2. marker가 전부 보이는 case와 clip 경계/부분 offscreen, ±0/subnormal/NaN fallback, overlapping/alpha markers, 1point fallback, 빈 trace/Failed/Incomplete/Pending/Unavailable 혼합을 대조한다. 원본 경로의 culling 외 추가 coverage 감소를 허용하지 않는다.
3. 1/16traces×500/2000points, 모든 marker shape/fill/size 경계, 빈도1/10/큰빈도, legend hide/highlight와 nearest-point ordinal/tooltip, 다중 plot/owner 삭제·Undo를 실제 headless frame으로 검증한다.
4. page의 실제 origin×zoom layer에서 zoom1/0.5/1.25/2, ppp1/1.5/2, pan/scroll/resize, inherited opacity0.45/disabled/clip/style 변경을 검사한다. identity-only standalone 통과를 실제 worksheet 통과로 대체하지 않는다.
5. source JSON/cache SHA/CurvePoints raw f64와 계산 실행0 계약 유지. 화면 cache가 Save/native export/Manual oldpoints/print draft exclusion과 무관함을 회귀로 확인한다. PDF vector/0image 회귀 포함.
6. observer-off before/after5교대에서 cold frame, warm draw, 후단 tessellation, **frame별 draw+tessellation 합**, cache hit/miss/eviction/actual capacities/peak transient를 기록한다. miss·pan·예산초과·많은 plots의 악화도 남긴다. 새 CPU와 기존 r160 기록을 통제된 동일실험인 것처럼 비교하지 않는다.

## 근거 identity

기존 probe main SHA=`0d163aff4c1bce998222bb0b610fcb410ce26f273ae8b0c1b334b6349ac787a8`, summary SHA=`3f73f2dc70865eb39d66534d983bc7b9f1c5190a65cb9edfd011ffe6eb040726`. frozen-harness.json은 before/candidate timing/allocation exe와 source identity를 보존한다. 이 기록들을 이번 작업에서 덮어쓰지 않았다.

읽은 registry source는 egui_plot0.37.0/egui0.36.2/epaint0.36.2다. Points source SHA=`ba53f7d60437d09e61877f058b6609fe7dc1e888c2ba56e4481d48dfe26f85a5`, plot.rs=`8fe3868e6cd057ccef7cda7f74f97d0c4d03aa4fe51a985407b79e0ae5b5a724`, tessellator.rs=`4f3b5809df42f2de7574ea41d6e1f0df3296908597cf1197c871dc0c08f10488`, context.rs=`c1152788996a49f6ebfb45d817d4e9541d731334c12817ee075d448c0e55eea3`.

current render_points.rs는 이전 candidate와 같은 SHA=`75c1295fe0288bfc1546e166c1d71d2890e18054aac0fa8340e97e0cd2d9c612`다. 읽은 plots.rs SHA=`1235a22f2ffa5e51e22ccb454e412715c1998009c7828fbbfb4a88de942d9d9d`는 r172 작업 중 관찰 snapshot이며 최종 release identity가 아니다. 원인 경로는 `egui_plot/items/points.rs:134`, `epaint/tessellator.rs:344/1000/1486/2220`, `egui_plot/plot.rs:952/1319/1676`, `egui/context.rs:2859`, `epaint/mesh.rs:121/147`, `egui/painter.rs:201/226`, `egui/layers.rs`의 최종 transform drain을 기준으로 읽었다.
