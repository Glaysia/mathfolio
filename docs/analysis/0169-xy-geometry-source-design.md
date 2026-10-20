# XY geometry source 분리 설계

2026-10-19. r167 동결 전의 읽기·설계 기록이다. 제품 Rust, 원본 설치 파일, GUI, 원본 runtime은 변경·실행하지 않았다. 원본 계약은 [0168](0168-native-xy-graph-region-geometry-contract.md)의 정적 CIL/XSD 증거를 사용한다. 아래 API는 아직 구현하지 않은 제안이다.

## 현재 손실과 필요한 최소 경계

`native_plot.rs:28–43`은 graph height와 region height를 `±50`으로 묶고, width는 같은 값으로 취급한다. `payload:176`은 이 파생 graph를 항상 저장한다. `parse_with_references:483`은 owning width/height/actualWidth/actualHeight를 허용하지만 읽지 않고, graph-size를 필수로 요구한다(`:516`). graph의 두 값은 f32→표시 문자열→f64 roundtrip 검사 `model_number`를 거쳐 width와 graph+50 height가 된다(`:571–619`). 그래서 graph-size 부재와 이 숫자 roundtrip 또는 ±50 역산 검사를 만족하지 못하는 유효 double은 현재 Opaque로 남는다. outer와 graph가 독립인 원본을 받아도 outer 요청/실측 값은 모델에 반영되지 않는다.

현재 `worksheet_ui.rs:5805`의 native importer는 이 parser 결과를 그대로 CanvasContent::Plot에 넣는다. 별도 outer 크기 적용은 없다. `plots.rs:537/636`의 화면·인쇄는 spec의 f32 크기를 외곽 allocation으로 쓰고, 내부 plot에는 height−50을 사용한다. 원본의 load/measure 수렴을 구현한 것이 아니다.

`export/plot_source.rs:49–60`은 same_source가 false이면 plot payload를 새로 만들고 owning actualWidth/actualHeight만 없앤다. owning width/height는 패치하지 않는다. `export.rs::new_region:1057–1175`도 새 Plot에 width/height를 쓰지 않는다. 따라서 graph 필드 하나만 추가하면 source 보존은 개선되지만 resize 시 outer request가 오래된 채 남거나, 새 export에 요청 크기가 빠지는 문제가 계속된다.

최소한 다음 세 상태는 구분해야 한다. 단순 `graph_size: Option<Size>` 하나로는 첫 두 상태를 표현할 수 없다.

| 상태 | 의미 | native graph-size 작성 |
| --- | --- | --- |
| 옛 JSON의 geometry 필드 부재 | 기존 Mathcraft source adapter | 기존 export의 width / height−50 정책을 **호환 경로에서만** 유지 |
| 새 native geometry carrier 존재, graph_size 부재 | 원본 graph-size element 부재 | element 생략 |
| 새 carrier와 graph_size 존재 | 원본 explicit f64 source | 보존한 두 값을 작성 |

첫 상태를 두 번째로 재해석하면 기존 `.mathcraft`의 export 의미와 해시가 달라진다. 반대로 두 번째를 첫 번째로 해석하면 원본에 없던 graph-size를 만들어 낸다.

## 제안 모델과 wire

권장 첫 구현은 PlotSpec에 아래 optional source carrier 하나를 추가한다. 별도의 appearance mirror나 매 프레임 raw XML parse는 필요 없다.

```rust
pub struct PlotNativeGeometry {
    pub graph_size: Option<PlotGraphSize>,
    pub requested_width: Option<f64>,
    pub requested_height: Option<f64>,
    pub actual_width: Option<f64>,
    pub actual_height: Option<f64>,
}
pub struct PlotGraphSize { pub width: f64, pub height: f64 }
// PlotSpec의 추가 필드:
pub native_geometry: Option<PlotNativeGeometry>
```

이름은 확정 API가 아니다. `None`은 오직 구형 source adapter다. imported source에는 graph 부재여도 `Some(carrier)`를 둔다. optional double은 원본 specified/absent를 보존하며, actual은 requested로 복사하지 않는다. width/height f32는 기존 앱의 표시·편집 allocation projection으로 남긴다. carrier가 있을 때 native source 권위는 carrier에 있고, f32 반올림값을 source에 되쓰지 않는다. 이 두 층은 목적이 다르므로 setter/검증 없이 임의로 동기화하면 안 된다.

