# 입력·계산 준비 지연과 이동 확정 시점

2026-10-16. 원본 실행, GUI, 스크린샷 없이 설치 DLL의 정적 REA 증거와 Rust headless 테스트로 조사했다. 이전 [0055](0055-screen-rendering-cost.md)의 글꼴·조판 개선만으로 입력 지연이 해결되었다고 판단하지 않는다.

## 재현된 병목과 수정

`editor_bridge::label_fragment`는 영역 하나를 처리할 때마다 이전 모든 정의를 담은 `HashMap<String, String>`을 복제했다. 정의 N개를 순서대로 처리하면 총 N(N−1)/2개의 항목과 문자열을 복제했다. 4,000개 정의에서는 약 800만 개 항목이다. 계산 스레드가 실제 평가를 담당하더라도 이 XML/문맥 라벨 준비는 UI 스레드에서 실행되어 드래그·확정 입력을 막을 수 있었다.

수정은 기존 정의를 읽기 전용으로 참조하고 함수의 bound variable만 작은 overlay에 넣는다. overlay → 기존 정의 → 기존 추론 순서를 유지한다. 정의의 추가는 이전과 같이 현재 영역의 라벨 해석이 끝난 다음에 수행한다. 명시 라벨, 단위·전역 정의, 함수 매개변수 shadowing, 표의 정의 누적 경계를 변경하지 않는다.

계산 준비도 다음과 같이 재사용한다.

- 같은 active calculation signature와 모든 편집기의 arena revision이면 계산 XML과 오류/영역 집합을 재사용한다. signature/revision 수집은 입력 확정·계산 준비에서만 수행하며 프레임마다 수행하지 않는다.
- 편집기의 유효성 검증 결과는 revision별로 재사용한다. 잘못된 draft는 이전 정상 모델로 조용히 계산하지 않고 기존 fault statement를 만든다.
- 모델에 이미 반영한 편집기 revision은 `to_portable`로 다시 변환하지 않는다.
- 계산 입력, 확정 입력, background 상태, worker job이 같은 XML을 `Arc<str>`로 공유한다. 큐 교체 시 거대한 문자열의 복제·해제를 반복하지 않는다.
- native ZIP 바이트와 배치·disabled 상태를 포함하는 기존 signature를 그대로 사용한다. 주소/길이만 비교하는 archive cache는 넣지 않았다. 같은 길이로 ZIP 내용을 바꾼 경우도 무효화된다.

worker의 submit/poll은 계산 종료를 기다리지 않는다. 이번 측정에서 큰 지연은 worker를 기다리는 mutex가 아니라 UI의 입력 준비에서 재현됐다. 기존 최신 세대 선택, 오래된 결과 폐기, Stop, Auto OFF, explicit calculation 순서는 유지한다.

## 측정

Windows, 동일 작업 트리의 Cargo `test` profile(자체 코드 opt-level 1, dependency opt-level 2), headless 실행이다. release/실제 창의 체감 FPS 측정이 아니다. 단일 실행 시간이며 통계적 보장이나 플랫폼 전체 성능으로 일반화하지 않는다. cold prepare는 XML 생성·문맥 라벨·표시용 재파싱을 포함한다.

| 입력 | XML 크기 | cold prepare 전 | cold prepare 후 | 같은 입력 retain 후 |
|---|---:|---:|---:|---:|
| scalar 정의 1,000개 | 244,159 B | 78.727 ms | 21.333 ms | 1.662 ms |
| scalar 정의 4,000개 | 989,017 B | 984.136 ms | 110.037 ms | 6.213 ms |
| 16×16 행렬 정의 100개 | 578,548 B | 48.420 ms | 46.649 ms | 4.448 ms |
| 실제 `1015_LCC_S.mcdx`, 영역 107개 | 76,319 B | 5.267 ms | 4.820 ms | 별도 측정 안 함 |

1,000→4,000 정의에서 커지던 quadratic 복제 비용이 실제로 줄었다. 행렬의 cold prepare 개선은 작으므로 모든 큰 입력의 지연이 해결되었다고 주장하지 않는다. 4,000개 문서의 signature 3회는 수정 후 16.453 ms였다. 전체 직렬화/라벨 처리는 여전히 남아 있다.

