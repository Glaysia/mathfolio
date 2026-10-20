# Prime 12의 계산 피드백과 편집 상태

2026-10-15. 설치된 Prime 12.0.0.1의 저장된 REA metadata와 원본 바이트를 대조했다. 원본 프로그램, CLR, COM, GUI, 스크린샷을 실행하지 않았다. 아래의 상태 전이는 정적 코드 관찰이며 화면 동작을 실측한 기록이 아니다.

## 조사 질문과 결론

| 질문 | 결론 | 범위 |
|---|---|---|
| 계산 중과 결과가 최신인 상태가 같은가? | 별도 상태다. Pending/Synchronized와 Computing/NotComputing을 각각 저장한다. | 확정, metadata·CIL |
| 수식을 수정하면 이전 결과를 즉시 폐기하는가? | 조사한 Pending 전환 경로는 이전 EngineResult의 payload를 유지한다. | 확정, 이 경로에 한함 |
| 늦게 도착한 결과는 어떻게 다루는가? | 계산 문맥 확인, 해당 인터페이스의 component cookie, 편집 모델의 수정 여부 검사라는 서로 다른 방어가 있다. | 확정, 보편적인 단일 generation 검사로 일반화 불가 |
| 오류는 워크시트 전체 문자열인가? | EngineError의 node handle로 수식의 ErrorNode를 찾는 경로가 있다. 경고는 별도 저장한다. | 확정, 시각 표현은 미확정 |
| 저장된 결과를 되살릴 때 숫자만 복구하는가? | RestoreResult는 결과, 오류, 경고, 동기화 상태를 함께 복구한다. | 확정, 호출된 내부 경로 전체 실행 검증은 없음 |

사용자가 작성하는 수식, 계산 요청, 현재 결과, 보관된 이전 결과를 구분하는 설계라는 해석이 가능하다. 이것은 아래 구조에서 얻은 **설계 해석**이며 원저자의 의도를 직접 확인한 진술은 아니다. 피드백을 계산 함수 목록 뒤에 붙는 장식으로 취급하면 이 구조를 놓친다.

## 증거의 식별과 재검증

모든 경로의 기준 디렉터리는 `C:\Program Files\PTC\Mathcad Prime 12.0.0.1`이다.

| DLL | SHA-256 | REA Evidence |
|---|---|---|
| Ptc.Calculation.dll | `88680b1a29536c632d0b705e71f5e727a0b02c64aa95af2157eedb2ada5e6885` | `ev_af7f38985f06b57c13d5262826e5f45f5f437b81377dcd93df83bd763278c4b7` |
| Ptc.Calculation.Core.dll | `cf42d8116d697a560f84467c8c32eb8dc1a15172205772eaf4c95a1125eb01bf` | `ev_dba9d90741e5de54a46787c04040beead54127498fdce476f67e1c7a86125f3e` |
| Ptc.EquationEditor.dll | `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223` | `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf` |
| Ptc.Controls.EquationControl.dll | `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1` | `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` |
| Ptc.Controls.Worksheet.dll | `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd` | `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b` |

재부팅 후 위 5개 설치 DLL의 SHA-256과 기존 `artifacts/rea/feedback/*-cil.json`의 123개 메서드 raw CIL SHA-256을 다시 대조했다. 모두 일치했다. 메서드 파일 offset, IL 크기, MVID, 해시는 ignored `artifacts/rea/ux/feedback-recovery-validation.json`에 남겼다. 원본 CIL/metadata는 ignored 증거에만 보관하고 이 문서에는 관찰한 계약만 적었다. EquationModel의 추가 focused evidence는 `ev_fd77c469a4e79dcea831eee774b27cd5a793ad0c324dea2b4b314892b5ec0dee`이다.

## 상태 축과 기존 결과의 수명

