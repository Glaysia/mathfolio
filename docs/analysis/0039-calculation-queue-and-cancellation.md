# 계산 중지·재개와 비동기 결과 계약

2026-10-16. 대상은 설치된 **PTC Mathcad Prime 12.0.0.1**이다. 기존 REA 정적 메타데이터를 재사용하고 원본 DLL SHA-256 및 선택 메서드 IL SHA를 다시 검사했다. 어셈블리 실행·CLR/COM·GUI·스크린샷을 사용하지 않았다. 원본 코드 본문은 이 문서에 복제하지 않는다. 이 기록은 [계산 명령·저장 결과](0032-calculation-commands-and-saved-results.md)를 보완한다.

## 증거와 재현

ignored `artifacts/rea/cancellation-decode.py` → `cancellation-cil.json`은 5개 배포 DLL의 메서드 73개를 해시 검증한 뒤 정적으로 디코딩한다. `cancellation-source-ledger.py` → `cancellation-source-ledger.json` 및 `cancellation-enums.json`은 파일 정체성, 메서드 IL SHA, 설치 도움말 해시, 메타데이터 enum 값 9개를 남긴다. `dnfile`은 PE 메타데이터를 읽으며 CLR로 원본을 로드하지 않는다.

| 원본 | SHA-256 | 재사용 REA Evidence |
| --- | --- | --- |
| `Ptc.Calculation.dll` | `88680b1a29536c632d0b705e71f5e727a0b02c64aa95af2157eedb2ada5e6885` | `ev_af7f38985f06b57c13d5262826e5f45f5f437b81377dcd93df83bd763278c4b7` |
| `Ptc.Controls.Worksheet.dll` | `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd` | `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b` |
| `mcfront12.dll` | `d9a7b63fa39957df8f1178e641e2dc9c4a669c08f6ab1c3a1c352e42bf8b9b79` | `ev_c3fcb93ec0882166661e887472307ea1cfa1942d6a520aea234d7b31ec9b307a` |
| `mcdepman.dll` | `e64e1bc7adb5f2671b45121d67f54b5aa1632f36117eeecdbf0878a282015ec7` | `ev_eadfc905fbbe01ace8ea1ba923edbbdd852f43dcd5cbe4e989602861affa65ba` |
| `Ptc.Calculation.Core.dll` | `cf42d8116d697a560f84467c8c32eb8dc1a15172205772eaf4c95a1125eb01bf` | `ev_dba9d90741e5de54a46787c04040beead54127498fdce476f67e1c7a86125f3e` |

설치 영문 `about_controlling_region_calculation.html`의 SHA는 `e9c3025f61e9a9bde4092c91613789eb3cf457814d92ce43e86f176d6773a005`다. 같은 도움말 디렉터리의 `general_keyboard_shortcuts.html`도 직접 읽고 해시를 ledger에 기록했다.

## 사용자에게 보이는 중지·재개

**도움말 관찰:** Calculation 탭의 Controls 그룹에 Stop All Calculations가 있으며 진행 중인 계산이 없으면 사용할 수 없다. AUTO에서 누르면 MANUAL로 바뀌고, 아직 계산하지 않은 영역이 흐리게 표시된다. Auto Calculation을 다시 켜면 마지막 미계산 영역부터 재개한다. 앞서 계산한 영역을 수정하거나 그 앞쪽에 새 영역을 넣으면 그 지점부터 재개한다.

MANUAL을 유지하면서 Calculate Worksheet로 전체를 계산하거나 Calculate Now로 마지막 보이는 영역까지 계산할 수도 있다. F5와 Ctrl+F5는 각각 Calculate Now와 전체 재계산이다. 중지 뒤에만 쓰는 별도의 Resume 버튼·새 계산 명령은 도움말에서 확인하지 않았다. Stop 전용 키보드 단축키는 이번 조사에서 확인하지 않았다.

`WorksheetCommands::.cctor` (`0x060003f0`)는 `StopRunningCalculations`와 `ToggleAutoCalculation`을 키 gesture 없는 RoutedUICommand 생성자로 만든다. 이것만으로 앱 전체에 다른 키 연결이 없다고 단정하지는 않는다. Escape 등을 원본 확인 없이 중지 단축키로 추가하지 않는다.

**CIL 관찰:** `WorksheetViewModel.HandleStopRunningCalculations` (`0x06000a72`)는 계산 실행을 일시 비활성화하고 변경을 알린 뒤, Auto가 켜져 있으면 끄고 실행 가능 상태를 다시 켠다. Stop은 문서를 영구적으로 계산 불가 상태로 만드는 명령이 아니다. 개별 Disable Region과도 구분된다.

