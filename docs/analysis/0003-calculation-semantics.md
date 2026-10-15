# Prime 12 계산·의존성·단위 의미 분석

2026-10-15. 대상은 사용자가 지정한 `C:\Program Files\PTC\Mathcad Prime 12.0.0.1`의 설치 산출물이다. 이 보고서는 실행 관찰을 주장하지 않는 정적 분석이다. 함수 이름을 나열하는 데 그치지 않고 CIL의 분기, 필드 읽기·쓰기, 호출 인자를 확인했다. 별도 런타임 프로브의 결과와 합쳐 호환성 명세를 확정해야 한다.

## 증거 및 재현 범위

원본 REA Evidence는 `artifacts/rea/mathcad/`에 보존되어 있다. 아래 약칭은 이 문서 안에서만 사용한다. 모든 메타데이터 토큰은 해당 SHA-256/MVID에만 유효하며 네이티브 주소가 아니다.

| 약칭 | 원본 파일 / REA Evidence ID | SHA-256 | MVID / REA coverage |
|---|---|---|---|
| C | `Ptc.Calculation.dll.members.json` / `ev_af7f38985f06b57c13d5262826e5f45f5f437b81377dcd93df83bd763278c4b7` | `88680b1a29536c632d0b705e71f5e727a0b02c64aa95af2157eedb2ada5e6885` | `03e41b0c-b105-4a30-bac6-2e163d022a5d` / complete |
| K | `calculation-core-members.json` / `ev_dba9d90741e5de54a46787c04040beead54127498fdce476f67e1c7a86125f3e` | `cf42d8116d697a560f84467c8c32eb8dc1a15172205772eaf4c95a1125eb01bf` | `f26e51c8-54b1-4d6f-8816-d7805b72d6c6` / complete |
| D | `mcdepman.dll.members.json` / `ev_eadfc905fbbe01ace8ea1ba923edbbdd852f43dcd5cbe4e989602861affa65ba` | `e64e1bc7adb5f2671b45121d67f54b5aa1632f36117eeecdbf0878a282015ec7` | `62f7319f-eacd-489c-a03c-e29e5ccff5b7` / partial |
| E | `mceng.dll.members.json` / `ev_e9409f7a05a2578d771e4cd5536fd610b536e38c923477650019e29473ba25ee` | `29d0bbb5cb7c123f17bce0558d983c47c76e78820f884408b6b4ac019b2eb15d` | `0d1bb366-3876-4252-b156-fc1e22571cda` / partial |

추가 REA CLI 실행으로 경계를 확인했다.

- `artifacts/rea/calculation-deep/mcdepman.boundaries.json`: `ev_85d87985fed4cf8d02ddfaa012c84bb227ba662ae527968086fb243bdf9946e9`. P/Invoke 선언 31개, 비 IL 구현 경계 64개, IL-only=false.
- `artifacts/rea/calculation-deep/mceng.boundaries.json`: `ev_4a82233dc1f7558fb0b713f32878c44da08071b8b08c89956baa72d614b963e7`. P/Invoke 선언 38개, 비 IL 구현 경계 43개, IL-only=false.

이 선언 개수는 실제 네이티브 export 검증 결과가 아니다. D/E의 partial과 조사하지 않은 함수는 그대로 미확인이다. 특히 `depman.NodeQueue`의 실제 worker 구현은 이 CIL 조사에서 복구되지 않았다.

`artifacts/rea/calculation-deep/`에는 분석용 스크립트와 파생 데이터를 저장했다.

- `*.focused.json`, `*.anchors.txt`: 원본 REA에서 관련 타입/메서드만 선택하고 토큰을 같은 어셈블리의 MethodDef/MemberRef/Field로 해석했다.
- `*.full-il.json`, `*.full-il.txt`: 설치 바이너리를 읽기 전용으로 직접 디코딩했다. REA가 anchors에서 생략하는 `ldc.i4.0`, `ldc.i4.1`, `ldarg`, 실제 switch 목적지 등을 보충했다. 디코딩한 **모든 메서드**에 대해 원본 파일 SHA-256과 REA의 raw `il_sha256` 일치를 assert했다. 대상 어셈블리를 로드/리플렉션/실행하지 않았다. opcode 표만 호스트 .NET의 기본 `System.Reflection.Emit.OpCodes`에서 읽었다.
- `*.constants.json`: ECMA-335 Constant 테이블과 Blob을 직접 읽어 열거값/기본값을 기록했다. 각 값에 Constant 테이블 파일 오프셋, Blob 오프셋·원본 hex, Field 토큰을 보존했다. 이것은 REA Evidence에서 파생한 별도 byte 분석이며 새로운 REA Evidence ID로 가장하지 않는다.
- 복호화·설치 파일 수정·원본 문서 수정은 수행하지 않았다. 독립 Rust 구현으로 복사할 제품 소스도 만들지 않았다.

## 1. 계산은 영역 순서·부모/자식·라벨을 가진 상태 모델이다

**관찰 C-1.** `CalculationController.HandleCalculatableAdded` (`0x0600001f`, CIL 시작 파일 오프셋 4416)는 `ICalculatableContainer.CalculatableChildren`를 순회한다(IL_0027..0067). 이후 `IEngine.AddRegionAfter`(IL_008e), `Child`(IL_00b7), `AddChild`(IL_00d1), `AddRegionBefore`(IL_00e2) 중 하나를 호출한다. 새 엔진 handle과 calculatable/container를 양방향 맵에 저장한다(IL_00e9..0116). aggregate는 재귀적으로 자식과 이벤트를 연결한다(IL_0123..019a).

**관찰 C-2.** 이동 처리 `0x06000021`(CIL 시작 5148)은 컨테이너의 자식 순서를 다시 훑고 기존 handle을 `MoveAfter`(IL_0098) 또는 자식 기준 `MoveBefore`(IL_00d5)에 전달한다. 삭제 처리 `0x06000022`는 `Engine.Remove`(IL_0028) 후 재귀 정리한다. 업데이트 `0x0600002e`는 `get_Rl`(IL_0001), 상태 변경, handle 조회 후 `Engine.Update`(IL_007e)를 호출한다.