Core metadata의 enum 상수는 `SynchronizationStatus.Pending=0`(`0x040000b6`), `Synchronized=3`(`0x040000b7`), `CalculatingStatus.NotComputing=2`(`0x040000b9`), `Computing=1`(`0x040000ba`)이다. `artifacts/rea/ux/feedback-enums.py`는 설치 PE의 Constant 테이블에서 이 네 값을 읽는다. 따라서 숫자 0/1을 한 개의 boolean처럼 해석하면 안 된다.

EquationControl의 `UpdateSynchronizationStatus`(`0x06000052`, 파일 header offset 5072)는 자체 `_syncStatus`가 Pending이거나 calculatable child 중 Pending이 있으면 전체를 Pending으로 만들고, 그렇지 않으면 Synchronized로 만든다. child 조건은 `0x06001454`의 `get_SynchronizationStatus == 0`이다. `UpdateCalculatingStatus`(`0x06000054`, offset 5256)는 child 중 Computing이 있으면 Computing을 선택한다. 해당 조건은 `0x06001455`의 `get_CalculatingStatus == 1`이다. 두 속성을 따로 갱신한다.

`UpdateEngineResult(SynchronizationStatus)`(`0x06000055`, offset 5336)는 기존 EngineResult와 새 상태로 Core 생성자 `0x06000051`을 호출한다. 이 생성자는 ResultNode, Errors, Warnings, SerializationTag를 이전 값에서 가져오고 SynchronizationStatus만 새 값으로 설정한다. ResultNode가 null이면 EmptySimpleResultNode를 만든다. 즉 **보관 중인 결과가 있다는 사실과 그 결과가 현재 수식에 맞는다는 사실은 다르다**. 이 경로가 화면에 어떤 색·숨김 정책을 적용하는지는 이번 조사로 확정하지 않았다.

CalculationController의 `HandleCalculatableUpdated`(`0x0600002e`, offset 6328, IL_0007–000e)는 해당 calculatable을 Pending으로 설정한다. 같은 메서드 IL_004d–0062의 result-owner 분기는 기존 EngineResult로 새 Pending EngineResult를 만들고, 이후 엔진 Update를 요청한다. 이것도 이전 payload를 보관하는 구체적인 경로다.

## 요청과 결과 적용의 경계

EquationModel의 `FireExpressionValueChanged`(`0x060001c0`, offset 38222, IL_0000–0007)는 `IsExpressionValueModified=true`를 먼저 설정한 뒤 변경 이벤트를 보낸다. `IsModified`(`0x06000119/11a`)와 `IsExpressionValueModified`(`0x060001b9/1ba`)는 서로 다른 필드다. `ResultScalingModified`(`0x060001c4/1c5`)도 별도 필드다. 저장 필요 여부, 수식 값 변경 여부, 표시 단위 변경 여부를 하나로 합칠 근거가 없다.

EquationControl의 `FireRequestEvaluation`(`0x06000153`, offset 20976, IL_0000–0018)는 expression-value modified를 지우고 result-scaling modified도 false로 만든 뒤 모델로부터 calculatable aggregate를 갱신한다. `ClearExpressionValueModified`(`0x06000218`, offset 32723)는 모델 flag를 false로 바꾸고 IsCommitted 속성 변경도 알린다. 이는 계산 요청이 단순 문자열 출력과 별개의 상태 전이라는 근거다. 이 경로의 force 인수, child 처리, control-only 요청은 구별되어 있으므로 모든 계산 요청이 같은 범위라고 단정하지 않는다.

EquationModel의 결과 적용 helper `0x06000137`(offset 28432, raw CIL SHA-256 `075f770f5d3aae29fabbaf30f2f96753b457187f0985d8efcf07e2e63f3cf59e`)는 RL index에 해당하는 evaluation node를 찾고, 없으면 반환한다. 이어 IL_0011–0019에서 `IsExpressionValueModified`가 true이면 결과를 적용하지 않는다. 이후 numeric/scriptable과 symbolic evaluation 경로를 구분한다. 이 검사는 편집 모델의 현재 상태 검사이며 작업마다 전역 세대 번호를 부여한다는 증거는 아니다.