`plot_source_model.rs:480–637`의 custom Serialize/Deserialize를 직접 수정해야 한다. 기존 `#[serde(skip...)]` annotation만으로는 이 serializer에 반영되지 않는다.

* carrier None: 기존 Legacy의 7개 필드와 AxisLists의 5개 필드를 기존 순서·값 그대로 출력한다. 기존 v1 snapshot/계산 서명 prefix 및 JSON 형태를 바꾸지 않는다.
* carrier Some: 기존 마지막 y_axis 뒤 `native_geometry` 필드 하나만 추가하고 SerializeStruct 필드 수를 +1 한다. carrier 내부 필드 순서도 고정한다.
* deserializer는 필드 부재를 None으로 읽고, duplicate 필드는 `Option<Option<...>>` 또는 별도 seen flag로 잡는다. `native_geometry:null`을 부재와 동일하게 받을지 거부할지 명시해야 한다. 첫 구현은 **explicit null 거부**를 권장한다. field가 존재하면 strict object만 허용하여 두 의미를 혼동하지 않는다.
* carrier 및 graph struct는 deny_unknown_fields, missing/중복 dimension 거부. `graph_size:null`은 native element 부재를 뜻하는 carrier 내부 형태로만 허용할 수 있다.
* f64 source는 serde_json float_roundtrip과 bit 비교를 유지한다. old graph source의 f32 decimal-roundtrip/±50 역산 제한을 새 f64 source에 적용하지 않는다. native 숫자 lexeme(예: `210.0` 대 `2.1e2`)까지 보존한다는 뜻은 아니다. unchanged archive는 원본 lexeme bytes를 유지한다.

현재 `canvas_signature.rs`는 Plot을 serde에 그대로 맡긴다. 새 hand-written plot hash writer는 필요 없다. old None JSON이 byte 동일하면 old persisted cache의 SHA도 그대로다. 새 carrier는 presentation signature에 포함되어 서로 다른 geometry source를 같은 서명으로 취급하지 않는다. active/disabled/order/native-archive/settings 계약을 완화하지 않는다.

## 유효 범위와 import

첫 codec 범위는 양수 finite f64 graph width/height, finite 양수 requested dimensions, finite nonnegative actual snapshot이다. 0 graph, negative 값, NaN/±Inf, Size.Empty의 (-Inf,-Inf), MC14 변환·XAML 전용 geometry, 알려지지 않은 attrs/children는 Err→원본 Opaque 보존으로 남긴다. XSD가 허용하는 모든 xs:double을 지원한다고 주장하지 않는다. 부분 width/height specified는 각각 Option으로 보존한다.

source double과 UI projection의 범위도 분리한다. `210.00000000001` 같은 값은 source에서 그대로 유지하고 화면에는 f32로 투영할 수 있다. 현재 앱 allocation 범위 120–2000 ×100–2000 밖의 outer request, f32 cast overflow나 비유한 projection 등은 첫 구현에서 raw fallback으로 남기는 것이 안전하다. clamp한 결과를 원본 크기라고 저장해서는 안 된다. graph source에 별도 예산 상한을 둘 경우 그 상한과 raw fallback을 명시하며, 원본 보편 제약이라고 설명하지 않는다.

native parser는 ordered children을 traces / optional graph-size / axes로 읽고 중복 graph-size·순서 위반·attr 누락을 계속 거부한다. explicit graph는 `f64` 전용 reader로 읽는다. 일반 line-weight 등 기존 f32 property reader는 건드릴 필요 없다. parent region geometry는 같은 DOM에서 읽어 carrier에 담는다. namespace, comment/PI, node/byte budget, unsupported metadata 보존 가드는 유지한다.

import의 표시 크기는 requested가 지정된 축에 그 요청의 안전한 f32 projection을 적용한다. 지정되지 않은 축은 기존 앱의 명시된 fallback layout policy를 사용한다. 이 fallback을 graph+50이나 actual→requested source 승격으로 저장하지 않는다. actual snapshot을 미지정 외곽의 표시 힌트로 사용할지는 별도 앱 정책으로 남긴다. 원본 최종 WPF layout을 알아냈다고 주장할 수 없으므로 첫 모델/codec 변경과 그 시각 정책을 한 번에 섞지 않는 편이 낫다.

