# XY 곡선 렌더링 비용 감사와 측정 설계

2026-10-19. r156 검증 완료 후 먼저 **읽기 전용 소스 감사와 구현 제안**을 작성하고, 승인된 후속 작업에서 아래의 prepared point 구현과 headless 비교를 진행했다. 최초 감사/설계와 실제 측정 결과를 구분한다. 원본 Mathcad 런타임이나 GUI는 사용하지 않았다. 앞부분의 byte 표는 소스로 도출한 배열 payload이며, 실측 결과는 마지막 절에 별도로 기록했다.

## 확인한 비용

`plots.rs::draw_trace`는 실제 `draw`와 `draw_print` 양쪽에서 모든 trace마다 호출된다.

| 현재 코드 | steady frame에서 하는 일 |
| --- | --- |
| `plots.rs:556` `Line::new(..., trace.points.clone())` | line이 있고 point가 둘 이상이면 전체 `Vec<[f64;2]>`를 매번 복제한다. source/result가 바뀌지 않은 idle도 같다. |
| `plots.rs:566` `step_by(frequency).copied().collect()` | marker가 있으면 표본 배열을 매번 새로 만든다. frequency=1도 전체 point 수에 비례한다. line과 marker를 함께 켜면 두 경로가 모두 실행된다. |
| `draw_trace`의 이름 생성 | 빈 label의 `format!("trace {}")`, label 복제, line용 name 복제도 frame마다 수행한다. point 배열과 별도인 작은 비용이다. |
| `PlotData::points` 첫 trace alias 복제 | 계산 결과 생성/저장 복원 때 생기는 별도 보관 비용이다. `draw`의 매 frame clone과 혼동하지 않는다. |

`egui_plot`은 현재 `Cargo.lock`의 0.37.0이며 registry checksum은 `90f3711a5032bb65a41011b23fcdc6c4bb599757b82cc0181d41e7d1de16986a`다. 로컬 dependency 소스에서 확인했다.

* `data.rs`의 `PlotPoints<'a>`는 `Owned(Vec<PlotPoint>)`, `Generator`, `Borrowed(&'a [PlotPoint])`를 지원한다. `Line::new`와 `Points::new` 모두 이를 받는다.
* `Vec<[f64;2]>` 입력은 `PlotPoints::new` → `FromIterator` → 각 원소 `PlotPoint::from`으로 넘어간다. 최적화된 iterator가 입력 Vec 할당을 재사용할 수 있으므로 **line마다 반드시 두 번 할당된다**고 주장하지 않는다. 앞단 `trace.points.clone()`의 배열 복제 자체는 명확하다.
* 현재 borrowed 어댑터는 `&[PlotPoint]`이고 `&[[f64;2]]`는 아니다. `PlotPoint`에는 `repr(C)` 등 배열과 layout 동일성을 보장하는 선언이 없다. 크기가 같아 보인다는 이유로 unsafe slice cast를 넣지 않는다.
* `items/series.rs::Line::shapes`는 각 point를 화면 좌표로 변환해 `values_tf: Vec<_>`를 만든다. `Line::find_closest`는 pointer hover에 대해 인접 segment를 순회한다. `bounds()`도 표본을 읽는다.
* `PlotUi::line/points`는 item을 Box로 보관한다. marker shape 생성·egui tessellation도 별도 작업이다. 따라서 borrowed 입력으로 바꾸어도 전체 frame의 할당이나 O(N) 작업이 사라지는 것은 아니다.

감사한 제품/외부 dependency 파일의 전체 SHA256과 byte 수는 [`xy-render-cost-audit-20261019.json`](../../artifacts/rea/xy-render-cost-audit-20261019.json)에 기록했다. 이것은 현재 읽은 파일의 identity이며 새 benchmark 산출물은 아니다.

## source만으로 확정할 수 있는 규모

원래 point가 `[f64;2]`이므로 point payload는 16 byte다. 표의 값은 line 입력 clone이 복사하는 좌표 payload의 합이며 allocator overhead·capacity slack·line shape·mesh·font를 제외한다.