**관찰 D-1.** `PositionalEnv.Lookup` (`0x06000376`, CIL 시작 78704)은 `m_pos`를 읽고 `Scope.Id`, 심볼, 현재 위치, 정수 1을 `DepManager.resolve`에 전달한다(IL_0021..002b). Constant 테이블에서 `ResolveMode.BEFORE=1`을 확인했다. 별도로 `AT=0`, `AT_OR_BEFORE=2`이다. `Depman.IsBoundAt` (`0x060003fa`)는 `is_bound`에 2를 전달한다(IL_0050..0051). 존재 확인과 값 조회가 같은 위치 모드를 쓰는 것은 아니다.

**관찰 D-2.** `Depman.GetDefinition` (`0x060003fe`, CIL 시작 57468)은 scope를 얻고 resolve(..., BEFORE)를 수행한다(IL_0038..0046). 없으면 `Scope.Parent`로 올라간다(IL_004f..0056). 발견한 정의가 `IsUndefine`이면 무효화한다(IL_0059..0063).

**해석.** 하나의 전역 `HashMap<String, Value>`를 호출 시점에 조회하는 것으로 원본의 위치별 정의·가림·자식 컨테이너·정의 해제를 표현할 수 없다. Rust 명세에는 region identity, 위치에서의 해석, scope chain, 라벨이 들어가야 한다. 다만 이 controller 조사만으로 `top/left/baseline`에서 자식 순서를 만드는 정확한 비교식을 증명하지는 못했다. geometry→순서 규칙은 UI/문서 계층 및 런타임 이동 실험과 결합해야 한다.

## 2. 심볼 라벨은 표시 색상 이상의 의미를 가진다

**관찰 D-3.** `DepManager.resolve` (`<Module>::depman.DepManager.resolve`, `0x06000047`, CIL 시작 35268, 706 bytes)은 다음을 실제 수행한다.

- `DSym.name`을 읽고 `AddLabels`를 호출한다(IL_0002, IL_001f..0022).
- scope ID와 이름이 맞는 후보를 순회하고 `Engine.Unmanaged._unifies`로 라벨 집합을 검사한다(IL_0176..019f).
- 후보의 `find_def_nodead` 결과 중 구조체 `+240`의 double 순서값이 더 큰 것을 선택한다(IL_01b7..01e2).
- wildcard 라벨 경로와 정확한 키 조회 경로가 분리되어 있다(IL_002d의 분기, IL_01f8..02a6).

`+240`이 어느 UI 좌표인지는 이 함수로 알 수 없다. 순서 비교에 사용되는 수치라는 사실만 확정한다. 이름·라벨·scope·정의 위치를 함께 비교한다는 결론은 호출 이름만이 아니라 조건 분기와 인자에서 나온다.

**관찰 C-3.** 결과의 `%labels`를 처리하는 `DispatchedEngineCallbackHandler.ProcessIdBoundLabels` (`0x060000b0`, CIL 시작 14488)는 `name:0xHHHHTTTT` 형식의 이름·handle·tag를 파싱하고(정규식 초기화 `0x06000090` IL_0001), handle→calculatable 매핑을 찾는다(IL_00ae..00b8). `ResolvedId.Name`, `.Tag`, `.Calculatable`를 채운다(IL_00c5..00da). 라벨은 바인딩 결과로 UI에 되돌아간다.

**호환성 의무.** `m`이라는 글자가 UNIT인지 VARIABLE인지 CONSTANT인지, 자동 추론된 라벨인지 사용자가 지정한 라벨인지 보존해야 한다. 아래첨자나 label 정보를 단순 문자열 변환 중 버리면 원본과 다른 심볼이 된다. 정확한 wildcard/unification 규칙 전체는 추가 조사 대상이다.

## 3. 재계산은 의존성·캐시·취소·UI 반영이 분리된다

**관찰 D-4.** `ActionChain.eval` (`0x06000008`, CIL 시작 7516)은 `CachingEnv.check_changed`가 참이면 `clear_result`를 부른다(IL_0006..0013). 캐시/다음 action이 없을 때 mutex를 놓고 `CheckInterrupt`, `Action.Invoke(env,state)`, `CheckInterrupt`를 수행한다(IL_004e..0095). finally에서 mutex를 되찾는다(IL_009c..00ac). 계산 중 환경이 바뀌거나 kill flag가 있으면 `SafeAbortException`을 던진다(IL_00ca..00e5, IL_0156). 성공한 `ActionResult.result/next/state`는 결과와 다음 action에 저장된다(IL_00e7..0141). `EvalChain` (`0x06000009`)은 next가 없어질 때까지 이 과정을 반복한다.

**관찰 D-5.** `CachingEnv.IsIdentical` (`0x0600037f`, CIL 시작 3908)은 dictionary 참조가 같으면 빠르게 true지만(IL_0000..0005), `%errors`가 있거나 `__region` 키면 동등으로 간주하지 않는다(IL_0012..004e). 일반 값은 동일 참조 또는 `Object.Equals`를 검사한다(IL_008b..0099). 이 조사에서 식별하지 않은 `isinst 0x0100001f` 타입도 항상 다른 것으로 처리한다. 그 타입을 추정하여 최적화하지 않는다.

**관찰 D-6.** `PositionBase.Calculate` (`0x06000206`, CIL 시작 79328)은 이미 ready한 결과를 재사용하고(IL_002d..003b), 계산 중 상태·timer를 관리하며(IL_003d..0088), 성공한 결과를 `LabeledDictionary`에 저장·lock하고(IL_019a..01b6), event를 Set한 후 메시지 3을 보낸다(IL_01cf..01e2). `PositionBase.Invalidate` (`0x0600020f`, CIL 시작 82636)는 invalid flag, TreatAsReady(false), ready event Reset, 메시지 0, 필요 시 interrupt callback, queue callback을 수행한다(IL_0043..00a5).