공유 parser 하나에서 이 처리를 해야 `import_native`, CacheIndex의 parse_with_references, own-file 재열기의 native warning 복원이 서로 다른 크기를 비교하지 않는다. ParsedPlot의 spec에 carrier를 채우면 기존 importer 호출부는 기본 경로를 유지할 수 있지만, allocation projection 적용 여부를 명시하는 helper가 필요하다.

## API와 source 변경 판정

권장 작은 API 경계는 다음과 같다.

* `PlotNativeGeometry::validate()`와 bit-exact 비교 helper.
* `PlotSpec::set_outer_size(width, height)` 또는 원자적 resize helper: UI projection과 **requested** source만 갱신하고 stale actual snapshot은 제거한다. graph source를 자동으로 height−50으로 만들지 않는다.
* `PlotSpec::set_graph_size(size)`는 별도 명시적 source edit. 실제 graph rect를 확보한 호출자만 사용한다. 현재 egui_plot response가 plot body 전체인지 inner plotting area인지 검증하기 전에는 source 갱신에 쓰지 않는다.
* `native_plot::same_source`는 현재의 trace/style/equation shape/legend 비교에 geometry를 추가하되, 모든 explicit source 값과 absence를 bit-exact 비교한다. opaque unknown을 known source로 간주하지 않는다.
* `native_plot::matches_retained_source` 같은 별도 export/import 호환 helper는 구형 carrier None 문서와 원본 archive 비교를 담당할 수 있다. 일반 same_source에 None==Some 기본값 규칙을 넣어 source 권위를 약화시키지 않는다.

마지막 helper가 중요한 이유는 기존 own 문서에 native_archive와 구형 PlotSpec/valid cache가 함께 저장돼 있기 때문이다. 새 parser는 그 archive에 explicit geometry를 넣지만 기존 저장 모델에는 없다. 양쪽을 무조건 다르다고 하면 편집하지 않은 원본 curve의 raw reuse가 사라진다. 반대로 carrier None을 모든 explicit geometry와 같다고 하면 resize/source guard가 무력해진다. bounded compatibility 경로는 **동일 archive source를 기존 reader 규칙으로 투영할 수 있는 경우에만** old width / graph-height+50 및 기존 식·style·axis 계약과 정확히 대조한다. 기존 reader가 거부했던 missing graph/f32 precision loss에는 이 경로를 적용하지 않는다. 성공해도 현재 모델·JSON·cache를 자동 migrate하지 않는다.

생각할 수 있는 더 작은 첫 단계는 graph만 tri-state로 모델링하고 outer attrs를 archive에만 남기는 것이다. 그러나 새 문서/복사/resize export에 outer specified를 재생성할 권위가 없고 actual도 carrier 없이 손실되므로, 0168의 source 분리 계약을 완료하는 설계로는 부족하다.

## export와 결과 보존

`payload()`는 carrier Some의 graph_size None이면 element를 생략하고 Some이면 f64 source만 기록한다. carrier None에서만 구형 생성 정책을 호환 유지한다. 새 blank 생성은 carrier Some+graph absent를 사용할 수 있지만, 이는 원본 final layout 복원과 다르며 blank 생성/renderer 배치 담당과 별도 합의가 필요하다. old Default/legacy samples는 변경하지 않는다.

`export/plot_source.rs::edit`은 source와 geometry를 구분해야 한다. 단순 move/unchanged geometry는 기존 wrapper와 payload bytes를 유지한다. outer size edit는 requested width/height를 패치하고 stale actual snapshot을 제거한다. graph/식/style edit는 새 payload를 쓰며 source-changed resultRef 정책은 현재 보수적 계약을 유지한다. 식만 고쳤다고 outer request를 지우거나 source f64를 UI 반올림값으로 덮지 않는다. unchanged native actual source는 그대로 유지하고, 어떤 편집이 actual snapshot을 무효화하는지는 명시한다.