`DispatchedSetSyncResultAndError`(`Ptc.Calculation.dll`, `0x060000aa`, offset 13652, raw CIL SHA-256 `5e60ab8a0b9a70531ac40cf96f33e54238cb01d03781d691b0290980cb0b6998`)는 IL_0000–0013에서 calculatable이 여전히 문맥 안에 있는지 검사하고, IL_0014–001c에서 cookie 검사를 통과해야 한다. 통과한 후 오류·결과를 적용하고 Synchronized로 설정한다.

**cookie 검사의 적용 범위는 한정되어 있다.** `get_IsComponentCookieUpToDate`(`0x060000a6`, offset 13432, raw CIL SHA-256 `71f6282e2560ea63ad94e19b0b0492b72da93994d5da96c734a73e782a7defa4`)는 `ICalculatableWithDataExchange`인 경우에만 저장 cookie와 `ComponentCookieMultiThreaded`를 비교한다. 일반 calculatable이면 true를 반환한다. `HandleCalculatableUpdated`의 cookie 증가 역시 그 인터페이스 분기에 있다. 이를 모든 수식에 적용되는 generation guard로 보고하면 틀리다.

## 결과·오류·경고의 복원

EquationControl의 `RestoreResult`(`0x060000cf`, offset 12448, raw CIL SHA-256 `822d3d3ee47e3b9f854f1f7e5037263e77b2b0ccb07e5a33b603a8d6eca14763`)는 비어 있는 EngineResult라면 반환한다. 나머지는 기존 결과를 saved로 잡고 모델 aggregate를 갱신한 뒤 다음을 수행한다.

- numeric evaluation의 result index와 scriptable-control 조건에 따라 calculatable child의 결과를 복원한다(IL_002a–0079).
- saved Errors에서 모델의 `HandleError`로 복원한다(IL_007b–00a6).
- saved Warnings가 있으면 `SetRedefinitionWarnings`로 복원한다(IL_00a6–00ce).
- saved SynchronizationStatus와 child 상태를 복원하고 전체 동기화 상태를 다시 합성한다(IL_00ce–0109).

이는 모든 undo가 이 메서드를 반드시 호출한다는 증거는 아니다. 본 조사에서는 이 복원 메서드의 실제 데이터 계약만 확정했다.

Core의 EngineResult 생성자 `0x06000054`(raw CIL SHA-256 `9b1b96c96c245bf291c49111091012669c6f5c0c46eba1524b6930fd8fe1187e`)는 계산된 nonempty result와 nonempty error collection이 동시에 전달되면 예외를 발생시킨다. 따라서 **현재 오류와 마지막 성공 값**을 함께 보관하려면 현재 결과 한 개의 value/error 쌍에 섞지 않고 보관용 상태를 분리해야 한다. warning과 value는 이 오류 조건과 다르다.

EquationModel의 `HandleError`(`0x0600013c`, offset 29836, raw CIL SHA-256 `69194623aa6ccd06a011d702b6408e472f537e1f275fd689de5ec0906207794e`)는 EngineError의 NodeHandle을 읽어 `FindChildWithErrorMatchingId`를 호출하고, 매칭 실패 시 equation root로 돌아가는 경로를 가진다(IL_009f–00bb). argument 및 evaluation/APPLY 문맥에 따른 추가 보정 후 ErrorNode를 설정한다. `SetRedefinitionWarnings`(`0x0600013e`)는 EngineWarnings를 별도로 설정한다. 오류 위치 표시를 단순한 지역 아래 문자열 한 줄로만 대체하면 이 계약의 일부만 구현한 것이다. 정확한 highlight 모양과 색은 미확정이다.

## 현재 Rust 구조와 다음 구현 경계