**관찰 C-4.** `EngineBuilder.CreateEngine` (`0x06000056`, CIL 시작 9676)에서 `SetSynchronousCallbacks(false)`가 명시된다(IL_000d..000e). `HandleEngineNodeCallback` (`0x0600000f`)은 메시지에 따라 computing/synchronization/result를 다르게 처리한다. `HandleEngineUpToDate` (`0x0600000d`)는 기다리는 event를 Set/Reset한다. `HandleWaitForCalculationIdle` (`0x0600001e`)은 wait handle, dispatcher DoEvents, disabled/timeout 예외를 다룬다. UI 결과 반영 완료와 API 호출 반환을 같은 사건으로 가정해서는 안 된다.

**관찰 C-5.** 자동 계산, 전체 재계산, 특정 위치까지 CalculateNow는 별도 경로다: `0x06000017/18`은 SetAutoCalc(true/false), `0x06000015`는 worker 중지→RecalculateAll→재시작, `0x06000016`은 선택 handle을 CalculateNow에 전달한다. `EngineStatuses`는 IsMultiThreads=0, IsRunScriptingOnRecalculate=1, IsCalculateAll=2, IsCalculateNow=3, IsAutoCalc=4로 구분된다.

**호환성 의무.** 독립 구현은 스레드 수나 내부 자료구조까지 복제할 필요는 없지만 사용자가 관찰하는 dirty/pending/computing/ready, 의존 영역만 갱신, 취소, 수동 계산, 비활성 영역의 의미는 일치시켜야 한다. 캐시를 매번 모두 폐기하면 random/file IO/외부 component 동작까지 달라질 수 있다. 단순 최종 수치 비교만으로 완료 판정하지 않는다.

## 4. 결과·오류·재정의 경고는 별도 통로다

**관찰 C-6.** `ProcessResult` (`0x060000af`, CIL 시작 14264)는 `%result`를 먼저 검사하고, 유효한 automation value를 결과 노드로 변환한다(IL_0001..0062). `%errors-new`는 오류 통로다(IL_0098..00b5). 결과/오류가 둘 다 없으면 synchronized without result/error로 처리한다(IL_00bb..00bc).

**관찰 C-7.** `ProcessWarningRedefinition` (`0x060000b2`, CIL 시작 15044)은 `%redefs` 리스트를 따로 읽어 `SetRedefinitionWarnings`를 호출한다(IL_0001..0036). `EngineWarningHandler.HandleWarning` (`0x06000063`)은 `Name`, `OldType`, `OldLocation`을 보존한다(IL_000b..002f). 타입 구분은 `ParseTypeString` (`0x06000064`)의 실제 분기로 Function=2, Matrix=1, Value=0이다. `K`의 경고 상수는 built-in function/unit/constant/system variable과 user variable/function/matrix를 따로 갖는다.

**관찰 C-8.** `ParentGlobalErrorResultsDictionary` (`0x060000ac`)는 부모 엔진 영역의 RL에 global/define이 있는지 검사하고, 부모의 `%errors-new` 중 `global_definitions` 관련 오류를 가져오는 경로가 있다. 이는 global 정의에 오류 전파 경로가 있다는 정적 관찰이며 구체적 global 우선순위 알고리즘을 이 함수에서 복구한 것은 아니다.

**호환성 의무.** 유효한 재정의를 계산 오류로 막아서는 안 된다. 원본의 경고 종류와 이전 정의 위치를 표시할 수 있도록 결과·오류·경고·resolved labels를 분리한다.

## 5. 단위는 유리수 지수·출력 체계·행렬 균일성까지 포함한다

**관찰 C-9.** `ValueToIResultNodeTransform.MakeResultNode` (`0x06000087`, CIL 시작 11404)는 scalar → string → function → matrix → symbolic 순으로 실제 proxy를 검사하고 해당 factory를 호출한다. 알 수 없는 타입은 `InvalidOperationException`이다(IL_00ba..00c4). `BuildScalar` (`0x06000089`, CIL 시작 11640)는 실수부/허수부 0 여부에 따라 real/imaginary/complex 노드를 만든 뒤 단위를 붙인다(IL_0000..0070).

**관찰 C-10.** `MakeUnitedValue` (`0x0600008b`, CIL 시작 11848)는 각 `UnitPower.Name`, `.Power.Numerator`, `.Power.Denominator`를 `AddUnit`에 전달한다(IL_0023..003e). `K`의 `Unit` 필드도 `CanonicalName`, `Symbol`, `PowerNumerator:i4`, `PowerDenominator:i4`이다. 차원을 정수 7개로만 저장하는 모델은 분수 거듭제곱 결과 전체를 표현하지 못한다.

**관찰 C-11.** `BuildLazyMatrix` (`0x0600008a`, CIL 시작 11772)는 요소 변환 callback을 가진 lazy matrix를 만들고(IL_0007..0018), `IsUniform && HasUnits`일 때 행렬 전체에 공통 단위를 붙인다(IL_001f..0036). 비균일 행렬은 요소마다 값/단위를 표현할 통로가 필요하다.

**관찰 C-12.** `MakeResultNode(IAutomationValue)` (`0x06000085`)는 `GetProxy(SimplifyUnits)`를 호출한다(IL_0012..0017). `HandleSimplifyUnitSystemUpdated` (`0x0600001a`)는 flag를 바꾸고 전체 재계산한다(IL_0018..001e). 단위 축약은 결과 표시 문자열 치환만으로 처리되지 않는다.

**관찰 C-13.** `EngineBuilder`의 초기 import RL에는 McdTranslator/McdDynamicEfi/McdUnitSystem{0}가 들어간다(`0x0600005b`, IL_0000). 사용자 단위 체계는 UnitDescriptor의 canonical name/RL/quantity로 unit tuple을 구성하고(`0x06000057`, IL_002c..006a), `set_unit_system_details`에 base/derived list를 준다. `HandleUnitSystemUpdated` (`0x0600001b`)는 worker 중지→import/custom RL Update→조건부 재시작이다. UnitSystems enum은 SI=0, US=1, CGS=2, Custom=3이다.

**관찰 C-14.** 생성 시 옵션은 `legacy_mode=16`, `16_bit_tags=true`, `labels_in_symbolics=true`, `auto_eval_range_loops=false`, `symbolic_engine="axiom"`으로 설정된다(`0x06000056`, IL_0014..0063). 이것은 이 빌드의 요청 옵션이며 symbolic engine의 전체 동작이나 배포 라이선스까지 입증하지 않는다.