| points/trace | traces | line 입력 clone payload/frame | marker frequency=1 추가 payload/frame |
| --- | --- | --- | --- |
| 500 | 1 | 8,000 B | 8,000 B |
| 2,000 | 1 | 32,000 B | 32,000 B |
| 500 | 16 | 128,000 B | 128,000 B |
| 2,000 | 16 | 512,000 B | 512,000 B |

line이 None이면 line clone은 없다. marker가 None이면 marker 배열 수집은 없다. marker frequency=f의 표본 수는 비어 있지 않은 데이터에서 `ceil(N/f)`다. single-point fallback marker도 별도 대조 대상으로 둔다. 이 표를 RSS, frame당 총 heap 할당량 또는 시간 절감 예상치로 부르지 않는다.

## 권장하는 첫 변경 범위

계산 결과의 기존 `Vec<[f64;2]>`, 저장 JSON, trace 순서·스타일·범위·implicit-domain을 그대로 둔다. renderer 전용 준비 데이터만 추가한다.

1. 곡선 결과를 설치/복원한 시점에 trace별 `Vec<egui_plot::PlotPoint>`를 한 번 만든다. root의 result owner와 명시적인 curve epoch/version을 사용한다. 매 frame 전체 point를 hash하거나 값 비교해서 유효성을 확인하지 않는다. 포인터 주소/Vec capacity만으로 cache identity를 삼지 않는다.
2. `draw_trace<'a>`에 준비된 trace를 빌리고 `PlotPoints::Borrowed(prepared.points.as_slice())`를 line에 전달한다. plot closure가 끝날 때까지 owner가 살아 있도록 lifetime을 연결한다. callback에서 원본 함수를 다시 평가하는 generator로 대체하지 않는다.
3. marker frequency=1은 같은 borrowed full slice를 쓴다. frequency>1은 curve epoch+frequency가 바뀔 때만 정확한 `step_by` subset을 준비한다. line/색/marker 모양만 바뀌면 좌표를 다시 만들 필요가 없다. frequency=0은 기존 모델 validation으로 계속 거절한다.
4. 최초 구현은 parent가 보유한 준비 cache를 borrow하는 경로가 가장 명확하다. egui temp memory에 넣어야 한다면 작은 Arc를 복제하고 payload Vec를 frame마다 clone하지 않는다. owner 수명 관리와 용량 제한을 명시한다.
5. 계산·native/own saved restore·Undo/Redo·region 제거/문서 교체·trace 수/순서 변경에서는 올바르게 갱신/제거한다. inline typing과 pan/zoom은 committed curve를 바꾸지 않으므로 준비 좌표를 재생성하지 않는다. 새 source가 Pending인 수동 모드도 이전 표시 curve와 그 준비 데이터를 함께 유지한다.

이 변경은 source/curve 의미를 바꾸지 않고 기존 egui_plot의 pan/zoom/hover/legend/line 스타일 처리에 맡긴다. 전체 screen-space shape/mesh cache는 첫 변경에 넣지 않는다. transform, auto-bounds, clip/size, DPI, hover highlight, legend 숨김, marker/style 등이 shape를 바꾸므로 identity 설계가 더 크다. 현재 불필요한 입력 복제를 먼저 분리해 측정한다. 다운샘플링, 표본 삭제, 재정렬, float32 저장, 동일 식 평가 memoization은 이 과제 밖이다.

## 측정 설계

기본 행렬은 500/2,000 points × 1/16 traces × idle/pointer/typing = **12 case**다. 구체 machine-readable 입력과 assertion 목록은 위 JSON에 들어 있다.