`export.rs::new_region`은 새 Plot의 requested dimensions를 wrapper에 기록해야 한다. carrier의 요청이 없으면 생략한다. 구형 None source에는 기존 spec width/height에서 요청을 작성할지 old no-wrapper export를 유지할지 별도 호환 결정이 필요하며, **첫 패치는 old export bytes 유지**를 권장한다. actual은 실측을 확보하지 않은 fresh 문서에서 합성하지 않는다.

native result reuse의 마지막 guard는 `export_into`의 unique_results / unique_references / ContextSnapshot / same_calculation_program이다. 현재 program proof는 owning GEOMETRY를 제외하지만 plot payload의 graph-size는 canonical source에 남는다. 이를 이번 패치에서 완화하지 않는다. graph edit가 결과를 Pending으로 만드는 현 보수적 정책은 유지하며, outer-only edit의 resultRef 재할당 여부를 바꾸려면 별도 proof가 필요하다. CacheIndex 및 native_plot_warnings의 same_source 호출도 새 carrier/legacy helper와 함께 점검해야 한다. 원본 결과를 화면에 쓰기 위해 geometry 차이를 무시하는 편법은 금지한다.

## 필요한 파일과 검증

| 파일/범위 | 최소 변경 |
| --- | --- |
| document/canvas.rs + 새 plot geometry module | optional carrier, f64 types/re-export, validation/원자적 setter |
| document/plot_source_model.rs | custom wire, None old bytes/default, new strict field reader |
| document/native_plot.rs | optional graph, f64/outer source reader·writer, strict comparison, bounded legacy adapter |
| document/export/plot_source.rs + export.rs | wrapper requested/actual patch, fresh region dimensions, result/source guard |
| app/worksheet_ui.rs native importer | authoritative outer→display projection 확인 |
| app/plots.rs + plots/inline_edit.rs + worksheet_ui/inline_plot_ui.rs | direct resize assignments를 setter로 연결; graph rect와 outer 분리; Undo 원자성 |
| app/worksheet_ui/native_plot_source.rs + native_plot_warnings.rs | archive compatibility 비교; stale/source-mismatch cache 승격 금지 |
| app print/page-break bounds | display size getter 재사용; 새 source를 f32로 덮지 않음 |

구현 순서는 (1) wire/codec와 독립 oracle, (2) export 및 native cache source guard, (3) resize/import/print 소비자 연결이 안전하다. 중간 단계만 제품에 채택해 source를 묵시적으로 잃는 상태로 동결하지 않는다.

필수 headless oracle:

1. 기존 Legacy/AxisLists JSON 독립 struct oracle, golden SHA, 활성/disabled signature, old cache 저장·재열기 bytes 불변. duplicate/native_geometry null/unknown/missing field 거부.
2. graph absent / explicit f64 / outer requested absent·partial·explicit / actual absent·0·explicit의 조합. `outer 600×400`, `graph 321.00000000001×217.125`, `actual 598.75×399.5`가 import→own save→native export→reimport에서 서로 덮지 않는지 검사.
3. absent graph와 explicit default가 다름; explicit +0/−0 unsupported 범위, NaN/Inf/negative/Size.Empty/raw attrs를 Opaque로 보존. UTF16 source와 unchanged native result/XML bytes 유지.
4. 구형 own source+archive+valid persisted cache의 no-edit reuse, 실제 식/graph/outer resize 변화의 no false-match, 없는 graph/old f32 roundtrip loss에는 legacy compatibility 우회 불가.
5. single resize Undo/Redo가 requested/graph/actual/projection 전체를 정확히 복원. 단순 move·문서 open·draw/print는 source geometry를 갱신하지 않고 계산 실행0회.
6. typed explicit geometry를 출력한 MCDX original XSD 검증, native resultRef ordinal/중복·오류·Pending guards 유지, vector PDF에 추가 bitmap/잘못된 page bounds 없음.

이번 기록은 실제 비용 측정이나 source 구현 완료를 뜻하지 않는다. finite source 보존은 원본 WPF의 임시 axis freeze, actual layout feedback, tolerance, MC14 변환, 모든 drag/legend gesture까지 재현한다는 주장과 분리한다.