**호환성 의무.** canonical unit identity와 사용자 표시기호를 분리하고, 유리수 지수, unit override, 비균일 행렬, SI/US/CGS/custom 결과 선택을 지원해야 한다. affine temperature/angle/frequency 등 수치 변환의 정확한 규칙은 여기서 복구하지 않았고 별도 런타임 확인이 필요하다.

## 6. 설치된 도움말에서 얻은 별도 계약

`artifacts/rea/calculation-deep/help-evidence.json`에 설치된 HTML 파일의 절대 경로, SHA-256, 추출 텍스트를 보존했다. 아래는 **동봉 문서의 설명**이며 실제 Prime 12 실행 검증과 구분한다. 일부 도움말에는 escaped worksheet50/math50 XML 예제가 함께 들어 있어 정확한 입력 프로브를 만들 때 재사용할 수 있다.

| 동봉 도움말 파일 | 문서가 설명하는 계약 | 필요한 실행 확인 |
|---|---|---|
| `Tutorials/gs_tutorial/task3-3_ordering_regions_and_applying_labels.html` | 정의는 평가 영역 위/왼쪽에 있어야 하고, 재정의는 그 이후에 적용되며 자기 자신으로 재정의 가능 | 동률 top, baseline/overlap, 이동 후 결과 |
| `about_user-defined_functions.html` | 함수 외부 변수의 정의 당시 값이 함수에 남음; 이름 대소문자 구분 | `a:=2; f(x):=a*x; a:=3; f(4)` 및 정의를 수정한 경우 |
| `global_definition_operator.html` | global은 워크시트 전체에 유효; global RHS는 위쪽 global 정의를 참조; local로 덮어쓸 수 없고 중복 global 금지 | 위/아래 global, global 의존 순서, local 충돌 |
| `about_labels.html` | 동일 이름에 서로 다른 라벨의 의미가 공존; 자동/사용자 라벨 차이 | UNIT m과 VARIABLE m 동시 사용 |
| `about_redefinition_warnings.html` | 재정의는 별도 경고이며 라벨이 다르면 다른 식별자; 경고 표시 설정은 컴퓨터별 | 경고와 계산 성공을 분리 |
| `range_variables.html` | range는 vector와 다르고 반복 역할을 함 | range 할당/행렬 생성/평가/plot 구별 |
| `frequency_and_angular_velocity_units.html` | radian은 무차원이고 Hz/rpm/Hza를 구분 | 값 변환 및 표시 단위 정확성 |
| `about_controlling_region_calculation.html` | 열기만으로 자동 재계산하지 않음; manual/stop/calculate-now/disabled 별도 의미; random/file IO cache 사례 | 저장 cache와 실제 재계산을 명시적으로 구분 |

특히 현재 `docs/PROTOTYPE.md`가 명시하는 **함수 외부 변수를 호출 시점에 조회하는 규칙**은 동봉 Mathcad 문서와 다른 독자 규칙이다. 이것을 그대로 확대하여 Mathcad 호환이라고 부르면 안 된다. 원본 실행값으로 고정할 회귀 사례가 필요하다.

## 7. detune 저장 캐시의 불일치 재확인

원본은 읽기만 했다. 파일 SHA-256은 `bd4b329ce3cb2de848843c80e3bd42570022dc54a2c7d335f898c234151c1cbf`이다. 새 파생 기록은 `artifacts/rea/calculation-deep/detune-region20.json`이다.

- `mathcad/worksheet.xml` region 20: VARIABLE `M`을 `scale(17.704, UNIT μH)`로 define, `resultRef=14`.
- `mathcad/result.xml` result 14: `calculation-status="Synchronized"`, `0.000171445 henry`.
- 단순 unit scale로 기대하는 `0.000017704 H`와 저장 cache가 다르다.

확인된 사실은 XML 입력과 저장 cache의 불일치까지다. 과거 편집 상태, 계산 동기화, 단위 재정의, 문서 손상 중 어느 원인인지 이 정적 증거만으로 단정하지 않는다. 원본을 별도 복사한 후 Prime에서 강제 재계산·완료 대기·저장한 결과를 비교해야 한다. 따라서 기존 50/51은 **저장 cache와의 일부 수치 일치율**이고 원본 계산기의 호환성 통과율이 아니다.

## 다음 구현을 막는 열린 질문

1. geometry→계산 순서의 정확한 비교 기준, 완전 동일 위치/중첩 영역, area/header/footer 포함 여부: 부분 답변. controller의 순서 소비는 확인했으나 순서 생성은 별도이다.
2. 함수 closure/global/중복 정의/자동 라벨의 실제 실행값과 오류 코드: 부분 답변. 문서·정적 근거가 있고 runtime oracle이 필요하다.
3. `mccore12`/`MCFront12`의 AST→RL→숫자 평가·오류 태그: 부분 답변. 후속 분석으로 실제 Lisp frontend→CoreFactory.NewAction→ArlTranslate→MplCompile 연결을 확인했다(아래 후속 절). 네이티브 변환기와 수치 커널 전체는 미해결이다.
4. matrix algebra, range, ORIGIN, vectorization, solver, symbolic, complex branches/NaN/Inf, precision/rounding: 부분/미해결. 이번 조사로 전체 수학 엔진을 복구했다고 주장하지 않는다.
5. fractional/affine units, default unit catalog 수치, custom unit substitution: 부분 답변. 표현 구조와 설정 경로를 확인했으나 전체 변환 알고리즘은 별도이다.
6. native worker queue의 우선순위·동시성, 중단 후 재개, 외부 입력의 cache 규칙: 부분 답변. 관리 CIL의 invalidation/cache/interrupt 경로는 확인했으나 native 부분은 남아 있다.

완료 기준은 화면상 비슷한 계산기가 아니라, 같은 worksheet 구조/영역 순서/라벨 입력에 대해 같은 값·오류·단위·재정의·수동 계산 행동을 보이는 독립 Rust 구현이다. 내부 코드 복제나 원본 DLL 런타임 의존은 이 결과물의 목표가 아니다.