| 명령·상태 | 확인한 계약 |
| --- | --- |
| Stop All Calculations | 현재 worker 계산을 중지하도록 요청; AUTO면 MANUAL로 전환; 미완료 영역 Pending 유지 |
| Auto Calculation OFF | 자동 계산 정책을 끄는 명령. worker 중지 호출과 같은 경로가 아님 |
| Auto Calculation ON | 기존 잠금을 풀고 자동 계산 queue를 깨워 미계산 영역부터 재개 |
| F5 / Calculate Now | 필요한 영역을 마지막 보이는 영역까지 계산하고 활성 영역을 강제 갱신하는 기존 명령 |
| Ctrl+F5 / Calculate Worksheet | 전체 강제 재계산이라는 기존 명령 |
| Disable Region | 선택 영역 자체를 계산에서 제외하며 Auto·F5·전체 계산으로 다시 실행하지 않음 |

## Auto OFF와 worker 중지는 별개다

`CalculationController.HandleStopEvaluating` (`0x06000012`)는 controller/context의 실행 가능 상태를 끄고 `Engine.StopWorkers`를 호출한다. 그 뒤 `SetSynchronousCallbacks(true)`와 `false`를 순서대로 호출한다. 이 전환이 정확히 어떤 callback을 어느 시점까지 drain하는지는 추가 native 경로 확인 없이 확정하지 않는다.

반면 `HandleStopAutoCalc` (`0x06000018`)는 `SetAutoCalc(false)`와 상태 저장을 수행하며 StopWorkers를 직접 호출하지 않는다. `depman.Depman.SetAutoCalc` (`0x060003f4`)는 OFF→ON 전환에서 `UnLockAll`을 수행하고 queue의 자동 계산 모드를 바꾼다. `NodeQueueImp.SetAutoCalc` (`0x0600044f`)는 자동 계산 flag를 바꾸고 ON 전환 때 queue에 메시지를 보낸다. 여기에는 AbortThread·StopWorkers 호출이 없다.

따라서 Auto OFF를 항상 Stop All과 동일한 즉시 취소로 구현했다고 해서 원본 동작이 입증되는 것은 아니다. 이미 실행 중인 독립 수식·컴포넌트·후속 작업의 정확한 경계별 완료 순서는 이번 증거만으로 확정하지 않는다.

## 중단은 계산 오류가 아니다

**메타데이터 관찰:** 원본은 계산 상태와 동기화 상태를 따로 둔다. `SynchronizationStatus`는 Pending=0, Synchronized=3이고 `CalculatingStatus`는 Computing=1, NotComputing=2다. 숫자 1을 Synchronized라고 가정하거나 NotComputing을 최신 결과와 동일시하면 안 된다. 엔진 노드 상태도 Invalid/Computing/NotComputing/Ready/TreatedReady로 별도 관리한다.

`EngineResult`의 상태 변경 생성자 (`0x06000051`)는 기존 ResultNode·Errors·Warnings·SerializationTag를 복사하고 동기화 상태만 교체한다. 즉 기존 값과 Pending은 함께 존재할 수 있다. Pending 전환은 값을 0으로 만들거나 취소 오류로 대체하는 동작이 아니다. 오류 생성 경로는 별도로 있으며 결과값과 실제 오류의 동시 보유도 제한한다.

**queue 관찰:** `NodeQueueImp.StopWorkers` (`0x06000446`)는 `SetNumThreads(0)`을 호출한다. 이 요청은 mailbox를 통해 queue에 전달되고 원하는 worker 수의 응답을 기다린다. `SetNumWorkers` (`0x06000437`)는 감소시킬 worker를 골라 available 및 busy worker 모두에 AbortThread를 호출할 수 있다. `AbortThread` (`0x06000436`)는 중단 flag·메시지와 `SharedThreadContext.Abort`를 사용한다.

`OnWorkerMessage` (`0x06000441`)는 SafeAbortException을 구분해 메시지 19를 보낸다. `OnQueueMessage` (`0x06000440`, IL `0x0463` 이후의 해당 분기)는 살아 있는 중단 Position을 다시 enqueue하고 완료 정리를 수행한다. 따라서 원본은 중단된 노드를 실패 결과로 확정하여 버리는 대신 재개 가능한 queue에 남길 수 있다. 죽은 Position은 종속 항목을 별도로 처리한다.

