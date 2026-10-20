# 0191 — 실제 LCC의 열기·입력과 workspace CPU

2026-10-19. 제공된1015/1016 원본으로 [r203 통합](0189-radix-and-string-sharing-integration.md)의 page_ui 경로와 [r204 통합](0192-affine-temperature-integration.md)의 실제 workspace_ui 경로를 별도로 측정했다. r204는 승인된 고정 소스이며 이후 IME 제품 작업과 이 측정의 소스 provenance를 섞지 않는다. workload가 달라 최적화 전후 비교나 체감 완료율이 아니다. [r203 전체 기록](../../artifacts/rea/lcc-latency-r203-20261019/README.md)에 실행체·원본 hash·10개 프로세스·360사례와 observer 차이를 보존했다. 최초 parser는 libtest 접두어 때문에 첫 JSON을 놓쳤지만 성공한36사례 원시 출력을 유지해 다시 해석했다. 그 프로세스를 재실행하거나 측정 표본을 교체하지 않았다.

대표 Text 사례의 5회 median은 열기+첫 페이지 약40ms, 따뜻한 Text 약0.5ms, Enter 약3ms였다. 첫 페이지의 glyph_layout 약14ms와 egui_wrapper 약6–7ms가 지배했다. 단, 매 사례마다 새로운 Context를 만들고 configure/font 파일 로드는 timer 밖이었으며, page_ui를 직접 호출했다. 반복 idle·zoom/ScrollArea·tessellation·GPU 지연 측정으로 확대하지 않는다. worker_remaining은 worker CPU가 아닌 polling wall wait이다. 상세 frame observer OFF에서도 PREP collector는 켜져 있었다.

원본 두 파일의 저장 result.xml에는 Matrix/String 결과가 없었고 소스의 실제 Math50 Matrix는10×1 표 열6개였다. [행렬 소유권 감사](../../artifacts/rea/numeric-matrix-sharing-next-20261019/README.md)를 근거로 String/Matrix 공유 표본을 LCC 버벅임의 주원인으로 주장하지 않는다.

후속 r204 검사는 실제 workspace_ui의 ScrollArea·layer transform·글꼴 zoom 경로를 호출했다. 새 Context와 같은 Context 재열기, zoom1/1.5/2, 첫 frame·10회 idle·실제 wheel 이동·안정 후10회 idle와 tessellation을 분리했다. Layer scale bits와 실제 offset/visible IDs 변화를 검사하며, 원문·결과·Pending·selection·generation·실행0회·Undo/Redo 불변을 지켰다. [초안 v1](../../artifacts/rea/lcc-workspace-profile-next-20261019/README.md)과 [v2](../../artifacts/rea/lcc-workspace-profile-next-20261019/v2/README.md)는 준비 당시 기록이다. correctness smoke 뒤 새 r204 소스를 동결해 측정했으며 숫자 개선을 주장하지 않는다.

v2는 egui FullOutput을 먼저 폐기한 뒤 검증해 최초 assertion 실패를 두 번째 drop panic으로 숨기지 않는다. 두 프로파일 테스트는 ignored이며 일반 기능 성공 수에 넣지 않는다. 측정할 때는 새 source manifest/archive/harness 해시와 CPU quiet 구간을 따로 고정한다. GUI·스크린샷·원본 Mathcad/CLR·렌더러·GPU 실행은 사용하지 않는다.

Main correctness smoke는 [OFF12사례](../../artifacts/rea/lcc-workspace-off-smoke-preflight2-20261019.log)와 [ON12사례](../../artifacts/rea/lcc-workspace-on-smoke-preflight-20261019.log)가 통과했다. 최초 [실패](../../artifacts/rea/lcc-workspace-off-smoke-preflight-20261019.log)는 제품 문제가 아니라 ID 재현 오류였다. egui ScrollArea는 IdSalt를 저장한 뒤 UI id에 결합하므로 직접 문자열을 넘기는 ID와 다르다. main은 같은 IdSalt를 결합하도록 수정했다. 이 smoke의 숫자는 quiet 성능 표본에 넣지 않는다.

## r204 고정 소스·실제 workspace 측정