## 후속: 실제 frontend 파일과 계산 경로

이 후속 조사는 GUI/COM/원본 앱 실행 없이 수행했다. 사용자가 GUI 작업 중이므로 파일 읽기·REA 정적 분석만 진행했다. 설치된 PTC Lisp 소스를 저장소에 복사하지 않았다. 원본 경로·파일 SHA-256·선택한 행 범위의 byte offset/length/SHA-256만 `artifacts/rea/calculation-deep/frontend-source-identities.json`에 저장했다.

### 정확한 로딩 대상

| 실제 설치 파일 | bytes | SHA-256 |
|---|---:|---|
| `lisp/front12.lisp` | 131011 | `5b15dd2d9066b253200133a5b5fd79263718a9f5f5d4e520cdadda5c104e01ee` |
| `lisp/lisp.lisp` | 12092 | `eb29abd468265f459b5a3a0f7040e0af615a1dea3425c5f10d2c76231afa3c5c` |
| `lisp/solve.lisp` | 26798 | `16e3a5ca35d46b29ee1e7b9de75928836d95f8255cde67ac393b79201c2fa873` |

세 파일은 평문 Lisp이다. `front12.lisp:5`가 `lisp.lisp`를, `front12.lisp:1050`이 `solve.lisp`를 불러오는 실제 로딩 식을 읽었다. 파일명이 우연히 비슷한 것을 근거로 연결하지 않았다.

관리 frontend는 **mcfront12.dll**에 있다. SHA-256 `d9a7b63fa39957df8f1178e641e2dc9c4a669c08f6ab1c3a1c352e42bf8b9b79`, MVID `4fa956fa-91f3-45a1-9dbc-f5b8d758abe1`이다. REA `mcfront12.focused.members.json`의 Evidence는 `ev_c3fcb93ec0882166661e887472307ea1cfa1942d6a520aea234d7b31ec9b307a`이다.

`MCFront12.DoInitialize` (`0x0600015c`, CIL 시작 35004)는 `mslisp.Factory.NewInterpreter`(IL_00ff), interpreter 특수변수 설정, `load_file`(IL_0192), `init`(IL_01a0)을 호출한다. 파일 경로 후보는 정적 필드의 실제 FieldRVA 데이터에서 `""`, `"lisp"`, `"../lisp"` 및 `front12.lisp`로 확인했다. 관련 바이트 위치는 `mcfront12.loading-path-fields.json`에 보존했다. Lisp의 `setup-load-path`(400–413행)는 기준 디렉터리의 부모와 상위 경로 아래 bin/mpl/lisp를 탐색 경로에 추가한다. **설정된 경로와 실제 파일 존재**는 확인했으며, 이 조사 자체가 런타임 파일 열기 이벤트를 관찰한 것은 아니다.

`MCFront12.init` (`0x06000152`, CIL 시작 17528)은 interpreter의 frontend 함수를 `utl_mgd.Function` 필드에 연결한다. 예를 들어 update, get-value-before-async, add-region-after, recalculate-all의 등록과 해당 필드의 `Invoke3/Invoke2/Invoke0` 호출이 각각 실제 CIL에서 이어진다. 따라서 일반 C# 타입만 분석하면 실제 정책을 놓친다.

### 입력 구조→frontend→core 연결

1. `front12.lisp:1195–1250`: 입력 문자열→TreeToRL.Convert→태그와 region identity를 가진 RL 읽기→preprocess→영역 상태/정의 분류→GlobalMap.OnUpdate.
2. `1819–1853`: Scan으로 구조·symbolic/solve/embedded 등의 flag를 얻고 필요한 경우 `%bindings`, range, subscript 해제 등을 전처리한다. 정적 의존성은 RHS를 포함한 식으로 별도 구한다.
3. `1883–1910`, `1988–2162`: 최상위 RL 종류별 dispatcher와 인자 개수 검증이 존재한다. define/global/eval/define-eval/symbolic/undefine은 서로 다른 연산이다. placeholder 오류도 분리한다.
4. `2218–2254`: evaluation action은 `_imports`, `_options`, `_toptop`에 의존한 뒤 `MCCore12.NewAction`을 만든다. 따라서 계산 옵션·단위/import 환경의 수정은 기존 식이 같은 문자열이어도 재번역/재계산을 유발할 수 있다.
5. `mccore12.dll`의 `MCCore12.NewAction`(`0x060001d7`)은 `CoreFactory.NewAction`(`0x060001cb`)으로 이어진다. `CoreFactory` Evidence는 `ev_6189932ca15fb8a71b67ef348811cfece01c5fb30a6e2d08873f8cff6e8ab807`; SHA-256 `4f1252b8ca801f04fdcabbecc833c9430771780fb9768f4c446a4a6f1c439b08`, MVID `6dc2ebfe-9d33-4170-9519-ccedaeaa3ea8`이다.
6. `CoreFactory.NewAction`(CIL 시작 18032)은 placeholder만 ConstAction으로 만들고(IL_0003..001c), 일반 식에는 translator/builder/module environment를 넘겨 `ArlTranslate`를 생성한다(IL_001f..0037).
7. `ArlTranslate` Evidence `ev_f28a742269836a9a601af3c35902f4514c560d2bf86f2f7f3f1f1bb51a8cbf0e`, `Invoke` 토큰 `0x0600019b`: TranslateHelper를 만들고 `Environment.ExecuteInterruptibly`로 실행한다(IL_0210..0220). 모듈 오류를 검사하고, exports에서 ModuleData/RedefMap/LabelMap/AlgorithmMap을 가져와(IL_0715..074f) 재정의·라벨·알고리즘 결과를 관리 형태로 바꾼 뒤 다음 `MplCompile` action을 만든다(IL_07aa). 이로써 단순 식 인터프리터를 넘어 번역·의존성·결과 메타데이터가 이어지는 경계까지 확인했다.