이것은 완료된 앞부분의 결과를 유지하면서 중단 지점 이후를 Pending으로 남기는 구현의 근거다. **중단된 단일 수식의 내부 반복 인덱스까지 그대로 이어 실행한다는 근거는 아니다.** 수식 단위로 다시 실행해야 할 수 있으며 중지 지연의 상한·외부 native 함수의 즉시 중단 가능성도 이번 조사로 입증하지 않았다.

## 편집과 늦은 결과의 처리

`CalculationController.HandleCalculatableUpdated` (`0x0600002e`)는 대상 영역을 Pending으로 바꾸고 engine update를 전달한다. `Depman.UpdateWithDef` (`0x060003e7`)는 mutex 안에서 해당 Position을 갱신하고 SafeEnqueue한다. PositionBase.update (`0x060001f8`)의 특정 상태 분기는 이전 ActionChain의 Kill·ClearDeps와 InterruptCallback을 호출한 다음 새 ActionChain과 정의·종속 관계를 등록한다. 이 분기의 native 상태 flag 전체 의미와 모든 수식의 타이밍은 미확인이다.

따라서 원본은 위치·의존성별 작업을 관리한다. 전체 문서를 매번 새 복사본으로 계산하고 queue의 마지막 문서 하나만 남기는 것과 동일한 구조라고 주장할 수 없다. 특정 편집 때문에 실행 중인 옛 작업이 교체·중단될 수 있다는 정적 근거와, 모든 편집에서 항상 전체 worker를 취소한다는 주장은 다르다.

결과 callback은 WPF Dispatcher.BeginInvoke로 전달된다. `DispatchedSetResult` (`0x0600009e`)와 `DispatchedSetError` (`0x060000a0`), 동기화 상태 전달 (`0x0600009b`)은 영역이 현재 계산 context에 남아 있는지 확인한다. 컴포넌트는 cookie 최신 여부도 확인한다. **cookie 검사는 ICalculatableWithDataExchange에만 적용되고 일반 수식에는 true를 반환한다.** 이를 모든 일반 수식의 문서 revision 검사가 원본에 있다는 근거로 확대하지 않는다.

## Mathcraft 구현에 적용할 경계

다음은 위 계약을 지키기 위한 **독립 설계 선택**이다. 원본의 스레드 구조·엔진 queue를 복제했다는 뜻은 아니다.

- UI thread에서 긴 계산을 실행하지 않고 background worker에서 수행한다.
- Stop은 cooperative cancellation으로 전달하고, 결과를 오류로 바꾸지 않으며 실행하지 않은 영역은 Pending으로 둔다.
- 이미 수용한 완료 결과는 유지한다. 취소 직전에 완료된 결과를 어느 범위까지 수용할지는 명확한 요청·revision 경계로 관리한다.
- 문서가 수정·교체·닫힌 뒤 도착한 옛 worker의 결과가 새 문서를 덮어쓰지 않도록 document/revision 식별자를 검사한다. 이는 전역 안전 정책이며 원본의 일반 수식 cookie 정책과 동일하다고 주장하지 않는다.
- queue를 최신 요청으로 합칠 때 명시적 전체 계산·Calculate Now 범위·활성 영역 강제 계산을 잃지 않도록 한다. 자동 편집 요청의 대체와 사용자 중지를 별개 원인으로 관리한다.
- 중단 이후 Auto ON·F5·Ctrl+F5는 기존 계산 명령의 범위를 유지한다. 재개를 위한 원본에 없는 새 버튼을 추가하지 않는다.

## 상태 표시와 미확인 범위

도움말은 계산 중 노란 표시, 계산 완료 시 초록 표시, AUTO/MANUAL 문자와 미계산 영역 dim을 설명한다. Stop 뒤 Pending이 남은 idle 상태의 정확한 표시 색, 중간 단계의 모든 상태 전이와 원본 실행 소요 시간은 확인하지 않았다. worker가 idle이라는 이유만으로 모든 결과를 Synchronized로 바꾸면 안 된다.

정적 검증은 원본 파일 및 IL 해시 재현을 통과했다. 독립 Rust worker·엔진의 cancellation 테스트 및 Windows/Linux 통합 검증 결과는 별도 구현·검증 기록을 기준으로 한다. 이번 문서의 조사만으로 해당 구현 완료나 원본 전체 scheduler 동등성을 주장하지 않는다.