[전체 기록](../../artifacts/rea/lcc-workspace-r204-20261019/README.md), [소스·실행체 identity](../../artifacts/rea/lcc-workspace-r204-20261019/identity.json), [명령·UTC·raw receipt](../../artifacts/rea/lcc-workspace-r204-20261019/execution.json), [전체 OFF/ON 요약](../../artifacts/rea/lcc-workspace-r204-20261019/summary.json), [counter 요약](../../artifacts/rea/lcc-workspace-r204-20261019/counter-summary.json)을 보존했다.

| 대상 | SHA-256 |
|---|---|
| r204 archive | `56cb9d7d23603f22bf39cf18b902fdc22042f3ca640e3db00ad2239fae188149` |
| source manifest | `6a8f5065075e70c4e9fd9d30716d7a38ba88b1ed7d9380efddcb037ae4b85f7f` |
| 고정 test harness | `2334037278af986e70a64319697e2d15cb3921bf794c6613a05cc2b3019074c0` |
| 실제 profile source | `1cfc6e395990d7fe0692076f9e17c4e12274d9ed6fcf3655ca59983b0c354df5` |

Root/A/C가 CPU 작업을 멈춘 `2026-10-19T14:27:30.610323Z`–`14:27:39.102705Z`에 OFF/ON, ON/OFF, OFF/ON, ON/OFF, OFF/ON으로5회 교대했다. 실행 전 현재 제품 파일을 고정 manifest와 대조했다. 같은 실행체의 observer 차이이며 제품 전후 비교가 아니다. 원본2개×zoom3개×lifecycle2개를 각 프로세스에서 실행해 **120사례·5,160 workspace frame**이 통과했다. ON의 열기+frame **2,640개 기록**은 exclusive 합+gap=total을 만족했다. 각 사례는 첫 frame·10 idle·pointer 위치·wheel·20 안정화·10 scrolled idle로43 frame이었다.

아래는 OFF의 ms 단위 수치다. 각 프로세스 안의 stage median을 구한 뒤5개 프로세스의 median을 취했다. 스크롤 최대는 각 프로세스의 이동/안정화 frame 최대값을 먼저 구한 뒤 median을 취했다.

| 원본 | zoom | Context | 열기 | 첫 frame 전체 | 초기 idle | wheel | 스크롤 최대 | 안정 후 idle | idle tessellation |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|
| 1015 | 1 | 새 Context | 18.082 | 27.102 | 0.554 | 0.559 | 2.620 | 0.502 | 0.179 |
| 1015 | 1 | 같은 Context 재열기 | 16.731 | 3.401 | 0.606 | 0.635 | 1.161 | 0.531 | 0.188 |
| 1015 | 1.5 | 새 Context | 17.719 | 22.707 | 0.386 | 0.400 | 1.853 | 0.373 | 0.111 |
| 1015 | 1.5 | 같은 Context 재열기 | 16.631 | 2.784 | 0.394 | 0.418 | 0.631 | 0.369 | 0.111 |
| 1015 | 2 | 새 Context | 16.925 | 18.321 | 0.245 | 0.242 | 1.578 | 0.250 | 0.067 |
| 1015 | 2 | 같은 Context 재열기 | 16.082 | 1.167 | 0.237 | 0.293 | 0.683 | 0.249 | 0.066 |
| 1016 | 1 | 새 Context | 17.566 | 25.283 | 0.537 | 0.532 | 2.136 | 0.481 | 0.172 |
| 1016 | 1 | 같은 Context 재열기 | 16.329 | 3.374 | 0.574 | 0.529 | 0.970 | 0.479 | 0.180 |
| 1016 | 1.5 | 새 Context | 17.286 | 22.961 | 0.385 | 0.395 | 1.838 | 0.371 | 0.105 |
| 1016 | 1.5 | 같은 Context 재열기 | 15.589 | 2.619 | 0.395 | 0.402 | 0.642 | 0.379 | 0.108 |
| 1016 | 2 | 새 Context | 16.753 | 18.346 | 0.242 | 0.242 | 1.613 | 0.247 | 0.070 |
| 1016 | 2 | 같은 Context 재열기 | 16.199 | 1.073 | 0.240 | 0.237 | 0.609 | 0.253 | 0.067 |