TreeToRL의 관찰 범위도 명확히 한다. REA `mcfront12.tree.members.json` Evidence `ev_42f667ece5df07d2db91fa71a548e033bcf29a7bf6da9f65ad8d248a79c5bfe0`의 `Convert`(`0x0600027b`, CIL 시작 48432)는 입력에서 `(texp …)` 부분을 찾아 따옴표/escape/괄호 중첩을 처리하고(IL_0111..01bf), 네이티브 TreeToRL을 호출한다(IL_01d6). 실제 구조 변환은 `0x060000b6`, native/unmanaged, RVA 6432 경계다. 별도 boundary Evidence `ev_c7bdff245a442ddc316fbac1bbd42da626addaa66eeb24cbaeb80eb9aead3f13`에 기록했다. **이 wrapper를 MCDX XML→전체 AST 변환기로 오인하지 않는다.** 구조 연산자 전체 매핑은 네이티브 쪽 추가 증거가 필요하다.

### Prime 모드의 global 처리: legacy 분기와 구별

`front12.lisp:207–208`의 legacy 조건은 현재 mode가 인자보다 작거나 같은지 검사한다. C에서 실제 설정하는 mode는 16이므로 legacy 15 분기를 따라 구현하면 잘못된다. 초기 Lisp의 기본값 15만 보고 Prime 모드를 판단해서도 안 된다. mode 16에서 업데이트는 `GlobalMap.OnUpdate`를 통과하고(1195–1250행), global도 현재 zone 안의 triple definition 경로를 사용한다(2442–2456행).

`Engine.GlobalMap` 정적 분석은 `mcfront12.globals.members.json`, Evidence `ev_4760e9e7fe89a4eeb7070a990f86b8febd7c0bd66b27a09837346c917b0c737d`에 보존했다. 실제 분기로 다음을 확인했다.

- `CheckGlobals`(`0x06000263`, CIL 시작 45972): 심볼별 top-level global 수가 2개 이상이면 multiple-global 오류(IL_001a..0024). 1개이면 top-level normal 정의에 normal/global 충돌 오류(IL_0028..0032, IL_0095..00d1).
- global이 top-level ancestor 자체가 아니면 별도 illegal-context 오류(IL_0053..007a).
- `SetZoneRecursive`(`0x0600026c`)는 top ancestor가 global인지 검사해 전체 자식 subtree의 계산 zone을 선택한다(IL_0002..0044).
- `NodeData.Update`(`0x0600024f`)는 이전 계산 노드를 제거한 후 error action 또는 add action을 실행한다(IL_0009..0044). 충돌이 생겼을 때 마지막 정의를 조용히 채택하는 구조가 아니다.
- 원래 문자열은 `multiple global definitions`, `normal and global definitions`, `illegal context for global definition`이다(`0x06000277`). 번역된 UI 오류 키와 혼동하지 않는다.

### Rust 구현에 사용할 작은 동작 계약

아래 표의 **정적 확정**은 읽은 정책/분기에서 직접 나오는 규칙이다. 사용자에게 보이는 수치 결과는 별도 원본 런타임 suite로 확인해야 한다. `global` 표기는 이 설명을 위한 개념 표기이며 현재 프로토타입의 문법 확장을 뜻하지 않는다.

| 사례 | 독립 동작 명세 | 근거와 남은 확인 |
|---|---|---|
| `global a=7`을 평가 영역 아래에 둠 | global zone이 normal 문서 zone보다 앞서며 위치 이동·global 충돌을 함께 갱신 | frontend 515–539, GlobalMap.SetZoneRecursive. 실제 표시값은 runtime oracle로 확정 |
| 같은 라벨의 `a:=2`와 `global a=7` | 마지막 값 우선 규칙을 적용하지 않고 해당 normal 정의에 충돌 진단 | GlobalMap.CheckGlobals 실제 분기 |
| 같은 심볼의 top-level global 2개 | global 중복 오류를 보존 | GlobalMap.CheckGlobals global count>1 |
| normal `x:=1` 후 `x:=x+1` | RHS 조회는 현재 정의 앞 환경을 사용; 정의 영역 이후에는 새 binding 적용 | D PositionalEnv.BEFORE + C 위치별 region. closure/동적 갱신은 runtime과 결합 |
| UNIT `m`과 VARIABLE `m` | 서로 다른 타입/라벨 binding을 유지, wildcard 자동 라벨은 별도 해석 | D 라벨 unification + C resolved labels |
| `ORIGIN`에 VARIABLE 라벨과 SYSTEM 라벨을 각각 부여 | 두 식별자를 단순 문자열 ORIGIN 하나로 합치지 않음; 시스템 ORIGIN 초기값은 0 | frontend 581–583, 3402–3446; 도움말의 실제 `<id labels="SYSTEM">` 예제 |
| `undefine`으로 SYSTEM ORIGIN 해제 | 기본값 0으로 복귀하는 특수 동작이 존재 | frontend 2092–2103, 3651–3724. TOL/CTOL=0.001, PRNPRECISION=4, PRNCOLWIDTH=8도 명시 |
| `x:=rhs`와 정의 즉시 평가/단위표시 | 정의와 evaluation/scale 출력을 별도 단계로 모델링. display용 unit override가 저장된 값/행렬을 변형하지 않아야 함 | frontend 2572–2657. RHS action의 중복 실행을 막고 출력 결과를 병합 |
| matrix/index LHS 정의 | 정의되는 이름은 index 표현식이 아니라 matrix base이며, index는 별도 의존성 | frontend get-defined-vars 3307–3324; make-lhs 3257–3280은 subscript 안의 이름을 임의 sym 접두어 변환하지 않음 |
| 인자 부족/과다/미완성 placeholder | 잘못된 입력을 0/빈 식으로 대체하지 않고 종류별 진단과 위치 태그 보존 | frontend 1890–1909: too_few_arguments, too_many_arguments, bad_placeholder |
| 수동 모드 CalculateNow | 지정 영역 이후의 계산 노드는 잠그고 해당 경계까지 계산 요청 | frontend 1177–1183. 전체 재계산과 다름 |
| 전체 재계산 | 필요 시 노드 잠금을 풀고 options 의존성을 바꿔 재계산 유발 | frontend 3482–3488. 식문자열만 cache key로 쓰면 누락 |

