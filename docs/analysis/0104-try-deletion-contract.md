# TRY 삭제·헤더 클릭 계약 — 2026-10-19

[0103](0103-try-control-contract.md)의 후속 정적 조사다. 원본 GUI·스크린샷·앱·CLR·COM을 실행하지 않았고 제품 코드는 변경하지 않았다. 기존 REA metadata/CIL을 재사용하고 설치 파일 및 선택한 원시 IL SHA를 대조했다. 상세 ledger는 ignored `artifacts/rea/try-delete-contract-20261019.md`, 전체 선택 CIL은 `feedback/try-delete-*.txt`·`try-pointer-cil.txt`에 보존했다. 초기 회신의 empty center→PH 추론은 아래에서 정정한다.

## 증거 신원

설치 기준은 `C:/Program Files/PTC/Mathcad Prime 12.0.0.1/`다.

| 기호 | 파일 | SHA-256 / MVID / REA Evidence |
| --- | --- | --- |
| E | `Ptc.EquationEditor.dll` | `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223` / `d8627dca-3258-470c-b4d1-c00bbe2f2441` / `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf` |
| C | `Ptc.Controls.EquationControl.dll` | `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1` / `cf1d7cb5-c73f-4618-8e6b-b83959d13b98` / `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` |

Direction은 Left=0, Right=1, Indeterminate=4다. E OperatorFamily.Program은 field `0x040006a8`, Constant I4 `03 00 00 00`=3이다. token은 해당 SHA/MVID 빌드에만 유효하며 같은 번호의 E/C 메서드는 다르다.

## 한 Delete 명령 안의 처리 단계

[0096](0096-program-control-authoring-contract.md)의 Backspace→DeleteLeft / Delete→DeleteRight → EquationModel.Delete → normal mode 계약을 재사용했다. E `bh.br` `0x060005c8`는 초기/현재 navigator, 삭제 방향, handled=false를 작업 상태에 둔다. `ap.b` `0x0600042e`는 **TRY handler `ap.aa`를 stage11·29에 두 차례** 등록한다. 그 사이 stage21 `ap.t`는 working navigator를 한 상태 이동한다. runner `0x06000432`는 handled=true에서 중단한다. 이 내부 이동을 별도 사용자 화살표 입력이나 별개 Undo로 나누지 않는다.

외부 Delete command가 한 Undo 경계를 소유한다. 독립 구현은 tree/caret/selection/scope를 함께 복원하고, 실패·무변경 때 redo를 보존해야 한다. 모든 PH 위치에서 어떤 선행 handler를 거쳐 TRY에 도달하는지까지 실행 검증한 것은 아니므로 아래는 **TRY handler에 도착한 navigator 상태**의 계약이다. 선택/문자 삭제 등 앞 단계와 주변 container의 일반 동작을 무시하지 않는다.

## Empty는 단순한 자손 PH 검색이 아니다

E `MathNode.AreAllChildrenEmpty` `0x06000c89`는 각 직접 child가 PH이거나, **parent의 inherent child이면서 재귀적으로 empty**일 때만 true다. child가 없는 노드 자체를 검사하면 true지만, non-inherent child를 그 이유로 empty 처리하지 않는다. 예를 들어 Break는 PH가 아니며 Program의 일반 statement로 놓이면 empty가 아니다.

TRY의 두 BODY는 inherent [0,1]이므로 BODY[PH]/BODY[PH]는 empty다. PROGRAM/BODY/RETURN의 자체 inherent children 기본값은 []다. constructor `0x06000f74 IL_0110..0116`은 fieldbg에 shared empty array를 넣고, .cctor `0x06001043`는 Int32[0]을 만든다. 따라서 BODY 안의 일반 nested Program[PH]를 무조건 empty로 보아서는 안 된다.

## TRY handler의 정확한 guard

E `ap.aa` `0x06000440`에서 N=현재 노드, side=CaretSide, dir=삭제 방향, T=대상 TRY다.