이는 median 표다. 양 observer의 개별 최대 표본은 첫 frame32.580ms, 초기 idle1.143ms, 안정 후 idle1.098ms, 이동/안정화2.822ms였다. 어떤 cold frame도27ms를 넘지 않았다는 뜻이 아니다.

ON의 초기/스크롤 후 idle **1,200 frame 전체**에서 glyph_layout·formula_typeset·native_layout_build 호출은 모두0이었다. retained cache는 작동했고, 매 idle마다 전체 글꼴·수식을 다시 조판한다는 근거는 없었다. region_scan은107영역을 방문하며 cache lookup·paint·tessellation은 남는다.

zoom1 ON의 새 Context 첫 frame은 glyph_layout14.861/14.255ms, egui_wrapper8.225/7.671ms였다(1015/1016). 같은 Context 재열기에서는 glyph_layout0.662/0.627ms였다. 열기의 큰 exclusive 비용은 visual_decode 약3.1–3.6ms, import_notes 약2.7ms, restore_regions 약2.1–2.7ms, restore_displays 약2.2–2.4ms였다. 독립 median을 더해 전체 median으로 만들지 않는다.

## 정확한 측정 한계와 후속

- OFF에도 같은 수동 UI·tessellation·drop timer가 있고 ON만 frame observer를 추가한다. 새 profile은 PREP를 켜지 않았다. 일부 ON/OFF median의 증감은 변동·관찰자 영향을 포함하므로 개선으로 해석하지 않는다. JSON·원문/결과 검증·파일 IO는 frame timer 밖이고 가벼운 metadata 읽기·count는 전체/gap에 포함한다.
- test profile은 workspace opt-level1/의존성2다. release/LTO, 전체 ribbon/eframe update, OS event loop, 렌더러·texture upload·GPU를 측정하지 않았다. headless pixels-per-point1·Context zoom1과 worksheet layer zoom을 따로 기록했다. 실제 모니터 DPI 성능 주장이 아니다.
- configure/font 파일 읽기는 timer 밖이고 lazy font/glyph 비용은 첫 frame 안에 있다. OS file cache·machine cold는 통제하지 않았다. glyph 비용을 시작 시점으로 옮겨 제거했다고 주장하지 않는다.
- 같은 Context는 font/UI 상태를 보존하되 재열기 전 ScrollArea fixture만 top으로 초기화했다. 앞선 scroll의 pointer가600,450에 남는 반면 새 Context 첫 frame에는 이전 pointer 이벤트가 없다. hover/scrollbar·glyph warming 차이를 raw mesh 기록에 보존했으며 pixel-identical 전후 비교가 아니다.
- 높은 zoom은 같은1200×900 viewport에 더 적은 문서를 표시한다. top visible regions는28→21→15, scroll 후는26/16/16이었다. 높은 zoom의 낮은 CPU를 같은 내용을 더 빨리 그린 것으로 비교하지 않는다. 문서·저장 결과 전체는 유지했다.
- r204 검사는 읽기 전용 frame이며 키 입력·Enter·worker CPU를 포함하지 않는다. r203 입력/worker-wall 측정을 합쳐 r204 전체 비용으로 만들지 않는다. 실제 사용자 입력·IME·장치·다른 문서/viewport는 별도 검증 대상이다.

이107영역 LCC 표본에서는 반복16ms 이상 idle/scroll CPU stall을 재현하지 못했다. 전체 앱이나 사용자 체감 문제가 해결됐다는 판정은 아니다. 후속은 열기·복원에서 같은 fragment를 반복 parse/변환하는지 먼저 확인하는 일이다. eager 결과·미지원 내용 검사·원문 보존을 줄이거나 무제한 cross-document cache를 추가하지 않는다. sub-ms idle·Matrix 공유·vendor mesh fork보다 실제 원본 저작/IME 공백과 측정으로 확인한 작업을 우선한다. [준비 v2와 고정 r204 source의 차이](../../artifacts/rea/lcc-workspace-r204-20261019/prepared-v2-vs-actual-r204.diff)도 보존했다.