정의는 numeric/symbolic/explicit 형태를 함께 관리하며 RHS action 생성은 한 번만 수행하는 경로가 있다(3187–3252행). 같은 변수에 대해 출력을 세 번 계산하는 것으로 구현하면 부작용·random·외부 데이터 입력이 달라질 수 있다. Rust의 공개 AST에는 정의/평가/단위 override/label/index/placeholder/region identity를 유지하고, 이 frontend와 **행동을 맞추는 독립 구현**을 해야 한다. PTC의 Lisp monad나 생성 코드를 옮기는 작업은 하지 않는다.

### 2026-10-15: 구조 편집기와 독립 계산기의 결합 검증

이 단계에서는 원본 프로그램, COM 및 GUI를 실행하지 않았다. 기존 정적 REA 증거와 사용자 문서 `detune.mcdx`를 읽기 전용으로 사용했다. 문서 SHA-256은 `bd4b329ce3cb2de848843c80e3bd42570022dc54a2c7d335f898c234151c1cbf`이다. 구현은 `mathcraft-compat/src/editor_bridge.rs`, `src/lib.rs`에 있으며 PTC의 소스나 생성 코드를 복제하지 않는다.

해당 문서의 수학 영역은 66개이다. 최상위 정의 63개 중 47개의 RHS는 `eval`이고, 최상위 `eval`은 3개이므로 현재 수식에서 요청한 출력은 50개이다. 35개 `ws:math`에는 계산식 외에 `ws:resultFormat` 또는 `ws:formattingOverride`가 붙어 있다. 이 두 종류는 계산식과 분리한다. 편집 bridge는 수식만 반환하며, 원래 wrapper/표시 형제의 보존 책임은 원본 아카이브를 가진 문서 계층에 있다. `disable-calc`를 포함한 다른 wrapper 속성 및 모르는 형제는 조용히 제거하지 않고 오류로 반환한다. `ws:text`/`ws:image`만 가진 영역은 계산 노드에서 제외하며, 모르는 영역 종류는 여전히 명시적인 미지원이다.

식별자는 라벨 문자열과 원래 `label-is-contextual` 여부, XAML `Span` 안의 문자와 PTC `Subscript`를 구조적으로 보존한다. XAML은 inert XML로 읽으며 실행하지 않는다. 계산에서 literal subscript를 단순 `_` 문자열로 합치면 `Z`의 literal subscript `in`과 이름 자체가 `Z_in`인 식별자가 충돌한다. 현재 독립 구현은 XML 식별자에 나타날 수 없는 내부 구분자를 사용해 이 두 identity를 구분한다. 지원하지 않는 rich identifier 속성/중첩은 raw XML 보존용 오류를 낸다. `ml:imag`의 `symbol="i"`/`"j"`도 보존한다. 원본 문서에 명시된 라벨은 계산에서도 그대로 사용한다. 편집 후 contextual label 전체를 다시 추론하는 Prime의 wildcard 정책을 구현한 것은 아니다.

정의 즉시 평가의 구현 계약은 `a:=a+1=`의 RHS를 이전 환경에서 한 번만 평가한 뒤 새 binding을 저장하고, 그 저장된 값을 출력하는 것이다. 예를 들어 이전 `a=2`일 때 저장값과 표시값은 모두 3이어야 하며 4가 되면 안 된다. `distance:=2 m=cm`은 canonical binding을 `2 m`로 유지하고 표시 payload만 `200`으로 만든다. 뒤의 `distance=`도 `2 m`이다. 함수의 free identifier는 정의 시점 snapshot을 참조한다. `a:=2; f(m,s):=a*m+s; a:=3; f(4,1)=`의 결과는 9이다. 신규 입력의 파라미터 `m`, `s`는 변수로 분류하여 기본 단위와 구분한다. 이는 제한된 독립 자동 라벨 정책이며 전체 Prime inference와 같다는 주장이 아니다.

추가한 scalar 함수는 `sin`, `cos`, `tan`, `ln`, `log`(10 기반), `exp`, `sqrt`, `abs`, `re`, `im`, `conj`이다. 복소수와 물리 차원을 보존하며, 삼각/로그/지수 함수에는 무차원 검사를 적용한다. `re`/`im`/`conj`/`abs`는 입력의 차원을 유지한다. 사용자 함수는 같은 라벨 identity의 기본 함수보다 우선하며, 실패한 사용자 정의도 기본 함수로 되돌리지 않고 오류를 전파한다. 함수명 identity를 읽을 수 있으면 잘못된 파라미터 선언도 실패한 binding을 남겨 기존 함수가 계속 실행되지 않게 한다. 이 함수들의 기본 수치 검사는 독립 구현 회귀 테스트이며 원본에서 새로 실행한 oracle은 아니다. 행렬의 일반 산술, solver, symbolic, 전체 범위 반복을 지원한다고 해석하면 안 된다.

검증 결과:

- `cargo test -p mathcraft-compat --locked`: 24개 테스트 통과. 기존 38개 원본 synthetic reference case를 검사하는 테스트도 포함한다.
- `cargo clippy -p mathcraft-compat --locked --all-targets -- -D warnings`: 통과.
- 원본 문서의 수학 66개를 `PortableTree`로 읽고 MathML로 저장한 뒤 다시 읽었을 때 66개 모두 구조가 동일하다.
- 이 66개 저장 수식을 별도의 새 테스트 MCDX에 넣어 독립 Rust로 재계산한 전체 영역 JSON은 원래 수식 입력을 계산한 JSON과 동일하다. geometry, resultRef, 오류, canonical/display 값도 포함한다.
- 독립 계산에서 수학 영역 66개의 오류는 0개, 현재 요청된 출력은 50개이다. 계산에 원본 저장 캐시는 사용하지 않았다.
- 원본 저장 캐시에는 숫자 결과가 51개 있다. 현재 출력이 있는 50개는 비교 시 united result의 canonical 값과 무단위 display payload를 구분하면 모두 일치한다(절대 허용치 `1e-12`, 상대 허용치 `1e-8`). 남은 result14는 현재 소스의 출력 요청이 없는 `M` 정의에 붙은 과거 증거로 보존하며 신규 출력/성공으로 세지 않는다.