1. N이 TRY이고 side==dir이면 즉시 반환한다(`IL_000c..0031`). 바깥 방향 삭제로 TRY를 강제 제거하지 않는다.
2. N이 TRY가 아니면 parent가 TRY, side!=dir, N.childCount==1, N[0]이 PH, parent TRY 전체가 empty여야 한다(`0032..0093`). canonical AST에서는 direct BODY의 안쪽 경계다. 그 밖의 populated BODY 경계로 일반화하지 않는다.
3. target을 T로 올린 뒤 handled=true다. side!=4이고 T가 populated면 **변경 없이 종료**한다(`009b..00b8`). 따라서 populated TRY의 outer Right+Backspace/Left+Delete를 내용과 함께 지우면 안 된다.
4. empty일 때 PH로 바꾸는 조건은 `(원래 Current가 inherent child && side!=dir)` 또는 `(원래 Current==T && side==Opposite(dir))`다(`00c8..012b`). root empty TRY의 outer inward 삭제와 엄격한 empty BODY inward 경계가 여기 해당한다.
5. 그 밖의 header center 경로는 attempted BODY.children 뒤에 handler BODY.children을 순서대로 append한 뒤 TRY를 attempted BODY로 교체한다(`012e..014b`). `MergeNode` `0x06000d2f`는 실제로 끝에 child 배열을 넣는 함수이며 식을 버리거나 뒤집지 않는다.
6. guarded previous Program merge를 먼저 시도하고, 실패하면 일반 `a6.a` repair를 수행한다(`0150..019a`). 마지막 focus는 아래 계약을 따른다.

### Empty center 정오표

**empty root TRY의 header center(side4)는 PH 교체가 아니다.** root Current는 inherent가 아니므로 `IL_00e0`에서 `00f5`로 가고 side4는 Opposite(dir)와 같지 않아 `012e`로 간다. 두 BODY[PH]를 BODY[PH,PH]로 합친다. TRY 자체가 상위 inherent slot에 있는 별도 상태와 혼동하지 않는다. 초기 회신의 “empty center도 모두 PH”는 이 분기를 놓친 추론이므로 폐기한다.

repair의 TransformTo handler `0x06000592`는 descriptor만 바꾸고 children을 보존한다. BODY의 허용 parent 밖이면 DFT `IL_443c..4535`가 PROGRAM으로 바꾼다. 두 precedence는 290으로 같아서 precedence-changed reparse flag가 켜지지 않는다. PROGRAM initializer `4040..40e1`은 nary/default count1이며 PH 금지·empty 흡수 panacea가 없다. 따라서 **root empty header의 canonical 결과는 Program[PH,PH] 보존 경로**다. 모든 외부 parent의 추가 repair까지 같은 결과라는 뜻은 아니다.

| TRY handler 입력 | 확인된 결과 |
| --- | --- |
| empty outer Right+Backspace / Left+Delete | 해당 TRY slot을 PH로 교체, PH Left navigator |
| empty root header center | 두 empty BODY merge 후 Program[PH,PH] 정규화 |
| canonical empty TRY의 direct BODY inward | 엄격한 guard 후 TRY→PH |
| populated outer inward | 처리 완료·무변경 |
| populated header center | attempted+handler statement 순서를 보존하여 wrapper 제거, guarded merge·repair |
| outer outward / 임의의 populated BODY boundary | 이 TRY handler의 rewrite 근거 없음; 주변 일반 삭제 경로를 보존 |

## 이전 Program 병합·caret

E `Utilities.MergeWithPreviousProgram` `0x06000f3b`는 다음이 모두 맞을 때만 병합한다. replacement가 Program family3, parent도 family3, index>0이고, 바로 앞 sibling이 Program family이거나 그 sibling의 **마지막 child가 Program family이면서 inherent**여야 한다. 이때 앞 eligible Program 끝에 replacement.children을 붙이고 현재 slot을 parent에서 제거한다. PH는 Program family가 아니므로 이 경로에 들어가지 않는다.