* fixture는 finite 합성 표본을 미리 만들고 bounds를 고정한다. source는 실제 PlotSpec, curve는 이미 계산된 PlotData로 시작한다. 같은 payload를 baseline과 candidate에 준다.
* idle: pointer를 도표 밖에 두고 input event 없이 반복한다. curve가 표시되는 clip 영역과 stable UI ID를 유지한다.
* pointer: 도표 안의 고정된 경로에 `PointerMoved`만 넣는다. click/drag/wheel 없이 hover 비용을 분리한다. pan/zoom은 별도 기능 대조로만 추가한다.
* typing: primary field에 실제 focus를 주고 Text/키 이벤트로 draft만 편집한다. Enter/focus-exit 없이 이전 curve를 유지한다. source와 input 길이가 다른 반복 사이에서 늘어나지 않도록 동일한 script를 기준 상태에서 재생한다. curve 생성·result JSON·fixture clone 비용은 계측 구간 밖에 둔다.
* 스타일 대조: line None+marker None, solid+circle frequency1/10, one-point fallback, dotted/dashed를 따로 측정한다. 기존 marker 수와 line 모양을 유지한다.
* context/font/layout를 최소 10 frame warm하고 120 frame을 모은다. frozen baseline/candidate 실행 파일을 5회 교대한다. OS별 수치를 분리하고 Windows/peets를 직접 비교한 speedup으로 부르지 않는다. 동시 Cargo/다른 CPU benchmark를 금지한다.
* `Context::run_ui`와 `Context::tessellate` 시간을 나눠 기록한다. output drop/free는 계측 제외인지 별도 section인지 명확히 한다. print path는 별도 once/snapshot 대조이며 idle UI 수치에 섞지 않는다.

### allocation을 측정하는 방법

별도 opt-in headless probe binary에 System allocator를 감싸는 counting GlobalAlloc을 둔다는 **제안**이다. 제품 allocator를 바꾸거나 일반 병렬 test의 전체 process allocation을 결과로 쓰지 않는다. thread-local gate를 미리 초기화하고 측정 thread만 센다. `alloc`, `alloc_zeroed`, `realloc`의 호출 수와 요청 byte, `dealloc` 호출 수를 따로 기록한다. fixture·input events·통계 JSON 생성은 gate 밖에서 한다. realloc 요청 byte의 합을 live memory/RSS라고 쓰지 않는다.

두 층을 나눠야 한다. 첫째 입력 어댑터 microprobe는 원본 배열 clone/변환과 prepared borrowed 전달을 비교하여 backend shape 비용과 분리한다. 둘째 실제 `draw`+tessellation은 frame 전체 비용을 측정한다. allocation instrumentation과 uninstrumented elapsed-time 실행도 분리한다. steady frame에서 **input point conversion/build 0회**를 목표로 하며 **entire egui allocation 0회**를 목표로 하지 않는다.

### 의미 보존과 계산 0 검사

headless draw-only 검사만으로 parent calculator가 0회라고 증명하지 않는다. 실제 worksheet CPU frame 검사도 붙인다. `inline_plot_ui`의 기존 실제 입력 회귀 경로를 재사용한다.

* 12 case 모두 mesh vertex position/UV, clip rect가 finite이며 mesh index가 vertex 범위 안인지 확인한다. 유효한 line case는 visible curve primitive가 있어야 한다. 출력이 사라져서 빠른 결과를 허용하지 않는다.
* source PlotSpec, 원본 PlotData의 모든 좌표/순서/스타일/implicit-domain을 bit 단위로 보존한다. baseline/candidate가 같은 입력에서 같은 egui primitive를 내는지 geometry/style 대조도 한다. snapshot을 위한 복제·직렬화는 측정 구간 밖이다.
* Manual 설정에서 idle/hover/typing 후 submitted generation, last_executed_regions, pending set, saved result marker가 바뀌지 않아야 한다. preparation/evaluator submission counter도 0을 확인한다. typing은 inline draft만 바뀌고 owner Undo가 늘지 않는다. Enter 후 commit/Pending·명시 F5의 기존 동작은 별도 기능검사로 보존한다.
* cache 준비 카운터는 warm idle/hover/typing에서 0이다. 곡선 교체, 동일 owner ID의 다른 curve, Undo/Redo, 문서 교체, trace 순서 변경, marker frequency 변경, 제거 뒤 ID 재사용을 각각 검사한다. stale 좌표를 빌리는 false hit를 금지한다.

## 최초 감사의 결론 (구현 전)

매 frame 원본 point 배열 복제와 marker subset 수집은 현재 소스에서 확정됐다. egui_plot의 borrowed API를 사용하려면 안전한 PlotPoint 준비 저장소가 필요하다. 제품 코드를 고치지 않았고 benchmark를 실행하지 않았으므로 실제 총 frame 시간·allocation 감소율은 아직 알 수 없다. backend의 화면 변환·hit testing·tessellation이 병목의 대부분일 가능성도 측정으로 구분해야 한다. 새 proposal은 원본 Mathcad 렌더링 구현이나 원본 성능과 동일하다는 주장이 아니다.