`mathcraft-document/src/canvas.rs`의 CanvasDocument에는 수식·텍스트·위치·원본 archive만 저장되고 caret/undo/results는 들어가지 않는다. `mathcraft-app/src/worksheet_ui.rs`는 document의 dirty flag와 별도 results map을 갖는다. 이 분리는 유지할 만하다. 현재 `recalculate()`는 동기식으로 전체 map을 지우고 다시 계산하므로 이 코드 자체에는 늦은 비동기 응답이 없으며, 재부팅을 이유로 비동기 세대 번호를 새로 도입할 필요는 없다.

작고 충실한 개선 후보는 **현재 결과 상태와 마지막 성공 결과의 수명을 분리하는 계산 모델**이다. 소스가 바뀌면 보관된 결과를 Pending으로 전환하되 새 수식의 현재 값으로 반환하지 않고, 성공하면 현재 결과로 대체하며, 실패하면 현재 오류와 보관된 값을 별도 슬롯에 유지한다. 이때 문맥/수식이 다른 region의 결과를 id만으로 재사용하지 않는다. 이는 원본의 payload 보관 구조를 참고한 구현 제안이며 지금의 동기식 앱이 원본과 같은 화면 정책을 이미 제공한다는 주장은 아니다. UI를 건드리지 않고 이 상태 전이를 headless unit test로 먼저 고정할 수 있다.

당장 원본과 동일하게 유지해야 하는 안전 조건은 현재 오류 발생 시 과거 값을 정상 결과로 내보내지 않는 것이다. 현 코드의 faults 처리(`row.value=None`, `row.display_value=None`)는 이 조건을 지키므로, 보관 결과 구조를 추가하더라도 이 동작을 되돌리면 안 된다. 후속 오류 위치 구현에는 compat Fault의 node 경로와 editor NodeId 사이의 명시적 매핑이 필요하며 원본의 obfuscated node handle 값을 그대로 복제할 필요는 없다.

## 50% 단계의 계산 피드백 연결

독립 앱은 이 정적 계약을 다음과 같이 실제 작성 흐름에 연결했다. 계산 리본의 자동 계산을 끄면 수식·배치 변경이 문서를 `계산 대기` 상태로 전환한다. 직전 결과 payload는 내부에 남아 있지만 화면의 현재 결과·오류·도표로 다시 사용하지 않는다. F9, 리본의 명시 계산, `=` 평가 요청 또는 자동 계산 재활성화가 동기 계산을 수행하면 대기 상태가 해제된다. 이는 원본의 Pending/payload 분리를 따른 독립 표시 정책이며, 원본의 정확한 흐림색·지연 시간 정책을 재현했다는 뜻은 아니다.

리본의 이전/다음 오류는 본문 배치 순서(top, left, stable ID)로 수식 오류와 도표 오류를 순환하며 해당 영역을 선택하고 화면에 드러낸다. 재정의 경고는 오류와 별도로 세고, 사용자가 표시를 끌 수 있으며, 경고가 있는 정의 옆에 독립 표식을 그린다. 오류가 있는 값은 여전히 직전 성공값을 정상 결과로 내보내지 않는다. Headless 앱 회귀 검사는 오류 순환·wrap, 자동/수동 전환, 대기 중 stale feedback 숨김, 명시 재계산, 재정의 경고의 오류 분리를 확인한다.

## 남은 질문

Pending 결과의 정확한 표시/숨김 정책, 계산 중 feedback 표시 지연, 오류 highlight geometry, cached native 결과가 표시되는 정확한 조건, 모든 undo 명령의 결과 복원 호출 관계는 미확정이다. 원본의 인터페이스 분기를 Rust 전체에 보편적인 정책으로 확대하지 않는다. 이번 조사에는 원본 실행·native reopening·화면 비교 검증이 없으며, static 계약과 독립 Rust 구현의 테스트를 구분해야 한다.