알려진 AST의 마지막 body mapping은 직접 initializer로 확인했다: While[1] `4148..4154`, For[2] `4290..429c`, IF_THEN[1] `485e..486a`, ELSE[0] `497e..4986`, ELSE_IF[1] `4a96..4aa2`, ALSO_IF[1] `4bb6..4bc2`, TRY[0,1] `4cde..4cea`. Pair-nary getter `0x06000cfe`는 `0x06000d31`로 실제 subdescriptor를 찾아 flat IF body의 inherent index를 계산한다. Return의 Program operand는 이 inherent 목록에 포함되지 않는다. 임의로 더 앞의 sibling, 마지막이 아닌 nested body, non-inherent child까지 병합하지 않는다.

previous Program 병합 성공 시 접합점 focus가 가장 우선이다. Backspace는 기존 앞 Program의 마지막 child Right, Delete는 새로 붙인 첫 child Left다. 이전 Program 병합이 없고 원래 NodeSide==Right(on error header)이면 attempted/handler 접합점으로 이동한다: Backspace→마지막 attempted child Right, Delete→첫 handler child Left. NodeSide==Left(try header)의 기본은 replacement Left navigator다. `bq.c`→`0x06000601`은 target의 첫/마지막 navigable state와 enter side를 정규화하므로 non-PH container에서 항상 raw container node 자체를 가리킨다고 단정하지 않는다.

## 헤더 클릭은 NodeSide이며 explicit part index가 아니다

C `VisualTryNode.get_NavigatableParts` `0x06000d6b`는 [TryOperator, OnErrorOperator]를 반환한다. `GetNodeAtPoint` `0x06000d75`는 body 영역을 child hit-test로 넘기고 header를 다음처럼 반환한다.

| 영역 | CaretSide | NodeSide | OperatorPartIndex |
| --- | --- | --- | --- |
| try header (`IL_00fb..0104`) | 4(center) | Left0 | null |
| on error header (`00b9..00c2`) | 4(center) | Right1 | null |
| right minor fence의 오른쪽 | Right1 | 4 | null |
| try operator의 왼쪽 | Left0 | 4 | null |

사용한 constructor `0x0600093e`는 default Nullable을 `0x06000940`으로 넘기므로 pair-nary IF의 explicit numeric part overload와 다르다. `VisualTryNodeEditor.GetSymbolNode` `0x06000d88`는 NodeSide Left→try, Right→on error이며 `UpdateOperatorSymbols` `0x06000d86`도 이 side로 강조/blink를 나눈다. 독립 editor가 두 header를 native NodeSide Left/Right·part None에 대응시키는 클릭 지원은 직접 원본 근거가 있다. 원본이 operator part index 0/1을 저장했다고 설명하면 안 된다.

## Raw IL hashes