별도의 실제 headless 이벤트 경로는 production과 같이 worker를 켜고 기존 정의의 숫자에 `egui::Event::Text`를 보내 draft를 만든 뒤 확정·전체 계산했다. native 창이나 GPU는 시작하지 않았다. 첫 입력 전에 cold 확정 입력 보존 비용을 분리 측정했으므로 key frame에 그 cold 비용을 숨겨 합산하지 않는다.

| 입력 | cold retain | key frame | commit submit | commit 후 worker 남은 시간 | clean 전체 계산 submit | 전체 계산 후 남은 시간 |
|---|---:|---:|---:|---:|---:|---:|
| scalar 4,000개 | 88.553 ms | 1.974 ms | 92.277 ms | 31.265 ms | 7.179 ms | 18.150 ms |
| 16×16 행렬 100개 | 48.964 ms | 0.765 ms | 62.488 ms | 12.593 ms | 5.838 ms | 3.662 ms |
| 실제 `1015_LCC_S.mcdx` | 4.793 ms | 0.720 ms | 7.051 ms | 2.006 ms | 0.443 ms | 0.283 ms |

worker 남은 시간은 submit 반환 이후 완료 poll까지다. 준비와 worker가 일부 겹치므로 순수 엔진 CPU 시간은 아니다. 같은 경로에서 빈 영역 pointer press/release는 scalar 4,000개에서 1.889/1.032 ms, 행렬에서 1.664/0.620 ms, 실제 문서에서 1.428/0.309 ms였다. 이 숫자는 영역을 실제로 이동하는 held-drag 측정은 아니다. [0055](0055-screen-rendering-cost.md)의 이전 4,000개 held-pointer 악화 결과도 이 측정으로 대체하지 않는다.

원본 계약을 적용한 이동 경로는 별도 headless pointer transaction으로 검사했다. egui의 실제 move-handle interaction rectangle을 읽어 press→서로 다른 delta 60회→release를 보냈다. 4,000개 정의에서 press 1.762 ms, delta 60회 총 95.972 ms(평균 1.600 ms/frame), drop 73.602 ms였다. delta 동안 모델 좌표·signature·dirty·undo·worker generation이 그대로이며 drop에서만 history와 worker submission이 한 번 생기는 것을 검증했다. 남아 있는 drop의 준비 비용을 숨기지 않는다. 이는 이전 버전과 동일한 held-drag workload의 전후 측정은 아니며, 이전 일반 pointer-held 측정과 직접적인 속도 배율로 비교하지 않는다. 로그는 `artifacts/rea/input-drag-4k-after-20261016.log`, 테스트는 `benchmark_4k_drag_deltas_do_not_prepare_calculation`이다.

재현 테스트:

```powershell
cargo test -p mathcraft-app benchmark_input_preparation_latency --locked --offline -- --ignored --nocapture
cargo test -p mathcraft-app benchmark_draft_commit_and_worker_latency --locked --offline -- --ignored --nocapture
```

보존 로그는 ignored 분석 디렉터리의 `artifacts/rea/input-latency-baseline-20261016.log`, `input-latency-after-20261016.log`, `input-events-after-20261016.log`다. 실제 fixture 경로는 `artifacts/rea/unit-selected-sources/1015_LCC_S.mcdx`다. GUI 검증은 수행하지 않았다.

## 원본 이동 경로의 정적 계약

이번 조사에서는 기존 REA의 method/call metadata를 재사용하되 설치 파일 SHA-256 및 각 method body IL SHA-256을 다시 검증했다. `dncil`로 파일의 메서드 바이트만 읽었으며 CLR/원본 코드를 실행하지 않았다.

| 파일 | SHA-256 | REA evidence |
|---|---|---|
| `Ptc.Controls.Worksheet.dll` | `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd` | `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b` |
| `Ptc.Controls.Whiteboard.dll` | `947ae0dea0c7a7fdbd1c377e4e7f2447dccd5e839111fec0c0789dafffc1cbd9` | `ev_b5601734c03d7bc9573770a236f1d7d0f7c2f9203276c86b4dff938362dc44ab` |