재현 자료는 ignored `artifacts/rea/calculation-deep/verify-detune-expanded.py`, `detune-expanded-verification.json`, `detune-independent-expanded.json`, `detune-bridge-roundtrip.json`, `detune-bridge-recalculated.json`에 있다. **저장 캐시와의 50/50 일치는 원본 엔진의 신규 실행과 비교한 정합성 결과가 아니다.** 전체 제품 호환을 입증하지 않는다.

### 행렬 순서의 확인 범위

K의 `XmcdToIResultNodeTransform.BuildMatrix`(`0x0600022e`, CIL 시작 파일 offset 14712)는 rows/cols를 읽어 eager matrix를 만들고 `AddElementsToMatrix`로 채운다. `AddElementsToMatrix`(`0x06000234`, 시작 15856)의 IL_0014..001e는 `(row,column,convertedChild)`로 `IEagerMatrixResultNode.SetElementAt`(`0x06000127`)을 호출한다. IL_0023..0035는 row를 증가시키고 `Rows`에 도달하면 column을 증가시키며 row를 0으로 되돌린다. 따라서 **결과 XML**의 linear children은 column-major이다. 새 decode 자료는 기존 K의 REA metadata와 원본 method byte hash를 재사용한 `calculation-core-members.json.full-il.json`에 있다.

이 결과 경로만으로 입력 식 MathML의 일반 행렬 순서까지 단정하지 않고, 다음의 별도 shipped example을 확인했다. `Help/en/PTC_Mathcad_Help/example_matrix_algebra.html:40–56`의 Math50 입력 matrix는 flat child를 `0,3,5,1,0,3,2,2,1`로 저장한다. 함께 설치된 `images/example_matrix_algebra0.png`를 직접 읽으면 행은 `(0,1,2)`, `(3,0,2)`, `(5,3,1)`이다. **따라서 입력 식 XML도 column-major이다.** 이 근거로 bridge의 XML↔editor 변환과 계산기 입력 변환을 확장했다. editor와 `Value::Matrix.elements`는 row-major이고, 원본 결과 비교용 `numeric_json()`만 column-major로 만든다. `A:=[1,2;3,4]; A[1,0]=`은 ORIGIN=0에서 3이고 SYSTEM ORIGIN=1이면 `A[2,1]=`이 3이다. 단일 index는 원본 `index_operator.html:49–80`에서 명시한 column vector에만 허용한다. 일반 행렬 산술이나 index LHS 정의는 아직 미지원이다.

### 범위 값과 top-level 반복 평가의 제한된 지원

shipped `example_using_range_variables_in_plots.html:145–153`의 입력 구조는 `range(sequence(start,second),end)`이다. step 없는 경우는 `range(start,end)`이다. `range_operators.html:55–102`는 기본 step=1, 명시한 second에서 step=`second-start`, 실수 bounds, 차원이 있으면 second 필수, endpoint가 step의 정수배가 아니면 마지막 endpoint 전에 멈춤을 설명한다. `range_variables.html:66–90`은 range가 vector가 아니며 일반 변수 정의 RHS의 scalar 대신 쓸 수 없다고 명시한다. `images/range_variable3.png`는 i=0..2의 평가를 괄호 없는 column `0,1,2`로, `range_variable8.png`는 `i+1=`을 괄호 없는 column `1,2,3`으로 표시한다.

독립 구현은 `Value::Range {start,step,end,count}`를 별도 타입으로 유지한다. scalar/matrix로 자동 변환하지 않는다. `Statement::Evaluate`에서 식이 한 개의 named range에 직접 의존하면 snapshot을 복제하여 각 점의 scalar binding을 대입하고 `Value::RangeValues {elements}`로 출력한다. 예를 들어 `i:=0..2; i+1=`은 `1,2,3`이고, `f(x):=x^2; f(i)=`는 `0,1,4`이다. 일반 `j:=i+1`은 여전히 scalar 오류/미지원이며 range alias도 허용하지 않는다. 서로 다른 range 두 개의 cartesian 순서, 사용자 함수의 captured free range 내부까지 추적하는 암묵 반복, index LHS 반복, symbolic 반복은 구현하지 않고 오류로 알린다. 직접 입력된 range의 범위는 최대 16,384개이며, 반대 방향으로 정의된 step이나 비유한 cardinality도 미지원으로 남긴다. 부동소수점의 정수 endpoint 부근에는 작은 rounding 허용치를 두는 독립 정책이며 원본의 정확한 rounding 계약은 아직 확인하지 않았다.

shipped HTML/PNG 전체 해시와 선택 범위 해시는 ignored `artifacts/rea/calculation-deep/shipped-help-structures-identities.json`에 기록했다. 이 자료는 설치된 도움말의 정적 증거이며 이번 단계에서 새 원본 실행은 하지 않았다. 원문 도움말/이미지/프로그램 소스는 저장소에 복제하지 않는다.

재확인용 핵심 SHA-256:

| 설치 도움말의 상대 경로 | SHA-256 |
|---|---|
| `example_matrix_algebra.html` | `983fea74e45014f51d1adc9469956cf67babd841944207319e14239b457bb064` |
| `images/example_matrix_algebra0.png` | `483c71ac76d242ab7294574bce687171b2e053f59a5bb4ff3dc7c67f363f0d1b` |
| `example_using_range_variables_in_plots.html` | `389d13171f85199e803465ef1c347509a3eec2cb43ce51c3bd02b63c6b5726a1` |
| `range_variables.html` | `4d2244201e63f926eb3094d5f8e2b74f1ef6fea7557036c76ca404bc939bb71b` |
| `range_operators.html` | `88c8276521d5db8e8f53e29db0c193a9eb98c4e654c3e8021960761ed6acbe32` |
| `index_operator.html` | `3d5997e15472d892c1c55026410eef52d751ccfb6d06f22d03a587d14f8cb2ad` |
| `images/range_variable3.png` | `e24154cb9158dc3bd9772567bd1a669ea92b3e3e684aeaac3f2b39815858cf32` |
| `images/range_variable8.png` | `94b4c5c8cba4583f1fd282760770198365122aa4b4f48ca22411284d56d109ac` |