| Assembly/token | Role | IL SHA256 |
|---|---|---|
| E `0x06000440` | TRY deletion | `9a27b217ca9d0376d5042857a97e45dd129f769af05ac71d2f6cb0676fc00e4a` |
| E `0x0600042e` | stage array | `b98fe5c621a8025d7612d3a83874e72e8dbdce39ea75c237217986068ee57263` |
| E `0x06000432` | stop-on-handled runner | `cf922c95dacd5e9ec4e196cc9d2c599b1b0d7a1361ad4b8ec866cd78996d477c` |
| E `0x06000447` | internal one-state movement | `f9d5b2c4750f7d13052c2480c0a391f12e19707443fbb0b0072a4159ebc6c0b3` |
| E `0x060005c8` | normal delete operation | `3acd6d0e157fedc948767a260648fd6ef6345f8126b53506c2dcb2615b245c1d` |
| E `0x06000c89` | recursive emptiness | `4a374f00763e5458f44741294009523a25ad1b9835cfd593e2604827ba84f002` |
| E `0x06000d2f` | append node children | `d1498fd3baaf3a358664c44308764a69419b46577e5c6f250f00ed2c8efc5789` |
| E `0x06000f3b` | previous Program merge | `4bf090d3af1fc1e8685bb1ea342c1e5a7e7ddcbc8aa1b6e60dc12dc24444e189` |
| E `0x060005fa` | boundary navigator builder | `7e3832027219873ebc87952c74ad2077d0c01a3ec7a8d927019f6d0c772accab` |
| E `0x06000601` | generated boundary state | `92526cd90824e55bbed4663f7257b82fa5743c0349052161c48131aa2fc49a82` |
| E `0x0600058a` | validity panacea repair | `9dd1ba695580acfdb117baf9ec9e66d935abec094dbb64371239ed30e72fd1fc` |
| E `0x06000592` | descriptor-only TransformTo | `7eb80db1fe6c3ee0d489e6224c31caad712587198cd1ef19368277593113e70a` |
| E `0x06000cfe` | actual inherent child indexes | `9c0f8e267c3cff50f4d6014285d24626382c1921cacae522f316dae1e5d5d64f` |
| E `0x06000d31` | pair child/subdescriptor mapping | `4260f8ed277ce3ad2357cc86f21f138be1ef55f15f7e590cc5a871ae30a2dda0` |
| E `0x06000d28` | parent-inherent membership | `2230c2025a8a5814e7d5a3699f9740a40858d889712fde7d7ab0817a597fd9c9` |
| E `0x06001043` | shared empty index array | `648355fa17b46451ffe1cd342d27f6d4ddab4fab53555c455cd7c3d4d6b5c70f` |
| C `0x06000d6b` | two navigable words | `e68a41f241afb03946ec8ee8d345f0d1d5144334956982f937510f87d5b209b2` |
| C `0x06000d75` | point hit | `6069ef686d5cf907dd6bf7e6f2351f70fd2e9f8440e374581dc736ced5680d90` |
| C `0x0600093e` | point constructor | `4f64f6417d00ce268741014a035eaed81d162226aabac99abf7dbd9ae60ac361` |
| C `0x06000940` | stored hit fields | `814f70f4d818b8b65fa756b548630a655271b846df41581aa1f3f722b6ab58c3` |
| C `0x06000d88` | selected header | `f17ad61511ff09ac9089c43738b1c843c352ad400936ed1c034e887850c73a7e` |
| C `0x06000d86` | header editor visuals | `15fa80e7dd8370658903c8dc06092674eda45d37dc84cc09bf8476466bb9fce7` |


남은 한계: 모든 선행 삭제 handler·selection·임의 parent repair와 원본 runtime Undo 병합 전체를 재현한 것은 아니다. 확인한 bounded 상태를 구현·회귀로 검증하고 미확정 경계는 무변경으로 보존한다.


## 새 TRY의 최초 placeholder에서 삭제

canonical root TRY(BODY[PH], BODY[PH])의 두 PH는 모두 center 상태 하나다. Descriptor.get_IsLeafOperator `0x06001021`는 IsPlaceholder이면 false를 반환하므로 break/continue처럼 양쪽 leaf edge를 만들지 않는다. bq constructor `0x060005ef`는 EquationRootNode 전체 navigator 목록을 만들고, Fixity None의 PH는 `0x0600060c`에서 center만 등록한다. Special의 `0x06000611`과 BODY의 left=true/right=false를 적용한 관련 순서는 다음과 같다.

```text
TRY Left → TRY center part0(NodeSide Left)
 → BODY0 Left → BODY0 center → PH0 center
 → TRY center part1(NodeSide Right)
 → BODY1 Left → BODY1 center → PH1 center
 → TRY Right
```

NavigatorState constructor `0x06001316`은 part count>1일 때 첫 part를 NodeSide Left, 마지막 part를 Right로 매핑한다. 이 키보드 목록의 numeric part와 pointer hit의 part null 계약은 구분한다.

PH는 BODY의 container slot 안에 있으므로 ContainerRoot는 PH 자신이다(`0x06000c56`). analysis builder `0x060004ad→04b3→04b8→04bf`는 Fixity None의 PH handle 하나만 만들고, `aw.a` `0x060004ab`의 Previous/Next는 모두 null이다. 따라서 stage21 `ap.t`의 인접 analysis handle을 통한 container-ancestor override는 이 두 상태에서 발생하지 않고, bq의 한 상태 이동이 그대로 적용된다. BODY.NoLogicOnlyVisual은 기본 false라 이동 후 parent로 재승격되지 않는다.