`WorksheetDragger.DoDrag` (`0x06000699`)는 `UpdatePositionsDuringDrag` (`0x0600069a`)를 호출한다. 그 closure `0x06000dc6`는 페이지 간 표시 offset을 계산하고 `UIElement.RenderTransform = TranslateTransform`을 설정하며 hit-test/visibility를 바꾼다. 여기에 문서 좌표 변경 명령은 없다.

반면 `DropTargetDrop` (`0x0600069b`)는 최종 worksheet offset을 구하고 `CleanupDrag` 후 `OnDragDropEnd(offset, items)`를 호출한다. `HookupDraggerHandlers` (`0x06000b14`)가 연결한 callback `0x06000be3`에서 worksheet `MoveItemsCommand` (`.ctor 0x06000e9c`)를 만들고 `WorksheetCommand.Execute` (`0x06000e57`)를 호출한다. 그 명령은 `Do` (`0x06000ea1`) → `MoveItems` (`0x06000ea4`) → content move command의 `Do` (`0x06000d3e`) → `MoveItemsToPositions` (`0x06000d41`)로 이어지고, 마지막 메서드 IL003a에서 `Ptc.Controls.Core.Whiteboard.IWhiteboardControl.SetItemLocation`을 호출한다.

따라서 해당 원본 worksheet 이동 경로는 **drag 중 시각 transform, drop에서 문서 이동 명령 확정**으로 구분된다. 원본의 모든 다른 이동/resize 경로나 엔진 notification 아래의 전체 동작까지 확인했다고 확대하지 않는다. 이 근거는 Rust에서 매 drag delta마다 계산 입력을 다시 만들지 않고 drop에서 한 번 무효화하는 변경을 뒷받침한다. 단순 성능 추측으로 원본 동작이라고 판단한 것이 아니다.

`WhiteboardSelectionDragger.MouseMove/MouseUp` (`0x06000190`/`0x0600018f`)는 rubber-band 선택 bounds와 선택 완료 이벤트 경로이며, 위 영역 이동과 혼동하지 않는다.

decoder/검증 결과: `artifacts/rea/input-drag-decode.py`, `input-drag-cil.json`, `input-drag-cil.txt`. JSON은 source evidence, 실제 artifact identity, IL digest, instruction offset, operand target token을 포함한다. 관련 기존 계약은 [0042](0042-edit-commit-contract.md)다.

## 검증과 남은 범위

새 회귀 3개는 XML 공유 여부, 잘못된 편집기의 fault/Undo, disabled·배치 변경·동일 크기 archive 변경의 무효화, 문서 재정렬에 따른 contextual identifier 라벨 변경을 확인했다. 별도의 local-overlay 회귀는 함수 매개변수 shadowing과 기존 정의 불변성을 확인했다. 이 4개와 worker 7개, background 통합 8개, contextual label 8개, spec table 7개가 통과했다. 추가 headless drag 회귀 5개도 모두 통과했다. ignored benchmark 3개도 직접 실행해 통과했다.

drag 회귀는 blank press에서 history를 만들지 않음, handle press에서 해당 영역만 캡처함, 여러 delta에서 source·dirty·undo·worker 제출 불변, drop에서 한 번 확정, zoom 1/2에서 좌표 변환, 다른 영역의 source·편집기를 유지하는 undo/redo를 검사한다. 추가로 마지막 이동과 release가 같은 frame에 도착하는 경우와 press→80px 이동이 같은 frame에 들어오는 경우도 검사했다. 검사 중 확인한 Full inverse history 및 release delta 손실을 root 구현에서 고친 뒤 같은 테스트를 다시 실행했다. egui는 release 시 press origin을 지우고 한 frame의 최종 hit target으로 drag 대상을 선택하므로, Rust 이동 transaction은 press event의 원래 위치와 좌표 변환을 보존해 처리한다. 실패 사례를 삭제하거나 허용 오차로 가리지 않았다. 결과 로그는 `artifacts/rea/input-drag-transactions-20261016.log`다.

큰 문서의 첫 준비·확정은 여전히 전체 XML/문맥 재구성을 포함한다. 이번 단계는 그 비용의 원인을 줄이고 매 프레임 반복을 피하기 위한 근거를 제공한다. headless 이벤트 결과가 실제 GUI의 모든 입력 체감이나 렌더 품질까지 검증하지는 않는다.