## 승인 후 반영한 구현

소유 `plots.rs`와 새 `plots/render_points.rs`에 `CurvePoints`를 추가했다. 원래 raw `Vec<[f64;2]>`를 이동해 Arc 안에 두고, 변경 가능한 Vec 접근을 제공하지 않는다. clone은 Arc를 공유한다. 따라서 `PlotData.points`의 첫 trace 호환 alias도 raw 좌표를 다시 복사하지 않는다. 기존 외부 native cache reader의 단일 생성 지점은 root가 `.into()`로 적응했다. JSON serializer는 raw slice를 써 기존 version/field/number 표현을 유지한다.

full `Vec<PlotPoint>`는 OnceLock으로 첫 렌더링 때 한 번 만든다. line과 frequency1 marker는 같은 full slice를 빌린다. frequency>1은 Mutex 안에 현재 frequency 하나와 Arc subset만 보관한다. frame에서는 고정 16-slot stack 배열로 sparse Arc를 붙잡고 plot closure에 slice를 빌려준다. 전체 점 배열의 복사·hash·비교를 frame cache 판정에 쓰지 않는다. frequency가 바뀌면 subset만 갱신하며, 이미 빌린 이전 Arc는 유효하다. 동일 raw 버퍼를 여러 trace가 서로 다른 frequency로 동시에 공유하면 이 한-entry sparse cache가 번갈아 재생성될 수 있다. 현재 계산 경로는 pair마다 별도 결과 버퍼를 만들며, 이 제한을 숨기려고 무제한 map을 만들지는 않았다.

`PartialEq`는 raw 값만 비교한다. Arc identity shortcut은 NaN이 없는 버퍼에서만 사용해 기존 Vec의 NaN 비반사성을 유지한다. ±0의 equality와 실제 저장 bit는 구분해서 보존한다. Debug도 준비 cache가 아닌 raw point를 출력한다. evaluator, plot source, 표본 수·순서, 스타일, native/own JSON 형식, 캐시 source signature는 바꾸지 않았다. pan/zoom/hover/화면 변환/tessellation은 기존 egui_plot 경로다.

새 회귀는 lazy 준비와 clone 공유, frequency 교체 뒤 기존 reader 생존, NaN/Infinity/±0 비교, 원래 saved JSON 전체 문자열과 `-0.0`/`1e-9` lexeme, 복원 시 cache 미준비 상태를 확인한다. 기존 oversized/nonfinite/malformed saved-data 검사를 유지했다. 소유 plots 검사 **28 PASS**, root가 작성한 실제 16-trace warm/hover/Text 후 source·curve·saved·history·submitted generation·Pending 불변 및 계산0 회귀 **1 PASS**, app all-target Clippy와 소유 rustfmt 검사가 통과했다. 이 숫자는 다음 전체 OS 통합 gate와 별도다.

## 고정 harness의 Windows 비교 결과

[`xy-render-cost-probe-20261019/summary.json`](../../artifacts/rea/xy-render-cost-probe-20261019/summary.json)에 전 case의 원시 결과·요약이 있다. 같은 main harness를 사용한 baseline/candidate와 allocation 활성/비활성 **4개 실행 파일**을 먼저 고정하고 SHA256을 `frozen-harness.json`에 기록했다. baseline은 수정 전 `plots.rs`/`inline_edit.rs` 사본이다. 측정 중 Cargo나 다른 CPU benchmark를 실행하지 않았다.

20 case는 설계의 기본 12 case와 각 크기/trace 조합의 marker frequency1/10 idle 대조 8개다. case당 10 warm frame 뒤 120 frame을 측정했다. allocation 비활성 실행 파일을 baseline/candidate 순서를 번갈아 5쌍 실행하고, 별도 counting allocator 실행 파일로 동일 입력을 각각 한 번 재생했다. 표의 시간은 각 실행 내 median의 5-run median이다. drawing과 tessellation을 분리했으며 fixture 생성·JSON validation·geometry fingerprint·output 해제는 계측 구간 밖이다.

2,000 points × 16 traces의 결과:

| 조건 | draw 이전 ms | draw 이후 ms | draw 요청 allocation B/frame 이전→이후 | allocation 호출/frame 이전→이후 |
| --- | --- | --- | --- | --- |
| idle | 0.3599 | 0.2815 | 795,234.0 → 283,242.0 | 205.0 → 189.0 |
| pointer_hover | 0.7123 | 0.5934 | 796,564.7 → 284,572.7 | 217.0 → 201.0 |
| inline_typing | 0.3399 | 0.3096 | 797,127.8 → 285,135.8 | 219.9 → 203.9 |
| idle / marker1 | 2.3676 | 2.1895 | 3,869,026.0 → 2,845,034.0 | 237.0 → 205.0 |
| idle / marker10 | 0.4994 | 0.4192 | 1,104,226.0 → 541,034.0 | 237.0 → 205.0 |

요청 allocation은 thread-local allocator의 alloc/alloc_zeroed 요청량 합계다. realloc은 JSON에 별도 기록했으며 이를 RSS·live heap이라고 부르지 않는다. line-only 16-trace case의 allocation 호출이 frame당 16회 줄었고, line+marker case는 32회 줄었다. 이는 point input 복사 제거와 일치하지만 frame 전체 allocation은 남아 있다.

속도 개선은 조건마다 다르다. 예를 들어 500-point 1-trace marker1의 draw median은 0.1016→0.1149ms로 오히려 커졌고, pointer 대조는 0.0517→0.0518ms로 거의 같았다. 가장 큰 marker1 case의 tessellation median은 40.7815→40.4642ms여서 draw 입력 복제 제거만으로 전체 곡선 화면 비용을 크게 줄였다고 볼 수 없다. GPU·전체 scene frame 시간을 측정하지 않았다. 측정한 drawing section의 차이를 제품 전체 반응성 개선율로 확대하지 않는다.

20 case × timing 10회 + allocation 2회의 **finite mesh 검사, 유효 index, visible vertex 존재, source/saved-curve 불변**이 모두 통과했다. 각 case의 baseline/candidate geometry FNV64 fingerprint와 vertex 수가 모두 일치했다. FNV는 test fingerprint이고 cryptographic/exhaustive 동등성 증명은 아니다. 실제 좌표/JSON의 의미 보존은 별도 단위·통합 회귀로 보강했다.

### cold와 메모리의 대가

lazy 준비는 첫 draw에서 비용을 낸다. 2,000×16 marker10 case의 cold draw median은 1.4944→1.5681ms, marker1은 3.7011→3.7812ms였다. 반대로 line-only idle은 1.4380→1.2869ms였다. 여기에는 cold context/font/layout도 포함돼 있으므로 point 준비만의 비용으로 분리 해석하지 않는다. raw Arc wrapping의 최초 NaN 검사 O(N), fixture 생성, 실제 파일 import 전체 비용은 이 cold-draw 수치에 포함되지 않았다.

모든 trace가 렌더링된 뒤 full prepared 좌표는 16N×trace_count byte를 추가로 유지한다. 호환 first-trace alias의 raw payload 복제 N×16 byte는 없어졌다. 2,000×16 line-only에서 baseline raw 좌표/alias payload는 544,000B, candidate raw+prepared는 1,024,000B로 **480,000B 증가**한다. marker10의 준비 subset 51,200B까지 포함하면 1,075,200B다. 1-trace의 full prepared payload는 없어진 alias 복사와 좌표 byte 수가 같지만 Arc/OnceLock/Mutex 등 metadata overhead는 추가된다. 이 값은 payload 모델이며 실제 heap/RSS가 아니다. cache를 아직 쓰지 않은 상태에서는 full/sparse 준비 배열이 없고, 마지막 Arc가 해제되면 함께 해제된다.

이 tradeoff를 감수해 반복 입력 복사를 없앴다. persistent prepared 메모리를 추가하면서 기존 raw 배열·saved JSON을 유지한 설계이므로, 데이터 크기가 커져도 공짜라는 주장은 하지 않는다. 현재 지원 한도(최대 2,000 points/trace, 16 traces)와 bounded sparse cache가 범위를 제한한다. Linux의 동일 제품 correctness는 다음 root 통합 gate에서 확인하며, 본 절의 timing/allocation은 Windows 실측만 기록했다.
