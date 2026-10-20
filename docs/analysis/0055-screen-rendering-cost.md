# 화면 글꼴 해상도와 조판 비용

2026-10-16. 앱을 실행하거나 GUI·스크린샷을 보지 않고, 현재 Rust 소스·잠긴 egui/epaint 0.36.2 소스와 headless 테스트로 조사했다. 아래의 글꼴 atlas와 cache 정책은 Mathcraft의 구현 교정이다. 원본 Mathcad가 같은 내부 알고리즘을 사용한다는 주장이 아니다. 기존 REA 글꼴 크기·서식 계약과 독립 STIX 글꼴 사용은 유지한다.

## 화면 글자가 확대된 bitmap처럼 보일 수 있었던 경로

`worksheet_ui::workspace_ui`는 문서를 `TSTransform::from_scaling(self.zoom)` 레이어에서 그린다. 반면 egui `Context::fonts`/`fonts_mut`는 창의 global pixels-per-point로 `FontsView`를 만든다. `TextShape::transform`은 이미 만들어진 행 mesh의 위치와 크기를 확대하며, 글리프를 더 많은 texel로 다시 래스터화하지 않는다. 따라서 OS 배율 1, worksheet zoom 2에서는 1배 atlas 글리프가 화면에서 2배로 늘어날 수 있었다. 같은 종이 좌표를 유지하는 것과 최종 출력 픽셀에 충분한 글꼴 해상도를 확보하는 것은 별개다.

근거는 로컬 Cargo registry의 egui 0.36.2 `context.rs::{fonts,fonts_mut}`, epaint 0.36.2 `shapes/text_shape.rs::TextShape::transform`, `text/fonts.rs::FontsView`, `text/text_layout.rs::layout`다. 원본 앱에 대한 새 실행·동적 관찰 없이 확인 가능한 현재 구현의 결함이다.

`typography::with_worksheet_zoom`과 `layout_job`/`layout_no_wrap`은 worksheet scope의 글꼴을 같은 egui atlas에 최종 화면 밀도로 조판한다. 공개 `epaint::text::layout(fonts_view.fonts, density, job)`를 사용하며, density는 **창 PPP × max(worksheet zoom, 1)** 이다. 축소는 기존 글리프를 downsample해 불필요한 낮은 해상도 사본을 만들지 않는다. 확대는 실제 glyph UV texel 크기를 늘린다. 문서의 font point 값·DIP 좌표나 ribbon의 전역 배율을 늘리는 방식이 아니다.

Galley의 문자 좌표·커서·baseline은 문서 단위로 유지하며, 그 mesh는 기존 레이어 변환으로 화면에 배치된다. `galley.pixels_per_point`는 창 tessellator의 consistency tag로 유지한다. atlas 해상도는 별도 density로 이미 결정됐다. 두 밀도를 혼동해 전역 DPI를 덮어쓰거나 glyph geometry를 한 번 더 확대하지 않는다. 실제 픽셀 격자에 맞추는 과정에서 zoom/DPI별 subpixel rounding 차이는 남는다.

formula, native rich text/table, worksheet TextEdit의 custom layouter와 page field 표시가 이 helper를 사용한다. UI chrome은 기존 조판을 사용한다. PDF는 별도 context의 1배 문서 조판을 사용하므로 화면 확대가 PDF 출력 크기나 vector font size로 전파되지 않는다. 화면의 GPU glyph atlas 자체를 벡터 PDF로 바꿨다는 의미는 아니다.

## 계속 발생하던 CPU 작업

기존 formula cache는 매번 `Arena` 전체와 저장 사본을 비교했다. 크기가 큰 AST에서는 변화 없는 조회도 노드 수에 비례했다. 캐시가 새로 만들어질 때 전체 arena를 복사하기도 했다. 여기에 같은 수식의 모든 hit rectangle을 한 번의 widget 호출에서 입력 전후 각각 다시 만들었다. glyph atlas의 높이가 커질 때도 모든 저장 수식을 재조판했다. atlas의 UV는 texel 좌표이므로 단순 크기 증가는 기존 glyph를 무효화하지 않는다.

다음과 같이 교정했다.

- Arena에 O(1) revision을 추가했다. allocation, 내부 node mutation, retirement, load, Undo/Redo restore에서 새 revision을 발급한다. 복제한 편집기 두 개가 각자 변경될 때도 같은 revision으로 충돌하지 않는다. caret/selection 이동은 revision을 바꾸지 않는다. 구조적 `Arena` equality의 의미와 문서 직렬화 형식은 유지한다.
- formula cache가 revision을 비교하며 arena 사본을 보관하지 않는다. readonly와 interactive 조판은 별도 cache key를 사용해 optional operand 표시 규칙을 섞지 않는다.
- hit geometry는 layout identity와 문서 origin이 바뀔 때 다시 만든다. region을 옮기거나 식을 수정하면 갱신하며, 그대로인 caret/selection frame은 재사용한다.
- `FontCacheKey`는 pixel density·worksheet zoom과 atlas generation을 포함한다. egui가 실제 atlas/글꼴을 재생성했는지는 매 pass stock galley cache의 sentinel Arc로 확인한다. 단순 atlas 크기 증가와 구별해 불필요한 재조판을 줄인다.
- 화면 clip 밖의 수식 글자·선·placeholder primitive는 painter에 넣지 않는다. 시각 bounds와 logical rect를 함께 사용해 공백 및 glyph overhang을 보존한다. 계산 데이터나 AST의 행·열을 제거하는 기능이 아니다.

## 테스트

다음 검사는 native window, GPU, Mathcad, 프린터를 시작하지 않는다.