선행 handler도 이 좁은 초기 상태를 가로막지 않는다. stage14 `ap.x→0x0600045a`는 prefix/infix/postfix 대상이며 PH Fixity0에서는 반환한다. stage19 `ap.p`는 nearest linear BODY의 한 child가 해당 방향 끝이므로 반환한다. stage20 `ap.u`의 방향 반전은 PH ContainerRoot 양쪽 boundary가 모두 PH라 성립하지 않는다(`0x06000605`). 이동 후 BODY는 outfix-only `ap.af` 대상이 아니며 one-child full/nary container는 `ap.ac`에서 반환하므로 TRY stage29로 도달한다.

| 최초 PH에서 누른 키 | stage21 이동 | TRY handler 결과 |
| --- | --- | --- |
| 첫 attempted PH + Backspace | BODY0 center | direct inherent empty BODY guard → 전체 TRY slot을 PH Left로 교체 |
| 둘째 handler PH + Backspace | BODY1 center | 같은 empty BODY guard → PH Left |
| 첫 attempted PH + Delete | TRY center part1 / NodeSide Right | root header merge → canonical Program[PH,PH], 첫 handler PH Left focus |
| 둘째 handler PH + Delete | TRY outer Right | side==dir이므로 TRY handler 반환; root에서 남은 handler도 rewrite하지 않아 무변경 |

마지막 경우를 억지로 대칭화하여 전체 TRY 삭제로 만들지 않는다. root TRY outer Right는 나머지 While/For/If handler 대상이 아니며 generic prefix/infix/postfix helper는 primal child가 있는 TRY를 건너뛰고, 마지막 Program-family handler도 해당하지 않는다. 외부 sibling/selection이 있는 별도 문맥으로 이 결과를 확대하지 않는다. 독립 구현은 위 최초 입력을 한 Delete Undo 단위로 재현하되 일반 PH 삭제에 모든 ancestor 구조 제거를 추가하지 않는다.

### 최초 PH 경로 추가 IL SHA-256

| Assembly/token | Role | IL SHA256 |
| --- | --- | --- |
| E `0x060005ef` | whole-root navigator constructor | `34096e81d4ca613530f6f1a433c14e00710a036b1c04d9d1be85bd57a0caed6a` |
| E `0x060005fd` | one-state directional navigation | `1ef1652a741d2b0bc3f7015f6ae4a1d203e55a22b68b2b8a0fe2dcfc2d0ba1f7` |
| E `0x06000600` | neighbor state index | `46ca7eee9126d02c7e6e6d739fe5973aea13db14544cee65e49d6d0578359ece` |
| E `0x0600060c` | fixity navigation dispatcher | `42ead008bbbf5ddbf30f54951ecfcd994c966bc6d13c0ac530b3222947eec087` |
| E `0x06000611` | Special state ordering | `66566f7338caa5db7796fecad83dd8bf87ff44b4d8b84075b44d1f867b7c7485` |
| E `0x06001316` | part to NodeSide mapping | `b3ae8dd499a4709ceb7741175397c7d410435a69e3a52f8e9eaee2e872bada50` |
| E `0x06001021` | PH is not leaf operator | `af5644535f4714ec88b4510dd1dd6af37c84e30c1dd62b283015bb61820a874e` |
| E `0x06000c56` | PH container root | `972c9d3809b54161a88ed249607f37b22dd36a55cfec4b1d32eed4a1733afffd` |
| E `0x060004b3` | analysis list builder | `cd23c8215c7d7e00c70e8d1d1c59062ab8b5deed30322432a34bbc285b41be61` |
| E `0x060004b8` | analysis fixity dispatcher | `be0adabb62e2672a277e935417359a442f3c75856b08be9a37b2321848258b0f` |
| E `0x060004bf` | single leaf handle | `596befde810a78d040e9bd4b9a3f711e8591c2677cea52b59a4147a2dc4c481b` |
| E `0x060004ab` | analysis previous/next | `709058a177d9df803a938bdda37818d2e063d80fb19ca88d98379c4eea2da2e9` |
| E `0x06000605` | directional boundary check | `974d81abf94c44fe29455738f7409ffc0e0b49ec24e5e443d6a2be7325082188` |