```powershell
cargo test -p mathcraft-app render_quality --locked --offline
cargo test -p mathcraft-app formula::tests:: --locked --offline
cargo test -p mathcraft-editor arena_revisions --locked --offline
```

글꼴 관련 3개, formula 45개(benchmark 1개 ignored), 새 editor revision 회귀가 통과했다. 글꼴 회귀는 OS PPP 1/1.5/2와 worksheet zoom 0.5/1/1.25/2를 조합한다. 실제 UV texel 크기와 최종 density 조판 결과, 문서 크기의 범위·커서 baseline·chrome 배율, 변환 후 tessellation의 유한 mesh를 확인한다. atlas 성장 시 generation 유지, 글꼴 교체 시 invalidation, 중첩 scope와 panic 시 배율 복원도 검사한다. 수식 회귀는 clone branch·Undo·재조판과 region 이동의 hit 유효성, 화면에 보이는 matrix cell만 painting하는 조건을 포함한다.

## 측정

아래는 Windows의 CPU-side headless 측정이다. GPU presentation, 입력기·창 시스템, 실제 사용자가 느끼는 지연 시간의 측정이 아니다. 시간 값을 correctness assertion으로 쓰지 않는다. 다른 작업과 동시 build가 있었으므로 작은 차이의 인과관계를 과장하지 않는다.

Release `benchmark_retained_formula_matrix`는 64×64 source matrix, 4097개 노드로 cache 조회 2000회와 interactive formula placement 120회를 수행한다. 후자는 한 headless pass 안의 child UI 배치이며 120개의 화면 frame을 뜻하지 않는다.

| 작업 | 수정 전 | 수정 후 |
| --- | ---: | ---: |
| 변화 없는 4097-node formula cache 조회 2000회 | 69.180 ms | 0.118 ms |
| interactive placement 120회 | 92.903 ms | 42.253 ms |

기존 `benchmark_saved_worksheet_idle_and_pointer_hold`도 동일한 viewport와 12×12 저장 결과를 가진 100/1000/4000개 영역으로 재실행했다. source나 결과를 계산하지 않는다.

| 영역 / 120 frames | 수정 전 | 수정 후 |
| --- | ---: | ---: |
| 1000 idle | 102.617 ms | 73.022 ms |
| 1000 pointer held | 98.923 ms | 68.196 ms |
| 4000 idle | 144.817 ms | 135.421 ms |
| 4000 pointer held | 148.791 ms | 171.621 ms |

마지막 조건은 이번 단일 측정에서 느려졌다. 이 결과를 숨기거나 전체 대규모 문서가 빨라졌다고 결론 내리지 않는다. 전체 worksheet 순회·문서 단위 작업과 동시 실행의 영향을 별도로 봐야 한다. isolated formula cache 개선만으로 모든 지연 원인이 제거됐다는 주장은 하지 않는다.

실제 fixture에는 `benchmark_native_saved_worksheet_screen`을 추가했다. `MATHCRAFT_SCREEN_PERF_SOURCE`로 사용자가 지정한 MCDX를 읽고 headless chrome/workspace를 그린다. 기본 ignored fixture `detune-preserved-v1.mcdx`의 SHA-256은 `39d7a6df70460a5281d30f63b5a99a170558d80b8364252f04817b7af51ac87b`다. 87개 영역·66개 저장 결과를 열었으며 모든 조건에서 executed=0, dirty=false를 확인했다. 이 fixture의 수정 전 화면 profile은 없으므로 아래는 **수정 후만**의 기록이다. profile은 최적화된 test/dev profile(opt-level 1, dependencies 2)이며 위 release 값과 직접 비교하지 않는다.

| OS PPP / zoom | 첫 frame | 안정화 후 120 frames |
| --- | ---: | ---: |
| 1 / 1 | 38.278 ms | 45.809 ms |
| 1 / 2 | 9.642 ms | 31.670 ms |
| 2 / 1 | 35.027 ms | 63.904 ms |
| 2 / 2 | 9.475 ms | 30.277 ms |

zoom 2에서는 종이의 일부만 viewport에 들어오므로 조판 밀도가 높아져도 steady-state CPU 시간이 작을 수 있다. 행 수나 DPI가 같지 않은 경우의 FPS 비교로 해석하지 않는다. 첫 context의 font initialization과 확대 시 glyph 생성도 구별한다.

재현:

```powershell
cargo test -p mathcraft-app --release benchmark_retained_formula_matrix --locked --offline -- --ignored --nocapture
cargo test -p mathcraft-app --release benchmark_saved_worksheet_idle_and_pointer_hold --locked --offline -- --ignored --nocapture
cargo test -p mathcraft-app benchmark_native_saved_worksheet_screen --locked --offline -- --ignored --nocapture
```

ignored 근거는 `artifacts/rea/screen-formula-{baseline,after}-20261016.log`, `screen-4k-{baseline,after}-20261016.log`, `screen-native-after-20261016.log`다.

## 남은 범위

실제 GPU texture filtering·display/subpixel 체감 품질, OS 입력 지연, 모든 DPI 모니터 이동, 모든 worksheet 종류의 end-to-end 지연은 GUI 금지 조건에서 검증하지 않았다. label/result 내용 equality는 아직 데이터 크기에 비례하며 전체 worksheet 작업의 다른 원인을 이번 formula 최적화와 구별해야 한다. 확대 중 새 글리프의 최초 생성, 아주 긴 rich text의 조판, atlas가 실제로 재생성되는 frame에는 추가 CPU 비용이 남는다. 문서 종류와 상태를 고정한 추가 profile 없이 이를 원본과 동등한 성능이라고 표현하지 않는다.
